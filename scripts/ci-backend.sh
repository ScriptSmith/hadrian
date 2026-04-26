#!/usr/bin/env bash
# Backend CI checks - Rust
# Usage: ./scripts/ci-backend.sh [--fix]

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

FIX_MODE=false
if [ "${1:-}" = "--fix" ]; then
    FIX_MODE=true
fi

cd "$ROOT_DIR"

step() {
    echo -e "\n${BLUE}==>${NC} ${YELLOW}$1${NC}"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

FAILED=0

# Check
step "Running cargo check"
echo -e "  ${BLUE}\$${NC} cargo check"
if cargo check; then
    success "Check passed"
else
    echo -e "${RED}✗${NC} Check failed"
    FAILED=1
fi

# Format
step "Checking formatting"
echo -e "  ${BLUE}\$${NC} cargo +nightly fmt -- --check"
if [ "$FIX_MODE" = true ]; then
    cargo +nightly fmt
    success "Formatted code"
else
    if cargo +nightly fmt -- --check; then
        success "Format check passed"
    else
        echo -e "${RED}✗${NC} Format check failed. Run with --fix to auto-format"
        FAILED=1
    fi
fi

# Clippy
step "Running Clippy"
echo -e "  ${BLUE}\$${NC} cargo clippy --all-targets --all-features -- -D clippy::correctness -W clippy::style -W clippy::complexity -W clippy::perf"
if [ "$FIX_MODE" = true ]; then
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features || true
    success "Applied clippy fixes"
else
    # Use -W (warn) instead of -D (deny) for style lints to allow gradual cleanup
    # Core correctness issues still fail
    if cargo clippy --all-targets --all-features -- \
        -D clippy::correctness \
        -W clippy::style \
        -W clippy::complexity \
        -W clippy::perf; then
        success "Clippy passed"
    else
        echo -e "${RED}✗${NC} Clippy failed"
        FAILED=1
    fi
fi

# Tests (unit + integration)
step "Running tests (unit + integration)"
echo -e "  ${BLUE}\$${NC} cargo test -- --include-ignored"
if cargo test -- --include-ignored; then
    success "Tests passed"
else
    echo -e "${RED}✗${NC} Tests failed"
    FAILED=1
fi

# Security audit
step "Security audit"
if command -v cargo-audit &> /dev/null; then
    if ! cargo audit; then
        echo -e "${RED}✗${NC} Security audit failed"
        FAILED=1
    fi
else
    echo "  cargo-audit not installed, skipping"
fi

# SQLite repos must use truncate_to_millis-bound RFC-3339 timestamps, not
# datetime('now'), so cursor pagination and TEXT comparisons stay consistent
# (see CLAUDE.md "Cursor pagination timestamps"). DEFAULT clauses in CREATE
# TABLE are fine (only fire when no value is bound), so we exclude them.
step "Checking for datetime('now') in SQLite query bodies"
if datetime_hits=$(grep -RIn "datetime('now')" src/db/sqlite \
    | grep -v "DEFAULT (datetime('now'))" || true) && [ -n "$datetime_hits" ]; then
    echo -e "${RED}✗${NC} datetime('now') found in SQLite repo queries; bind truncate_to_millis(Utc::now()) instead:"
    echo "$datetime_hits"
    FAILED=1
else
    success "No stray datetime('now') in SQLite query bodies"
fi

# Summary
echo ""
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All backend checks passed!${NC}"
    exit 0
else
    echo -e "${RED}Some checks failed${NC}"
    exit 1
fi
