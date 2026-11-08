# How the Node event-test expectations were derived

`node.test.mjs` drives the built package with committed XDR, never with values
copied from the implementation. This file records the derivation so a reviewer
can redo it with the standard library alone (no installs, no network).

## Source of the vote_cast bytes

`tests/fixtures/seed-v2/rpc/0066-getTransaction-final-s3-vote-s1-transfer-executed-sgg-delegate-1.json`,
`response.result.events.contractEventsXdr[0][0]` — a real testnet event emitted
by contract `946484410e1b6132417d7df19506db97fd4563534c7b26028f5a0902877422d5`.
The bytes are re-attributed to the registered governor id
`CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX`
(`GOVERNOR_CONTRACT_ID` in `crates/core/src/adapters/script3.rs`) because
`matches_event` correctly refuses a non-governor contract id.

The Phase-1 vectors are still used only for the **negative** shape case: that
capture encodes the vote amount as `SCV_U128` (`0x09`), which the Script3
adapter refuses (`expected I128`), so it is not a valid positive vote_cast.

## Event layout (offset 52 onwards in the event XDR)

```
topic[0]  0000000f 00000009 "vote_cast" 000000   SCV_SYMBOL
topic[1]  00000003 00000000                     SCV_U32 = 0 (proposal 0)
topic[2]  00000012 00000000 00000000 <32 bytes>  SCV_ADDRESS / account / ed25519
value     00000010 00000001 00000002            SCV_VEC of 2
          00000003 00000001                     support = 1
          0000000a 0000000000000000 000002ba7def3000
                                                SCV_I128 = 3_000_000_000_000
```

Base64 forms used by the test:

```
topic[0]  AAAADwAAAAl2b3RlX2Nhc3QAAAA=
topic[1]  AAAAAwAAAAA=
topic[2]  AAAAEgAAAAAAAAAAzT8nsj+8SDy+0Q+IpClijkUvvaZ8oVUmL8Vs4A==
value     AAAAEAAAAAEAAAACAAAAAwAAAAEAAAAKAAAAAAAAAAAAAAK6fe8wAA==
```

## Voter address

The address payload is the 32-byte key
`cd3f27b23fbc483cbed10f88a429628e452fbda67ca155262fc56ce051623510`.
StrKey encoding is base32 of `version byte (6 << 3) || key || crc16-xmodem(LE)`:

```
GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235
```

The literal replaces the previous self-referential expectation
(`voter: event.payload.voter`), which could never disagree with the code.

## Payload-shape case

`VALUE_PROPOSAL_CREATED_2_ELEMENTS` is a 2-element vector. The pinned upstream
`contracts/governor/src/events.rs` declares five data elements for
`proposal_created` (title, description, action, vote_start, vote_end), so the
adapter must fail with `proposal_created data must be 5 elements, got 2`; the
test asserts that message, not a silent fallback.
