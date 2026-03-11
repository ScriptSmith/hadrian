# Key Files

## Backend — Core

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

## Backend — Providers & Routing

- `src/providers/` — LLM providers (openai, anthropic, bedrock, vertex, azure_openai)
- `src/routing/resolver.rs` — Dynamic provider resolution
- `src/models/dynamic_provider.rs` — Dynamic provider model
- `src/routes/admin/dynamic_providers.rs` — Dynamic provider admin endpoints
- `src/routes/admin/me_providers.rs` — Self-service provider endpoints
- `src/jobs/provider_health_check.rs` — Background provider health monitoring

## Backend — Auth & RBAC

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

## Backend — Knowledge Bases / RAG

- `src/services/document_processor.rs` — File processing, text extraction, chunking
- `src/services/file_search.rs` — Vector search, re-ranking, result formatting
- `src/services/file_search_tool.rs` — file_search tool interception for Responses API
- `src/cache/vector_store/` — Vector store backends (pgvector, Qdrant, etc.)
- `src/db/repos/vector_stores.rs` — Vector store and file metadata repository
- `src/jobs/vector_store_cleanup.rs` — Background cleanup for soft-deleted stores
- `src/models/vector_store.rs` — VectorStore and VectorStoreFile models

## Backend — Usage, Cost & Observability

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

## Backend — WASM

- `src/wasm.rs` — WASM entry point: `HadrianGateway` struct, request/response conversion, router construction, default config
- `src/compat.rs` — WASM compatibility: `AssertSend`, `WasmHandler`, `wasm_routing` module, `spawn_detached`, `impl_wasm_handler!` macro
- `src/lib.rs` — Library exports (crate type `cdylib` + `rlib` for wasm-pack)
- `src/db/wasm_sqlite/bridge.rs` — `wasm_bindgen` FFI to `globalThis.__hadrian_sqlite` (sql.js bridge)
- `src/db/wasm_sqlite/types.rs` — `WasmParam`, `WasmValue`, `WasmRow`, `WasmDecode` trait with type conversions
- `src/db/sqlite/backend.rs` — SQLite backend abstraction: cfg-switched `Pool`/`Row`/`BackendError` type aliases, `RowExt`/`ColDecode` traits
- `src/middleware/types.rs` — Shared middleware types (`AuthzContext`, `AdminAuth`, `ClientInfo`) extracted from layers for WASM compatibility
- `scripts/build-wasm.sh` — Build script (invokes `wasm-pack`, copies sql-wasm.wasm)

## Backend — Other

- `src/catalog/` — Model catalog registry
- `src/jobs/model_catalog_sync.rs` — Background model catalog sync worker
- `src/dlq/` — Dead letter queue
- `src/events/mod.rs` — Event system
- `src/retention/` — Data retention enforcement
- `src/config/auth.rs` — `RbacConfig` for system policies
- `src/db/postgres/users.rs` — Postgres user repo (including `add_to_org` constraint handling)
- `src/db/sqlite/users.rs` — SQLite user repo

## Frontend — Chat

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

## Frontend — Tools & Services

- `ui/src/services/pyodide/` — Python execution via Pyodide WASM
- `ui/src/services/quickjs/` — JavaScript execution via QuickJS WASM
- `ui/src/services/duckdb/` — SQL queries via DuckDB WASM
- `ui/src/services/mcp/` — MCP client and protocol types
- `ui/src/services/opfs/` — OPFS audio storage
- `ui/src/components/ToolExecution/` — Tool execution timeline UI
- `ui/src/components/Artifact/` — Artifact rendering (charts, tables, images, code)

## Frontend — WASM / Service Worker

- `ui/src/service-worker/sw.ts` — Service worker: intercepts API calls, lazily initializes `HadrianGateway` WASM module, routes requests through Axum router
- `ui/src/service-worker/sqlite-bridge.ts` — sql.js bridge: `globalThis.__hadrian_sqlite` with `init_database()`, `query()`, `execute()`, `execute_script()`; persists to IndexedDB with debounced save
- `ui/src/service-worker/register.ts` — Service worker registration with `CLAIM` message handling for hard refreshes
- `ui/src/service-worker/wasm.d.ts` — Type declarations for the WASM module exports
- `ui/src/components/WasmSetup/WasmSetup.tsx` — Three-step setup wizard with OpenRouter OAuth, Ollama detection, manual API key entry
- `ui/src/components/WasmSetup/WasmSetupGuard.tsx` — Guard component: auto-shows wizard when no providers configured, handles OAuth callback
- `ui/src/components/WasmSetup/openrouter-oauth.ts` — OpenRouter OAuth PKCE flow (code verifier in sessionStorage)
- `ui/src/routes/AppRoutes.tsx` — Routes extracted from App.tsx

## Frontend — Pages & Layout

- `ui/src/pages/studio/` — Studio feature (image gen, TTS, transcription)
- `ui/src/components/Studio/` — Studio UI components
- `ui/src/components/UsageDashboard/` — Reusable usage dashboard with `UsageScope` discriminated union
- `ui/src/pages/MyUsagePage.tsx` — Self-service usage page at `/usage`
- `ui/src/components/AdminLayout/` — Dedicated admin area with its own sidebar
- `ui/src/components/AppLayout/` — Main app layout with chat sidebar
- `ui/src/components/VectorStores/` — Vector store UI components
- `ui/src/api/` — Generated API client

## Helm Chart

- `helm/hadrian/` — Chart directory (Chart.yaml, values.yaml, values.schema.json)
- `helm/hadrian/templates/` — Kubernetes manifests (deployment, configmap, secret, service, ingress, HPA, PDB, etc.)
- `helm/hadrian/README.md` — Chart documentation with examples

## Documentation

- `docs/content/docs/` — MDX documentation pages
- `docs/content/docs/api/` — Auto-generated OpenAPI documentation
- `docs/lib/source.ts` — Content source configuration
- `docs/lib/openapi.ts` — OpenAPI integration
- `docs/components/story-embed.tsx` — Storybook iframe wrapper
- `docs/scripts/generate-openapi-docs.ts` — OpenAPI page generator
