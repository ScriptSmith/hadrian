#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Generating config JSON schema..."
cargo run --features json-schema -- schema --output "$ROOT_DIR/docs/public/config-schema.json"
echo "Done: docs/public/config-schema.json"
