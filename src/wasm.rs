//! WASM entry point for running Hadrian in the browser.
//!
//! Exports a [`HadrianGateway`] struct that can be instantiated from JavaScript
//! (service worker). Requests are dispatched via an Axum [`Router`] — the same
//! routing engine used by the native server — so path parameters, method matching,
//! and fallback handling all work identically.
//!
//! # Architecture
//!
//! The gateway runs entirely in the browser's service worker thread:
//! - HTTP requests are intercepted by the service worker's `fetch` event handler
//! - Converted from `web_sys::Request` → `http::Request` → Axum Router → service calls
//! - Responses converted back to `web_sys::Response` for the browser
//! - Provider API calls (OpenAI, Anthropic) go through `reqwest` which uses
//!   the browser's `fetch()` API on wasm32
//! - SQLite database via sql.js (in-memory) through JS FFI bridge
//!
//! # Route Handler Reuse
//!
//! Route handlers are shared with the native server via [`admin_v1_routes()`] and
//! [`api_v1_routes()`]. The WASM router injects `Extension<AdminAuth>`,
//! `Extension<AuthzContext>`, and `Extension<ClientInfo>` layers so handlers can
//! extract them identically. Only handlers with genuinely different WASM behavior
//! (health check, auth stub, conversations stub) are defined here.
//!
//! # Axum Send compatibility
//!
//! Axum requires handler futures to be `Send`, but on wasm32 `reqwest`/`wasm-bindgen`
//! futures are `!Send`. The [`crate::compat::wasm_routing`] module provides drop-in
//! replacements for `axum::routing::{get, post, ...}` that wrap handlers in
//! [`crate::compat::WasmHandler`], asserting `Send` since wasm32 is single-threaded.

use std::{collections::HashMap, sync::Arc};

use axum::{
    Extension, Json, Router,
    extract::State,
    response::{IntoResponse, Response},
};
use wasm_bindgen::prelude::*;

use crate::{
    auth::Identity,
    authz::AuthzEngine,
    catalog,
    compat::wasm_routing::get,
    config, db, events, jobs,
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    pricing, providers, services,
};

/// Browser-based Hadrian gateway.
///
/// Instantiated once in the service worker and reused for all requests.
#[wasm_bindgen]
pub struct HadrianGateway {
    router: Router,
}

