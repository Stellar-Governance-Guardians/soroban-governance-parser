#!/usr/bin/env bash
# Negative tests for the CI helpers. Offline and deterministic: runs on every PR.
#
# Each case asserts both the exit status and, where it matters, the message, so
# a helper that silently succeeds (or fails without saying why) breaks this test.
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0
cases=0

pass() { echo "ok   $1"; cases=$((cases + 1)); }
fail() { echo "FAIL $1"; cases=$((cases + 1)); failures=$((failures + 1)); }

# ---------------------------------------------------------------------------
# retry: succeeds when a command succeeds on a later attempt
# ---------------------------------------------------------------------------
flaky="$tmp/flaky.sh"
cat > "$flaky" <<'EOS'
#!/usr/bin/env bash
n=$(cat "$COUNT_FILE" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$COUNT_FILE"
[ "$n" -ge 2 ]
EOS
chmod +x "$flaky"
if COUNT_FILE="$tmp/count" bash "$here/retry" 3 0 -- "$flaky" >/dev/null 2>&1; then
  pass "retry-succeeds-after-one-failure"
else
  fail "retry-succeeds-after-one-failure"
fi

# retry: a persistent failure exits non-zero
if bash "$here/retry" 2 0 -- false >/dev/null 2>&1; then
  fail "retry-exhausted-fails (exit was 0)"
else
  pass "retry-exhausted-fails"
fi

# retry: rejects a missing -- separator instead of running the wrong command
if bash "$here/retry" 1 0 true >/dev/null 2>&1; then
  fail "retry-requires-separator (exit was 0)"
else
  pass "retry-requires-separator"
fi

# ---------------------------------------------------------------------------
# install-wasm-pack: installs a checksum-verified tarball from a local URL
# ---------------------------------------------------------------------------
fake_version="9.9.9"
fake_dir="$tmp/wasm-pack-v${fake_version}-x86_64-unknown-linux-musl"
mkdir -p "$fake_dir"
cat > "$fake_dir/wasm-pack" <<'EOS'
#!/usr/bin/env bash
echo "wasm-pack 9.9.9"
EOS
chmod +x "$fake_dir/wasm-pack"
tar -czf "$tmp/fake.tar.gz" -C "$tmp" "wasm-pack-v${fake_version}-x86_64-unknown-linux-musl"
fake_sha="$(sha256sum "$tmp/fake.tar.gz" | cut -d' ' -f1)"

if WASM_PACK_VERSION="$fake_version" \
   WASM_PACK_SHA256="$fake_sha" \
   WASM_PACK_URL="file://$tmp/fake.tar.gz" \
   WASM_PACK_HOME="$tmp/home-ok" \
   bash "$here/install-wasm-pack.sh" >/dev/null 2>&1 \
   && [ -x "$tmp/home-ok/bin/wasm-pack" ] \
   && "$tmp/home-ok/bin/wasm-pack" --version | grep -q "$fake_version"; then
  pass "install-accepts-verified-tarball"
else
  fail "install-accepts-verified-tarball"
fi

# install: a checksum mismatch must fail loudly and install nothing
err="$(WASM_PACK_VERSION="$fake_version" \
       WASM_PACK_SHA256="0000000000000000000000000000000000000000000000000000000000000000" \
       WASM_PACK_URL="file://$tmp/fake.tar.gz" \
       WASM_PACK_HOME="$tmp/home-bad" \
       bash "$here/install-wasm-pack.sh" 2>&1)"
status=$?
if [ "$status" -ne 0 ] \
   && [ ! -e "$tmp/home-bad/bin/wasm-pack" ] \
   && printf '%s' "$err" | grep -q "sha256 mismatch"; then
  pass "install-rejects-checksum-mismatch"
else
  fail "install-rejects-checksum-mismatch (status=$status)"
fi

# install: missing required input fails instead of downloading an unpinned tool
if WASM_PACK_VERSION="" WASM_PACK_SHA256="$fake_sha" \
     bash "$here/install-wasm-pack.sh" >/dev/null 2>&1; then
  fail "install-requires-version (exit was 0)"
else
  pass "install-requires-version"
fi

# install-binaryen: seeds the wasm-pack cache from a verified local tarball
# ---------------------------------------------------------------------------
bdir="$tmp/binaryen-version_117"
mkdir -p "$bdir/bin"
cat > "$bdir/bin/wasm-opt" <<'EOS'
#!/usr/bin/env bash
echo "wasm-opt version 117"
EOS
chmod +x "$bdir/bin/wasm-opt"
tar -czf "$tmp/binaryen-fake.tar.gz" -C "$tmp" binaryen-version_117
bsha="$(sha256sum "$tmp/binaryen-fake.tar.gz" | cut -d' ' -f1)"

if BINARYEN_URL="file://$tmp/binaryen-fake.tar.gz" \
   BINARYEN_SHA256="$bsha" \
   BINARYEN_CACHE_KEY="testkey" \
   WASM_PACK_CACHE="$tmp/bcache-ok" \
   bash "$here/install-binaryen.sh" >/dev/null 2>&1 \
   && [ -x "$tmp/bcache-ok/wasm-opt-testkey/bin/wasm-opt" ]; then
  pass "binaryen-seeds-cache"
else
  fail "binaryen-seeds-cache"
fi

# install-binaryen: checksum mismatch must fail loudly and seed nothing
berr="$(BINARYEN_URL="file://$tmp/binaryen-fake.tar.gz" \
        BINARYEN_SHA256="0000000000000000000000000000000000000000000000000000000000000000" \
        BINARYEN_CACHE_KEY="testbad" \
        WASM_PACK_CACHE="$tmp/bcache-bad" \
        bash "$here/install-binaryen.sh" 2>&1)"
bstatus=$?
if [ "$bstatus" -ne 0 ] \
   && [ ! -e "$tmp/bcache-bad/wasm-opt-testbad/bin/wasm-opt" ] \
   && printf '%s' "$berr" | grep -q "sha256 mismatch"; then
  pass "binaryen-rejects-checksum-mismatch"
else
  fail "binaryen-rejects-checksum-mismatch (status=$bstatus)"
fi

echo "helpers: $((cases - failures)) passed, ${failures} failed"
[ "$failures" -eq 0 ]
