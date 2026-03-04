# CLAUDE.md

Hadrian is an AI Gateway that provides a unified OpenAI-compatible API for routing requests to multiple LLM providers.

Its purpose is to provide a high-quality, high-performance, production-ready AI Gateway for LLMs that's COMPLETELY open source and free to use with no restrictions.

All 'enterprise' features are fully supported and free to use. It should run on anything from a Raspberry Pi to globally distributed multi-node multi-region cloud infrastructure.

Code and documentation quality should be very high, and the project should be well-maintained.

It should also provide the best in class interface for interacting with multiple models in a single conversation. It should support complex modes of interaction and push forward the state of the art.

Features:

- Web UI: multimodel chat, chat modes, reasoning, frontend tools (Python/JS/SQL/Charts), MCP, admin panel
- Studio: image generation, TTS, transcription/translation with multimodel execution, and cost tracking
- Single binary, single config file deployment
- OpenAI-compatible API with OpenAPI docs and Scalar UI
- Multi-tenancy (organizations, teams, projects, users)
- Auth: API keys, OIDC, OAuth, Identity-Aware Proxy (IAP), CEL-based RBAC
- Budget enforcement, usage tracking, cost tracking with microcents precision and forecasting (MSTL/ETS)
- Guardrails (blocklist, PII, content limits), response validation
- Dynamic model routing, provider health checks, fallbacks
- Dynamic providers: user/org/team/project-scoped custom provider management
- Model catalog: models.dev integration with background sync
- Image generation, audio (TTS, transcription, translation)
- Knowledge Bases / RAG: file upload, text extraction, chunking, vector search, re-ranking
- Integrations: SQLite/Postgres, Redis, OpenTelemetry, Vault, S3

The backend is written in Rust and uses Axum for routing and middleware.
The frontend is written in React and TypeScript, with TailwindCSS for styling.

## General guidelines

- Write high-quality, readable, and maintainable code worthy of a senior software engineer or architect
- Write idiomatic code using modern language features that are terse and not overly complicated or verbose
- Rely on linting, formatting, and type checking to catch issues and write clean code
- Aim for high test coverage. Write tests for all new code
- Architect for data-intensive workloads but support single-user use cases as well
- Consider the performance implications of all existing and new code
- There has not been a release yet, so don't worry about breaking changes or maintaining backwards compatibility in the backend and frontend
  - Modify the existing database migration file and schema as needed
  - Make sure sqlite and postgres are kept in sync
  - Update the API, modules, classes, functions, types, etc. as needed
  - Update the config file as needed
- Don't leave behind unused imports, `todo!`s, or dead code.
  - Implement the functionality or explain why it can't be done yet
  - For functionality that will be useful in the future, prompt the user to keep it / implement that functionality

## Specific guidelines

Read the files in the `agent_instructions` directory for details on the following, as needed:

- `adding_admin_endpoint.md` — Adding admin endpoints (includes pagination patterns)
- `adding_frontend_tool.md` — Adding frontend tools
- `adding_provider.md` — Adding LLM providers
- `database_changes.md` — Database migrations and schema changes
- `modifying_chat_ui.md` — Chat UI performance patterns (stores, selectors, memoization). Read before making changes to the chat UI.

## Backend

### Build & Development Commands

```bash
cargo build                     # Build project
cargo build --release           # Release build
cargo test                      # Run unit tests
cargo test -- --ignored         # Run integration tests
cargo clippy                    # Lint code
cargo +nightly fmt              # Format code (requires nightly)
cargo fix                       # Fix lints
cargo run                       # Run with default config (hadrian.toml)
cargo run -- --config path.toml # Run with custom config
cd deploy/tests && pnpm test    # Run end-to-end tests with testcontainers
./scripts/coverage.sh           # Generate code coverage report
```

### Cargo Features

Hierarchical feature profiles (default: `full`):

- **`tiny`** — OpenAI + Test providers, no DB, no embedded assets (stateless proxy)
- **`minimal`** — tiny + all providers (Anthropic, Azure, Bedrock, Vertex), SQLite, embedded UI, embedded catalog, wizard (dev/Windows/embedded)
- **`standard`** — minimal + Postgres, Redis, OTLP, Prometheus, SSO, CEL, doc extraction, OpenAPI docs, S3, secrets managers (AWS/Azure/GCP/Vault)
- **`full`** — standard + SAML, Kreuzberg, ClamAV
- **`headless`** — all `full` features except embedded assets (UI, docs, catalog). Used by `cargo install` and for deployments that serve the frontend separately.

