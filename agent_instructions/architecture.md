# Architecture

## Multi-tenancy Hierarchy

- **Organization** → top-level container; can have many **Users**, **Teams**, and **Projects**
- **Team** → belongs to an Organization; can have many **Users** and **Projects**
- **User** → belongs to an Organization (and optionally Teams); can own **Projects**
- **Project** → owned by Organization, Team, or User; serves as workspace boundary

**Resources** (owned by Teams, Users, or Projects): Conversations, Providers, API Keys, Vector Stores, Files

## Principal Model

The Principal abstraction represents "who is making the request" regardless of credential type:

- **User**: Human identity from OIDC/SAML/proxy or user-owned API key
- **ServiceAccount**: Machine identity with explicit roles (service account-owned API key)
- **Machine**: Shared credential (org/team/project-owned API key, no roles)

Service accounts enable RBAC for API key authentication by providing roles that flow into CEL policy evaluation. When an API key owned by a service account is used, the service account's roles are mapped through `role_mapping` and included in the RBAC Subject.

All admin endpoints use `authz.require()` for role-based access control. See `src/routes/admin/teams.rs` as a reference implementation.

## Authorization (RBAC)

Two-tier CEL-based RBAC system:

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

## Membership Model

**Membership Source Tracking:**
Organization and project memberships track their source for auditability:
- `manual` — Added by an admin via API/UI
- `jit` — Just-In-Time provisioned during SSO authentication
- `scim` — Provisioned via SCIM protocol from an IdP

**Single-Org Membership Constraint:**
Each user can only belong to one organization at a time. Enforced by a database unique index (`idx_org_memberships_single_org`), which is race-condition safe and returns a conflict error when violated.

## Per-Organization SSO

Each organization can configure its own identity provider (OIDC or SAML), replacing the global OIDC configuration.

- SSO configs stored in the database per organization (`org_sso_configs` table)
- Client secrets stored in an external secrets manager (Vault, AWS, etc.)
- OIDC authenticators lazily loaded when first needed
- SSO enforcement modes: `optional`, `test` (shadow mode), `required`
- Bearer token validation extracts org from JWT claim and validates against that org's IdP
- Gateway JWT flow: decode `iss` → per-org registry lookup → lazy-load from DB → fall back to global JWT validator
- `GatewayJwtRegistry` pre-loaded at startup and kept in sync by SSO config CRUD
- `AppState.global_jwt_validator` caches the global JWT validator so JWKS isn't re-fetched per request

## Request Flow

1. **Client** sends request to gateway
2. **Middleware Pipeline** processes in order: init usage tracker → authenticate → check budget
3. **Route Handler** parses model string, resolves provider (static config or dynamic from DB)
4. **LLM Provider** forwards request, streams response
5. **Usage Tracking** records tokens/cost asynchronously with full principal attribution (user, org, project, team, service account)

## Document Processing Flow (RAG)

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

## Chat Modes

The chat UI supports multiple interaction modes via pluggable handlers. The Mode Runner dispatches to mode-specific handlers that orchestrate LLM streams and aggregate responses.

**Available modes:** synthesized, chained, debated, council, hierarchical, refined, routed, critiqued, elected, tournament, consensus, scattershot, explainer, confidence

Modes use **instance IDs** (not model IDs) for role assignment to support multiple instances of the same model with different settings.

## Frontend Tools

Client-side tool execution runs in the browser via WASM. When the LLM returns `tool_calls`, the Tool Executor Registry dispatches to the appropriate executor:

- **Pyodide** — Python execution (numpy, pandas, matplotlib available)
- **QuickJS** — JavaScript execution (sandboxed)
- **DuckDB** — SQL queries against uploaded CSV/Parquet files
- **Vega** — Chart generation from Vega-Lite specs
- **HTML** — Sandboxed iframe preview

Tool results are sent back to the LLM to continue the conversation. Artifacts (charts, tables, images) are displayed inline in the chat.

## Provider Features

- **Thinking/Reasoning**: Anthropic extended thinking, OpenAI O1/O3 reasoning, Bedrock/Vertex native conversion. Configurable budget tokens and effort levels.
- **Prompt Caching**: Anthropic `cache_control` messages, tracks cache creation/read tokens in usage.
- **Image Support**: Base64 input (all providers), URL-based input for Anthropic (HTTPS only), image generation via `/v1/images/generations`.
- **Audio Support**: TTS (`/v1/audio/speech`), transcription (`/v1/audio/transcriptions`), translation (`/v1/audio/translations`).

## Studio

Multi-model tool execution UI for image generation, TTS, transcription, and translation. Supports simultaneous execution across providers with cost tracking. Uses OPFS for client-side audio storage.

## Dynamic Providers

Users, orgs, teams, and projects can configure their own LLM providers at runtime. Credentials stored via secrets manager integration. Resolved during request routing with caching.

## Model Catalog

Embedded model metadata from models.dev with background sync worker. Provides capabilities, pricing, context limits, and modalities per model. Configurable via `[features.model_catalog]`.

## Cost Tracking & Forecasting

Usage tracked in microcents precision (1/1,000,000 of a dollar). `X-Cost-Microcents` response header. Forecasting via MSTL (14+ days data) with AutoETS fallback. 95% prediction intervals and budget exhaustion projection.

## Caching

- In-memory cache for single-node deployments (`src/cache/`)
- Redis required for multi-node deployments (for cache invalidation sync)
- Cache API keys, user data, and provider configs
- Invalidate cache on write operations

## Performance Considerations

- Database queries in API hot path should use caching
- Avoid allocations in frequently called code
- Use Cow<str> instead of String::from() where possible
