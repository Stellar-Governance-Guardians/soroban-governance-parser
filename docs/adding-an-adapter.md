# Adding a governor adapter

A new governor is one module behind the `GovernorAdapter` trait
(`crates/core/src/adapter.rs`). Nothing outside the adapter should need to know
the governor exists — that is the whole point of the trait.

## The trait

Implement these in a new `crates/core/src/adapters/<governor>.rs`, then add
`pub mod <governor>;` to `crates/core/src/adapters/mod.rs`:

| method | responsibility |
|---|---|
| `name()` | stable adapter id, e.g. `script3-soroban-governor` |
| `matches_event(event)` | is this event *ours*? contract id + topic symbol + exact arity; return `false` on any doubt |
| `normalize_event(event)` | decode topics/data into `NormalizedEvent`; `Err` on an unexpected shape |
| `decode_call(call, spec)` | decode a proposal action into `DecodedCall`; use `spec` only for argument *names* |

Governor-specific helpers (state/tally reconstruction, action enums, contextual
rules projection) live in the same file.

## Rules that are not optional

1. **Fail closed.** Unknown variant ⇒ a preserved `Unknown` / `Err`, never a
   plausible guess. Missing field ⇒ `Err`. Absent spec ⇒ `decoding: "unverified"`
   with positional args, never invented names.
2. **No mock data.** The adapter is pure: it takes `RawEvent` / `RawCall` /
   `ScVal` values. It must not import `tests/fixtures` (CI enforces this).
3. **Every shape is cited.** Transcribe each decoded structure from the pinned
   upstream source at the SHA in `scripts/upstream/upstream.lock.json`, and add a
   row to a `docs/adapters/<governor>.md` source-to-behavior table.
4. **Differential test it.** Add offline tests that decode committed raw captures
   and assert equality with the on-chain reads. If a read you need was not
   captured, extend the capture tooling, re-capture, and commit — do not invent
   the value.
5. **Register the claims.** Anything the README then states about the adapter
   gets a `claims.json` entry.

## Sketch

```rust
// crates/core/src/adapters/acme.rs
use crate::adapter::{GovernorAdapter, RawCall, RawEvent};
use crate::error::AdapterError;
use crate::types::{DecodedCall, DecodingStatus, NormalizedEvent};

pub const ADAPTER_NAME: &str = "acme-governor";

pub struct AcmeAdapter;

impl GovernorAdapter for AcmeAdapter {
    fn name(&self) -> &'static str { ADAPTER_NAME }

    fn matches_event(&self, event: &RawEvent<'_>) -> bool {
        // contract id + topic symbol + exact arity; false on doubt
        todo!()
    }

    fn normalize_event(&self, event: &RawEvent<'_>) -> Result<NormalizedEvent, AdapterError> {
        todo!()
    }

    fn decode_call(
        &self,
        call: &RawCall<'_>,
        spec: Option<&crate::spec::ContractSpec>,
    ) -> Result<DecodedCall, AdapterError> {
        // names only from spec; positional + Unverified otherwise
        todo!()
    }
}
```

## Checklist before a PR

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
- [ ] `OFFLINE=1 bash scripts/check-claims.sh` (add claims for new README statements)
- [ ] `python3 scripts/check-deployments.py` if you registered fixtures
- [ ] differential tests cover every captured proposal of the governor
