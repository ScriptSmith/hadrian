#!/usr/bin/env bash
# Generate code coverage reports for Rust backend and TypeScript frontend
# Usage: ./scripts/coverage.sh [backend|frontend|all] [--html]

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
COVERAGE_DIR="$ROOT_DIR/coverage"

TARGET="${1:-all}"
HTML_REPORT=false
if [[ "${*}" == *"--html"* ]]; then
    HTML_REPORT=true
fi

step() {
    echo -e "\n${BLUE}==>${NC} ${YELLOW}$1${NC}"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

error() {
    echo -e "${RED}✗${NC} $1"
}

# Create coverage directories
mkdir -p "$COVERAGE_DIR/rust"
mkdir -p "$COVERAGE_DIR/typescript"

run_backend_coverage() {
    step "Running Rust backend coverage"

    # Check if cargo-llvm-cov is installed
    if ! command -v cargo-llvm-cov &> /dev/null; then
        error "cargo-llvm-cov not installed. Install with: cargo install cargo-llvm-cov"
        return 1
    fi

    cd "$ROOT_DIR"

    # Clean previous coverage data
    cargo llvm-cov clean --workspace

    if [ "$HTML_REPORT" = true ]; then
        # Generate HTML report
        cargo llvm-cov --html --output-dir "$COVERAGE_DIR/rust" -- --include-ignored
        success "HTML report generated at $COVERAGE_DIR/rust/html/index.html"
    fi

    # Generate LCOV report (for badges and CI)
    cargo llvm-cov --lcov --output-path "$COVERAGE_DIR/rust/lcov.info" -- --include-ignored
    success "LCOV report generated at $COVERAGE_DIR/rust/lcov.info"

    # Generate JSON summary for badge generation
    cargo llvm-cov --json --output-path "$COVERAGE_DIR/rust/coverage.json" -- --include-ignored
    success "JSON report generated at $COVERAGE_DIR/rust/coverage.json"

    # Print summary
    echo ""
    cargo llvm-cov report --summary-only
}

run_frontend_coverage() {
    step "Running TypeScript frontend coverage"

    cd "$ROOT_DIR/ui"

    # Check if node_modules exists
    if [ ! -d "node_modules" ]; then
        step "Installing dependencies"
        pnpm install --frozen-lockfile
    fi

    if [ "$HTML_REPORT" = true ]; then
        # Run tests with coverage and HTML report
        pnpm vitest run --project=storybook --coverage --coverage.reporter=html --coverage.reporter=lcov --coverage.reporter=json-summary --coverage.reportsDirectory="$COVERAGE_DIR/typescript" || true
        success "HTML report generated at $COVERAGE_DIR/typescript/index.html"
    else
        # Run tests with coverage (lcov and json-summary only)
        pnpm vitest run --project=storybook --coverage --coverage.reporter=lcov --coverage.reporter=json-summary --coverage.reportsDirectory="$COVERAGE_DIR/typescript" || true
    fi

    success "LCOV report generated at $COVERAGE_DIR/typescript/lcov.info"

    # Print summary if json-summary exists
    if [ -f "$COVERAGE_DIR/typescript/coverage-summary.json" ]; then
        echo ""
        echo "Frontend Coverage Summary:"
        jq -r '.total | "  Lines: \(.lines.pct)% (\(.lines.covered)/\(.lines.total))\n  Statements: \(.statements.pct)% (\(.statements.covered)/\(.statements.total))\n  Functions: \(.functions.pct)% (\(.functions.covered)/\(.functions.total))\n  Branches: \(.branches.pct)% (\(.branches.covered)/\(.branches.total))"' "$COVERAGE_DIR/typescript/coverage-summary.json" 2>/dev/null || echo "  (summary not available)"
    fi
}

generate_badge_data() {
    step "Generating badge data"

    # Extract Rust coverage percentage from JSON
    if [ -f "$COVERAGE_DIR/rust/coverage.json" ]; then
        RUST_PCT=$(jq -r '.data[0].totals.lines.percent // 0' "$COVERAGE_DIR/rust/coverage.json" 2>/dev/null || echo "0")
        RUST_PCT_INT=$(printf "%.0f" "$RUST_PCT")
        echo "Rust coverage: ${RUST_PCT_INT}%"
    fi

    # Extract TypeScript coverage percentage
    if [ -f "$COVERAGE_DIR/typescript/coverage-summary.json" ]; then
        TS_PCT=$(jq -r '.total.lines.pct // 0' "$COVERAGE_DIR/typescript/coverage-summary.json" 2>/dev/null || echo "0")
        TS_PCT_INT=$(printf "%.0f" "$TS_PCT")
        echo "TypeScript coverage: ${TS_PCT_INT}%"
    fi

    # Generate badge JSON for shields.io endpoint badge
    cat > "$COVERAGE_DIR/badge-rust.json" << EOF
{
  "schemaVersion": 1,
  "label": "rust coverage",
  "message": "${RUST_PCT_INT:-0}%",
  "color": "$(coverage_color "${RUST_PCT_INT:-0}")"
}
EOF

    cat > "$COVERAGE_DIR/badge-typescript.json" << EOF
{
  "schemaVersion": 1,
  "label": "typescript coverage",
  "message": "${TS_PCT_INT:-0}%",
  "color": "$(coverage_color "${TS_PCT_INT:-0}")"
}
EOF

    success "Badge data generated in $COVERAGE_DIR"
}

coverage_color() {
    local pct=$1
    if [ "$pct" -ge 80 ]; then
        echo "brightgreen"
    elif [ "$pct" -ge 60 ]; then
        echo "green"
    elif [ "$pct" -ge 40 ]; then
        echo "yellow"
    elif [ "$pct" -ge 20 ]; then
        echo "orange"
    else
        echo "red"
    fi
}

# Main execution
case "$TARGET" in
    backend|rust)
        run_backend_coverage
        ;;
    frontend|typescript|ts)
        run_frontend_coverage
        ;;
    all)
        run_backend_coverage
        run_frontend_coverage
        generate_badge_data
        ;;
    *)
        echo "Usage: $0 [backend|frontend|all] [--html]"
        echo ""
        echo "Options:"
        echo "  backend|rust      Run Rust backend coverage only"
        echo "  frontend|ts       Run TypeScript frontend coverage only"
        echo "  all               Run both (default)"
        echo "  --html            Generate HTML reports (in addition to LCOV)"
        echo ""
        echo "Output: ./coverage/"
        exit 1
        ;;
esac

echo ""
echo -e "${GREEN}Coverage reports generated in $COVERAGE_DIR${NC}"
