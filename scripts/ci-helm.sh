#!/usr/bin/env bash
# Helm CI checks
# Usage: ./scripts/ci-helm.sh

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
HELM_DIR="$ROOT_DIR/helm/hadrian"

step() {
    echo -e "\n${BLUE}==>${NC} ${YELLOW}$1${NC}"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

FAILED=0

# Check if helm is installed
if ! command -v helm &> /dev/null; then
    echo -e "${RED}Error: helm is not installed${NC}"
    echo "Install helm: https://helm.sh/docs/intro/install/"
    exit 1
fi

# Add bitnami repo if not present
if ! helm repo list 2>/dev/null | grep -q bitnami; then
    step "Adding Bitnami repo"
    helm repo add bitnami https://charts.bitnami.com/bitnami
fi

# Update dependencies
step "Updating dependencies"
helm dependency update "$HELM_DIR"

# Lint
step "Linting chart"
if helm lint "$HELM_DIR"; then
    success "Lint passed"
else
    echo -e "${RED}✗${NC} Lint failed"
    FAILED=1
fi

# Lint (strict)
step "Linting chart (strict mode)"
if helm lint "$HELM_DIR" --strict; then
    success "Strict lint passed"
else
    echo -e "${RED}✗${NC} Strict lint failed"
    FAILED=1
fi

# Template with default values
step "Templating chart (default values)"
if helm template test "$HELM_DIR" --debug > /dev/null; then
    success "Template passed"
else
    echo -e "${RED}✗${NC} Template failed"
    FAILED=1
fi

# Template with PostgreSQL enabled
step "Templating chart (PostgreSQL)"
if helm template test "$HELM_DIR" --debug \
    --set postgresql.enabled=true \
    --set postgresql.auth.password=testpassword \
    --set gateway.database.type=postgres > /dev/null; then
    success "PostgreSQL template passed"
else
    echo -e "${RED}✗${NC} PostgreSQL template failed"
    FAILED=1
fi

# Template with Redis enabled
step "Templating chart (Redis)"
if helm template test "$HELM_DIR" --debug \
    --set redis.enabled=true \
    --set redis.auth.password=testpassword \
    --set gateway.cache.type=redis > /dev/null; then
    success "Redis template passed"
else
    echo -e "${RED}✗${NC} Redis template failed"
    FAILED=1
fi

# Validate values.schema.json if ajv-cli is available
if command -v ajv &> /dev/null; then
    step "Validating values against schema"
    if ajv validate -s "$HELM_DIR/values.schema.json" -d "$HELM_DIR/values.yaml" --spec=draft2020 -c ajv-formats 2>/dev/null; then
        success "Schema validation passed"
    else
        echo -e "${RED}✗${NC} Schema validation failed"
        FAILED=1
    fi
else
    echo -e "${YELLOW}ajv-cli not installed, skipping schema validation${NC}"
    echo "  Install with: npm install -g ajv-cli ajv-formats"
fi

# Summary
echo ""
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All helm checks passed!${NC}"
    exit 0
else
    echo -e "${RED}Some checks failed${NC}"
    exit 1
fi