```bash
cargo build --no-default-features --features tiny       # Smallest binary
cargo build --no-default-features --features minimal    # Fast compile
cargo build --no-default-features --features standard   # Typical deployment
cargo build                                             # Full (default)
cargo build --no-default-features --features headless   # Full features, no embedded assets
```

Run `hadrian features` to list enabled/disabled features at runtime. CI tests `minimal`, `standard`, and `headless` profiles; Windows uses `minimal` to avoid OpenSSL.

To use the ls command, use `/usr/bin/ls` instead of `ls` which will use exa.
To use the sleep command, don't use `-s 5`, use `sleep 5s`.

Server runs on `http://0.0.0.0:8080` by default.

After making changes to the backend, run the following:
- `cargo check` to check for compile errors
- `cargo clippy` to lint code
- `cargo +nightly fmt` to format code (requires nightly)
- `cargo test` to run tests

### CI Pipeline

GitHub Actions workflow (`.github/workflows/ci.yml`) runs:
- Backend: format, clippy, build, test, security audit (`cargo audit`, `cargo-deny`)
- Frontend: lint, format, type check, build, security audit (`pnpm audit`)
- Cross-platform builds (Linux, macOS Intel/ARM, Windows)
- Docker build (shared image used by E2E tests)
- E2E tests (TypeScript/Playwright with testcontainers, needs Docker build)
- OpenAPI conformance check
- Documentation build

### Release Pipeline

GitHub Actions workflow (`.github/workflows/release.yml`) triggers on version tags (`v*`) or manual dispatch (with dry-run option):
- Builds frontend assets (UI, Storybook, docs) in a shared job
- Builds release binaries for each target/feature combination:
  - `x86_64-unknown-linux-gnu` (full, standard, minimal, tiny)
  - `x86_64-unknown-linux-musl` (standard, minimal, tiny)
  - `aarch64-unknown-linux-gnu` (standard, minimal, tiny)
  - `aarch64-apple-darwin` (full, standard, minimal, tiny)
  - `x86_64-pc-windows-msvc` (standard, minimal, tiny)
- Creates GitHub Release with archives and SHA256 checksums (tag push only)
- Dry-run mode builds artifacts and prints a summary without creating a release

Helm chart workflow (`.github/workflows/helm.yml`) runs:
- `helm lint` (standard and strict mode)
- `helm template` with matrix of configurations (PostgreSQL, Redis, Ingress, etc.)
- Schema validation with `ajv-cli`
- Integration tests in ephemeral kind cluster

## Architecture Overview

### Multi-tenancy Hierarchy

- **Organization** → top-level container; can have many **Users**, **Teams**, and **Projects**
- **Team** → belongs to an Organization; can have many **Users** and **Projects**
- **User** → belongs to an Organization (and optionally Teams); can own **Projects**
- **Project** → owned by Organization, Team, or User; serves as workspace boundary

**Resources** (owned by Teams, Users, or Projects):

- Conversations
- Providers
- API Keys
- Vector Stores
- Files

### Principal Model

The Principal abstraction represents "who is making the request" regardless of credential type:

- **User**: Human identity from OIDC/SAML/proxy or user-owned API key
- **ServiceAccount**: Machine identity with explicit roles (service account-owned API key)
- **Machine**: Shared credential (org/team/project-owned API key, no roles)

Service accounts enable RBAC for API key authentication by providing roles that flow into CEL policy evaluation. When an API key owned by a service account is used, the service account's roles are mapped through `role_mapping` and included in the RBAC Subject.

All admin endpoints use `authz.require()` for role-based access control. See `src/routes/admin/teams.rs` as a reference implementation.

### Authorization (RBAC)

Hadrian uses a two-tier CEL-based RBAC system:

1. **System Policies** (global) — Defined in `hadrian.toml`, controlled by platform operators
2. **Organization Policies** (per-org) — Stored in database, managed by org admins at runtime via `/admin/v1/orgs/{org_slug}/rbac-policies`

**Evaluation order:**
1. Check if RBAC disabled → allow all
2. Evaluate system policies (config) in priority order → if match, return decision
3. If `org_id` provided, evaluate org policies (database) in priority order → if match, return decision
4. No match → apply `default_effect` (typically "deny" for admin, "allow" for API)

