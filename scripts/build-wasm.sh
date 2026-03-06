#!/usr/bin/env bash
# Build the WASM module for browser deployment.
#
# Prerequisites:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-pack
#
# Usage:
#   ./scripts/build-wasm.sh              # Build WASM module
#   ./scripts/build-wasm.sh --release    # Build optimized WASM module

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
OUT_DIR="$ROOT_DIR/ui/public/wasm"

PROFILE="dev"
WASM_PACK_FLAGS="--dev"
if [ "${1:-}" = "--release" ]; then
    PROFILE="release"
    WASM_PACK_FLAGS="--release"
fi

echo "==> Building Hadrian WASM module (profile: $PROFILE)"

# Ensure wasm32 target is installed
if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    echo "==> Installing wasm32-unknown-unknown target"
    rustup target add wasm32-unknown-unknown
fi

# Build with wasm-pack
# --dev skips wasm-opt (avoids bulk-memory feature mismatch)
# --release runs wasm-opt for size optimization
cd "$ROOT_DIR"
wasm-pack build \
    --target web \
    --out-dir "$OUT_DIR" \
    $WASM_PACK_FLAGS \
    -- \
    --no-default-features \
    --features wasm

# Copy sql.js WASM binary alongside the Hadrian WASM output.
# The sqlite-bridge.ts service worker code loads it from /wasm/sql-wasm.wasm.
SQLJS_WASM="$ROOT_DIR/ui/node_modules/sql.js/dist/sql-wasm.wasm"
if [ -f "$SQLJS_WASM" ]; then
    cp "$SQLJS_WASM" "$OUT_DIR/sql-wasm.wasm"
    echo "==> Copied sql-wasm.wasm to $OUT_DIR"
else
    echo "WARNING: sql-wasm.wasm not found at $SQLJS_WASM — run 'pnpm install' in ui/ first"
fi

echo "==> WASM build complete: $OUT_DIR"
echo "    Files:"
ls -lh "$OUT_DIR"/*.wasm "$OUT_DIR"/*.js 2>/dev/null || true
