#![no_main]
// Fuzz the contractspecv0 path on arbitrary bytes: the WASM custom-section
// extractor plus the XDR spec parser. Must fail closed, never panic.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = soroban_governance_core::spec::parse_contract_spec(data);
});
