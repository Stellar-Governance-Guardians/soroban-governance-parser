//! `sgp` — soroban-governance-parser CLI.
//!
//! Subcommands:
//! - decode-scval: base64 XDR ScVal -> JSON (pure, offline)
//! - spec-from-wasm: contract WASM file -> parsed contractspecv0 JSON (offline)
//! - fetch-spec: contract id -> live RPC fetch -> parsed contractspecv0 JSON
//! - health: RPC getHealth summary (retention window verification)

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
    }
    Ok(())
}
