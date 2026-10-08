#!/usr/bin/env bash
# Idempotent tool bootstrap for the parser devcontainer.
#
# Installs the tools the repo's scripts expect that are not devcontainer
# features: wasm-pack (P2 packaging), gitleaks (pre-push secret scan) and the
# Stellar CLI (contract builds / TTL extension). Every step is skipped when the
# tool is already present, and the script fails loudly if a download fails
# rather than silently leaving a half-configured box (charter: fail closed).
#
# Nothing secret is written here. Funded testnet keys for the seed-v2 workflow
# come only from Codespaces secrets / env vars (see README); `.seed/` and
# `.env*` stay gitignored.
set -euo pipefail

say() { printf '\n==> %s\n' "$1"; }

# --- Rust components already pinned by rust-toolchain.toml --------------------
# `cargo`/`rustup` come from the rust devcontainer feature at 1.96; the
# toolchain file installs rustfmt/clippy and both wasm targets on first use.
say "Rust toolchain"
rustup show active-toolchain || true
rustc --version

# --- wasm-pack ----------------------------------------------------------------
say "wasm-pack"
if command -v wasm-pack >/dev/null 2>&1; then
  wasm-pack --version
else
  curl -sSf https://rustwasm.github.io/wasm-pack/installer/init.sh | sh
  wasm-pack --version
fi

# --- gitleaks -----------------------------------------------------------------
# Pinned release; bump deliberately. Must be >= 8.21 (the release that added
# multiple `[[allowlists]]`, which .gitleaks.toml uses): older builds silently
# ignore the allowlist and report the known fixture false positives.
say "gitleaks"
GITLEAKS_VERSION="8.30.1"
if command -v gitleaks >/dev/null 2>&1; then
  gitleaks version
else
  arch="$(uname -m)"
  case "$arch" in
    x86_64) gl_arch="x64" ;;
    aarch64) gl_arch="arm64" ;;
    *) echo "unsupported arch for gitleaks: $arch" >&2; exit 1 ;;
  esac
  tmp="$(mktemp -d)"
  curl -sSfL "https://github.com/gitleaks/gitleaks/releases/download/v${GITLEAKS_VERSION}/gitleaks_${GITLEAKS_VERSION}_linux_${gl_arch}.tar.gz" \
    | tar -xz -C "$tmp" gitleaks
  sudo install -m 0755 "$tmp/gitleaks" /usr/local/bin/gitleaks
  rm -rf "$tmp"
  gitleaks version
fi

# --- Stellar CLI --------------------------------------------------------------
# Needed for `stellar contract build` (wasm32v1-none) and TTL extension. Built
# from source via cargo because no devcontainer feature ships it.
say "stellar-cli"
if command -v stellar >/dev/null 2>&1; then
  stellar --version
else
  cargo install --locked stellar-cli
  stellar --version
fi

say "done. Run: cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"
