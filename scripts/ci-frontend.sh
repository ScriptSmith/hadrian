#!/usr/bin/env bash
# Frontend CI checks - React/TypeScript
# Usage: ./scripts/ci-frontend.sh [--fix] [--skip-storybook]

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
UI_DIR="$ROOT_DIR/ui"

FIX_MODE=false
SKIP_STORYBOOK=false

for arg in "$@"; do
    case $arg in
        --fix)
            FIX_MODE=true
            ;;
        --skip-storybook)
            SKIP_STORYBOOK=true
            ;;
    esac
done

cd "$UI_DIR"

step() {
    echo -e "\n${BLUE}==>${NC} ${YELLOW}$1${NC}"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

FAILED=0

# Check if node_modules exists
if [ ! -d "node_modules" ]; then
    step "Installing dependencies"
    pnpm install --frozen-lockfile
fi

# Lint
step "Linting"
echo -e "  ${BLUE}\$${NC} pnpm lint"
if [ "$FIX_MODE" = true ]; then
    pnpm lint:fix || true
    success "Applied lint fixes"
else
    if pnpm lint; then
        success "Lint passed"
    else
        echo -e "${RED}✗${NC} Lint failed. Run with --fix to auto-fix"
        FAILED=1
    fi
fi

# Format
step "Checking format"
echo -e "  ${BLUE}\$${NC} pnpm format:check"
if [ "$FIX_MODE" = true ]; then
    pnpm format:fix || pnpm format --write || true
    success "Formatted code"
else
    if pnpm format:check; then
        success "Format check passed"
    else
        echo -e "${RED}✗${NC} Format check failed. Run with --fix to auto-format"
        FAILED=1
    fi
fi

# Type check (not in TODO.md but useful)
step "Type checking"
echo -e "  ${BLUE}\$${NC} pnpm exec tsc --noEmit"
if pnpm exec tsc --noEmit; then
    success "Type check passed"
else
    echo -e "${RED}✗${NC} Type check failed"
    FAILED=1
fi

# Build (not in TODO.md but useful)
step "Building"
echo -e "  ${BLUE}\$${NC} pnpm build"
if pnpm build; then
    success "Build passed"
else
    echo -e "${RED}✗${NC} Build failed"
    FAILED=1
fi

# Unit tests
step "Running unit tests"
echo -e "  ${BLUE}\$${NC} pnpm test"
if pnpm test; then
    success "Unit tests passed"
else
    echo -e "${RED}✗${NC} Unit tests failed"
    FAILED=1
fi

# Storybook
if [ "$SKIP_STORYBOOK" = false ]; then
    step "Building Storybook"
    echo -e "  ${BLUE}\$${NC} pnpm storybook:build"
    if pnpm storybook:build; then
        success "Storybook build passed"

        # Run storybook tests
        if grep -q "test-storybook" package.json; then
            step "Running Storybook tests"
            echo -e "  ${BLUE}\$${NC} pnpm test-storybook"
            if npx concurrently -k -s first -n "SB,TEST" \
                "npx http-server storybook-static --port 6006 --silent" \
                "npx wait-on tcp:127.0.0.1:6006 && pnpm test-storybook" 2>/dev/null; then
                success "Storybook tests passed"
            else
                echo -e "${RED}✗${NC} Storybook tests failed"
                FAILED=1
            fi
        fi
    else
        echo -e "${RED}✗${NC} Storybook build failed"
        FAILED=1
    fi
else
    echo -e "${YELLOW}Skipping Storybook checks${NC}"
fi

# Summary
echo ""
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All frontend checks passed!${NC}"
    exit 0
else
    echo -e "${RED}Some checks failed${NC}"
    exit 1
fi
