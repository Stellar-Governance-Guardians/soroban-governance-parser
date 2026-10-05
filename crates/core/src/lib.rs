//! Sans-IO core of the soroban-governance-parser.
//!
//! No network access, no filesystem access, no global state. Every function
//! takes data in and returns data or an explicit error. Fail-closed: any
//! decode failure yields an `Err`, never a silent default.

pub mod adapter;
pub mod error;
pub mod risk;
pub mod scval;
pub mod spec;
pub mod types;
pub mod wasm;

pub use error::{AdapterError, DecodeError};