#[wasm_bindgen]
impl HadrianGateway {
    /// Initialize the gateway with sql.js database. Called once from the service worker.
    #[wasm_bindgen(constructor)]
    pub async fn new() -> Result<HadrianGateway, JsError> {
        tracing_wasm::set_as_global_default();
        tracing::info!("Initializing Hadrian WASM gateway");

        let config = wasm_default_config();
        let http_client = reqwest::Client::new();

        // No secret manager in WASM — API keys are stored directly in SQLite
        // (which is persisted to IndexedDB). Using MemorySecretManager would lose
        // secrets when the service worker restarts.

        let event_bus = Arc::new(events::EventBus::with_capacity(
            config.features.websocket.channel_capacity,
        ));

        // Initialize sql.js database via JS bridge
        let pool = db::wasm_sqlite::WasmSqlitePool::new();
        pool.init()
            .await
            .map_err(|e| JsError::new(&format!("DB init failed: {e}")))?;

        tracing::info!("Running database migrations");
        pool.run_migrations()
            .await
            .map_err(|e| JsError::new(&format!("Migrations failed: {e}")))?;

        let db = Arc::new(db::DbPool::from_wasm_sqlite(pool));
        let file_storage: Arc<dyn services::FileStorage> =
            Arc::new(services::DatabaseFileStorage::new(db.clone()));
        let svc = services::Services::new(db.clone(), file_storage, 1024, 512_000);

        // Bootstrap default user and org (auth=none)
        let default_user_id = match crate::app::AppState::ensure_default_user(&svc).await {
            Ok(id) => {
                tracing::info!(user_id = %id, "Default anonymous user available");
                Some(id)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create default user");
                None
            }
        };

        let default_org_id = match crate::app::AppState::ensure_default_org(&svc).await {
            Ok(id) => {
                tracing::info!(org_id = %id, "Default local organization available");
                Some(id)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to create default organization");
                None
            }
        };

        if let (Some(uid), Some(oid)) = (default_user_id, default_org_id) {
            if let Err(e) =
                crate::app::AppState::ensure_default_org_membership(&svc, uid, oid).await
            {
                tracing::warn!(error = %e, "Failed to add user to default organization");
            }
        }

        let state = crate::app::AppState {
            http_client,
            config: Arc::new(config.clone()),
            db: Some(db),
            services: Some(svc),
            cache: None,
            secrets: None,
            dlq: None,
            pricing: Arc::new(config.pricing.clone()),
            circuit_breakers: providers::CircuitBreakerRegistry::new(),
            provider_health: jobs::ProviderHealthStateRegistry::new(),
            #[cfg(feature = "sso")]
            oidc_registry: None,
            #[cfg(feature = "saml")]
            saml_registry: None,
            #[cfg(feature = "jwt")]
            gateway_jwt_registry: None,
            policy_registry: None,
            response_cache: None,
            semantic_cache: None,
            input_guardrails: None,
            output_guardrails: None,
            event_bus,
            file_search_service: None,
            #[cfg(any(
                feature = "document-extraction-basic",
                feature = "document-extraction-full"
            ))]
            document_processor: None,
            default_user_id,
            default_org_id,
            provider_metrics: Arc::new(services::ProviderMetricsService::new()),
            model_catalog: catalog::ModelCatalogRegistry::new(),
            static_models_cache: Arc::new(tokio::sync::RwLock::new(Default::default())),
        };

        let router = build_wasm_router(state, default_user_id, default_org_id);

        tracing::info!("Hadrian WASM gateway initialized (with database)");
        Ok(HadrianGateway { router })
    }

    /// Handle a fetch request from the service worker.
    ///
    /// Converts `web_sys::Request` → Axum Router dispatch → `web_sys::Response`.
    pub async fn handle(&self, request: web_sys::Request) -> Result<web_sys::Response, JsError> {
        let http_request = convert_request(&request).await?;

        let response = tower::ServiceExt::oneshot(self.router.clone(), http_request)
            .await
            .map_err(|e| JsError::new(&format!("Router error: {e}")))?;

        convert_response(response).await
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

fn build_wasm_router(
    state: crate::app::AppState,
    default_user_id: Option<uuid::Uuid>,
    default_org_id: Option<uuid::Uuid>,
) -> Router {
    // Build permissive authz context for WASM (no RBAC in browser)
    let engine = Arc::new(
        AuthzEngine::new(config::RbacConfig {
            enabled: false,
            ..Default::default()
        })
        .expect("Failed to create disabled RBAC engine"),
    );
    let authz = AuthzContext::permissive(engine);

    let admin_auth = AdminAuth {
        identity: Identity {
            external_id: "anonymous".to_string(),
            email: Some("anonymous@localhost".to_string()),
            name: Some("Anonymous User".to_string()),
            user_id: default_user_id,
            roles: vec!["admin".to_string()],
            idp_groups: Vec::new(),
            org_ids: default_org_id
                .map(|id| vec![id.to_string()])
                .unwrap_or_default(),
            team_ids: Vec::new(),
            project_ids: Vec::new(),
        },
    };

    // Shared route builders from the actual server code.
    // Merge public admin routes (ui config) into the admin router so we can nest once.
    let admin_routes = crate::routes::admin::admin_v1_routes()
        .merge(crate::routes::admin::public_admin_v1_routes());
    let api_routes = crate::routes::api::api_v1_routes();

    Router::new()
        // WASM-specific handlers (genuinely different behavior)
        .route("/health", get(health_check))
        .route("/auth/me", get(auth_me))
        // Shared routes from actual server code
        .nest("/admin/v1", admin_routes)
        .merge(api_routes)
        // Inject extensions that middleware would normally provide
        .layer(Extension(admin_auth))
        .layer(Extension(authz))
        .layer(Extension(ClientInfo::default()))
        .fallback(fallback_handler)
        .with_state(state)
}

// ─────────────────────────────────────────────────────────────────────────────
// WASM-specific handlers (genuinely different behavior)
// ─────────────────────────────────────────────────────────────────────────────

async fn health_check() -> Response {
    Json(serde_json::json!({"status": "ok", "mode": "wasm"})).into_response()
}

async fn auth_me(State(state): State<crate::app::AppState>) -> Response {
    Json(serde_json::json!({
        "external_id": "anonymous",
        "email": "anonymous@localhost",
        "name": "Anonymous User",
        "user_id": state.default_user_id,
        "roles": ["admin"],
        "idp_groups": [],
    }))
    .into_response()
}

async fn fallback_handler() -> Response {
    error_response(404, "Not found")
}

// ─────────────────────────────────────────────────────────────────────────────
// Request / Response conversion
// ─────────────────────────────────────────────────────────────────────────────

/// Convert `web_sys::Request` → `http::Request<axum::body::Body>`.
async fn convert_request(
    req: &web_sys::Request,
) -> Result<http::Request<axum::body::Body>, JsError> {
    let method_str = req.method();
    let url = web_sys::Url::new(&req.url()).map_err(|_| JsError::new("Invalid request URL"))?;

    // The frontend uses /api/v1/ but backend routes are /v1/
    let raw_path = url.pathname();
    let path = raw_path
        .strip_prefix("/api/v1/")
        .map(|rest| format!("/v1/{rest}"))
        .unwrap_or(raw_path);

    let search = url.search();
    let uri = if search.is_empty() {
        path
    } else {
        format!("{path}{search}")
    };

    tracing::debug!(method = %method_str, uri = %uri, "WASM gateway handling request");

    let method: http::Method = method_str
        .parse()
        .map_err(|_| JsError::new("Invalid HTTP method"))?;

    // Read body for methods that may have one.
    // Use array_buffer() instead of text() to correctly handle binary content
    // (multipart form-data, file uploads, audio).
    let body = if method == http::Method::POST
        || method == http::Method::PUT
        || method == http::Method::PATCH
    {
        let buf = wasm_bindgen_futures::JsFuture::from(
            req.array_buffer()
                .map_err(|_| JsError::new("Failed to read request body"))?,
        )
        .await
        .map_err(|_| JsError::new("Failed to read request body"))?;

        let uint8 = js_sys::Uint8Array::new(&buf);
        let bytes = uint8.to_vec();
        if bytes.is_empty() {
            axum::body::Body::empty()
        } else {
            axum::body::Body::from(bytes)
        }
    } else {
        axum::body::Body::empty()
    };

    let mut builder = http::Request::builder().method(method).uri(&uri);

    // Copy headers
    let headers = req.headers();
    let entries = js_sys::try_iter(&headers).ok().flatten();
    if let Some(iter) = entries {
        for entry in iter {
            if let Ok(entry) = entry {
                let pair = js_sys::Array::from(&entry);
                if let (Some(key), Some(value)) = (pair.get(0).as_string(), pair.get(1).as_string())
                {
                    if let (Ok(name), Ok(val)) = (
                        http::header::HeaderName::from_bytes(key.as_bytes()),
                        http::header::HeaderValue::from_str(&value),
                    ) {
                        builder = builder.header(name, val);
                    }
                }
            }
        }
    }

    builder
        .body(body)
        .map_err(|e| JsError::new(&format!("Failed to build request: {e}")))
}

/// Convert `axum::Response` → `web_sys::Response`.
///
/// Streaming responses (SSE) are returned with a `ReadableStream` body so the
/// browser receives chunks in real time. Non-streaming responses are fully
/// buffered first (simpler and fine for small JSON payloads).
async fn convert_response(response: Response) -> Result<web_sys::Response, JsError> {
    let (parts, body) = response.into_parts();

    let init = web_sys::ResponseInit::new();
    init.set_status(parts.status.as_u16());

    let headers = web_sys::Headers::new().unwrap();
    for (key, value) in &parts.headers {
        if let Ok(v) = value.to_str() {
            let _ = headers.set(key.as_str(), v);
        }
    }

    let is_sse = parts
        .headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.contains("text/event-stream"));

    if is_sse {
        init.set_headers(&headers.into());
        let stream = body_to_readable_stream(body);
        web_sys::Response::new_with_opt_readable_stream_and_init(Some(&stream), &init)
            .map_err(|_| JsError::new("Failed to create streaming response"))
    } else {
        let bytes = http_body_util::BodyExt::collect(body)
            .await
            .map_err(|e| JsError::new(&format!("Failed to read response body: {e}")))?
            .to_bytes();

        if !parts.headers.contains_key(http::header::CONTENT_TYPE) && !bytes.is_empty() {
            let _ = headers.set("content-type", "application/json");
        }
        init.set_headers(&headers.into());

        let body_js = if bytes.is_empty() {
            None
        } else {
            let uint8 = js_sys::Uint8Array::from(bytes.as_ref());
            Some(uint8.into())
        };

        web_sys::Response::new_with_opt_buffer_source_and_init(body_js.as_ref(), &init)
            .map_err(|_| JsError::new("Failed to create response"))
    }
}