**Middleware usage:**
- `authz.require(resource, action)` — System policies only (admin endpoints)
- `authz.require_api(resource, action)` — System + org policies (API endpoints)

### Membership Model

**Membership Source Tracking:**
Organization and project memberships track their source for auditability:
- `manual` — Added by an admin via API/UI
- `jit` — Just-In-Time provisioned during SSO authentication
- `scim` — Provisioned via SCIM protocol from an IdP

**Single-Org Membership Constraint:**
Each user can only belong to one organization at a time. This is enforced by a database unique index (`idx_org_memberships_single_org`), which is race-condition safe and returns a conflict error when violated.

### Per-Organization SSO

Per-org SSO allows each organization to configure its own identity provider (OIDC or SAML), replacing the global OIDC configuration. This enables multi-tenant deployments where different organizations use different IdPs.

**Key concepts:**
- SSO configs are stored in the database per organization (`org_sso_configs` table)
- Client secrets are stored in an external secrets manager (Vault, AWS, etc.)
- OIDC authenticators are lazily loaded when first needed
- SSO enforcement modes: `optional`, `test` (shadow mode), `required`
- Bearer token validation extracts org from JWT claim and validates against that org's IdP
- Gateway JWT flow: decode `iss` → per-org registry lookup → lazy-load from DB → fall back to global JWT validator
- `GatewayJwtRegistry` is pre-loaded at startup and kept in sync by SSO config CRUD
- `AppState.global_jwt_validator` caches the global JWT validator so JWKS isn't re-fetched per request

### Request Flow

1. **Client** sends request to gateway
2. **Middleware Pipeline** processes in order: init usage tracker → authenticate → check budget
3. **Route Handler** parses model string, resolves provider (static config or dynamic from DB)
4. **LLM Provider** forwards request, streams response
5. **Usage Tracking** records tokens/cost asynchronously with full principal attribution (user, org, project, team, service account)

### Document Processing Flow (RAG)

1. **File Upload** (`POST /v1/files`) — Store raw file in database
2. **Add to Vector Store** (`POST /v1/vector_stores/{id}/files`) — Trigger processing
   - Note: 'Vector Stores' are called 'Knowledge Bases' in the UI. Do not refer to them as 'Vector Stores' there.
3. **Document Processor** (inline or queue mode):
   - Extract text via Kreuzberg (PDF, DOCX, HTML, etc.)
   - OCR for scanned documents (optional)
   - Chunk text (auto or fixed-size strategy)
   - Generate embeddings per chunk
   - Store in vector database with `processing_version`
4. **Shadow-copy cleanup** — Delete old chunks only after successful processing
5. **File status** updated to "completed" or "failed"

Key patterns:
- **Shadow-copy**: New chunks stored with `processing_version`, old deleted only on success
- **Idempotent re-processing**: Failed files can be re-added to trigger reprocessing
- **Stale detection**: In-progress files auto-reset after timeout (default 30 min)

### Chat Modes Architecture

The chat UI supports multiple interaction modes via pluggable handlers. The Mode Runner dispatches to mode-specific handlers that orchestrate LLM streams and aggregate responses.

**Available modes:** synthesized, chained, debated, council, hierarchical, refined, routed, critiqued, elected, tournament, consensus, scattershot, explainer, confidence

Modes use **instance IDs** (not model IDs) for role assignment to support multiple instances of the same model with different settings.

### Frontend Tools Architecture

Client-side tool execution runs in the browser via WASM. When the LLM returns `tool_calls`, the Tool Executor Registry dispatches to the appropriate executor:

- **Pyodide** — Python execution (numpy, pandas, matplotlib available)
- **QuickJS** — JavaScript execution (sandboxed)
- **DuckDB** — SQL queries against uploaded CSV/Parquet files
- **Vega** — Chart generation from Vega-Lite specs
- **HTML** — Sandboxed iframe preview

Tool results are sent back to the LLM to continue the conversation. Artifacts (charts, tables, images) are displayed inline in the chat.

### Provider Features

