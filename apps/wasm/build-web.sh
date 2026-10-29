#!/usr/bin/env bash
# Build the browser package: the WASM module plus wasm-bindgen's `web` glue.
#
# Outputs:
#   target/wasm32-unknown-unknown/release/paper_trails_wasm.wasm   (Rust cdylib)
#   apps/wasm/web/pkg/paper_trails_wasm.js                          (ES module glue)
#   apps/wasm/web/pkg/paper_trails_wasm_bg.wasm                     (copied module)
#   apps/wasm/web/pkg/*.d.ts                                        (unused; harmless)
#
# Requirements: rustup target wasm32-unknown-unknown, and a wasm-bindgen-cli
# whose version matches Cargo.lock (enforced below).

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."   # repository root

lock_version="$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | grep '^version' | head -n1 | sed 's/.*"\(.*\)".*/\1/')"
if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "error: wasm-bindgen CLI not found. Install it with:" >&2
  echo "  cargo install wasm-bindgen-cli --version ${lock_version:-<Cargo.lock version>}" >&2
  exit 1
fi
cli_version="$(wasm-bindgen --version | awk '{print $2}')"
if [ -n "${lock_version:-}" ] && [ "$cli_version" != "$lock_version" ]; then
  echo "error: wasm-bindgen-cli $cli_version does not match Cargo.lock $lock_version" >&2
  echo "  cargo install wasm-bindgen-cli --version $lock_version --force" >&2
  exit 1
fi

cargo build -p paper-trails-wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir apps/wasm/web/pkg \
  target/wasm32-unknown-unknown/release/paper_trails_wasm.wasm

echo "browser package ready: apps/wasm/web/pkg/"
echo "serve it with:         node apps/wasm/web/serve.js"
