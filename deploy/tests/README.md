# E2E Test Suite

End-to-end tests for Hadrian Gateway using Testcontainers and Vitest.

## Prerequisites

- Node.js 20+
- pnpm 9+
- Docker (for testcontainers)

## Running Tests

```bash
# Install dependencies
pnpm install

# Generate API client from OpenAPI spec
pnpm generate-client

# Run all tests
pnpm test

# Run tests with UI
pnpm test:ui

# Run tests in watch mode
pnpm test:watch

# Run a specific workspace
pnpm test -- --project=basic
pnpm test -- --project=infrastructure
pnpm test -- --project=auth

# Run a specific test file
pnpm test -- sqlite.test.ts

# Run tests matching a pattern
pnpm test -- --grep "health"
```

For auth tests that use Playwright (SAML browser automation), install browsers first:

```bash
npx playwright install chromium --with-deps
```

## Directory Structure

```
deploy/tests/
├── docker-compose -> ../        # Symlink to compose files
├── coverage/                    # API coverage reports
├── src/
│   ├── client/                  # Generated OpenAPI client
│   ├── fixtures/                # Test fixtures (compose helpers, keycloak, etc.)
│   ├── reporters/               # Custom Vitest reporters
│   ├── tests/
│   │   ├── basic/               # Basic deployment tests (sqlite, postgres, redis)
│   │   ├── infrastructure/      # Infrastructure tests (HA, clustering, observability)
│   │   ├── auth/                # Authentication tests (keycloak, SAML, university)
│   │   └── shared/              # Shared test suites (health, CRUD, streaming, etc.)
│   └── utils/                   # Utilities (retry, coverage tracking, fetch wrapper)
├── vitest.config.ts             # Base Vitest configuration
└── vitest.workspace.ts          # Workspace configuration (parallel execution)
```

## Test Workspaces

Tests are organized into three workspaces that can run in parallel:

| Workspace | Description | Files |
|-----------|-------------|-------|
| `basic` | Core deployment scenarios (SQLite, PostgreSQL, Redis) | `src/tests/basic/*.test.ts` |
| `infrastructure` | Advanced infrastructure (HA, clustering, observability) | `src/tests/infrastructure/*.test.ts` |
| `auth` | Authentication flows (OIDC, SAML, RBAC) | `src/tests/auth/*.test.ts` |

Each test file runs in its own Docker environment with unique project names. Port allocation is handled via environment variables passed to docker-compose, with each test file using unique host ports to avoid conflicts during parallel execution. Testcontainers' `getMappedPort()` retrieves the actual allocated ports.

## Adding New Deployment Tests

1. **Create a test file** in the appropriate workspace directory:
   ```typescript
   // src/tests/basic/my-deployment.test.ts
   import { describe, beforeAll, afterAll } from "vitest";
   import {
     startComposeEnvironment,
     createTrackedClient,
     type StartedComposeEnvironment,
   } from "../../fixtures";
   import { createConfig } from "../../client/client";
   import type { Client } from "../../client/client";
   import { runHealthCheckTests } from "../shared/health-checks";
   import { runAdminApiCrudTests } from "../shared/admin-api-crud";

   describe("My Deployment", () => {
     let env: StartedComposeEnvironment;
     let gatewayUrl: string;
     let client: Client;
     const testName = "my-deployment";

     beforeAll(async () => {
       // Port allocation - use unique ports to avoid conflicts with other tests
       // Check existing test files for used ports and pick unused ones
       const gatewayPort = 8091;
       const redisPort = 6382;

       env = await startComposeEnvironment({
         projectName: "hadrian-e2e-my-deployment",
         composeFile: "docker-compose.my-deployment.yml",
         waitForServices: {
           gateway: { port: 8080, path: "/health" },
         },
         env: {
           GATEWAY_PORT: String(gatewayPort),
           REDIS_PORT: String(redisPort),
         },
       });
       gatewayUrl = env.getServiceUrl("gateway", 8080);
       client = createTrackedClient(createConfig({ baseUrl: gatewayUrl }));
     }, 300_000);

     afterAll(async () => {
       await env?.stop();
     });

     // Run shared test suites
     runHealthCheckTests(() => ({ url: gatewayUrl, client }));
     runAdminApiCrudTests(() => ({ url: gatewayUrl, client, testName }));

     // Add deployment-specific tests
     describe("My Specific Tests", () => {
       // ...
     });
   });
   ```

