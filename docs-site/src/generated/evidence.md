# Evidence

Generated from `deployments.json` by `docs-site/scripts/gen-evidence.py` (sha256 `b8c71c34bd5bdd64183b6dae39bdbcd0641a5d8d7084cc751925f4e7e14a3a6d`). Do not edit by hand. The registry is checked against the committed fixtures and upstream pins by `scripts/check-deployments.py` in the PR gate; this page renders it.

Every contract id below was deployed on live testnet with the recorded transaction hash, or captured from the public source named in `tests/fixtures/README.md`. There are no mock ids.

## Network: testnet

| Field | Value |
|---|---|
| RPC | `https://soroban-testnet.stellar.org` |
| Passphrase | `Test SDF Network ; September 2015` |
| RPC retention window | 120,960 ledgers |
| Retention verified | 2026-10-05 |
| Retention evidence | `tests/fixtures/phase1-proof.json` |

### Contracts (8)

| Key | Contract id | Purpose | Deployed at ledger | Deploy tx |
|---|---|---|---|---|
| `fixtureGovernor` | [`CDJWPKSQ4NA67PKT…`](https://stellar.expert/explorer/testnet/contract/CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE) | Real WASM-backed governance-shaped contract for parser proof. NOT a production governor. | 5,035,901 | [`a37b875db6c6…`](https://stellar.expert/explorer/testnet/tx/a37b875db6c67674801f604c6a0dea1194cea6ab205bfa5252d55cecdb6d9bda) |
| `seedV2Script3.script3Votes` | [`CCAUJK6V6GIYQANK…`](https://stellar.expert/explorer/testnet/contract/CCAUJK6V6GIYQANKCV2JDCMCLGMQD42JCF426TQHHMSXW2F5ZROM4IKY) | votes (checkpointed voting power) for the Script3 governor | 5,083,606 | [`9896582ef9b4…`](https://stellar.expert/explorer/testnet/tx/9896582ef9b4919c7c611ec9458ac7e5346239a6fe4fde40931e4c001a596bd5) |
| `seedV2Script3.script3Governor` | [`CCKGJBCBBYNWCMSB…`](https://stellar.expert/explorer/testnet/contract/CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX) | the pinned Script3 governor | 5,083,608 | [`9508d28ca1ae…`](https://stellar.expert/explorer/testnet/tx/9508d28ca1ae070d981ff7899a6673c6ec8c3815cc9811fdddb6e34a4574761d) |
| `seedV2Script3.script3MockSubcall` | [`CAQCXFI6YSXWGCB3…`](https://stellar.expert/explorer/testnet/contract/CAQCXFI6YSXWGCB37PRZVG5YNLLJMY45IWLUUTFMTXFIAF2MOZ5DMGJ7) | Script3-side out-of-surface target used by the s4/o4 unknown-contract-call proposals | 5,083,610 | [`45395fede4b9…`](https://stellar.expert/explorer/testnet/tx/45395fede4b9156b180b47b4882f010cbc205dace1c54dc5fdc3f68dbfffd2b8) |
| `seedV2OpenZeppelin.ozToken` | [`CA77UOHZIPLJKPRV…`](https://stellar.expert/explorer/testnet/contract/CA77UOHZIPLJKPRVQ7FOYRT5ATAFQYVDTVWRF4ZHNCN2OK274CCQ4V2R) | the pinned OpenZeppelin votes/fungible token (holds voting power) | 5,083,612 | [`026fd6edeeea…`](https://stellar.expert/explorer/testnet/tx/026fd6edeeea238f8c52cdfe89592d033e34a36a57d534553063b21435b1a006) |
| `seedV2OpenZeppelin.ozGovernor` | [`CAONSV2R2YCNJS6D…`](https://stellar.expert/explorer/testnet/contract/CAONSV2R2YCNJS6DSN366M5LXLHWT46ES57MD6KOM725YFQSGRLHDCMM) | the pinned OpenZeppelin governor | 5,083,614 | [`fed6be425422…`](https://stellar.expert/explorer/testnet/tx/fed6be4254223aae40de63bfe44961c73f05f46f7abbf07c8b4d7294a30ee751) |
| `seedV2OpenZeppelin.ozUpgradeableV1` | [`CC3SMQABLUGPGUD5…`](https://stellar.expert/explorer/testnet/contract/CC3SMQABLUGPGUD56JCY56JTRLMB4JGW5WMNBUVEU5DTN44NMWUR64DZ) | upgrade target for the s2/o2 contract-upgrade proposals | 5,083,616 | [`468c69a101d9…`](https://stellar.expert/explorer/testnet/tx/468c69a101d9da1a9b797fdbf86734a543148c8f7afb198b09ae6d725116c7c9) |
| `seedV2OpenZeppelin.ozMockSubcall` | [`CAXHKR66BEHSTAX4…`](https://stellar.expert/explorer/testnet/contract/CAXHKR66BEHSTAX4QYKEOCFKLOSUKHSCVADVHBT5WA4IJDE3EBCNVX3O) | OpenZeppelin-side out-of-surface target used by the s4/o4 unknown-contract-call proposals | 5,083,618 | [`aec4e10d1de6…`](https://stellar.expert/explorer/testnet/tx/aec4e10d1de6a4f3c24542a249922f0cd7755d7f1400bd016f54fe855f624d11) |

### WASM hashes

| Key | WASM sha256 | Bytes | Upload tx |
|---|---|---|---|
| `fixtureGovernor` | `a54dd248e58f0fa945c718ac3badfa03dbaa1642d2a08dc409caeaf3d3555b38` | 2,951 | [`5b6fc833ed21…`](https://stellar.expert/explorer/testnet/tx/5b6fc833ed2120cd1616bedfb5be5ba91d8d2b5224c9d2fd52b5cf62b145f03d) |
| `seedV2Script3.script3Votes` | `e8bbc883b2c7fcd8716b07271929eca801fb40f2d3cf5964b139fa741c40f787` | 24,926 | not recorded |
| `seedV2Script3.script3Governor` | `16033b38b740b1d5af82025a671490b3cea2a0df59b7ffad005f3f7ca3fa27f0` | 39,581 | not recorded |
| `seedV2Script3.script3MockSubcall` | `670215400fffd3d0f6303000d18dfd5bab0e2b1a7b26219e4a8a3bce08eab763` | 7,652 | not recorded |
| `seedV2OpenZeppelin.ozToken` | `b5363f54a65e4134f838c03ae9b90718cd7913be5bb3b446aa731e72c7d97e4b` | 23,856 | not recorded |
| `seedV2OpenZeppelin.ozGovernor` | `77a0400e446947330f8b80f3a57f078d375a793e7aacf3674f5289bacb66edf7` | 26,096 | not recorded |
| `seedV2OpenZeppelin.ozUpgradeableV1` | `1018bca637343ad206e568dd78a89f0b2a2f768477947be2546682113216c248` | 15,640 | not recorded |
| `seedV2OpenZeppelin.ozMockSubcall` | `670215400fffd3d0f6303000d18dfd5bab0e2b1a7b26219e4a8a3bce08eab763` | 7,652 | not recorded |

## Totals

- Registered contracts: **8**
- Networks: **1**

The contract ids above are testnet deployments. Testnet contracts can be removed, archived or lose their TTL entries; `sgp ttl --contract <id>` checks the live remaining ledgers and fails closed when entries are missing.
