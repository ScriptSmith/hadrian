# WASM Build

The WASM build runs the full Hadrian Axum router inside a browser service worker, enabling a zero-backend deployment at app.hadriangateway.com.

## Backend Architecture

**Request flow:**
1. Service worker intercepts `fetch` events matching `/v1/`, `/admin/v1/`, `/health`, `/auth/`, `/api/`
2. `web_sys::Request` is converted to `http::Request` (with `/api/v1/` → `/v1/` path rewriting)
3. Request is dispatched through the same Axum `Router` used by the native server
4. `http::Response` is converted back to `web_sys::Response`
5. LLM API calls use `reqwest` which delegates to the browser's `fetch()` API

**Three-layer gating strategy:**
1. **Cargo features** (`wasm` vs `server`) — Controls what modules/dependencies are included
2. **`#[cfg(target_arch = "wasm32")]`** — Handles Send/Sync differences (`AssertSend`, `async_trait(?Send)`, `spawn_local` vs `tokio::spawn`)
3. **`#[cfg(feature = "server")]`** / **`#[cfg(feature = "concurrency")]`** — Gates server-only functionality (middleware layers, `TaskTracker`, `UsageLogBuffer`)

**Database:** `WasmSqlitePool` is a zero-size type; actual SQLite runs in JavaScript via sql.js. Queries cross the FFI boundary via `wasm_bindgen` extern functions. The `backend.rs` abstraction provides cfg-switched type aliases (`Pool`, `Row`, `BackendError`) and traits (`ColDecode`, `RowExt`) so SQLite repo code compiles against either `sqlx::SqlitePool` or `WasmSqlitePool` without changes.

**Persistence:** Database is persisted to IndexedDB with a debounced save (500ms) after write operations.

**Auth:** WASM mode uses `AuthMode::None` with a bootstrapped anonymous user and org. Permissive `AuthzContext` and `AdminAuth` extensions are injected as layers.

**Setup flow:** `WasmSetupGuard` detects if providers are configured; if not, shows a setup wizard (`WasmSetup`) supporting OpenRouter OAuth (PKCE), Ollama auto-detection, and manual API key entry for OpenAI/Anthropic/etc.

**Known limitations:**
- Streaming responses are fully buffered (no real-time SSE token streaming for LLM calls)
- No usage tracking (no `TaskTracker`/`UsageLogBuffer` in WASM)
- No caching layer, rate limiting, or budget enforcement
- Module service workers require Chrome 91+ / Edge 91+ (Firefox support may be limited)

## Building

```bash
./scripts/build-wasm.sh           # Dev build
./scripts/build-wasm.sh --release # Release build
```

## Frontend Development

The WASM mode is controlled by the `VITE_WASM_MODE=true` environment variable. When set:
- The Vite dev server uses a custom service worker plugin instead of `VitePWA`
- The proxy configuration is disabled (service worker handles API routing)
- `main.tsx` registers the service worker before rendering React
- `App.tsx` wraps the app in `WasmSetupGuard`

```bash
# Build WASM module first (from repo root)
./scripts/build-wasm.sh

# Then run frontend in WASM mode
cd ui && VITE_WASM_MODE=true pnpm dev
```

The service worker (`sw.ts`) is built separately from the Vite bundle using esbuild (via the custom `wasmServiceWorkerPlugin` in `vite.config.ts`). In dev mode it's compiled on each request; in production it's written to `dist/sw.js` during the `writeBundle` hook.

## Modifying WASM Code

- The `wasm_routing` module (`src/compat.rs`) provides drop-in replacements for `axum::routing::{get, post, put, patch, delete}` — route modules use cfg-switched imports
- All async trait definitions use `#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]` / `#[cfg_attr(not(target_arch = "wasm32"), async_trait)]`
- The `backend.rs` abstraction means SQLite repo code is written once — modify repos normally and both native/WASM will compile
- Server-only routes (multipart file upload, audio transcription/translation) are excluded with `#[cfg(feature = "server")]`
