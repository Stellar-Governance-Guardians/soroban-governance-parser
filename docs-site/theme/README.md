# Vendored theme assets

`mermaid.min.js` is the Mermaid diagram renderer, vendored so the documentation
site loads no external script.

| Field | Value |
|---|---|
| Project | <https://github.com/mermaid-js/mermaid> |
| Version | 11.6.0 (version string embedded in the bundle) |
| License | MIT — `Copyright (c) 2014 - 2022 Knut Sveidqvist` |
| Vendored from | the npm package `mermaid@11.6.0` dist bundle |

`mermaid-init.js` in this directory is written for this site: it initializes the
vendored build with `startOnLoad` and picks the dark theme when the mdBook theme
is dark. It is not the upstream init script.

Replacing the bundle is a PR that states the new version, updates the table
above, and confirms `docs-site/scripts/check-docs-claims.py` still passes (it
asserts that `additional-js` never points outside this site).
