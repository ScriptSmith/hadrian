#!/bin/bash
# Generate OpenAPI spec and save to ui/src/api/openapi.json
#
# Usage: ./scripts/generate-openapi.sh [--no-build]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_FILE="$PROJECT_ROOT/ui/src/api/openapi.json"

# Export OpenAPI spec directly from the binary
echo "Exporting OpenAPI spec..."
cargo run -- openapi --output "$OUTPUT_FILE"

# Generate the client SDK
cd "${PROJECT_ROOT}/ui" && pnpm run generate-api

echo "Done!"