- **Thinking/Reasoning**: Anthropic extended thinking, OpenAI O1/O3 reasoning, Bedrock/Vertex native conversion. Configurable budget tokens and effort levels.
- **Prompt Caching**: Anthropic `cache_control` messages, tracks cache creation/read tokens in usage.
- **Image Support**: Base64 input (all providers), URL-based input for Anthropic (HTTPS only), image generation via `/v1/images/generations`.
- **Audio Support**: TTS (`/v1/audio/speech`), transcription (`/v1/audio/transcriptions`), translation (`/v1/audio/translations`).

### Studio

Multi-model tool execution UI for image generation, TTS, transcription, and translation. Supports simultaneous execution across providers with cost tracking. Uses OPFS for client-side audio storage.

### Dynamic Providers

Users, orgs, teams, and projects can configure their own LLM providers at runtime. Credentials stored via secrets manager integration. Resolved during request routing with caching.

### Model Catalog

Embedded model metadata from models.dev with background sync worker. Provides capabilities, pricing, context limits, and modalities per model. Configurable via `[features.model_catalog]`.

### Cost Tracking & Forecasting

Usage tracked in microcents precision (1/1,000,000 of a dollar). `X-Cost-Microcents` response header. Forecasting via MSTL (14+ days data) with AutoETS fallback. 95% prediction intervals and budget exhaustion projection.

### Performance Considerations

- Database queries in API hot path should use caching
- Avoid allocations in frequently called code
- Use Cow<str> instead of String::from() where possible

## Testing

- Unit tests go in the same file as the code (`#[cfg(test)]`)
- E2E tests use the TypeScript test suite in `deploy/tests/` with testcontainers
- Test both SQLite and PostgreSQL paths for database code

### Provider Testing (Wiremock)

Provider e2e tests use recorded fixtures instead of live API calls:
- Fixtures in `tests/fixtures/providers/{provider}/` (JSON request/response pairs)
- Tests in `src/tests/provider_e2e.rs` using `rstest` for parameterization
- Adding a provider = add `ProviderTestSpec` + fixture files
- Record new fixtures: `cargo run --bin record_fixtures -- --help`
- Set `HADRIAN_TEST_DEBUG=1` to save test responses to `tests/fixtures/providers/_debug/`

### University E2E Tests

Comprehensive deployment tests with Keycloak OIDC and CEL-based RBAC policies:

```bash
cd deploy/tests && pnpm test university    # Run university tests
cd deploy/tests && pnpm test -- --grep "CEL"  # Run tests matching pattern
```

Tests cover:
- OIDC authentication flow (token acquisition, claim verification)
- CEL policy enforcement (cross-org isolation, role boundaries)
- Budget enforcement and usage tracking
- RAG/vector stores with cross-org permission isolation
- Streaming API (SSE format, chunked responses)

## API Conventions

- All admin endpoints under `/admin/v1/`
- OpenAI-compatible endpoints under `/v1/`
  - All endpoints should conform to the OpenAI OpenAPI spec, with clearly-marked hadrian-specific extensions
  - Mark extension fields with `**Hadrian Extension:**` at the start of their doc comment; run `./scripts/openapi-conformance.py` to verify
  - Reference specs in `openapi/` directory (OpenAI, Anthropic, OpenRouter) — use local copies, fetch with `./scripts/fetch-openapi-specs.sh`
- Use plural nouns for resources (`/admin/v1/users`, not `/user`)
- Return JSON with consistent error shapes

### Cursor-Based Pagination

All list endpoints use cursor-based (keyset) pagination for stable, performant navigation. Do not use offset-based pagination.

**Query parameters:**
- `limit` (optional): Max records per page (default: 100, max: 1000)
- `cursor` (optional): Opaque base64 cursor from previous response
- `direction` (optional): `forward` (default) or `backward`

**Response format:**
```json
{
  "data": [...],
  "pagination": {
    "limit": 100,
    "has_more": true,
    "next_cursor": "MTczMzU4MDgwMDAwMDphYmMxMjM0...",
    "prev_cursor": null
  }
}
```

**Important:** Truncate timestamps to milliseconds when creating entities, since cursors use millisecond precision. This prevents comparison issues in SQLite (which stores DateTime as TEXT).

See `agent_instructions/adding_admin_endpoint.md` for implementation patterns (route handler, repository SQL, cursor encoding).

## Configuration

- Config file: `hadrian.toml` (TOML format)
- Environment variables: use `${VAR_NAME}` syntax for interpolation
- Secrets are automatically redacted in logs and API responses
- See `src/config/` for all configuration options

### Top-Level Config Sections

