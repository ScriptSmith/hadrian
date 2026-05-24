# Testing

## Unit Tests

Unit tests go in the same file as the code (`#[cfg(test)]`). Test both SQLite and PostgreSQL paths for database code.

## E2E Tests

Build the gateway image first, then run the suite:

```bash
docker build -t hadrian:local .   # From repo root — builds the image the compose files reference
cd deploy/tests && pnpm test      # Run all E2E tests
```

Uses TypeScript test suite with testcontainers. The docker-compose files
(`deploy/docker-compose.*.yml`) reference `image: hadrian:local`, and the test
harness rebuilds it on every `compose.up()` by default — which is slow. Pre-build
it once with the command above and set `SKIP_BUILD=1` to reuse it:

```bash
docker build -t hadrian:local .
cd deploy/tests && SKIP_BUILD=1 pnpm test
```

Rebuild the image when you want to test your latest changes; `SKIP_BUILD=1` runs
against whatever `hadrian:local` currently points at.

## Provider Testing (Wiremock)

Provider e2e tests use recorded fixtures instead of live API calls:
- Fixtures in `tests/fixtures/providers/{provider}/` (JSON request/response pairs)
- Tests in `src/tests/provider_e2e.rs` using `rstest` for parameterization
- Adding a provider = add `ProviderTestSpec` + fixture files
- Record new fixtures: `cargo run --bin record_fixtures -- --help`
- Set `HADRIAN_TEST_DEBUG=1` to save test responses to `tests/fixtures/providers/_debug/`

## University E2E Tests

Comprehensive deployment tests with Keycloak OIDC and CEL-based RBAC policies:

```bash
cd deploy/tests && pnpm test university       # Run university tests
cd deploy/tests && pnpm test -- --grep "CEL"  # Run tests matching pattern
```

Tests cover:
- OIDC authentication flow (token acquisition, claim verification)
- CEL policy enforcement (cross-org isolation, role boundaries)
- Budget enforcement and usage tracking
- RAG/vector stores with cross-org permission isolation
- Streaming API (SSE format, chunked responses)
