#!/usr/bin/env bash
# Local CI script - runs all checks that would run in CI
# Usage: ./scripts/ci.sh [--backend] [--frontend] [--docker]

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# Default: run everything except e2e and docker
RUN_BACKEND=true
RUN_FRONTEND=true
RUN_DOCS=true
RUN_HELM=true
RUN_E2E=false
RUN_DOCKER=false

# Parse arguments
for arg in "$@"; do
    case $arg in
        --backend)
            RUN_BACKEND=true
            RUN_FRONTEND=false
            RUN_DOCS=false
            RUN_HELM=false
            ;;
        --frontend)
            RUN_BACKEND=false
            RUN_FRONTEND=true
            RUN_DOCS=false
            RUN_HELM=false
            ;;
        --docs)
            RUN_BACKEND=false
            RUN_FRONTEND=false
            RUN_DOCS=true
            RUN_HELM=false
            ;;
        --helm)
            RUN_BACKEND=false
            RUN_FRONTEND=false
            RUN_DOCS=false
            RUN_HELM=true
            ;;
        --e2e)
            RUN_BACKEND=false
            RUN_FRONTEND=false
            RUN_DOCS=false
            RUN_HELM=false
            RUN_E2E=true
            ;;
        --docker)
            RUN_DOCKER=true
            ;;
        --all)
            RUN_BACKEND=true
            RUN_FRONTEND=true
            RUN_DOCS=true
            RUN_HELM=true
            RUN_E2E=true
            RUN_DOCKER=true
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --backend   Run backend checks only"
            echo "  --frontend  Run frontend checks only"
            echo "  --docs      Run docs checks only"
            echo "  --helm      Run helm checks only"
            echo "  --e2e       Run e2e tests only (requires Docker)"
            echo "  --docker    Include Docker build"
            echo "  --all       Run everything including e2e and Docker"
            echo "  --help      Show this help"
            exit 0
            ;;
    esac
done

cd "$ROOT_DIR"