| Section | Description |
|---------|-------------|
| `[server]` | HTTP server (host, port, TLS, CORS, trusted proxies, security headers) |
| `[database]` | SQLite or PostgreSQL connection, pool settings, read replicas |
| `[cache]` | In-memory or Redis cache for sessions, rate limits, API key lookups |
| `[auth]` | Authentication mode (`none`, `api_key`, `idp`, `iap`), API key settings, per-org SSO, RBAC (CEL policies), session config |
| `[providers]` | LLM providers (OpenAI, Anthropic, Bedrock, Vertex, Azure), retries, fallbacks, health checks |
| `[limits]` | Rate limits, budget enforcement, request size limits |
| `[features]` | Feature flags (see below) |
| `[observability]` | Logging, tracing (OTLP), metrics (Prometheus), usage tracking, response validation |
| `[ui]` | Web UI settings, branding, file upload limits, admin panel |
| `[pricing]` | Model pricing for cost calculation and budget enforcement |
| `[secrets]` | External secrets managers (Vault, AWS Secrets Manager, Azure Key Vault, GCP) |
| `[retention]` | Data retention policies for automatic purging |
| `[storage]` | File storage backend (local filesystem, S3-compatible) |

### Key Provider Options

- `[providers.<name>]` — Define providers (openai, anthropic, bedrock, vertex, azure_openai, test)
- `fallback_providers` — List of providers to try on 5xx errors
- `retries` — Per-provider retry settings (max_attempts, delays, backoff)
- `health_check` — Background health monitoring
- `circuit_breaker` — Automatic provider disabling on repeated failures
- `streaming_buffer` — Buffer size for SSE streaming

### Feature Flags

- `[features.file_search]` — Knowledge Bases / RAG / vector search (embedding model, vector backend, chunking, reranking)
- `[features.file_processing]` — RAG document ingestion (text extraction, OCR, chunking)
- `[features.guardrails]` — Input/output guardrails (blocklist, PII detection, moderation APIs)
- `[features.response_caching]` — Response caching with optional semantic similarity matching
- `[features.image_fetching]` — Fetch images from URLs for vision models
- `[features.model_catalog]` — Model metadata enrichment from models.dev
- `[features.websocket]` — WebSocket for real-time events
- `[features.vector_store_cleanup]` — Background cleanup for soft-deleted vector stores

## Caching

- In-memory cache for single-node deployments (`src/cache/`)
- Redis required for multi-node deployments (for cache invalidation sync)
- Cache API keys, user data, and provider configs
- Invalidate cache on write operations

## Key Files

### Backend — Core

- `src/main.rs` — Entry point only (module declarations, `main()`)
- `src/app.rs` — `AppState` struct/construction, `build_app()` router setup, embedded assets
- `src/init.rs` — Service initialization helpers (providers, secrets, embeddings)
- `src/cli/` — CLI commands (`mod.rs` dispatch, `server.rs`, `worker.rs`, `bootstrap.rs`, `migrate.rs`, `init.rs`, `features.rs`, `openapi.rs`)
- `src/config/mod.rs` — Configuration structures
- `src/routes/api/` — API handlers split by domain (`chat.rs`, `embeddings.rs`, `models.rs`, `images.rs`, `audio.rs`, `files.rs`, `vector_stores.rs`)
- `src/routes/admin/` — Admin handlers
- `src/middleware/` — Axum middleware layers (auth, authz, rate limiting, security headers)
- `src/db/repos/` — Repository traits for data access
- `src/db/repos/cursor.rs` — Cursor-based pagination types (`Cursor`, `ListParams`, `ListResult`)
- `openapi/` — Reference OpenAPI specs for providers
- `src/openapi.rs` — OpenAPI schema and `PaginationMeta` type

### Backend — Providers & Routing

- `src/providers/` — LLM providers (openai, anthropic, bedrock, vertex, azure_openai)
- `src/routing/resolver.rs` — Dynamic provider resolution
- `src/models/dynamic_provider.rs` — Dynamic provider model
- `src/routes/admin/dynamic_providers.rs` — Dynamic provider admin endpoints
- `src/routes/admin/me_providers.rs` — Self-service provider endpoints
- `src/jobs/provider_health_check.rs` — Background provider health monitoring

### Backend — Auth & RBAC

