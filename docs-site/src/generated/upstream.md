# Upstream pins

Generated from `docs-site/sources.lock` by `docs-site/scripts/gen-upstream.py`. Do not edit by hand.

The indexer and dashboard pages on this site are summaries written in this repository, not embedded copies of those repositories' own documentation. Each summary names the exact revision it was written against below, and the bytes of that revision are vendored under `docs-site/pinned/` and hashed by the PR gate. The nightly `docs-live` workflow re-fetches the pinned file and fails if the content no longer matches — so a silent change upstream cannot leave this site claiming something stale.

| Repository | Path | Commit | sha256 |
|---|---|---|---|
| https://github.com/Stellar-Governance-Guardians/governance-event-indexer | `README.md` | `caa292a034e915803a90ec14cdac40b3db023df9` | `af7053996de75a27bfdb1e83bdac637671a6b2484995aab127ba092d4870cfab` |
| https://github.com/Stellar-Governance-Guardians/delegate-portal-dashboard | `README.md` | `54f6180763e85007fe9e9e257995319e0eb04d36` | `c3907619d02a854f1215d9b90f4a026d2e979ed134a7632c61314da2a348aa8e` |

Bumping a pin is a parser-repo PR that shows the vendored byte diff and the review on this site that the summary still matches. This happens at the end of every phase.

## What is not pinned

Nothing from a sibling repository is rendered as prose on this site today. If a later phase embeds sibling documentation directly, it must enter through this lock file first.