/// Create a pull-based `ReadableStream` that yields body frames as they arrive.
fn body_to_readable_stream(body: axum::body::Body) -> web_sys::ReadableStream {
    use std::{cell::RefCell, rc::Rc};

    let body = Rc::new(RefCell::new(body));

    let pull = Closure::wrap(Box::new(move |controller: JsValue| -> js_sys::Promise {
        let body = body.clone();
        wasm_bindgen_futures::future_to_promise(async move {
            use http_body_util::BodyExt;

            let mut body = body.borrow_mut();
            match body.frame().await {
                Some(Ok(frame)) => {
                    if let Some(data) = frame.data_ref() {
                        let uint8 = js_sys::Uint8Array::from(data.as_ref());
                        let enqueue: js_sys::Function =
                            js_sys::Reflect::get(&controller, &"enqueue".into())
                                .unwrap()
                                .unchecked_into();
                        let _ = enqueue.call1(&controller, &uint8);
                    }
                    // Non-data frames (trailers) are ignored; pull will be called again.
                }
                Some(Err(_)) | None => {
                    let close: js_sys::Function =
                        js_sys::Reflect::get(&controller, &"close".into())
                            .unwrap()
                            .unchecked_into();
                    let _ = close.call0(&controller);
                }
            }
            Ok(JsValue::UNDEFINED)
        })
    }) as Box<dyn FnMut(JsValue) -> js_sys::Promise>);

    let source = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&source, &"pull".into(), pull.as_ref().unchecked_ref());
    pull.forget(); // leak closure so it lives as long as the stream

    web_sys::ReadableStream::new_with_underlying_source(&source).unwrap()
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn error_response(status: u16, message: &str) -> Response {
    let code = axum::http::StatusCode::from_u16(status)
        .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    (
        code,
        Json(serde_json::json!({
            "error": {
                "message": message,
                "type": "error",
                "code": status,
            }
        })),
    )
        .into_response()
}