- `src/auth/principal.rs` — Principal derivation and Subject conversion
- `src/models/service_account.rs` — Service account model and validation
- `src/routes/admin/service_accounts.rs` — Service account admin endpoints
- `src/middleware/layers/authz.rs` — Request authorization middleware, service account role injection
- `src/authz/engine.rs` — CEL evaluation engine
- `src/authz/registry.rs` — `PolicyRegistry` with per-org caching
- `src/models/org_rbac_policy.rs` — Org policy models
- `src/services/org_rbac_policies.rs` — Policy service with CEL validation
- `src/routes/admin/org_rbac_policies.rs` — Org RBAC policy admin endpoints
- `src/routes/admin/org_sso_configs.rs` — SSO config CRUD endpoints
- `src/services/org_sso_configs.rs` — SSO config service layer
- `src/middleware/layers/admin.rs` — Admin middleware, per-org JWT validation
- `src/routes/auth.rs` — Auth routes, lazy OIDC authenticator loading
- `src/auth/gateway_jwt.rs` — Per-org gateway JWT validator registry (issuer → org routing)
- `src/auth/discovery.rs` — Shared OIDC discovery with SSRF validation

### Backend — Knowledge Bases / RAG

- `src/services/document_processor.rs` — File processing, text extraction, chunking
- `src/services/file_search.rs` — Vector search, re-ranking, result formatting
- `src/services/file_search_tool.rs` — file_search tool interception for Responses API
- `src/cache/vector_store/` — Vector store backends (pgvector, Qdrant, etc.)
- `src/db/repos/vector_stores.rs` — Vector store and file metadata repository
- `src/jobs/vector_store_cleanup.rs` — Background cleanup for soft-deleted stores
- `src/models/vector_store.rs` — VectorStore and VectorStoreFile models

### Backend — Usage, Cost & Observability

- `src/models/usage.rs` — `UsageLogEntry` with principal attribution fields
- `src/services/usage.rs` — Usage analytics service (scoped queries by org, team, project, user, API key)
- `src/routes/admin/usage.rs` — Usage admin endpoints including self-service `/admin/v1/me/usage/*`
- `src/usage_buffer.rs` — Async usage buffering
- `src/usage_sink.rs` — OTLP usage export with attribution attributes
- `src/services/forecasting.rs` — Cost forecasting (MSTL/ETS)
- `src/pricing/` — Model pricing calculations
- `src/guardrails/` — Input/output guardrails (blocklist, PII, moderation APIs)
- `src/validation/` — Response validation against OpenAI schema
- `src/observability/siem/` — SIEM formatters

### Backend — Other

- `src/catalog/` — Model catalog registry
- `src/jobs/model_catalog_sync.rs` — Background model catalog sync worker
- `src/dlq/` — Dead letter queue
- `src/events/mod.rs` — Event system
- `src/retention/` — Data retention enforcement
- `src/config/auth.rs` — `RbacConfig` for system policies
- `src/db/postgres/users.rs` — Postgres user repo (including `add_to_org` constraint handling)
- `src/db/sqlite/users.rs` — SQLite user repo

### Frontend — Chat

- `ui/src/stores/streamingStore.ts` — Token streaming state (ephemeral)
- `ui/src/stores/conversationStore.ts` — Persistent messages (IndexedDB)
- `ui/src/stores/chatUIStore.ts` — UI preferences (session-only)
- `ui/src/stores/mcpStore.ts` — MCP server connections (localStorage)
- `ui/src/stores/websocketStore.ts` — WebSocket events
- `ui/src/stores/debugStore.ts` — Debug capture
- `ui/src/pages/chat/modes/` — Mode handlers (14 modes)
- `ui/src/pages/chat/modes/runner.ts` — Mode execution orchestration
- `ui/src/pages/chat/modes/types.ts` — ModeHandler interface and context types
- `ui/src/pages/chat/utils/toolExecutors.ts` — Tool executor registry and implementations
- `ui/src/components/ChatMessageList/ChatMessageList.tsx` — Virtualized message list
- `ui/src/components/MultiModelResponse/MultiModelResponse.tsx` — Model response cards
- `ui/src/components/ModeProgress/` — Mode-specific progress UI components
- `ui/src/hooks/useAutoScroll.ts` — Smart auto-scroll behavior
- `ui/src/hooks/useIndexedDB.ts` — IndexedDB persistence for conversations

### Frontend — Tools & Services