step() {
    echo -e "\n${BLUE}==>${NC} ${YELLOW}$1${NC}"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

fail() {
    echo -e "${RED}✗${NC} $1"
    exit 1
}

FAILED=()

run_check() {
    local name="$1"
    shift
    step "$name"
    echo -e "  ${BLUE}\$${NC} $*"
    if "$@"; then
        success "$name passed"
    else
        FAILED+=("$name")
        echo -e "${RED}✗${NC} $name failed"
    fi
}

# Backend checks
if [ "$RUN_BACKEND" = true ]; then
    echo -e "\n${BLUE}═══════════════════════════════════════${NC}"
    echo -e "${BLUE}       BACKEND CHECKS (Rust)${NC}"
    echo -e "${BLUE}═══════════════════════════════════════${NC}"

    run_check "Check" cargo check

    run_check "Format check (nightly)" cargo +nightly fmt -- --check

    run_check "Clippy" cargo clippy --all-targets --all-features -- -D clippy::correctness -W clippy::style

    run_check "Tests (unit + integration)" cargo test -- --include-ignored

    run_check "Security audit" cargo audit || true  # Don't fail on audit warnings
fi

# Frontend checks
if [ "$RUN_FRONTEND" = true ]; then
    echo -e "\n${BLUE}═══════════════════════════════════════${NC}"
    echo -e "${BLUE}       FRONTEND CHECKS (React)${NC}"
    echo -e "${BLUE}═══════════════════════════════════════${NC}"

    cd "$ROOT_DIR/ui"

    if [ ! -d "node_modules" ]; then
        step "Installing dependencies"
        pnpm install --frozen-lockfile
    fi

    run_check "Lint" pnpm lint

    run_check "Format check" pnpm format:check

    run_check "Type check" pnpm exec tsc --noEmit

    run_check "Build" pnpm build

    run_check "Unit tests" pnpm test

    # Storybook checks (optional - can be slow)
    if [ -f "package.json" ] && grep -q "storybook:build" package.json; then
        run_check "Storybook build" pnpm storybook:build

        # Run storybook tests if test-storybook exists
        if grep -q "test-storybook" package.json; then
            step "Storybook tests"
            echo "  (Starting storybook server and running tests...)"
            if npx concurrently -k -s first -n "SB,TEST" \
                "npx http-server storybook-static --port 6006 --silent" \
                "npx wait-on tcp:127.0.0.1:6006 && pnpm test-storybook" 2>/dev/null; then
                success "Storybook tests passed"
            else
                FAILED+=("Storybook tests")
                echo -e "${RED}✗${NC} Storybook tests failed"
            fi
        fi
    fi

    cd "$ROOT_DIR"
fi

# Docs checks
if [ "$RUN_DOCS" = true ]; then
    echo -e "\n${BLUE}═══════════════════════════════════════${NC}"
    echo -e "${BLUE}       DOCS CHECKS (Fumadocs)${NC}"
    echo -e "${BLUE}═══════════════════════════════════════${NC}"

    # Build Storybook first (docs embeds it via symlink)
    cd "$ROOT_DIR/ui"

    if [ ! -d "node_modules" ]; then
        step "Installing UI dependencies"
        pnpm install --frozen-lockfile
    fi

    if [ ! -d "storybook-static" ] || [ "$ROOT_DIR/ui/src" -nt "storybook-static" ]; then
        run_check "Storybook build" pnpm storybook:build
    else
        echo -e "${YELLOW}  Storybook already built, skipping...${NC}"
    fi

    # Build docs
    cd "$ROOT_DIR/docs"

    if [ ! -d "node_modules" ]; then
        step "Installing docs dependencies"
        pnpm install --frozen-lockfile
    fi

    run_check "Docs lint" pnpm lint

    run_check "Docs format check" pnpm format:check

    run_check "Docs type check" pnpm types:check

    run_check "Docs build" pnpm build

    cd "$ROOT_DIR"
fi

# Helm checks
if [ "$RUN_HELM" = true ]; then
    echo -e "\n${BLUE}═══════════════════════════════════════${NC}"
    echo -e "${BLUE}       HELM CHECKS${NC}"
    echo -e "${BLUE}═══════════════════════════════════════${NC}"

    HELM_DIR="$ROOT_DIR/helm/hadrian"

    if ! command -v helm &> /dev/null; then
        echo -e "${YELLOW}Helm not installed, skipping helm checks${NC}"
    else
        # Add bitnami repo if not present
        if ! helm repo list 2>/dev/null | grep -q bitnami; then
            step "Adding Bitnami repo"
            helm repo add bitnami https://charts.bitnami.com/bitnami
        fi

        step "Updating dependencies"
        helm dependency update "$HELM_DIR"

        run_check "Helm lint" helm lint "$HELM_DIR"

        run_check "Helm lint (strict)" helm lint "$HELM_DIR" --strict

        run_check "Helm template" helm template test "$HELM_DIR" --debug > /dev/null
    fi
fi

# E2E tests
if [ "$RUN_E2E" = true ]; then
    echo -e "\n${BLUE}═══════════════════════════════════════${NC}"
    echo -e "${BLUE}       E2E TESTS (TypeScript)${NC}"
    echo -e "${BLUE}═══════════════════════════════════════${NC}"

    if ! command -v docker &> /dev/null; then
        echo -e "${RED}Error: docker is required for e2e tests${NC}"
        FAILED+=("E2E tests (docker not found)")
    else
        cd "$ROOT_DIR/deploy/tests"

        if [ ! -d "node_modules" ]; then
            step "Installing E2E test dependencies"
            pnpm install --frozen-lockfile
        fi

        # Generate API client if needed
        if [ ! -d "src/client" ] || [ "$ROOT_DIR/openapi/hadrian.openapi.json" -nt "src/client/index.ts" ]; then
            step "Generating API client"
            pnpm generate-client
        fi

        run_check "E2E tests" pnpm test

        cd "$ROOT_DIR"
    fi
fi

# Docker build
if [ "$RUN_DOCKER" = true ]; then
    echo -e "\n${BLUE}═══════════════════════════════════════${NC}"
    echo -e "${BLUE}       DOCKER BUILD${NC}"
    echo -e "${BLUE}═══════════════════════════════════════${NC}"

    run_check "Docker build" docker build -t hadrian:ci-test .
fi

# Summary
echo -e "\n${BLUE}═══════════════════════════════════════${NC}"
echo -e "${BLUE}       SUMMARY${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"

if [ ${#FAILED[@]} -eq 0 ]; then
    echo -e "\n${GREEN}All checks passed!${NC}\n"
    exit 0
else
    echo -e "\n${RED}Failed checks:${NC}"
    for check in "${FAILED[@]}"; do
        echo -e "  ${RED}✗${NC} $check"
    done
    echo ""
    exit 1
fi
