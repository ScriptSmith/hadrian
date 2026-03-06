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
//! # Axum Send compatibility
//!
//! Axum requires handler futures to be `Send`, but on wasm32 `reqwest`/`wasm-bindgen`
//! futures are `!Send`. Each handler wraps its async body with [`wasm_compat!`] which
//! runs the `!Send` work inside `spawn_local` and returns a `Send`-safe oneshot future.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use wasm_bindgen::prelude::*;

use crate::{
    catalog, compat::wasm_compat, config, db, events, jobs, models, pricing, providers, secrets,
    services,
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

        let secrets: Arc<dyn secrets::SecretManager> =
            Arc::new(secrets::MemorySecretManager::new());

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
        let svc = services::Services::new(db.clone(), file_storage, 1024);

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
            secrets: Some(secrets),
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
        };

        let router = build_wasm_router(state);

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
            .unwrap();

        convert_response(response).await
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Router
// ─────────────────────────────────────────────────────────────────────────────

fn build_wasm_router(state: crate::app::AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/v1/models", get(list_models))
        .route("/admin/v1/ui/config", get(get_ui_config))
        .route("/auth/me", get(auth_me))
        .route("/admin/v1/organizations", get(list_organizations))
        .route(
            "/admin/v1/organizations/{slug}/projects",
            get(list_org_projects),
        )
        .route(
            "/admin/v1/organizations/{slug}/prompts",
            get(list_org_prompts),
        )
        .route(
            "/admin/v1/me/providers",
            get(list_my_providers).post(create_my_provider),
        )
        .route(
            "/admin/v1/me/providers/test-credentials",
            post(test_provider_credentials),
        )
        .route(
            "/admin/v1/users/{user_id}/conversations/accessible",
            get(list_conversations),
        )
        .fallback(fallback_handler)
        .with_state(state)
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn health_check() -> Response {
    Json(serde_json::json!({"status": "ok", "mode": "wasm"})).into_response()
}

async fn list_models(State(state): State<crate::app::AppState>) -> Response {
    wasm_compat!(async move {
        let (svc, user_id) = match services_and_user(&state) {
            Ok(v) => v,
            Err(r) => return r,
        };

        // Collect all enabled providers for this user (paginate through all pages)
        let mut providers = Vec::new();
        let mut params = db::ListParams {
            limit: Some(100),
            ..Default::default()
        };
        loop {
            match svc
                .providers
                .list_enabled_by_user(user_id, params.clone())
                .await
            {
                Ok(page) => {
                    let has_more = page.has_more;
                    let next = page.cursors.next;
                    providers.extend(page.items);
                    if !has_more {
                        break;
                    }
                    match next {
                        Some(cursor) => params.cursor = Some(cursor),
                        None => break,
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to list providers");
                    return error_response(500, "Failed to list models");
                }
            }
        }

        // Look up user's org for scoped model IDs
        let org_slug = svc
            .users
            .get_org_memberships_for_user(user_id)
            .await
            .ok()
            .and_then(|m| m.into_iter().next())
            .map(|m| m.org_slug);

        // Resolve models for each provider (fetching from API when stored list is empty)
        let mut all_models = Vec::new();
        for provider in &providers {
            let model_names = resolve_provider_models(provider, &state).await;
            for model_name in &model_names {
                let scoped_id = if let Some(ref slug) = org_slug {
                    format!(":org/{slug}/:user/{user_id}/{}/{model_name}", provider.name)
                } else {
                    format!(":user/{user_id}/{}/{model_name}", provider.name)
                };
                all_models.push(serde_json::json!({
                    "id": scoped_id,
                    "object": "model",
                    "owned_by": provider.name,
                    "source": "dynamic",
                    "provider_name": provider.name,
                }));
            }
        }

        Json(serde_json::json!({"object": "list", "data": all_models})).into_response()
    })
}

async fn get_ui_config(State(state): State<crate::app::AppState>) -> Response {
    let config = &state.config;
    Json(serde_json::json!({
        "branding": {
            "title": config.ui.branding.title,
            "tagline": config.ui.branding.tagline,
            "logo_url": config.ui.branding.logo_url,
            "logo_dark_url": config.ui.branding.logo_dark_url,
            "favicon_url": config.ui.branding.favicon_url,
            "colors": {},
            "colors_dark": null,
            "fonts": null,
            "footer_text": config.ui.branding.footer_text,
            "footer_links": [],
            "show_version": config.ui.branding.show_version,
            "version": null,
            "login": null,
        },
        "chat": {
            "enabled": config.ui.chat.enabled,
            "default_model": config.ui.chat.default_model,
            "available_models": config.ui.chat.available_models,
            "file_uploads_enabled": config.ui.chat.file_uploads.enabled,
            "max_file_size_bytes": config.ui.chat.file_uploads.max_size_bytes,
            "allowed_file_types": config.ui.chat.file_uploads.allowed_types,
        },
        "admin": {
            "enabled": config.ui.admin.enabled,
        },
        "auth": {
            "methods": ["none"],
            "oidc": null,
        },
    }))
    .into_response()
}

async fn auth_me(State(state): State<crate::app::AppState>) -> Response {
    Json(serde_json::json!({
        "external_id": "anonymous",
        "email": "anonymous@localhost",
        "name": "Anonymous User",
        "user_id": state.default_user_id,
        "roles": ["super_admin"],
        "idp_groups": [],
    }))
    .into_response()
}

async fn list_organizations(State(state): State<crate::app::AppState>) -> Response {
    wasm_compat!(async move {
        let svc = match services(&state) {
            Ok(s) => s,
            Err(r) => return r,
        };
        match svc.organizations.list(db::ListParams::default()).await {
            Ok(result) => paginated(result),
            Err(e) => {
                tracing::error!(error = %e, "Failed to list organizations");
                error_response(500, "Failed to list organizations")
            }
        }
    })
}

async fn list_org_projects(
    State(state): State<crate::app::AppState>,
    Path(slug): Path<String>,
) -> Response {
    wasm_compat!(async move {
        let svc = match services(&state) {
            Ok(s) => s,
            Err(r) => return r,
        };
        let org = match resolve_org(svc, &slug).await {
            Ok(org) => org,
            Err(r) => return r,
        };
        match svc
            .projects
            .list_by_org(org.id, db::ListParams::default())
            .await
        {
            Ok(result) => paginated(result),
            Err(e) => {
                tracing::error!(error = %e, "Failed to list projects");
                error_response(500, "Failed to list projects")
            }
        }
    })
}

async fn list_org_prompts(
    State(state): State<crate::app::AppState>,
    Path(slug): Path<String>,
) -> Response {
    wasm_compat!(async move {
        let svc = match services(&state) {
            Ok(s) => s,
            Err(r) => return r,
        };
        let org = match resolve_org(svc, &slug).await {
            Ok(org) => org,
            Err(r) => return r,
        };
        match svc
            .prompts
            .list_by_owner(
                models::PromptOwnerType::Organization,
                org.id,
                db::ListParams::default(),
            )
            .await
        {
            Ok(result) => paginated(result),
            Err(e) => {
                tracing::error!(error = %e, "Failed to list prompts");
                error_response(500, "Failed to list prompts")
            }
        }
    })
}

async fn list_my_providers(State(state): State<crate::app::AppState>) -> Response {
    wasm_compat!(async move {
        let (svc, user_id) = match services_and_user(&state) {
            Ok(v) => v,
            Err(r) => return r,
        };
        match svc
            .providers
            .list_by_user(user_id, db::ListParams::default())
            .await
        {
            Ok(result) => {
                let items: Vec<models::DynamicProviderResponse> =
                    result.items.into_iter().map(Into::into).collect();
                paginated(db::ListResult {
                    items,
                    has_more: result.has_more,
                    cursors: result.cursors,
                })
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to list providers");
                error_response(500, "Failed to list providers")
            }
        }
    })
}

async fn create_my_provider(
    State(state): State<crate::app::AppState>,
    Json(input): Json<models::CreateSelfServiceProvider>,
) -> Response {
    wasm_compat!(async move {
        let (svc, user_id) = match services_and_user(&state) {
            Ok(v) => v,
            Err(r) => return r,
        };

        if !is_supported_provider_type(&input.provider_type) {
            return error_response(
                422,
                &format!("Unsupported provider type '{}'", input.provider_type),
            );
        }

        let create_input = models::CreateDynamicProvider {
            name: input.name,
            owner: models::ProviderOwner::User { user_id },
            provider_type: input.provider_type,
            base_url: input.base_url,
            api_key: input.api_key,
            config: input.config,
            models: input.models,
        };

        // No secrets manager in WASM — key stored directly in DB
        match svc.providers.create(create_input, None).await {
            Ok(provider) => {
                let resp: models::DynamicProviderResponse = provider.into();
                (axum::http::StatusCode::CREATED, Json(resp)).into_response()
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to create provider");
                error_response(500, &format!("Failed to create provider: {e}"))
            }
        }
    })
}

async fn test_provider_credentials(
    State(state): State<crate::app::AppState>,
    Json(input): Json<models::CreateSelfServiceProvider>,
) -> Response {
    wasm_compat!(async move {
        if !is_supported_provider_type(&input.provider_type) {
            return error_response(
                422,
                &format!("Unsupported provider type '{}'", input.provider_type),
            );
        }

        let provider = models::DynamicProvider {
            id: uuid::Uuid::nil(),
            name: input.name,
            owner: models::ProviderOwner::User {
                user_id: uuid::Uuid::nil(),
            },
            provider_type: input.provider_type,
            base_url: input.base_url,
            api_key_secret_ref: input.api_key,
            config: input.config,
            models: input.models.unwrap_or_default(),
            is_enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        // None for secrets → raw key used as-is
        let result =
            services::DynamicProviderService::run_connectivity_test(&provider, &state, None).await;
        Json(result).into_response()
    })
}

async fn list_conversations() -> Response {
    // Conversations are stored client-side in IndexedDB — return empty list
    paginated(db::ListResult::<serde_json::Value> {
        items: vec![],
        has_more: false,
        cursors: db::PageCursors::default(),
    })
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

    // Read body for methods that may have one
    let body = if method == http::Method::POST
        || method == http::Method::PUT
        || method == http::Method::PATCH
    {
        let text = wasm_bindgen_futures::JsFuture::from(
            req.text()
                .map_err(|_| JsError::new("Failed to read request body"))?,
        )
        .await
        .map_err(|_| JsError::new("Failed to read request body"))?;

        match text.as_string() {
            Some(s) => axum::body::Body::from(s),
            None => axum::body::Body::empty(),
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
async fn convert_response(response: Response) -> Result<web_sys::Response, JsError> {
    let (parts, body) = response.into_parts();

    let bytes = http_body_util::BodyExt::collect(body)
        .await
        .map_err(|e| JsError::new(&format!("Failed to read response body: {e}")))?
        .to_bytes();

    let init = web_sys::ResponseInit::new();
    init.set_status(parts.status.as_u16());

    let headers = web_sys::Headers::new().unwrap();
    for (key, value) in &parts.headers {
        if let Ok(v) = value.to_str() {
            let _ = headers.set(key.as_str(), v);
        }
    }
    // Ensure content-type is set for JSON responses
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

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn services(state: &crate::app::AppState) -> Result<&services::Services, Response> {
    state
        .services
        .as_ref()
        .ok_or_else(|| error_response(503, "Services not initialized"))
}

fn services_and_user(
    state: &crate::app::AppState,
) -> Result<(&services::Services, uuid::Uuid), Response> {
    let svc = services(state)?;
    let user_id = state
        .default_user_id
        .ok_or_else(|| error_response(503, "Default user not available"))?;
    Ok((svc, user_id))
}

async fn resolve_org(
    svc: &services::Services,
    slug: &str,
) -> Result<models::Organization, Response> {
    match svc.organizations.get_by_slug(slug).await {
        Ok(Some(org)) => Ok(org),
        Ok(None) => Err(error_response(404, "Organization not found")),
        Err(e) => {
            tracing::error!(error = %e, "Failed to look up organization");
            Err(error_response(500, "Failed to look up organization"))
        }
    }
}

/// Resolve model names for a dynamic provider.
///
/// If the provider has an explicit models list, use it. Otherwise, fetch from the
/// provider's API (matching the native server's behavior in `routes/api/models.rs`).
async fn resolve_provider_models(
    provider: &models::DynamicProvider,
    state: &crate::app::AppState,
) -> Vec<String> {
    if !provider.models.is_empty() {
        return provider.models.clone();
    }

    let Ok(config) =
        crate::routing::resolver::dynamic_provider_to_config(provider, state.secrets.as_ref())
            .await
    else {
        return Vec::new();
    };

    crate::providers::list_models_for_config(
        &config,
        &provider.name,
        &state.http_client,
        &state.circuit_breakers,
    )
    .await
    .map(|r| r.data.into_iter().map(|m| m.id).collect())
    .unwrap_or_default()
}

fn is_supported_provider_type(pt: &str) -> bool {
    matches!(
        pt,
        "openai" | "open_ai" | "openai_compatible" | "anthropic" | "test"
    )
}

fn paginated<T: serde::Serialize>(result: db::ListResult<T>) -> Response {
    Json(serde_json::json!({
        "data": result.items,
        "pagination": {
            "limit": 100,
            "has_more": result.has_more,
            "next_cursor": result.cursors.next.as_ref().map(|c| c.encode()),
            "prev_cursor": result.cursors.prev.as_ref().map(|c| c.encode()),
        }
    }))
    .into_response()
}

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
    config::GatewayConfig {
        server: config::ServerConfig::default(),
        database: config::DatabaseConfig::None,
        cache: config::CacheConfig::None,
        auth: config::AuthConfig {
            mode: config::AuthMode::None,
            ..Default::default()
        },
        providers: config::ProvidersConfig::default(),
        limits: config::LimitsConfig::default(),
        features: config::FeaturesConfig::default(),
        observability: config::ObservabilityConfig::default(),
        ui: config::UiConfig::default(),
        docs: config::DocsConfig::default(),
        pricing: pricing::PricingConfig::default(),
        secrets: config::SecretsConfig::None,
        retention: config::RetentionConfig::default(),
        storage: config::StorageConfig::default(),
    }
}
