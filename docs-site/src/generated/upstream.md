# Upstream pins

Generated from `docs-site/sources.lock` by `docs-site/scripts/gen-upstream.py`. Do not edit by hand.

The indexer and dashboard pages on this site are summaries written in this repository, not embedded copies of those repositories' own documentation. Each summary names the exact revision it was written against below, and the bytes of that revision are vendored under `docs-site/pinned/` and hashed by the PR gate. The nightly `docs-live` workflow re-fetches the pinned file and fails if the content no longer matches — so a silent change upstream cannot leave this site claiming something stale.

| Repository | Path | Commit | sha256 |
|---|---|---|---|
| https://github.com/Stellar-Governance-Guardians/governance-event-indexer | `README.md` | `68aa657c68cb1cc1fca1f87da5f55ec74e7272bf` | `b78f22a64605059e2db939671280835a4ee072947a0d7c1cbe7200da44ea49b3` |
| https://github.com/Stellar-Governance-Guardians/delegate-portal-dashboard | `README.md` | `0c622709848637eb5908d9d0432475ce1cc92fe6` | `9406bedbc6fcdddbde6278e6bb0ee166390035241fd3d6810660ca899b73bb6c` |

Bumping a pin is a parser-repo PR that shows the vendored byte diff and the review on this site that the summary still matches. This happens at the end of every phase.

## What is not pinned

Nothing from a sibling repository is rendered as prose on this site today. If a later phase embeds sibling documentation directly, it must enter through this lock file first.
