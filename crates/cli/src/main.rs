//! `sgp` — soroban-governance-parser CLI.
//!
//! Subcommands:
//! - decode-scval: base64 XDR ScVal -> JSON (pure, offline)
//! - spec-from-wasm: contract WASM file -> parsed contractspecv0 JSON (offline)
//! - fetch-spec: contract id -> live RPC fetch -> parsed contractspecv0 JSON
//! - health: RPC getHealth summary (retention window verification)
//! - ttl: read contract instance/code TtlEntries (protocol 29), optional min-remaining gate

mod rpc;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "sgp",
    version,
    about = "Soroban governance payload parser (fail-closed)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Decode a base64 XDR ScVal into JSON. Offline, pure.
    DecodeScval {
        /// base64 XDR of an ScVal (or "-" to read stdin)
        xdr_base64: String,
    },
    /// Extract and parse the contractspecv0 section from a WASM file. Offline.
    SpecFromWasm {
        /// path to contract .wasm
        wasm: PathBuf,
    },
    /// Fetch a contract's WASM from a Soroban RPC and parse its contractspecv0.
    FetchSpec {
        /// contract id (strkey C...)
        #[arg(long)]
        contract: String,
        /// Soroban RPC URL (default: SDF public testnet)
        #[arg(long, default_value = rpc::DEFAULT_TESTNET_RPC)]
        rpc: String,
    },
    /// Print RPC health: latest ledger, oldest ledger, retention window.
    Health {
        #[arg(long, default_value = rpc::DEFAULT_TESTNET_RPC)]
        rpc: String,
    },
    /// Read a contract's instance/code TTL entries (protocol 29) and check
    /// remaining ledgers. Fails closed: missing TTL entries are errors.
    Ttl {
        /// contract id (strkey C...)
        #[arg(long)]
        contract: String,
        #[arg(long, default_value = rpc::DEFAULT_TESTNET_RPC)]
        rpc: String,
        /// exit 1 if remaining ledgers fall below this threshold
        #[arg(long)]
        min_remaining: Option<u32>,
    },
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::DecodeScval { xdr_base64 } => {
            let input = if xdr_base64 == "-" {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .map_err(|e| format!("stdin read failed: {e}"))?;
                buf
            } else {
                xdr_base64
            };
            let json = rpc::decode_scval_b64(&input)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?
            );
        }
        Command::SpecFromWasm { wasm } => {
            let bytes =
                std::fs::read(&wasm).map_err(|e| format!("cannot read {}: {e}", wasm.display()))?;
            let spec = soroban_governance_core::spec::parse_contract_spec(&bytes)
                .map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&spec).map_err(|e| e.to_string())?
            );
        }
        Command::FetchSpec { contract, rpc } => {
            let client = rpc::RpcClient::new(&rpc);
            let (latest, oldest, window) = client.health().await.map_err(|e| e.to_string())?;
            let wasm_bytes = client
                .fetch_contract_wasm(&contract)
                .await
                .map_err(|e| e.to_string())?;
            let spec = soroban_governance_core::spec::parse_contract_spec(&wasm_bytes)
                .map_err(|e| e.to_string())?;
            let out = serde_json::json!({
                "source": {
                    "rpc": rpc,
                    "contractId": contract,
                    "wasmBytes": wasm_bytes.len(),
                    "atLedger": latest,
                    "oldestLedger": oldest,
                    "ledgerRetentionWindow": window,
                },
                "spec": spec,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
            );
        }
        Command::Health { rpc } => {
            let client = rpc::RpcClient::new(&rpc);
            let (latest, oldest, window) = client.health().await.map_err(|e| e.to_string())?;
            let out = serde_json::json!({
                "rpc": rpc,
                "latestLedger": latest,
                "oldestLedger": oldest,
                "ledgerRetentionWindow": window,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
            );
        }
        Command::Ttl {
            contract,
            rpc,
            min_remaining,
        } => {
            cmd_ttl(contract, rpc, min_remaining).await?;
        }
    }
    Ok(())
}

async fn cmd_ttl(contract: String, rpc: String, min_remaining: Option<u32>) -> Result<(), String> {
    let client = rpc::RpcClient::new(&rpc);
    let (latest, _, _) = client.health().await.map_err(|e| e.to_string())?;
    let ttl = client
        .contract_ttl(&contract, latest)
        .await
        .map_err(|e| e.to_string())?;
    let out = serde_json::json!({
        "contractId": contract,
        "rpc": rpc,
        "latestLedger": ttl.latest_ledger,
        "instance": {
            "liveUntilLedger": ttl.instance_live_until,
            "remainingLedgers": ttl.instance_live_until.saturating_sub(ttl.latest_ledger),
        },
        "code": {
            "liveUntilLedger": ttl.code_live_until,
            "remainingLedgers": ttl.code_live_until.saturating_sub(ttl.latest_ledger),
        },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?
    );
    if let Some(min) = min_remaining {
        let lows: Vec<&str> = [
            (ttl.instance_live_until, "instance"),
            (ttl.code_live_until, "code"),
        ]
        .iter()
        .filter(|(until, _)| until.saturating_sub(ttl.latest_ledger) < min)
        .map(|(_, name)| *name)
        .collect();
        if !lows.is_empty() {
            return Err(format!(
                "TTL below minimum remaining {min} ledgers: {}",
                lows.join(", ")
            ));
        }
    }
    Ok(())
}
