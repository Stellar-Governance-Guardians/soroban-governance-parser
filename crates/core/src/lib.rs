//! Sans-IO core of the soroban-governance-parser.
//!
//! No network access, no filesystem access, no global state. Every function
//! takes data in and returns data or an explicit error. Fail-closed: any
//! decode failure yields an `Err`, never a silent default.

pub mod adapter;
pub mod adapters;
pub mod checkpoint;
pub mod error;
pub mod risk;
pub mod scval;
pub mod simulate;
pub mod spec;
pub mod tally;
pub mod types;
pub mod wasm;

pub use error::{AdapterError, DecodeError};
