#!/usr/bin/env bash
# Fetch the models.dev catalog for embedding in the gateway binary
#
# Usage: ./scripts/fetch-model-catalog.sh
#
# This script downloads the model catalog from models.dev and saves it to
# data/models-dev-catalog.json. The file is gitignored and must be fetched
# before building the gateway.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
DATA_DIR="$ROOT_DIR/data"
OUTPUT_FILE="$DATA_DIR/models-dev-catalog.json"
API_URL="https://models.dev/api.json"

mkdir -p "$DATA_DIR"

echo "Fetching model catalog from $API_URL..."

if command -v curl &> /dev/null; then
    curl -sSL "$API_URL" -o "$OUTPUT_FILE"
elif command -v wget &> /dev/null; then
    wget -q "$API_URL" -O "$OUTPUT_FILE"
else
    echo "Error: Neither curl nor wget found. Please install one of them."
    exit 1
fi

# Validate JSON
if command -v jq &> /dev/null; then
    if ! jq empty "$OUTPUT_FILE" 2>/dev/null; then
        echo "Error: Downloaded file is not valid JSON"
        rm -f "$OUTPUT_FILE"
        exit 1
    fi

    # Count providers and models
    PROVIDER_COUNT=$(jq 'length' "$OUTPUT_FILE")
    MODEL_COUNT=$(jq '[.[].models | length] | add' "$OUTPUT_FILE")
    echo "Downloaded catalog with $PROVIDER_COUNT providers and $MODEL_COUNT models"
else
    echo "Warning: jq not found, skipping JSON validation"
fi

echo "Saved to $OUTPUT_FILE"
