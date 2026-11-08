# How the Node event-test expectations were derived

`node.test.mjs` drives the built package with committed XDR, never with values
copied from the implementation. This file records the derivation so a reviewer
can redo it with the standard library alone (no installs, no network).

## Inputs

| constant | source |
|---|---|
| `TOPIC_VOTE_CAST` | `tests/fixtures/phase1-fixture-events.json`, `vote_cast` topic[0] |
| `TOPIC_PROPOSAL_ID_1` | same capture, `vote_cast` topic[1] (u32 `1`) |
| `TOPIC_VOTER` | same capture, `vote_cast` topic[2] (address) |
| `VALUE_VOTE_CAST` | same capture, `vote_cast` value |

The capture was emitted by the fixture contract
`CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE`. `matches_event`
requires the registered governor id, so the tests re-attribute the same bytes to
`CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX`
(`GOVERNOR_CONTRACT_ID` in `crates/core/src/adapters/script3.rs`).

## Value vector

`VALUE_VOTE_CAST` decodes, byte by byte, as:

```
00000010  ScVal::Vec
00000001  present
00000002  length 2
00000003 00000001        element 0: ScVal::U32  = 1
00000009 0000000000000000 00000000000f4240
                          element 1: ScVal::I128 = 1_000_000
```

Hence `support: 1` and `amount: "1000000"` (the parser serialises i128 as a
decimal string).

## Voter address

`TOPIC_VOTER` is `ScVal::Address` (type `0x12`) followed by
`SC_ADDRESS_TYPE_ACCOUNT` (`00000000`), the public-key discriminant
(`00000000`, ed25519) and the 32-byte key:

```
1808d8b4e11bae4259b0f0c9a95c2a220fd8cf7dcfdfbdf67d1b58decd26080a
```

StrKey encoding is base32 of `version byte (6 << 3) || key || crc16-xmodem(key, LE)`:

```
GAMARWFU4EN24QSZWDYMTKK4FIRA7WGPPXH57PPWPUNVRXWNEYEAUP3C
```

That literal replaces the previous self-referential expectation
(`voter: event.payload.voter`), which could never disagree with the code.

## Payload-shape case

`VALUE_PROPOSAL_CREATED_2_ELEMENTS` is a 2-element vector. The pinned upstream
`contracts/governor/src/events.rs` declares five data elements for
`proposal_created` (title, description, action, vote_start, vote_end), so the
adapter must fail with `proposal_created data must be 5 elements, got 2`; the
test asserts that message, not a silent fallback.
