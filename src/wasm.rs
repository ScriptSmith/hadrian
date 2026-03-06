//! WASM entry point for running Hadrian in the browser.
//!
//! Exports a [`HadrianGateway`] struct that can be instantiated from JavaScript
//! (service worker). Requests are dispatched to the service layer via a
//! match-based router (Axum's Router requires `Send` futures, which aren't
//! available on wasm32).
//!
//! # Architecture
//!
//! The gateway runs entirely in the browser's service worker thread:
//! - HTTP requests are intercepted by the service worker's `fetch` event handler
//! - Converted from `web_sys::Request` → match-based dispatch → service calls
//! - Responses converted back to `web_sys::Response` for the browser
//! - Provider API calls (OpenAI, Anthropic) go through `reqwest` which uses
//!   the browser's `fetch()` API on wasm32
//! - SQLite database via sql.js (in-memory) through JS FFI bridge

use std::sync::Arc;

use wasm_bindgen::prelude::*;

use crate::{catalog, config, db, events, jobs, models, pricing, providers, secrets, services};

/// Browser-based Hadrian gateway.
///
/// Instantiated once in the service worker and reused for all requests.
#[wasm_bindgen]
pub struct HadrianGateway {
    state: crate::app::AppState,
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

        tracing::info!("Hadrian WASM gateway initialized (with database)");
        Ok(HadrianGateway { state })
    }

    /// Handle a fetch request from the service worker.
    ///
    /// Match-based dispatcher — routes to service layer calls directly.
    pub async fn handle(&self, request: web_sys::Request) -> Result<web_sys::Response, JsError> {
        let method = request.method();
        let url =
            web_sys::Url::new(&request.url()).map_err(|_| JsError::new("Invalid request URL"))?;
        let raw_path = url.pathname();
        let query_string = url.search();

        // The frontend uses /api/v1/ but backend routes are /v1/
        let path = raw_path
            .strip_prefix("/api/v1/")
            .map(|rest| format!("/v1/{rest}"))
            .unwrap_or(raw_path);

        tracing::debug!(method = %method, path = %path, "WASM gateway handling request");

        let response = match (method.as_str(), path.as_str()) {
            // Health check
            ("GET", "/health") => self.health_check(),

            // Models
            ("GET", "/v1/models") => self.list_models().await,

            // UI config
            ("GET", "/admin/v1/ui/config") => self.get_ui_config(),

            // Auth
            ("GET", "/auth/me") => self.auth_me(),

            // Organizations
            ("GET", "/admin/v1/organizations") => self.list_organizations().await,

            // Self-service providers
            ("GET", "/admin/v1/me/providers") => self.list_my_providers(&query_string).await,
            ("POST", "/admin/v1/me/providers/test-credentials") => {
                let body = read_request_body(&request).await?;
                self.test_provider_credentials(&body).await
            }
            ("POST", "/admin/v1/me/providers") => {
                let body = read_request_body(&request).await?;
                self.create_my_provider(&body).await
            }

            // Dynamic org routes: /admin/v1/organizations/{slug}/...
            _ => self.handle_dynamic_route(&method, &path, &query_string, &request).await,
        };

        Ok(response)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Route handlers
// ─────────────────────────────────────────────────────────────────────────────

impl HadrianGateway {
    fn services(&self) -> Result<&services::Services, web_sys::Response> {
        self.state
            .services
            .as_ref()
            .ok_or_else(|| json_error_response(503, "Services not initialized"))
    }

    fn default_user_id(&self) -> Result<uuid::Uuid, web_sys::Response> {
        self.state
            .default_user_id
            .ok_or_else(|| json_error_response(503, "Default user not available"))
    }

    fn health_check(&self) -> web_sys::Response {
        json_response(200, r#"{"status":"ok","mode":"wasm"}"#)
    }

    async fn list_models(&self) -> web_sys::Response {
        let svc = match self.services() {
            Ok(s) => s,
            Err(r) => return r,
        };
        let user_id = match self.default_user_id() {
            Ok(id) => id,
            Err(r) => return r,
        };

        // List all enabled providers for the default user, then aggregate models
        let params = db::ListParams::default();
        match svc.providers.list_enabled_by_user(user_id, params).await {
            Ok(result) => {
                let mut models = Vec::new();
                for provider in &result.items {
                    for model_name in &provider.models {
                        models.push(serde_json::json!({
                            "id": format!("{}:{}", provider.name, model_name),
                            "object": "model",
                            "created": 0,
                            "owned_by": provider.name,
                        }));
                    }
                }
                json_value_response(
                    200,
                    &serde_json::json!({
                        "object": "list",
                        "data": models,
                    }),
                )
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to list models");
                json_error_response(500, "Failed to list models")
            }
        }
    }

    fn get_ui_config(&self) -> web_sys::Response {
        let config = &self.state.config;
        // Return a minimal UI config with auth mode = none
        let auth_methods = vec!["none"];

        let response = serde_json::json!({
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
                "methods": auth_methods,
                "oidc": null,
            },
        });
        json_value_response(200, &response)
    }

    fn auth_me(&self) -> web_sys::Response {
        let response = serde_json::json!({
            "external_id": "anonymous",
            "email": "anonymous@localhost",
            "name": "Anonymous User",
            "user_id": self.state.default_user_id,
            "roles": ["super_admin"],
            "idp_groups": [],
        });
        json_value_response(200, &response)
    }

    /// Route requests with dynamic path segments (e.g. org slug, user ID).
    async fn handle_dynamic_route(
        &self,
        method: &str,
        path: &str,
        _query_string: &str,
        _request: &web_sys::Request,
    ) -> web_sys::Response {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        match (method, segments.as_slice()) {
            // GET /admin/v1/organizations/{slug}/projects
            ("GET", ["admin", "v1", "organizations", slug, "projects"]) => {
                self.list_org_projects(slug).await
            }
            // GET /admin/v1/organizations/{slug}/prompts
            ("GET", ["admin", "v1", "organizations", slug, "prompts"]) => {
                self.list_org_prompts(slug).await
            }
            // GET /admin/v1/users/{user_id}/conversations/accessible
            ("GET", ["admin", "v1", "users", _user_id, "conversations", "accessible"]) => {
                self.list_accessible_conversations().await
            }
            _ => {
                tracing::debug!(method, path, "No matching WASM route");
                json_error_response(404, "Not found")
            }
        }
    }

    async fn list_organizations(&self) -> web_sys::Response {
        let svc = match self.services() {
            Ok(s) => s,
            Err(r) => return r,
        };

        let params = db::ListParams::default();
        match svc.organizations.list(params).await {
            Ok(result) => paginated_response(result),
            Err(e) => {
                tracing::error!(error = %e, "Failed to list organizations");
                json_error_response(500, "Failed to list organizations")
            }
        }
    }

    async fn list_my_providers(&self, _query_string: &str) -> web_sys::Response {
        let svc = match self.services() {
            Ok(s) => s,
            Err(r) => return r,
        };
        let user_id = match self.default_user_id() {
            Ok(id) => id,
            Err(r) => return r,
        };

        let params = db::ListParams::default();
        match svc.providers.list_by_user(user_id, params).await {
            Ok(result) => {
                // Map to response type (hides secret refs)
                let items: Vec<models::DynamicProviderResponse> =
                    result.items.into_iter().map(Into::into).collect();
                let mapped = db::ListResult {
                    items,
                    has_more: result.has_more,
                    cursors: result.cursors,
                };
                paginated_response(mapped)
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to list providers");
                json_error_response(500, "Failed to list providers")
            }
        }
    }

    async fn create_my_provider(&self, body: &str) -> web_sys::Response {
        let svc = match self.services() {
            Ok(s) => s,
            Err(r) => return r,
        };
        let user_id = match self.default_user_id() {
            Ok(id) => id,
            Err(r) => return r,
        };

        let input: models::CreateSelfServiceProvider = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(e) => return json_error_response(422, &format!("Invalid request body: {e}")),
        };

        // Validate provider type
        if !is_supported_provider_type(&input.provider_type) {
            return json_error_response(
                422,
                &format!("Unsupported provider type '{}'", input.provider_type),
            );
        }

        // Convert to internal CreateDynamicProvider
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
                json_value_response(201, &serde_json::json!(resp))
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to create provider");
                json_error_response(500, &format!("Failed to create provider: {e}"))
            }
        }
    }

    async fn list_org_projects(&self, org_slug: &str) -> web_sys::Response {
        let svc = match self.services() {
            Ok(s) => s,
            Err(r) => return r,
        };

        let org = match svc.organizations.get_by_slug(org_slug).await {
            Ok(Some(org)) => org,
            Ok(None) => return json_error_response(404, "Organization not found"),
            Err(e) => {
                tracing::error!(error = %e, "Failed to look up organization");
                return json_error_response(500, "Failed to look up organization");
            }
        };

        let params = db::ListParams::default();
        match svc.projects.list_by_org(org.id, params).await {
            Ok(result) => paginated_response(result),
            Err(e) => {
                tracing::error!(error = %e, "Failed to list projects");
                json_error_response(500, "Failed to list projects")
            }
        }
    }

    async fn list_org_prompts(&self, org_slug: &str) -> web_sys::Response {
        let svc = match self.services() {
            Ok(s) => s,
            Err(r) => return r,
        };

        let org = match svc.organizations.get_by_slug(org_slug).await {
            Ok(Some(org)) => org,
            Ok(None) => return json_error_response(404, "Organization not found"),
            Err(e) => {
                tracing::error!(error = %e, "Failed to look up organization");
                return json_error_response(500, "Failed to look up organization");
            }
        };

        let params = db::ListParams::default();
        match svc
            .prompts
            .list_by_owner(models::PromptOwnerType::Organization, org.id, params)
            .await
        {
            Ok(result) => paginated_response(result),
            Err(e) => {
                tracing::error!(error = %e, "Failed to list prompts");
                json_error_response(500, "Failed to list prompts")
            }
        }
    }

    async fn list_accessible_conversations(&self) -> web_sys::Response {
        // Return empty list — conversations are stored client-side in IndexedDB
        empty_paginated_response()
    }

    async fn test_provider_credentials(&self, body: &str) -> web_sys::Response {
        let input: models::CreateSelfServiceProvider = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(e) => return json_error_response(422, &format!("Invalid request body: {e}")),
        };

        if !is_supported_provider_type(&input.provider_type) {
            return json_error_response(
                422,
                &format!("Unsupported provider type '{}'", input.provider_type),
            );
        }

        // Build a transient provider for connectivity testing
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
            services::DynamicProviderService::run_connectivity_test(&provider, &self.state, None)
                .await;
        json_value_response(200, &serde_json::json!(result))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Check if a provider type is supported in the WASM build.
