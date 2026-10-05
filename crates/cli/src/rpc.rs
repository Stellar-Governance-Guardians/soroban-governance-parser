//! IO layer: Soroban JSON-RPC client. All network access for the CLI lives
//! here; the core crate remains sans-IO. Fail-closed: RPC errors, missing
//! entries and malformed XDR are explicit errors.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use stellar_xdr::{
    ContractDataDurability, Hash, LedgerKey, LedgerKeyContractCode, LedgerKeyContractData, Limits,
    ReadXdr, ScAddress, ScVal, WriteXdr,
};

pub const DEFAULT_TESTNET_RPC: &str = "https://soroban-testnet.stellar.org";

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("HTTP/RPC failure: {0}")]
    Transport(String),
    #[error("RPC returned error: {0}")]
    Rpc(String),
    #[error("unexpected RPC response shape: {0}")]
    Shape(String),
}

pub struct RpcClient {
    pub url: String,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct RpcEnvelope {
    result: Option<Value>,
    error: Option<Value>,
}

#[derive(Deserialize)]
struct GetHealthResult {
    #[serde(rename = "latestLedger")]
    pub latest_ledger: u32,
    #[serde(rename = "oldestLedger")]
    pub oldest_ledger: u32,
    #[serde(rename = "ledgerRetentionWindow")]
    pub ledger_retention_window: u32,
}

impl RpcClient {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_mins(1))
                .build()
                .unwrap_or_default(),
        }
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        let resp = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| RpcError::Transport(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| RpcError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(RpcError::Transport(format!("HTTP {status}: {text}")));
        }
        let env: RpcEnvelope =
            serde_json::from_str(&text).map_err(|e| RpcError::Shape(e.to_string()))?;
        if let Some(err) = env.error {
            return Err(RpcError::Rpc(err.to_string()));
        }
        env.result
            .ok_or_else(|| RpcError::Shape("missing result".into()))
    }

    pub async fn health(&self) -> Result<(u32, u32, u32), RpcError> {
        let v: GetHealthResult = serde_json::from_value(self.call("getHealth", json!({})).await?)
            .map_err(|e| RpcError::Shape(e.to_string()))?;
        Ok((v.latest_ledger, v.oldest_ledger, v.ledger_retention_window))
    }

    /// Fetch the contract WASM bytes for a contract id (strkey C...).
    /// Two ledger-entry hops: ContractData(ledger_key_contract_instance) ->
    /// executable wasm hash -> ContractCode(hash).
    pub async fn fetch_contract_wasm(&self, contract_id: &str) -> Result<Vec<u8>, RpcError> {
        let hash = contract_strkey_to_hash(contract_id)
            .map_err(|e| RpcError::Shape(format!("bad contract id: {e}")))?;
        let key = LedgerKey::ContractData(LedgerKeyContractData {
            contract: ScAddress::Contract(stellar_xdr::ContractId(hash)),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
        });
        let entry_xdr = self.get_ledger_entry(&key).await?;
        let entry = stellar_xdr::LedgerEntryData::from_xdr_base64(&entry_xdr, Limits::none())
            .map_err(|e| RpcError::Shape(format!("entry XDR: {e}")))?;
        let contract_data = match entry {
            stellar_xdr::LedgerEntryData::ContractData(d) => d,
            other => {
                return Err(RpcError::Shape(format!(
                    "expected ContractData entry, got {}",
                    discriminant(&other)
                )))
            }
        };
        let instance = match contract_data.val {
            ScVal::ContractInstance(i) => i,
            other => {
                return Err(RpcError::Shape(format!(
                    "expected ContractInstance value, got {}",
                    scval_discriminant(&other)
                )))
            }
        };
        let wasm_hash = match instance.executable {
            stellar_xdr::ContractExecutable::Wasm(h) => h,
            other => {
                return Err(RpcError::Shape(format!(
                    "contract is not WASM-backed (executable: {}); no contractspecv0 available",
                    executable_discriminant(&other)
                )))
            }
        };
        let code_key = LedgerKey::ContractCode(LedgerKeyContractCode { hash: wasm_hash });
        let code_xdr = self.get_ledger_entry(&code_key).await?;
        let code = stellar_xdr::LedgerEntryData::from_xdr_base64(&code_xdr, Limits::none())
            .map_err(|e| RpcError::Shape(format!("code XDR: {e}")))?;
        match code {
            stellar_xdr::LedgerEntryData::ContractCode(c) => Ok(c.code.into()),
            _ => Err(RpcError::Shape("expected ContractCode entry".into())),
        }
    }

    async fn get_ledger_entry(&self, key: &LedgerKey) -> Result<String, RpcError> {
        let key_b64 = key
            .to_xdr_base64(Limits::none())
            .map_err(|e| RpcError::Shape(format!("key XDR: {e}")))?;
        let result = self
            .call("getLedgerEntries", json!({ "keys": [key_b64] }))
            .await?;
        let entries = result
            .get("entries")
            .and_then(Value::as_array)
            .ok_or_else(|| RpcError::Shape("missing entries array".into()))?;
        let first = entries
            .first()
            .ok_or_else(|| RpcError::Shape("no entry returned (missing from ledger?)".into()))?;
        first
            .get("xdr")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| RpcError::Shape("entry missing xdr field".into()))
    }
}

fn contract_strkey_to_hash(s: &str) -> Result<Hash, String> {
    let decoded = stellar_strkey::Contract::from_string(s).map_err(|e| e.to_string())?;
    Ok(Hash(decoded.0))
}

fn discriminant(d: &stellar_xdr::LedgerEntryData) -> &'static str {
    use stellar_xdr::LedgerEntryData as L;
    match d {
        L::Account(_) => "Account",
        L::Trustline(_) => "Trustline",
        L::Offer(_) => "Offer",
        L::Data(_) => "Data",
        L::ClaimableBalance(_) => "ClaimableBalance",
        L::LiquidityPool(_) => "LiquidityPool",
        L::ConfigSetting(_) => "ConfigSetting",
        L::ContractData(_) => "ContractData",
        L::ContractCode(_) => "ContractCode",
        L::Ttl(_) => "Ttl",
    }
}

fn scval_discriminant(v: &ScVal) -> &'static str {
    match v {
        ScVal::ContractInstance(_) => "ContractInstance",
        ScVal::Vec(_) => "Vec",
        ScVal::Map(_) => "Map",
        ScVal::Bytes(_) => "Bytes",
        ScVal::Symbol(_) => "Symbol",
        _ => "other",
    }
}

fn executable_discriminant(e: &stellar_xdr::ContractExecutable) -> &'static str {
    use stellar_xdr::ContractExecutable as C;
    match e {
        C::Wasm(_) => "wasm",
        C::StellarAsset => "stellar_asset",
        C::ExternalRef(_) => "external_ref",
    }
}

/// Decode a base64 XDR ScVal to JSON (re-exported helper for the CLI).
pub fn decode_scval_b64(xdr_base64: &str) -> Result<Value, String> {
    let bytes = STANDARD
        .decode(xdr_base64.trim())
        .map_err(|e| format!("input is not valid base64: {e}"))?;
    let scval = ScVal::from_xdr(&bytes, Limits::none())
        .map_err(|e| format!("input is not valid XDR ScVal: {e}"))?;
    soroban_governance_core::scval::scval_to_json(&scval).map_err(|e| e.to_string())
}
