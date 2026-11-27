> **Status: implemented.** The OpenZeppelin adapter ships for event matching,
> normalization, proposal state (`proposal_state` captures `o1`–`o6`) and
> `proposal_created` calldata projection. Event payload layouts are verified
> against the pinned source and the deployed governor's embedded spec; there is
> no OpenZeppelin `getEvents` capture yet, and the proposal id is format-checked
> rather than recomputed. The indexer can therefore decode this governor's
> events as soon as they are ingested.

{{#include ../../../docs/adapters/openzeppelin.md}}
