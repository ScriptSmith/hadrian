# Run all CI checks (sequential)
check: check-rust check-ui check-docs check-openapi check-helm check-security

# Run all CI checks including slow ones
check-all: check check-features check-e2e

# Backend checks (matches CI backend job)
check-rust:
    #!/usr/bin/env bash
    set -e
    run() {
        echo "→ $*"
        if ! output=$("$@" 2>&1); then
            echo "$output"
            exit 1
        fi
    }
    run cargo +nightly fmt -- --check
    run cargo clippy --all-targets --all-features -- -D clippy::correctness -W clippy::style
    run cargo check
    run cargo test -- --include-ignored

# Feature matrix checks (matches CI feature-check job)
# Usage: just check-features [profile]
# Examples: just check-features, just check-features tiny, just check-features minimal
check-features profile="all":
    #!/usr/bin/env bash
    set -e
    run() {
        echo "→ (features/$1) ${*:2}"
        if ! output=$("${@:2}" 2>&1); then
            echo "$output"
            exit 1
        fi
    }
    profiles="{{ profile }}"
    if [ "$profiles" = "all" ]; then
        profiles="tiny minimal standard full"
    fi
    for p in $profiles; do
        run "$p" cargo check --no-default-features --features "$p" --all-targets
        run "$p" cargo clippy --no-default-features --features "$p" --all-targets -- -D warnings
        run "$p" cargo test --no-default-features --features "$p" --all-targets -- --include-ignored
    done

# Frontend checks (matches CI frontend job)
check-ui:
    #!/usr/bin/env bash
    set -e
    cd ui
    run() {
        echo "→ (ui) $*"
        if ! output=$("$@" 2>&1); then
            echo "$output"
            exit 1
        fi
    }
    run pnpm lint
    run pnpm format:check
    run pnpm exec tsc --noEmit
    run pnpm build
    run pnpm test
    run pnpm test-storybook
    run pnpm storybook:build

# Documentation checks (matches CI docs job)
check-docs:
    #!/usr/bin/env bash
    set -e
    run() {
        echo "→ $*"
        if ! output=$("$@" 2>&1); then
            echo "$output"
            exit 1
        fi
    }
    # Build storybook first (docs embeds via symlink)
    echo "→ (ui) pnpm storybook:build"
    (cd ui && pnpm storybook:build) 2>&1 || exit 1
    cd docs
    run pnpm lint
    run pnpm format:check
    run pnpm types:check
    run pnpm build

# OpenAPI conformance check (matches CI openapi-conformance job)
check-openapi:
    #!/usr/bin/env bash
    set -e
    echo "→ openapi-conformance"
    ./scripts/openapi-conformance.py

# Helm checks (matches CI helm-lint job)
check-helm:
    #!/usr/bin/env bash
    set -e
    run() {
        echo "→ (helm) $*"
        if ! output=$("$@" 2>&1); then
            echo "$output"
            exit 1
        fi
    }
    run helm lint helm/hadrian
    run helm lint helm/hadrian --strict
    run helm template test helm/hadrian

# Security audits (matches CI security-audit, cargo-deny, security-frontend jobs)
check-security:
    #!/usr/bin/env bash
    set -e
    run() {
        echo "→ $*"
        if ! output=$("$@" 2>&1); then
            echo "$output"
            exit 1
        fi
    }
    run cargo audit
    run cargo deny check
    echo "→ (ui) pnpm audit"
    (cd ui && pnpm audit --audit-level=high) 2>&1 || exit 1

# E2E tests (matches CI e2e job)
check-e2e:
    #!/usr/bin/env bash
    set -e
    cd deploy/tests
    run() {
        echo "→ (e2e) $*"
        if ! output=$("$@" 2>&1); then
            echo "$output"
            exit 1
        fi
    }
    run pnpm generate-client
    run pnpm test

# Auto-fix formatting and lint issues
fix: fix-rust fix-ui fix-docs

# Fix backend formatting and lints
fix-rust:
    #!/usr/bin/env bash
    set -e
    echo "→ cargo +nightly fmt"
    cargo +nightly fmt
    echo "→ cargo clippy --fix"
    cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged

# Fix frontend formatting and lints
fix-ui:
    #!/usr/bin/env bash
    set -e
    cd ui
    echo "→ (ui) pnpm lint:fix"
    pnpm lint:fix
    echo "→ (ui) pnpm format"
    pnpm format

# Fix docs formatting and lints
fix-docs:
    #!/usr/bin/env bash
    set -e
    cd docs
    echo "→ (docs) pnpm lint:fix"
    pnpm lint:fix
    echo "→ (docs) pnpm format"
    pnpm format

# Run independent groups in parallel
check-parallel:
    #!/usr/bin/env bash
    set -e

    just check-rust &
    pid_rust=$!
    just check-ui &
    pid_ui=$!
    just check-docs &
    pid_docs=$!
    just check-openapi &
    pid_openapi=$!
    just check-helm &
    pid_helm=$!
    just check-security &
    pid_security=$!

    failed=0
    wait $pid_helm || failed=1
    wait $pid_openapi || failed=1
    wait $pid_security || failed=1
    wait $pid_docs || failed=1
    wait $pid_rust || failed=1
    wait $pid_ui || failed=1

    exit $failed