- `ui/src/services/pyodide/` — Python execution via Pyodide WASM
- `ui/src/services/quickjs/` — JavaScript execution via QuickJS WASM
- `ui/src/services/duckdb/` — SQL queries via DuckDB WASM
- `ui/src/services/mcp/` — MCP client and protocol types
- `ui/src/services/opfs/` — OPFS audio storage
- `ui/src/components/ToolExecution/` — Tool execution timeline UI
- `ui/src/components/Artifact/` — Artifact rendering (charts, tables, images, code)

### Frontend — Pages & Layout

- `ui/src/pages/studio/` — Studio feature (image gen, TTS, transcription)
- `ui/src/components/Studio/` — Studio UI components
- `ui/src/components/UsageDashboard/` — Reusable usage dashboard with `UsageScope` discriminated union
- `ui/src/pages/MyUsagePage.tsx` — Self-service usage page at `/usage`
- `ui/src/components/AdminLayout/` — Dedicated admin area with its own sidebar
- `ui/src/components/AppLayout/` — Main app layout with chat sidebar
- `ui/src/components/VectorStores/` — Vector store UI components
- `ui/src/api/` — Generated API client

### Helm Chart

- `helm/hadrian/` — Chart directory (Chart.yaml, values.yaml, values.schema.json)
- `helm/hadrian/templates/` — Kubernetes manifests (deployment, configmap, secret, service, ingress, HPA, PDB, etc.)
- `helm/hadrian/README.md` — Chart documentation with examples

### Documentation

- `docs/content/docs/` — MDX documentation pages
- `docs/content/docs/api/` — Auto-generated OpenAPI documentation
- `docs/lib/source.ts` — Content source configuration
- `docs/lib/openapi.ts` — OpenAPI integration
- `docs/components/story-embed.tsx` — Storybook iframe wrapper
- `docs/scripts/generate-openapi-docs.ts` — OpenAPI page generator

## Debugging Tips

- Set `RUST_LOG=debug` for verbose logging
- Use `observability.logging.format = "pretty"` for human-readable logs
- Check `/health` endpoint for database connectivity
- Documentation at `/docs`, API reference at `/api/docs` (Scalar)

## Frontend

The UI is in the `ui/` directory and uses:
- React 19 with TypeScript
- TailwindCSS for styling
- Storybook for component development
- @tanstack/react-query for data fetching
- hey-api for OpenAPI client generation

```bash
pnpm install           # Install dependencies
pnpm dev               # Start dev server
pnpm build             # Production build
pnpm lint              # Lint code
pnpm format            # Format code
pnpm storybook         # Component development
pnpm test-storybook    # Run Storybook tests with vitest
pnpm openapi-ts        # Regenerate from /api/openapi.json
```

### Frontend Conventions

- Run the `./scripts/generate-openapi.sh` script to generate the OpenAPI client
- Use React Query for all API calls (via generated hey-api client)
- Components are in `ui/src/components/` with PascalCase directories.
- Pages and large components should be broken down into multiple components.
- Each component must have a `.stories.tsx` file for Storybook
- Prefer Tailwind utility classes over custom CSS

#### Accessibility (WCAG 2.1 AA)

All UI components must meet WCAG 2.1 AA standards. Two tools enforce this automatically:

- **`eslint-plugin-jsx-a11y`** — Static linting (runs with `pnpm lint`). Catches missing labels, invalid ARIA attributes, etc.
- **`@storybook/addon-a11y`** — Runtime axe-core testing (runs with `pnpm test-storybook`). Set to `error` mode in `ui/.storybook/preview.ts` — all story files must pass.

When writing new components:
- Add `aria-label` to icon-only buttons (e.g., `aria-label="Copy code"`)
- Associate form controls with labels (`useId()` + `htmlFor`, or `aria-label` for switches/toggles)
- Use theme CSS variables for text colors — don't hard-code Tailwind colors below `-700` (light) or above `-400` (dark) on white/dark backgrounds
- Don't reduce text opacity (no `/60`, `/70`, `/80` suffixes on `text-muted-foreground`)
- Add `sr-only` text for empty table headers (action columns) and visually hidden labels
- Add `tabIndex={0}` to scrollable containers that aren't natively focusable
- For Storybook false positives (landmark nesting, heading order in isolation), suppress per-story via `parameters.a11y.config.rules` — never disable globally