2. **Create the Docker Compose file** at `deploy/docker-compose.my-deployment.yml`

3. **Use shared test suites** where appropriate:
   - `runHealthCheckTests` - Basic health endpoint verification
   - `runAdminApiCrudTests` - Organization, team, user, API key CRUD
   - `runChatCompletionsTests` - Chat completion endpoints
   - `runRedisConnectivityTests` - Redis cache verification
   - `runPostgresDataTests` - PostgreSQL data persistence
   - `runRagEndpointTests` - RAG/Vector store workflows
   - `runUsageAndStreamingTests` - Usage tracking and streaming
   - `runCelPolicyEnforcementTests` - CEL RBAC policy verification

4. **Run your tests**:
   ```bash
   pnpm test -- my-deployment.test.ts
   ```

## API Coverage Tracking

The test suite tracks API endpoint coverage against the OpenAPI spec. Coverage data is collected during test runs and aggregated into a report.

**How it works:**
1. Each test worker writes API call records to a temp file (`coverage/.coverage-data-{pid}.jsonl`)
2. The custom reporter (`src/reporters/api-coverage.ts`) aggregates all worker files
3. A coverage report is generated at `coverage/api-coverage.json`

**Viewing coverage:**
```bash
# After running tests
cat coverage/api-coverage.json | jq '.summary'

# Example output:
# {
#   "endpoints": { "covered": 24, "total": 175, "percentage": 13.71 },
#   "parameters": { "covered": 45, "total": 892, "percentage": 5.04 },
#   "statusCodes": { "covered": 18, "total": 312, "percentage": 5.77 }
# }
```

**CI thresholds:** The CI workflow can enforce minimum coverage thresholds via environment variables:
- `API_COVERAGE_ENDPOINTS_THRESHOLD` - Minimum endpoint coverage percentage
- `API_COVERAGE_PARAMETERS_THRESHOLD` - Minimum parameter coverage percentage
- `API_COVERAGE_STATUS_CODES_THRESHOLD` - Minimum status code coverage percentage

## Troubleshooting

### Container startup timeouts

If tests fail during container startup:
1. Increase the timeout in `beforeAll` (default: 300s)
2. Check Docker daemon is running: `docker ps`
3. Check available disk space: `df -h`
4. Check Docker logs: `docker logs <container-id>`

### Port conflicts

Each test file uses unique host port numbers to avoid conflicts during parallel execution. If you see port conflicts:
1. Ensure no stale containers are running: `docker ps -a`
2. Clean up orphaned containers: `docker system prune`
3. Each test file should use a unique `projectName` in `startComposeEnvironment()`
4. Check that port numbers don't overlap with other test files (see port assignments in each `beforeAll`)

### Test isolation failures

Tests depend on state from earlier tests within the same file. If a test fails with "prerequisite not set":
1. An earlier test in the chain failed - check the logs
2. The shared test suite expects specific data created by previous tests
3. Review the test order in the shared test file

### Keycloak/Authentik not ready

Auth tests wait for IdP readiness. If timeouts occur:
1. Increase `maxRetries` in `waitForKeycloak()` or `waitForAuthentik()`
2. Check IdP container logs for startup errors
3. Realm imports can take time - blueprints/realm files may need optimization

### API client out of sync

If you see type errors or missing endpoints:
```bash
# Regenerate the client from the OpenAPI spec
pnpm generate-client
```

### Coverage data not aggregating

If coverage report shows 0 calls:
1. Ensure tests use `trackedFetch()` or `createTrackedClient()` for API calls
2. Check that temp files are being created: `ls coverage/.coverage-data-*`
3. Verify the reporter is configured in `vitest.config.ts`