fn is_supported_provider_type(pt: &str) -> bool {
    matches!(
        pt,
        "openai" | "open_ai" | "openai_compatible" | "anthropic" | "test"
    )
}

/// Read the full request body as a UTF-8 string.
async fn read_request_body(request: &web_sys::Request) -> Result<String, JsError> {
    let body = wasm_bindgen_futures::JsFuture::from(
        request
            .text()
            .map_err(|_| JsError::new("Failed to read request body"))?,
    )
    .await
    .map_err(|_| JsError::new("Failed to read request body"))?;

    body.as_string()
        .ok_or_else(|| JsError::new("Request body is not valid text"))
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

/// Build a JSON `web_sys::Response` from a string body.
fn json_response(status: u16, body: &str) -> web_sys::Response {
    let init = web_sys::ResponseInit::new();
    init.set_status(status);

    let headers = web_sys::Headers::new().unwrap();
    headers.set("Content-Type", "application/json").unwrap();
    init.set_headers(&headers.into());

    web_sys::Response::new_with_opt_str_and_init(Some(body), &init).unwrap()
}

/// Build a JSON response from a serializable value.
fn json_value_response(status: u16, value: &serde_json::Value) -> web_sys::Response {
    json_response(status, &value.to_string())
}

/// Build a paginated JSON response from a `ListResult<T>`.
fn paginated_response<T: serde::Serialize>(result: db::ListResult<T>) -> web_sys::Response {
    json_value_response(
        200,
        &serde_json::json!({
            "data": result.items,
            "pagination": {
                "limit": 100,
                "has_more": result.has_more,
                "next_cursor": result.cursors.next.as_ref().map(|c| c.encode()),
                "prev_cursor": result.cursors.prev.as_ref().map(|c| c.encode()),
            }
        }),
    )
}

/// Build an empty paginated JSON response.
fn empty_paginated_response() -> web_sys::Response {
    json_value_response(
        200,
        &serde_json::json!({
            "data": [],
            "pagination": {
                "limit": 100,
                "has_more": false,
                "next_cursor": null,
                "prev_cursor": null,
            }
        }),
    )
}

/// Build a JSON error response.
fn json_error_response(status: u16, message: &str) -> web_sys::Response {
    let body = serde_json::json!({
        "error": {
            "message": message,
            "type": "error",
            "code": status,
        }
    })
    .to_string();
    json_response(status, &body)
}
