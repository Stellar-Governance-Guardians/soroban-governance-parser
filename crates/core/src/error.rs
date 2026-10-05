use thiserror::Error;

/// Fail-closed decoding errors. Every variant carries enough context for an
/// operator to diagnose; none is silently swallowed or defaulted.
#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("input is not valid base64: {0}")]
    Base64(String),

    #[error("input is not valid XDR: {0}")]
    Xdr(String),

    #[error("input is not a valid WebAssembly binary: {0}")]
    Wasm(String),

    #[error("contract WASM has no `contractspecv0` custom section; decoding cannot be verified")]
    MissingContractSpec,

    #[error("unsupported ScVal construct: {0}")]
    UnsupportedScVal(String),

    #[error("JSON serialization failed: {0}")]
    Json(String),
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("decode error in adapter `{adapter}`: {source}")]
    Decode {
        adapter: &'static str,
        #[source]
        source: DecodeError,
    },

    #[error("event topic does not match adapter `{0}`; refusing to guess a decoding")]
    TopicMismatch(&'static str),

    #[error("governor implementation is not recognized: {0}")]
    UnknownGovernor(String),
}