/// Create a minimal config suitable for WASM browser operation.
fn wasm_default_config() -> config::GatewayConfig {
    use config::{PageConfig, PageStatus};

    let notice = |msg: &str| PageConfig::Detailed {
        status: PageStatus::Notice,
        notice_message: Some(format!("This feature requires Hadrian Server. {msg}")),
    };
    config::GatewayConfig {
        server: config::ServerConfig {
            allow_loopback_urls: true,
            allow_private_urls: true,
            ..Default::default()
        },
        database: config::DatabaseConfig::None,
        cache: config::CacheConfig::None,
        auth: config::AuthConfig {
            mode: config::AuthMode::None,
            ..Default::default()
        },
        providers: config::ProvidersConfig {
            default_provider: Some("test".to_string()),
            providers: HashMap::from([(
                "test".to_string(),
                config::ProviderConfig::Test(config::TestProviderConfig {
                    model_name: "test-model".to_string(),
                    failure_mode: config::TestFailureMode::None,
                    timeout_secs: 30,
                    allowed_models: Vec::new(),
                    model_aliases: HashMap::new(),
                    models: HashMap::new(),
                    retry: config::RetryConfig::default(),
                    circuit_breaker: config::CircuitBreakerConfig::default(),
                    fallback_providers: Vec::new(),
                    model_fallbacks: HashMap::new(),
                    health_check: config::ProviderHealthCheckConfig::default(),
                    catalog_provider: None,
                    sovereignty: None,
                }),
            )]),
        },
        limits: config::LimitsConfig::default(),
        features: config::FeaturesConfig {
            static_models_cache: config::StaticModelsCacheConfig {
                refresh_interval_secs: 0,
            },
            ..Default::default()
        },
        observability: config::ObservabilityConfig::default(),
        ui: config::UiConfig {
            pages: config::PagesConfig {
                teams: notice("Team management needs multi-user authentication."),
                api_keys: notice("API key management is not available in browser mode."),
                usage: notice("Usage tracking is not available in browser mode."),
                admin: config::AdminPagesConfig {
                    organizations: notice(
                        "Organization management is not available in browser mode.",
                    ),
                    teams: notice("Team management needs multi-user authentication."),
                    service_accounts: notice("Service accounts are not available in browser mode."),
                    users: notice("User management is not available in browser mode."),
                    sso: notice("Single sign-on is not available in browser mode."),
                    api_keys: notice("API key management is not available in browser mode."),
                    provider_health: notice(
                        "Provider health monitoring is not available in browser mode.",
                    ),
                    pricing: notice("Pricing configuration requires usage tracking."),
                    usage: notice("Usage tracking is not available in browser mode."),
                    audit_logs: notice("Audit logging is not available in browser mode."),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        },
        docs: config::DocsConfig::default(),
        pricing: pricing::PricingConfig::default(),
        secrets: config::SecretsConfig::None,
        retention: config::RetentionConfig::default(),
        storage: config::StorageConfig::default(),
        sovereignty: config::SovereigntyConfig::default(),
    }
}