After making changes to the frontend, run the following:
- `pnpm lint:fix` to fix lint errors
- `pnpm format` to format code
- `pnpm test-storybook` to run Storybook tests
- `pnpm build` to build the production bundle

Lint, formatting, and a11y errors must be resolved before finishing a change. If they need to be ignored, always prompt the user to explain why.

### Chat UI Performance

The chat UI is designed for high-performance multi-model streaming. When modifying chat components, preserve these patterns:

- **6 Zustand stores**: streamingStore (ephemeral tokens), conversationStore (IndexedDB), chatUIStore (session), mcpStore (localStorage), websocketStore (real-time events), debugStore (debug capture)
- **Surgical selectors**: Always use provided selector hooks (e.g., `useStreamContent(model)`), never subscribe to entire stores
- **Memoization**: Components use custom `arePropsEqual` comparators; parent callbacks must use `useCallback`
- **Virtualization**: `ChatMessageList` uses `@tanstack/react-virtual`; streaming responses render outside virtualization
- **Model instances**: Streams and messages are keyed by instance ID (not model ID) to support multiple copies of the same model with different settings

See `agent_instructions/modifying_chat_ui.md` for full details on stores, selectors, memoization patterns, and component responsibilities.

## Documentation

The documentation site is in `docs/` and uses Fumadocs (Next.js-based). It builds to static HTML that can be embedded in the gateway binary or served from a CDN.

The docs pages need to be kept up-to-date with the code. If code changes are related to docs pages, update them with information users (not developers) need to know. Run `find docs/content -name '*.mdx' | sort` to see current docs pages and check if any need updating after a code change.

Read the docs at https://www.fumadocs.dev/llms.txt before updates to docs pages. Always use this as a reference before starting any task.

Quick start: https://www.fumadocs.dev/docs/index.mdx

Note that eg. `/docs/navigation` means fetch `https://www.fumadocs.dev/docs/navigation.mdx`

Fetching from the fumadocs domain requires using curl in bash.

### Build & Development Commands

```bash
cd docs
pnpm install           # Install dependencies
pnpm dev               # Development server at http://localhost:3000
pnpm build             # Build static site to docs/out/
pnpm lint:fix          # Fix lint errors
pnpm format            # Format code
pnpm generate:openapi  # Regenerate API docs from OpenAPI spec
```

### Architecture

- **Static export**: Builds to `docs/out/` for embedding or serving
- **OpenAPI integration**: API reference pages auto-generated from `openapi/hadrian.openapi.json`
- **Storybook embeds**: UI components are embedded via iframe from Storybook for complete style isolation
  - Symlink `docs/public/storybook` → `../../ui/storybook-static`
  - Use `<StoryEmbed storyId="component-name--story" />` in MDX
  - Requires building Storybook before docs: `cd ui && pnpm storybook:build`

### Writing Guidelines

When writing documentation:

- Start every page with a one-sentence summary of what it covers
- Use active voice, second person, present tense, imperative mood ("Run the command" not "You should run the command")
- Front-load keywords in headings ("Redis Configuration" not "How to Configure Redis")
- Use realistic data in examples ("acme-corp", "production-api-key") not "foo/bar"
- Use the storybook embeds to show component examples
- Code blocks: always specify language, show complete working examples, include expected output
- Keep pages focused — if past 1500 words, consider splitting
- End pages with "Next Steps" linking to related topics
- Run the linter and formatter after making changes

## Security Rules

### Authorization enforcement rule
Every admin endpoint handler **must** extract `Extension(authz): Extension<AuthzContext>` and call `authz.require(resource, action)` before performing any operation. No exceptions. Reference `routes/admin/teams.rs` for the pattern.

### Database scoping rule
All `get_by_id()` repository calls from admin handlers with org context **must** use org-scoped variants (e.g., `get_by_id_and_org()`). Unscoped `get_by_id()` is only for internal/system code paths.

### URL validation rule
Any user-supplied URL the server will make HTTP requests to **must** go through `validate_base_url()` to block SSRF.

### Error message rule
Error messages returned to clients **must not** include internal paths, UUIDs, infrastructure details, or secret manager references.

### Credential handling rule
Never return provider credentials in API responses. Never fall back to treating a secret reference as a literal value.

### Security defaults rule
Security-relevant defaults must be fail-closed: invalid credentials = 401, `fail_on_evaluation_error` = true, IAP auth requires explicit `trusted_proxies`.
