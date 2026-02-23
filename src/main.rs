use std::{path::PathBuf, sync::Arc};

#[cfg(feature = "utoipa")]
use axum::Json;
#[cfg(any(feature = "embed-ui", feature = "embed-docs"))]
use axum::response::Response;
#[cfg(any(feature = "sso", feature = "saml"))]
use axum::routing::post;
use axum::{Router, routing::get};
#[cfg(any(feature = "embed-ui", feature = "embed-docs"))]
use axum::{body::Body, response::IntoResponse};
use clap::Parser;
#[cfg(any(feature = "embed-ui", feature = "embed-docs"))]
use http::StatusCode;
use http::header;
use reqwest::{self, Client};
#[cfg(any(feature = "embed-ui", feature = "embed-docs"))]
use rust_embed::Embed;
use tokio_util::task::TaskTracker;
use tower_http::{
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
#[cfg(feature = "utoipa")]
use utoipa_scalar::{Scalar, Servable};

mod api_types;
mod auth;
pub mod authz;
mod cache;
mod catalog;
mod config;
mod db;
mod dlq;
pub mod events;
mod guardrails;
mod jobs;
mod middleware;
mod models;
pub mod observability;
mod ontology;
pub mod openapi;
mod pricing;
mod providers;
mod retention;
mod routes;
mod routing;
#[cfg(feature = "sso")]
pub mod scim;
mod secrets;
pub mod services;
mod streaming;
mod usage_buffer;
mod usage_sink;
mod validation;
#[cfg(feature = "wizard")]
mod wizard;

#[cfg(test)]
mod tests;

/// Embedded UI assets from ui/dist directory.
/// These are compiled into the binary at build time.
#[cfg(feature = "embed-ui")]
#[derive(Embed)]
#[folder = "ui/dist"]
#[allow_missing = true]
struct UiAssets;

/// Embedded documentation site assets from docs/out directory.
/// These are compiled into the binary at build time.
#[cfg(feature = "embed-docs")]
#[derive(Embed)]
#[folder = "docs/out"]
#[allow_missing = true]
struct DocsAssets;

/// Handler for serving embedded UI assets
#[cfg(feature = "embed-ui")]
async fn serve_embedded_asset(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    serve_embedded_file(&path)
}

/// Handler for serving embedded UI index at root
#[cfg(feature = "embed-ui")]
async fn serve_embedded_index() -> Response {
    serve_embedded_file("index.html")
}

#[cfg(feature = "embed-ui")]
fn serve_embedded_file(path: &str) -> Response {
    // Try to get the file, or fall back to index.html for SPA routing
    let file = UiAssets::get(path).or_else(|| UiAssets::get("index.html"));

    match file {
        Some(content) => {
            // rust-embed with mime-guess feature provides mimetype in metadata
            let content_type = content.metadata.mimetype();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap(),
    }
}

/// Add routes for serving static UI files
fn add_ui_routes(app: Router<AppState>, config: &config::GatewayConfig) -> Router<AppState> {
    use config::AssetSource;

    let ui_path = config.ui.path.trim_end_matches('/');

    match &config.ui.assets.source {
        AssetSource::Filesystem { path } => {
            let assets_path = std::path::Path::new(path);
            let index_file = assets_path.join("index.html");

            if !assets_path.exists() {
                tracing::warn!(path = %path, "UI assets directory does not exist");
                return app;
            }

            tracing::info!(path = %path, ui_path = %ui_path, "Serving UI from filesystem");

            // ServeDir with fallback to index.html for SPA routing
            let serve_dir = ServeDir::new(path).fallback(ServeFile::new(&index_file));

            // Add cache-control header for assets
            let cache_control = config.ui.assets.cache_control.clone();
            let serve_dir_with_headers = tower::ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_str(&cache_control).unwrap_or_else(|_| {
                        header::HeaderValue::from_static("public, max-age=3600")
                    }),
                ))
                .service(serve_dir);

            if ui_path.is_empty() || ui_path == "/" {
                // Serve at root - use fallback_service so other routes take precedence
                app.fallback_service(serve_dir_with_headers)
            } else {
                // Serve at a specific path
                app.nest_service(ui_path, serve_dir_with_headers)
            }
        }
        #[cfg(feature = "embed-ui")]
        AssetSource::Embedded => {
            tracing::info!(ui_path = %ui_path, "Serving UI from embedded assets");

            // Create routes for embedded assets (stateless, so use Router<()>)
            let embedded_routes: Router<()> = Router::new()
                .route("/", get(serve_embedded_index))
                .route("/{*path}", get(serve_embedded_asset));

            if ui_path.is_empty() || ui_path == "/" {
                // Serve at root - use fallback so other routes take precedence
                app.fallback_service(embedded_routes)
            } else {
                // Serve at a specific path - convert to service for nesting
                app.nest_service(ui_path, embedded_routes)
            }
        }
        #[cfg(not(feature = "embed-ui"))]
        AssetSource::Embedded => {
            tracing::warn!(
                "Embedded UI assets requested but 'embed-ui' feature is not enabled, skipping"
            );
            app
        }
        AssetSource::Cdn { base_url } => {
            tracing::info!(base_url = %base_url, "UI assets served from CDN (no server-side serving)");
            app
        }
    }
}

/// Handler for serving embedded docs assets
#[cfg(feature = "embed-docs")]
async fn serve_docs_embedded_asset(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    serve_docs_embedded_file(&path)
}

/// Handler for serving embedded docs index at root
#[cfg(feature = "embed-docs")]
async fn serve_docs_embedded_index() -> Response {
    serve_docs_embedded_file("index.html")
}

/// Serve a file from the embedded docs assets.
/// Unlike the SPA UI, docs use static site routing:
/// - Try exact path first
/// - If path ends with /, try path + index.html
/// - If path doesn't end with /, try path/index.html
/// - Return 404 if not found (no fallback to root index.html)
#[cfg(feature = "embed-docs")]
fn serve_docs_embedded_file(path: &str) -> Response {
    // Try exact path first
    if let Some(content) = DocsAssets::get(path) {
        return build_docs_response(content);
    }

    // For paths ending with /, try index.html
    if path.ends_with('/') {
        let index_path = format!("{}index.html", path);
        if let Some(content) = DocsAssets::get(&index_path) {
            return build_docs_response(content);
        }
    } else {
        // For paths without trailing slash, try path/index.html
        let index_path = format!("{}/index.html", path);
        if let Some(content) = DocsAssets::get(&index_path) {
            return build_docs_response(content);
        }
    }

    // Return 404
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}

#[cfg(feature = "embed-docs")]
fn build_docs_response(content: rust_embed::EmbeddedFile) -> Response {
    let content_type = content.metadata.mimetype();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(Body::from(content.data.into_owned()))
        .unwrap()
}

/// Add routes for serving static documentation files
fn add_docs_routes(app: Router<AppState>, config: &config::GatewayConfig) -> Router<AppState> {
    use config::AssetSource;

    let docs_path = config.docs.path.trim_end_matches('/');

    match &config.docs.assets.source {
        AssetSource::Filesystem { path } => {
            let assets_path = std::path::Path::new(path);

            if !assets_path.exists() {
                tracing::warn!(path = %path, "Documentation assets directory does not exist");
                return app;
            }

            tracing::info!(path = %path, docs_path = %docs_path, "Serving documentation from filesystem");

            // ServeDir without SPA fallback (static site)
            let serve_dir = ServeDir::new(path);

            // Add cache-control header for assets
            let cache_control = config.docs.assets.cache_control.clone();
            let serve_dir_with_headers = tower::ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::if_not_present(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_str(&cache_control).unwrap_or_else(|_| {
                        header::HeaderValue::from_static("public, max-age=3600")
                    }),
                ))
                .service(serve_dir);

            // Docs are always at a specific path (never root)
            app.nest_service(docs_path, serve_dir_with_headers)
        }
        #[cfg(feature = "embed-docs")]
        AssetSource::Embedded => {
            tracing::info!(docs_path = %docs_path, "Serving documentation from embedded assets");

            // Create routes for embedded assets (stateless)
            let embedded_routes: Router<()> = Router::new()
                .route("/", get(serve_docs_embedded_index))
                .route("/{*path}", get(serve_docs_embedded_asset));

            // Docs are always at a specific path (never root)
            app.nest_service(docs_path, embedded_routes)
        }
        #[cfg(not(feature = "embed-docs"))]
        AssetSource::Embedded => {
            tracing::warn!(
                "Embedded docs assets requested but 'embed-docs' feature is not enabled, skipping"
            );
            app
        }
        AssetSource::Cdn { base_url } => {
            tracing::info!(base_url = %base_url, "Documentation assets served from CDN (no server-side serving)");
            app
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub http_client: Client,
    pub config: Arc<config::GatewayConfig>,
    pub db: Option<Arc<db::DbPool>>,
    pub services: Option<services::Services>,
    pub cache: Option<Arc<dyn cache::Cache>>,
    pub secrets: Option<Arc<dyn secrets::SecretManager>>,
    pub dlq: Option<Arc<dyn dlq::DeadLetterQueue>>,
    pub pricing: Arc<pricing::PricingConfig>,
    /// Registry of circuit breakers for providers.
    /// Shared across requests to persist failure tracking.
    pub circuit_breakers: providers::CircuitBreakerRegistry,
    /// Registry of provider health check states.
    /// Updated by background health checker, queried by admin API.
    pub provider_health: jobs::ProviderHealthStateRegistry,
    /// Task tracker for background tasks (usage logging, etc.)
    /// Ensures all spawned tasks complete during graceful shutdown.
    pub task_tracker: TaskTracker,
    /// Shared OIDC authenticator (if global OIDC auth is configured in config file).
    /// This holds the session store which persists across requests.
    #[cfg(feature = "sso")]
    pub oidc_authenticator: Option<Arc<auth::OidcAuthenticator>>,
    /// Registry of per-organization OIDC authenticators.
    /// Loaded from org_sso_configs table at startup for multi-tenant SSO.
    #[cfg(feature = "sso")]
    pub oidc_registry: Option<Arc<auth::OidcAuthenticatorRegistry>>,
    /// Registry of per-organization SAML authenticators.
    /// Loaded from org_sso_configs table at startup for multi-tenant SSO.
    #[cfg(feature = "saml")]
    pub saml_registry: Option<Arc<auth::SamlAuthenticatorRegistry>>,
    /// Registry of per-organization RBAC policies.
    /// Loaded from org_rbac_policies table at startup for per-org authorization.
    pub policy_registry: Option<Arc<authz::PolicyRegistry>>,
    /// Async buffer for usage log entries.
    /// Batches writes to reduce database pressure.
    pub usage_buffer: Option<Arc<usage_buffer::UsageLogBuffer>>,
    /// Response cache for chat completions.
    /// Caches deterministic responses to reduce latency and costs.
    pub response_cache: Option<Arc<cache::ResponseCache>>,
    /// Semantic cache for chat completions.
    /// Uses vector similarity to find cached responses for semantically similar requests.
    pub semantic_cache: Option<Arc<cache::SemanticCache>>,
    /// Input guardrails evaluator for pre-request content filtering.
    /// Evaluates user input against guardrails policies before sending to the LLM.
    pub input_guardrails: Option<Arc<guardrails::InputGuardrails>>,
    /// Output guardrails evaluator for post-response content filtering.
    /// Evaluates LLM output against guardrails policies before returning to the user.
    pub output_guardrails: Option<Arc<guardrails::OutputGuardrails>>,
    /// Event bus for broadcasting server events to WebSocket subscribers.
    /// Used for real-time monitoring dashboards and push notifications.
    pub event_bus: Arc<events::EventBus>,
    /// File search service for RAG (Retrieval Augmented Generation).
    /// Used by the file_search tool in the Responses API to search vector stores.
    pub file_search_service: Option<Arc<services::FileSearchService>>,
    /// Document processor for chunking and embedding files added to vector stores.
    /// Used by the Vector Store Files API to process uploaded files.
    #[cfg(any(
        feature = "document-extraction-basic",
        feature = "document-extraction-full"
    ))]
    pub document_processor: Option<Arc<services::DocumentProcessor>>,
    /// Default user ID for when auth is disabled.
    /// Created on startup to allow anonymous users to create conversations.
    pub default_user_id: Option<uuid::Uuid>,
    /// Default organization ID for when auth is disabled.
    /// Created on startup to allow anonymous users to create projects.
    pub default_org_id: Option<uuid::Uuid>,
    /// Provider metrics service for querying LLM provider statistics.
    /// Uses Prometheus when configured, or local /metrics parsing for single-node.
    pub provider_metrics: Arc<services::ProviderMetricsService>,
    /// Model catalog registry for enriching API responses with model metadata.
    /// Loaded from embedded data at startup and optionally synced at runtime.
    pub model_catalog: catalog::ModelCatalogRegistry,
}

impl AppState {
    pub async fn new(config: config::GatewayConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Build a single shared HTTP client for all outbound provider requests.
        // This is efficient because reqwest maintains per-host connection pools internally,
        // so each provider (OpenAI, Anthropic, etc.) gets its own pool of connections.
        // See HttpClientConfig docs for architecture details and tuning options.
        let http_client = config
            .server
            .http_client
            .build_client()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

        tracing::debug!(
            timeout_secs = config.server.http_client.timeout_secs,
            connect_timeout_secs = config.server.http_client.connect_timeout_secs,
            pool_max_idle_per_host = config.server.http_client.pool_max_idle_per_host,
            http2_prior_knowledge = config.server.http_client.http2_prior_knowledge,
            "HTTP client configured"
        );

        // Initialize event bus early so it can be passed to services
        // Use channel capacity from WebSocket config
        let event_bus = Arc::new(events::EventBus::with_capacity(
            config.features.websocket.channel_capacity,
        ));

        // Initialize database and services if configured
        #[allow(unreachable_patterns)]
        let (db, services) = match &config.database {
            config::DatabaseConfig::None => (None, None),
            _ => {
                let pool = db::DbPool::from_config(&config.database).await?;
                // Run migrations on startup
                pool.run_migrations().await?;
                let db = Arc::new(pool);

                // Create file storage backend from config
                let file_storage = services::create_file_storage(&config.storage.files, db.clone())
                    .await
                    .map_err(|e| format!("Failed to initialize file storage: {}", e))?;

                tracing::info!(
                    backend = %file_storage.backend_name(),
                    "File storage backend initialized"
                );

                let max_expr_len = config.auth.rbac.max_expression_length;
                let services = services::Services::with_event_bus(
                    db.clone(),
                    file_storage,
                    event_bus.clone(),
                    max_expr_len,
                );
                (Some(db), Some(services))
            }
        };

        // Initialize cache if configured
        let cache: Option<Arc<dyn cache::Cache>> = match &config.cache {
            config::CacheConfig::None => None,
            config::CacheConfig::Memory(cfg) => Some(Arc::new(cache::MemoryCache::new(cfg))),
            config::CacheConfig::Redis(cfg) => {
                #[cfg(feature = "redis")]
                {
                    Some(Arc::new(cache::RedisCache::from_config(cfg).await?))
                }
                #[cfg(not(feature = "redis"))]
                {
                    let _ = cfg;
                    return Err("Redis cache configured but 'redis' feature not enabled. \
                        Rebuild with: cargo build --features redis"
                        .into());
                }
            }
        };

        // Initialize secrets manager based on configuration
        let secrets: Arc<dyn secrets::SecretManager> = match &config.secrets {
            config::SecretsConfig::None => {
                // Default behavior: use env vars for local mode, memory for db mode
                if db.is_some() {
                    tracing::warn!(
                        "No secrets manager configured. Using in-memory storage which does NOT \
                         persist across restarts. Per-org SSO will fail after restart. \
                         Configure [secrets] in hadrian.toml for production use."
                    );
                    Arc::new(secrets::MemorySecretManager::new())
                } else {
                    Arc::new(secrets::EnvSecretManager)
                }
            }
            config::SecretsConfig::Env => Arc::new(secrets::EnvSecretManager),
            #[cfg(feature = "vault")]
            config::SecretsConfig::Vault(vault_config) => {
                use config::VaultAuth;
                use secrets::SecretManager;

                // Build VaultConfig based on auth method
                let vault_cfg = match &vault_config.auth {
                    VaultAuth::Token { token } => {
                        secrets::VaultConfig::new(&vault_config.address, token)
                    }
                    VaultAuth::AppRole {
                        role_id,
                        secret_id,
                        auth_mount,
                    } => secrets::VaultConfig::with_approle(
                        &vault_config.address,
                        role_id,
                        secret_id,
                    )
                    .with_auth_mount(auth_mount),
                    VaultAuth::Kubernetes {
                        role,
                        token_path,
                        auth_mount,
                    } => {
                        // Read the ServiceAccount token from the file
                        let jwt = std::fs::read_to_string(token_path).map_err(|e| {
                            format!(
                                "Failed to read Kubernetes ServiceAccount token from '{}': {}",
                                token_path, e
                            )
                        })?;
                        secrets::VaultConfig::with_kubernetes(
                            &vault_config.address,
                            role,
                            jwt.trim(),
                        )
                        .with_auth_mount(auth_mount)
                    }
                }
                .with_mount(&vault_config.mount)
                .with_path_prefix(&vault_config.path_prefix);

                let manager = secrets::VaultSecretManager::new(vault_cfg)
                    .await
                    .map_err(|e| format!("Failed to create Vault client: {}", e))?;

                // Verify connectivity on startup
                manager
                    .health_check()
                    .await
                    .map_err(|e| format!("Vault health check failed: {}", e))?;

                let auth_method = match &vault_config.auth {
                    VaultAuth::Token { .. } => "token",
                    VaultAuth::AppRole { .. } => "approle",
                    VaultAuth::Kubernetes { .. } => "kubernetes",
                };

                tracing::info!(
                    address = %vault_config.address,
                    mount = %vault_config.mount,
                    path_prefix = %vault_config.path_prefix,
                    auth_method = %auth_method,
                    "Connected to Vault secrets manager"
                );

                Arc::new(manager)
            }
            #[cfg(feature = "secrets-aws")]
            config::SecretsConfig::Aws(aws_config) => {
                use secrets::SecretManager;

                let mut cfg = match &aws_config.region {
                    Some(region) => secrets::AwsSecretsManagerConfig::new(region),
                    None => secrets::AwsSecretsManagerConfig::from_env(),
                }
                .with_prefix(&aws_config.prefix);

                if let Some(endpoint_url) = &aws_config.endpoint_url {
                    cfg = cfg.with_endpoint_url(endpoint_url);
                }

                let manager = secrets::AwsSecretsManager::new(cfg)
                    .await
                    .map_err(|e| format!("Failed to create AWS Secrets Manager client: {}", e))?;

                // Verify connectivity on startup
                manager
                    .health_check()
                    .await
                    .map_err(|e| format!("AWS Secrets Manager health check failed: {}", e))?;

                tracing::info!(
                    region = ?aws_config.region,
                    prefix = %aws_config.prefix,
                    "Connected to AWS Secrets Manager"
                );

                Arc::new(manager)
            }
            #[cfg(feature = "secrets-azure")]
            config::SecretsConfig::Azure(azure_config) => {
                use secrets::SecretManager;

                let cfg = secrets::AzureKeyVaultConfig::new(&azure_config.vault_url)
                    .with_prefix(&azure_config.prefix);

                let manager = secrets::AzureKeyVaultManager::new(cfg)
                    .await
                    .map_err(|e| format!("Failed to create Azure Key Vault client: {}", e))?;

                // Verify connectivity on startup
                manager
                    .health_check()
                    .await
                    .map_err(|e| format!("Azure Key Vault health check failed: {}", e))?;

                tracing::info!(
                    vault_url = %azure_config.vault_url,
                    prefix = %azure_config.prefix,
                    "Connected to Azure Key Vault"
                );

                Arc::new(manager)
            }
            #[cfg(feature = "secrets-gcp")]
            config::SecretsConfig::Gcp(gcp_config) => {
                use secrets::SecretManager;

                let cfg = secrets::GcpSecretManagerConfig::new(&gcp_config.project_id)
                    .with_prefix(&gcp_config.prefix);

                let manager = secrets::GcpSecretManager::new(cfg)
                    .await
                    .map_err(|e| format!("Failed to create GCP Secret Manager client: {}", e))?;

                // Verify connectivity on startup
                manager
                    .health_check()
                    .await
                    .map_err(|e| format!("GCP Secret Manager health check failed: {}", e))?;

                tracing::info!(
                    project_id = %gcp_config.project_id,
                    prefix = %gcp_config.prefix,
                    "Connected to GCP Secret Manager"
                );

                Arc::new(manager)
            }
        };

        // Initialize model catalog registry from embedded data (if available)
        let model_catalog = catalog::ModelCatalogRegistry::new();
        match catalog::embedded_catalog() {
            Some(json) => match model_catalog.load_from_json(&json) {
                Ok(()) => {
                    tracing::info!(
                        model_count = model_catalog.model_count(),
                        "Loaded embedded model catalog"
                    );
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to parse embedded model catalog");
                }
            },
            None => {
                tracing::info!(
                    "No embedded model catalog available; \
                     enable the 'embed-catalog' feature or configure runtime sync"
                );
            }
        }

        // Initialize pricing from defaults + config + provider configs + catalog
        let pricing = Arc::new(pricing::PricingConfig::from_config_with_catalog(
            &config.pricing,
            &config.providers,
            Some(&model_catalog),
        ));

        // Initialize dead-letter queue if configured
        let dlq = dlq::create_dlq(&config.observability.dead_letter_queue, db.as_ref())
            .await
            .map_err(|e| format!("Failed to initialize DLQ: {}", e))?;

        if dlq.is_some() {
            tracing::info!("Dead-letter queue initialized");
        }

        // Initialize circuit breaker registry from provider config
        let circuit_breakers = providers::CircuitBreakerRegistry::from_config_with_event_bus(
            &config.providers,
            event_bus.clone(),
        );

        // Get session config from UI auth config
        // Note: Global OIDC config has been removed. Session config is used for per-org SSO.
        #[cfg(feature = "sso")]
        let session_config = match &config.auth.admin {
            Some(config::AdminAuthConfig::Session(config)) => config.clone(),
            _ => config::SessionConfig::default(),
        };

        // Initialize per-org OIDC authenticator registry from database
        // This replaces the global OIDC authenticator
        #[cfg(feature = "sso")]
        let (oidc_authenticator, oidc_registry) = if let Some(ref svc) = services {
            // Create session store for org authenticators (shared across all orgs)
            let enhanced = session_config.enhanced.enabled;
            let session_store = auth::create_session_store_with_enhanced(cache.clone(), enhanced);

            // Get default session config
            let default_session_config = session_config.clone();

            // No default redirect URI - per-org SSO configs must specify their own
            let default_redirect_uri: Option<String> = None;

            match auth::OidcAuthenticatorRegistry::initialize_from_db(
                &svc.org_sso_configs,
                secrets.as_ref(),
                session_store.clone(),
                default_session_config.clone(),
                default_redirect_uri.clone(),
            )
            .await
            {
                Ok(registry) => {
                    let count = registry.len().await;
                    if count > 0 {
                        tracing::info!(count, "Per-org SSO authenticator registry initialized");
                    } else {
                        tracing::debug!("Per-org SSO registry initialized (empty, will lazy load)");
                    }
                    // Always create the registry to support lazy loading from database
                    (None, Some(Arc::new(registry)))
                }
                Err(e) => {
                    // Create an empty registry instead of None - this allows lazy loading
                    // to work when requests come in, even if startup initialization failed
                    tracing::warn!(
                        error = %e,
                        "Failed to initialize org SSO registry from database, \
                         creating empty registry for lazy loading"
                    );
                    let empty_registry = auth::OidcAuthenticatorRegistry::new(
                        session_store,
                        default_session_config,
                        default_redirect_uri,
                    );
                    (None, Some(Arc::new(empty_registry)))
                }
            }
        } else {
            (None, None)
        };

        // Initialize per-org SAML authenticator registry from database
        #[cfg(feature = "saml")]
        let saml_registry = if let Some(ref svc) = services {
            // Create session store for org authenticators (shared across all orgs)
            let enhanced = session_config.enhanced.enabled;
            let session_store = auth::create_session_store_with_enhanced(cache.clone(), enhanced);

            // Get default session config
            let default_session_config = session_config.clone();

            // Build default ACS URL from server config
            let default_acs_url = format!(
                "{}://{}:{}/auth/saml/acs",
                if config.server.tls.is_some() {
                    "https"
                } else {
                    "http"
                },
                config.server.host,
                config.server.port
            );

            match auth::SamlAuthenticatorRegistry::initialize_from_db(
                &svc.org_sso_configs,
                secrets.as_ref(),
                session_store,
                default_session_config,
                default_acs_url,
            )
            .await
            {
                Ok(registry) if !registry.is_empty().await => {
                    tracing::info!(
                        count = registry.len().await,
                        "Per-org SAML authenticator registry initialized"
                    );
                    Some(Arc::new(registry))
                }
                Ok(_) => {
                    tracing::debug!("No SAML org SSO configs found, registry empty");
                    None
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to initialize SAML org SSO registry");
                    None
                }
            }
        } else {
            None
        };

        // Initialize per-org RBAC policy registry from database
        let policy_registry = if let (Some(svc), Some(db_pool)) = (&services, &db)
            && config.auth.rbac.enabled
        {
            let engine = Arc::new(
                authz::AuthzEngine::new(config.auth.rbac.clone())
                    .expect("Failed to create AuthzEngine for policy registry"),
            );

            // Get config values for the registry
            let version_check_ttl =
                std::time::Duration::from_millis(config.auth.rbac.policy_cache_ttl_ms);
            let max_cached_orgs = config.auth.rbac.max_cached_orgs;
            let eviction_batch_size = config.auth.rbac.policy_eviction_batch_size;

            if config.auth.rbac.lazy_load_policies {
                // Lazy loading: policies loaded on-demand when org is first accessed
                let registry = authz::PolicyRegistry::new_lazy(
                    engine,
                    config.auth.rbac.default_effect,
                    cache.clone(),
                    db_pool.org_rbac_policies(),
                    version_check_ttl,
                    max_cached_orgs,
                    eviction_batch_size,
                );
                tracing::info!(
                    max_cached_orgs,
                    eviction_batch_size,
                    "RBAC policy registry initialized (lazy loading)"
                );
                Some(Arc::new(registry))
            } else {
                // Eager loading: load all policies at startup
                match authz::PolicyRegistry::initialize_from_db(
                    &svc.org_rbac_policies,
                    engine,
                    config.auth.rbac.default_effect,
                    cache.clone(),
                    db_pool.org_rbac_policies(),
                    version_check_ttl,
                    max_cached_orgs,
                    eviction_batch_size,
                )
                .await
                {
                    Ok(registry) => {
                        let org_count = registry.org_count().await;
                        let policy_count = registry.policy_count().await;
                        if org_count > 0 {
                            tracing::info!(
                                org_count,
                                policy_count,
                                max_cached_orgs,
                                "RBAC policy registry initialized (eager loading)"
                            );
                        } else {
                            tracing::debug!("RBAC policy registry initialized (no org policies)");
                        }
                        Some(Arc::new(registry))
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to initialize RBAC policy registry");
                        None
                    }
                }
            }
        } else {
            None
        };

        // Initialize usage log buffer with configured buffer settings and EventBus
        let usage_buffer = {
            let buffer_config =
                usage_buffer::UsageBufferConfig::from(&config.observability.usage.buffer);
            let buffer = Arc::new(usage_buffer::UsageLogBuffer::with_event_bus(
                buffer_config,
                event_bus.clone(),
            ));
            Some(buffer)
        };

        // Initialize response cache if configured and cache is available
        let response_cache = match (&config.features.response_caching, &cache) {
            (Some(caching_config), Some(cache_instance)) if caching_config.enabled => {
                tracing::info!(
                    ttl_secs = caching_config.ttl_secs,
                    only_deterministic = caching_config.only_deterministic,
                    max_size_bytes = caching_config.max_size_bytes,
                    "Response caching enabled"
                );
                Some(Arc::new(cache::ResponseCache::new(
                    cache_instance.clone(),
                    caching_config.clone(),
                )))
            }
            (Some(caching_config), None) if caching_config.enabled => {
                tracing::warn!(
                    "Response caching is enabled but no cache backend is configured. \
                     Add [cache] configuration to enable response caching."
                );
                None
            }
            _ => None,
        };

        // Create the task tracker for background tasks
        let task_tracker = TaskTracker::new();

        // Initialize semantic cache if configured
        let semantic_cache = Self::init_semantic_cache(
            &config,
            cache.as_ref(),
            db.as_ref(),
            &circuit_breakers,
            http_client.clone(),
            &task_tracker,
        )
        .await;

        // Initialize input guardrails if configured
        let input_guardrails = match &config.features.guardrails {
            Some(guardrails_config) => {
                match guardrails::InputGuardrails::from_config(guardrails_config, &http_client) {
                    Ok(Some(evaluator)) => {
                        tracing::info!(
                            provider = %evaluator.provider_name(),
                            "Input guardrails enabled"
                        );
                        Some(Arc::new(evaluator))
                    }
                    Ok(None) => {
                        tracing::debug!("Input guardrails disabled or not configured");
                        None
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to initialize input guardrails");
                        None
                    }
                }
            }
            None => None,
        };

        // Initialize output guardrails if configured
        let output_guardrails = match &config.features.guardrails {
            Some(guardrails_config) => {
                match guardrails::OutputGuardrails::from_config(guardrails_config, &http_client) {
                    Ok(Some(evaluator)) => {
                        tracing::info!(
                            provider = %evaluator.provider_name(),
                            "Output guardrails enabled"
                        );
                        Some(Arc::new(evaluator))
                    }
                    Ok(None) => {
                        tracing::debug!("Output guardrails disabled or not configured");
                        None
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to initialize output guardrails");
                        None
                    }
                }
            }
            None => None,
        };

        // Initialize file search service if configured
        // This requires both semantic cache components (embedding service + vector store)
        // and file_search configuration
        let file_search_service = Self::init_file_search_service(
            &config,
            db.as_ref(),
            &circuit_breakers,
            http_client.clone(),
        )
        .await;

        // Initialize document processor for RAG file processing
        // This reuses the embedding service and vector store from file_search_service
        #[cfg(any(
            feature = "document-extraction-basic",
            feature = "document-extraction-full"
        ))]
        let document_processor = Self::init_document_processor(
            &config,
            db.as_ref(),
            services.as_ref(),
            file_search_service.as_ref(),
        );

        // Create default user and organization when auth is disabled (for anonymous access)
        let (default_user_id, default_org_id) = if config.auth.admin.is_none() {
            if let Some(ref svc) = services {
                let user_id = match Self::ensure_default_user(svc).await {
                    Ok(id) => {
                        tracing::info!(user_id = %id, "Default anonymous user available");
                        Some(id)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to create default user");
                        None
                    }
                };

                let org_id = match Self::ensure_default_org(svc).await {
                    Ok(id) => {
                        tracing::info!(org_id = %id, "Default local organization available");
                        Some(id)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to create default organization");
                        None
                    }
                };

                // Add user to org if both exist
                if let (Some(uid), Some(oid)) = (user_id, org_id)
                    && let Err(e) = Self::ensure_default_org_membership(svc, uid, oid).await
                {
                    tracing::warn!(error = %e, "Failed to add user to default organization");
                }

                (user_id, org_id)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Initialize provider metrics service
        // Uses Prometheus API when prometheus_query_url is configured, otherwise local /metrics
        let provider_metrics = {
            #[cfg(feature = "prometheus")]
            {
                if let Some(ref prometheus_url) = config.observability.metrics.prometheus_query_url
                {
                    match services::ProviderMetricsService::with_prometheus(prometheus_url) {
                        Ok(svc) => {
                            tracing::info!(
                                prometheus_url = %prometheus_url,
                                "Provider metrics using Prometheus backend"
                            );
                            Arc::new(svc)
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "Failed to create Prometheus client, falling back to local metrics"
                            );
                            Arc::new(services::ProviderMetricsService::from_prometheus_handle(
                                observability::metrics::get_prometheus_handle(),
                            ))
                        }
                    }
                } else {
                    tracing::info!("Provider metrics using local /metrics endpoint");
                    Arc::new(services::ProviderMetricsService::from_prometheus_handle(
                        observability::metrics::get_prometheus_handle(),
                    ))
                }
            }
            #[cfg(not(feature = "prometheus"))]
            Arc::new(services::ProviderMetricsService::new())
        };

        Ok(Self {
            http_client,
            config: Arc::new(config),
            db,
            services,
            cache,
            secrets: Some(secrets),
            dlq,
            pricing,
            circuit_breakers,
            provider_health: jobs::ProviderHealthStateRegistry::new(),
            task_tracker,
            #[cfg(feature = "sso")]
            oidc_authenticator,
            #[cfg(feature = "sso")]
            oidc_registry,
            #[cfg(feature = "saml")]
            saml_registry,
            policy_registry,
            usage_buffer,
            response_cache,
            semantic_cache,
            input_guardrails,
            output_guardrails,
            event_bus,
            file_search_service,
            #[cfg(any(
                feature = "document-extraction-basic",
                feature = "document-extraction-full"
            ))]
            document_processor,
            default_user_id,
            default_org_id,
            provider_metrics,
            model_catalog,
        })
    }

    /// Ensure a default user exists for anonymous access when auth is disabled.
    /// Uses a well-known external_id so the same user is used across restarts.
    /// Race-safe: tries to create first, falls back to lookup on conflict.
    async fn ensure_default_user(
        services: &services::Services,
    ) -> Result<uuid::Uuid, Box<dyn std::error::Error + Send + Sync>> {
        use crate::db::DbError;

        const ANONYMOUS_EXTERNAL_ID: &str = "anonymous";

        // Try to create first to avoid TOCTOU race between multiple instances
        let user = models::CreateUser {
            external_id: ANONYMOUS_EXTERNAL_ID.to_string(),
            email: Some("anonymous@localhost".to_string()),
            name: Some("Anonymous User".to_string()),
        };

        match services.users.create(user).await {
            Ok(created) => Ok(created.id),
            Err(DbError::Conflict(_)) => {
                // Already exists (created by another instance) — look it up
                let existing = services
                    .users
                    .get_by_external_id(ANONYMOUS_EXTERNAL_ID)
                    .await?
                    .ok_or("Default user conflict but not found")?;
                Ok(existing.id)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Ensure a default organization exists for anonymous access when auth is disabled.
    /// Uses a well-known slug so the same organization is used across restarts.
    /// Race-safe: tries to create first, falls back to lookup on conflict.
    async fn ensure_default_org(
        services: &services::Services,
    ) -> Result<uuid::Uuid, Box<dyn std::error::Error + Send + Sync>> {
        use crate::db::DbError;

        const LOCAL_ORG_SLUG: &str = "local";

        // Try to create first to avoid TOCTOU race between multiple instances
        let org = models::CreateOrganization {
            slug: LOCAL_ORG_SLUG.to_string(),
            name: "Local".to_string(),
        };

        match services.organizations.create(org).await {
            Ok(created) => Ok(created.id),
            Err(DbError::Conflict(_)) => {
                // Already exists (created by another instance) — look it up
                let existing = services
                    .organizations
                    .get_by_slug(LOCAL_ORG_SLUG)
                    .await?
                    .ok_or("Default org conflict but not found")?;
                Ok(existing.id)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Ensure the default user is a member of the default organization.
    async fn ensure_default_org_membership(
        services: &services::Services,
        user_id: uuid::Uuid,
        org_id: uuid::Uuid,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::{db::DbError, models::MembershipSource};
        // Try to add the user to the org - if they're already a member, this will fail
        // with a unique constraint violation which we can ignore
        match services
            .users
            .add_to_org(user_id, org_id, "member", MembershipSource::Manual)
            .await
        {
            Ok(()) => {
                tracing::debug!(user_id = %user_id, org_id = %org_id, "Added user to organization");
                Ok(())
            }
            Err(DbError::Conflict(_)) => {
                tracing::debug!(user_id = %user_id, org_id = %org_id, "User already member of organization");
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Check if the gateway is in "full" mode (has database and cache)
    pub fn is_full_mode(&self) -> bool {
        self.db.is_some() && self.cache.is_some()
    }

    /// Initialize semantic cache if configured.
    ///
    /// Spawns the background embedding worker on the provided task tracker.
    async fn init_semantic_cache(
        config: &config::GatewayConfig,
        cache: Option<&Arc<dyn cache::Cache>>,
        db: Option<&Arc<db::DbPool>>,
        circuit_breakers: &providers::CircuitBreakerRegistry,
        http_client: Client,
        task_tracker: &TaskTracker,
    ) -> Option<Arc<cache::SemanticCache>> {
        #[cfg(not(feature = "database-postgres"))]
        let _ = &db;
        // Check if semantic caching is configured
        let semantic_config = match &config.features.response_caching {
            Some(caching_config) if caching_config.enabled => match &caching_config.semantic {
                Some(semantic) if semantic.enabled => semantic,
                _ => return None,
            },
            _ => return None,
        };

        // Ensure we have a cache backend
        let cache_instance = match cache {
            Some(c) => c.clone(),
            None => {
                tracing::warn!(
                    "Semantic caching is enabled but no cache backend is configured. \
                     Add [cache] configuration to enable semantic caching."
                );
                return None;
            }
        };

        // Get the embedding provider configuration
        let provider_config = match config.providers.get(&semantic_config.embedding.provider) {
            Some(cfg) => cfg,
            None => {
                tracing::warn!(
                    provider = %semantic_config.embedding.provider,
                    "Semantic caching is enabled but embedding provider '{}' is not configured. \
                     Add the provider to [providers] configuration.",
                    semantic_config.embedding.provider
                );
                return None;
            }
        };

        // Create embedding service
        let embedding_service = match cache::EmbeddingService::new(
            &semantic_config.embedding,
            provider_config,
            circuit_breakers,
            http_client,
        ) {
            Ok(service) => Arc::new(service),
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "Failed to create embedding service for semantic caching"
                );
                return None;
            }
        };

        // Create vector store based on configuration
        let vector_store: Arc<dyn cache::vector_store::VectorBackend> = match &semantic_config
            .vector_backend
        {
            #[cfg(feature = "database-postgres")]
            config::SemanticVectorBackend::Pgvector {
                table_name,
                index_type,
                distance_metric,
            } => {
                // Ensure we have a PostgreSQL database
                let pg_pool = match db.and_then(|d| d.pg_write_pool()) {
                    Some(pool) => pool.clone(),
                    None => {
                        tracing::warn!(
                            "Semantic caching with pgvector requires PostgreSQL database. \
                                 Configure [database] with type = \"postgres\"."
                        );
                        return None;
                    }
                };

                let store = cache::vector_store::PgvectorStore::new(
                    pg_pool,
                    table_name.clone(),
                    semantic_config.embedding.dimensions,
                    index_type.clone(),
                    *distance_metric,
                );

                // Initialize the pgvector table
                if let Err(e) = store.initialize().await {
                    tracing::error!(
                        error = %e,
                        "Failed to initialize pgvector store for semantic caching"
                    );
                    return None;
                }

                Arc::new(store)
            }
            #[cfg(not(feature = "database-postgres"))]
            config::SemanticVectorBackend::Pgvector { .. } => {
                tracing::warn!(
                    "Semantic caching with pgvector requires the 'database-postgres' feature. \
                         Rebuild with --features database-postgres or use a different vector backend."
                );
                return None;
            }
            config::SemanticVectorBackend::Qdrant {
                url,
                api_key,
                qdrant_collection_name,
                distance_metric,
            } => {
                let store = cache::vector_store::QdrantStore::new(
                    url.clone(),
                    api_key.clone(),
                    qdrant_collection_name.clone(),
                    semantic_config.embedding.dimensions,
                    *distance_metric,
                );

                // Initialize the Qdrant index
                if let Err(e) = store.initialize().await {
                    tracing::error!(
                        error = %e,
                        "Failed to initialize Qdrant store for semantic caching"
                    );
                    return None;
                }

                Arc::new(store)
            }
        };

        // Create the semantic cache with background worker
        let (semantic_cache, worker) = cache::SemanticCache::new(
            cache_instance,
            vector_store,
            embedding_service,
            semantic_config.clone(),
        );

        // Spawn the background embedding worker
        task_tracker.spawn(worker);

        tracing::info!(
            similarity_threshold = semantic_config.similarity_threshold,
            top_k = semantic_config.top_k,
            embedding_provider = %semantic_config.embedding.provider,
            embedding_model = %semantic_config.embedding.model,
            "Semantic caching enabled"
        );

        Some(Arc::new(semantic_cache))
    }

    /// Initialize file search service if configured.
    ///
    /// The file search service requires:
    /// - A database for vector store metadata
    /// - An embedding service for generating query embeddings
    /// - A vector store for similarity search
    ///
    /// The embedding configuration is taken from the semantic caching config if available,
    /// since file search typically uses the same embedding model.
    async fn init_file_search_service(
        config: &config::GatewayConfig,
        db: Option<&Arc<db::DbPool>>,
        circuit_breakers: &providers::CircuitBreakerRegistry,
        http_client: Client,
    ) -> Option<Arc<services::FileSearchService>> {
        // Check if file_search is enabled
        let file_search_config = match &config.features.file_search {
            Some(cfg) if cfg.enabled => cfg,
            _ => return None,
        };

        // File search requires a database
        let db = match db {
            Some(d) => d.clone(),
            None => {
                tracing::warn!(
                    "File search is enabled but no database is configured. \
                     Add [database] configuration to enable file search."
                );
                return None;
            }
        };

        // Get embedding configuration with priority:
        // 1. file_search.embedding (explicit RAG config)
        // 2. response_caching.semantic.embedding (semantic cache config)
        // 3. vector_search.embedding (legacy vector search config)
        let embedding_config = file_search_config
            .embedding
            .as_ref()
            .or_else(|| {
                config
                    .features
                    .response_caching
                    .as_ref()
                    .and_then(|rc| rc.semantic.as_ref())
                    .map(|sc| &sc.embedding)
            })
            .or_else(|| {
                config
                    .features
                    .vector_search
                    .as_ref()
                    .map(|vs| &vs.embedding)
            });

        let embedding_config = match embedding_config {
            Some(cfg) => cfg,
            None => {
                tracing::warn!(
                    "File search is enabled but no embedding configuration found. \
                     Configure [features.file_search.embedding], \
                     [features.response_caching.semantic.embedding], or \
                     [features.vector_search.embedding] to enable file search."
                );
                return None;
            }
        };

        // Get the embedding provider configuration
        let provider_config = match config.providers.get(&embedding_config.provider) {
            Some(cfg) => cfg,
            None => {
                tracing::warn!(
                    provider = %embedding_config.provider,
                    "File search is enabled but embedding provider '{}' is not configured. \
                     Add the provider to [providers] configuration.",
                    embedding_config.provider
                );
                return None;
            }
        };

        // Create embedding service
        let embedding_service = match cache::EmbeddingService::new(
            embedding_config,
            provider_config,
            circuit_breakers,
            http_client.clone(),
        ) {
            Ok(service) => Arc::new(service),
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "Failed to create embedding service for file search"
                );
                return None;
            }
        };

        // Get vector backend configuration with priority:
        // 1. file_search.vector_backend (explicit RAG config - RECOMMENDED)
        // 2. response_caching.semantic.vector_backend (semantic cache config - for backward compat)
        // 3. Default pgvector with "rag_chunks" table
        //
        // Using separate vector storage for RAG ensures:
        // - RAG chunks are stored in clearly named tables (rag_chunks vs semantic_cache_embeddings)
        // - Independent configuration for RAG vs semantic caching
        // - No confusion about what data is in which table
        let vector_store: Arc<dyn cache::vector_store::VectorBackend> = if let Some(rag_backend) =
            &file_search_config.vector_backend
        {
            // Priority 1: Explicit RAG vector backend configuration
            match rag_backend {
                #[cfg(feature = "database-postgres")]
                config::RagVectorBackend::Pgvector {
                    table_name,
                    index_type,
                    distance_metric,
                } => {
                    let pg_pool = match db.pg_write_pool() {
                        Some(pool) => pool.clone(),
                        None => {
                            tracing::warn!(
                                "File search with pgvector requires PostgreSQL database. \
                                     Configure [database] with type = \"postgres\"."
                            );
                            return None;
                        }
                    };

                    // For RAG, the table_name IS the chunks table (not a prefix)
                    // We create a PgvectorStore but only use the chunks operations
                    let store = cache::vector_store::PgvectorStore::new(
                        pg_pool,
                        // Use a dummy name for semantic cache table since we won't use it
                        // The chunks table will be "{table_name}_chunks" but we want
                        // the table_name to BE the chunks table, so we strip "_chunks"
                        // if it's there, otherwise prepend with a prefix
                        format!("{}_semantic", table_name.trim_end_matches("_chunks")),
                        embedding_config.dimensions,
                        index_type.clone(),
                        *distance_metric,
                    );

                    if let Err(e) = store.initialize().await {
                        tracing::error!(
                            error = %e,
                            "Failed to initialize pgvector store for file search"
                        );
                        return None;
                    }

                    tracing::info!(
                        table_name = %table_name,
                        "RAG using dedicated pgvector table"
                    );

                    Arc::new(store)
                }
                #[cfg(not(feature = "database-postgres"))]
                config::RagVectorBackend::Pgvector { .. } => {
                    tracing::warn!(
                        "File search with pgvector requires the 'database-postgres' feature. \
                             Rebuild with --features database-postgres or use a different vector backend."
                    );
                    return None;
                }
                config::RagVectorBackend::Qdrant {
                    url,
                    api_key,
                    qdrant_collection_name,
                    distance_metric,
                } => {
                    let store = cache::vector_store::QdrantStore::new(
                        url.clone(),
                        api_key.clone(),
                        qdrant_collection_name.clone(),
                        embedding_config.dimensions,
                        *distance_metric,
                    );

                    if let Err(e) = store.initialize().await {
                        tracing::error!(
                            error = %e,
                            "Failed to initialize Qdrant store for file search"
                        );
                        return None;
                    }

                    tracing::info!(
                        collection_name = %qdrant_collection_name,
                        "RAG using dedicated Qdrant index"
                    );

                    Arc::new(store)
                }
            }
        } else if let Some(semantic_config) = config
            .features
            .response_caching
            .as_ref()
            .and_then(|rc| rc.semantic.as_ref())
        {
            // Priority 2: Fall back to semantic cache vector backend (backward compatibility)
            // Note: This shares tables with semantic cache, which may cause confusion
            tracing::info!(
                "RAG using semantic cache vector backend. Consider configuring \
                     [features.file_search.vector_backend] for dedicated RAG storage."
            );

            match &semantic_config.vector_backend {
                #[cfg(feature = "database-postgres")]
                config::SemanticVectorBackend::Pgvector {
                    table_name,
                    index_type,
                    distance_metric,
                } => {
                    let pg_pool = match db.pg_write_pool() {
                        Some(pool) => pool.clone(),
                        None => {
                            tracing::warn!(
                                "File search with pgvector requires PostgreSQL database. \
                                     Configure [database] with type = \"postgres\"."
                            );
                            return None;
                        }
                    };

                    let store = cache::vector_store::PgvectorStore::new(
                        pg_pool,
                        table_name.clone(),
                        embedding_config.dimensions,
                        index_type.clone(),
                        *distance_metric,
                    );

                    if let Err(e) = store.initialize().await {
                        tracing::error!(
                            error = %e,
                            "Failed to initialize pgvector store for file search"
                        );
                        return None;
                    }

                    Arc::new(store)
                }
                #[cfg(not(feature = "database-postgres"))]
                config::SemanticVectorBackend::Pgvector { .. } => {
                    tracing::warn!(
                        "File search with pgvector requires the 'database-postgres' feature. \
                             Rebuild with --features database-postgres or use a different vector backend."
                    );
                    return None;
                }
                config::SemanticVectorBackend::Qdrant {
                    url,
                    api_key,
                    qdrant_collection_name,
                    distance_metric,
                } => {
                    let store = cache::vector_store::QdrantStore::new(
                        url.clone(),
                        api_key.clone(),
                        qdrant_collection_name.clone(),
                        embedding_config.dimensions,
                        *distance_metric,
                    );

                    if let Err(e) = store.initialize().await {
                        tracing::error!(
                            error = %e,
                            "Failed to initialize Qdrant store for file search"
                        );
                        return None;
                    }

                    Arc::new(store)
                }
            }
        } else {
            // Priority 3: Default pgvector with "rag_chunks" table
            #[cfg(not(feature = "database-postgres"))]
            {
                tracing::warn!(
                    "File search requires a vector store backend. Configure \
                         [features.file_search.vector_backend] or rebuild with --features database-postgres."
                );
                return None;
            }

            #[cfg(feature = "database-postgres")]
            {
                let pg_pool = match db.pg_write_pool() {
                    Some(pool) => pool.clone(),
                    None => {
                        tracing::warn!(
                            "File search requires a vector store backend. Configure \
                                 [features.file_search.vector_backend] or use PostgreSQL."
                        );
                        return None;
                    }
                };

                // Use "rag_chunks" as the default table name (clear naming)
                let store = cache::vector_store::PgvectorStore::new(
                    pg_pool,
                    "rag".to_string(), // Creates "rag" for semantic + "rag_chunks" for RAG
                    embedding_config.dimensions,
                    config::PgvectorIndexType::IvfFlat,
                    config::DistanceMetric::default(), // Cosine (default)
                );

                if let Err(e) = store.initialize().await {
                    tracing::error!(
                        error = %e,
                        "Failed to initialize pgvector store for file search"
                    );
                    return None;
                }

                tracing::info!("RAG using default pgvector table 'rag_chunks'");

                Arc::new(store)
            }
        };

        // Create reranker if enabled
        let reranker: Option<Arc<dyn services::Reranker>> = if file_search_config.rerank.enabled {
            // Create a provider for the reranker using the same config as embeddings
            match Self::create_reranker_provider(
                provider_config,
                &embedding_config.provider,
                circuit_breakers,
            ) {
                Ok(provider) => {
                    let reranker = services::LlmReranker::new(
                        provider,
                        http_client.clone(),
                        file_search_config.rerank.clone(),
                        embedding_config.provider.clone(),
                    );
                    tracing::info!(
                        model = ?file_search_config.rerank.model,
                        max_results_to_rerank = file_search_config.rerank.max_results_to_rerank,
                        batch_size = file_search_config.rerank.batch_size,
                        timeout_secs = file_search_config.rerank.timeout_secs,
                        "LLM reranker enabled for file search"
                    );
                    Some(Arc::new(reranker) as Arc<dyn services::Reranker>)
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Failed to create reranker provider, LLM re-ranking will be disabled"
                    );
                    None
                }
            }
        } else {
            None
        };

        let service = services::FileSearchService::new(
            db,
            embedding_service,
            vector_store,
            reranker,
            services::FileSearchServiceConfig {
                default_max_results: file_search_config.max_results_per_search,
                default_threshold: file_search_config.score_threshold,
                retry: file_search_config.retry.clone(),
                circuit_breaker: file_search_config.circuit_breaker.clone(),
                rerank: file_search_config.rerank.clone(),
            },
        );

        tracing::info!(
            max_results = file_search_config.max_results_per_search,
            score_threshold = file_search_config.score_threshold,
            max_iterations = file_search_config.max_iterations,
            rerank_enabled = file_search_config.rerank.enabled,
            "File search service enabled"
        );

        Some(Arc::new(service))
    }

    /// Create a provider instance for the reranker.
    ///
    /// Uses the same provider configuration as the embedding service.
    fn create_reranker_provider(
        provider_config: &config::ProviderConfig,
        provider_name: &str,
        circuit_breakers: &providers::CircuitBreakerRegistry,
    ) -> Result<Arc<dyn providers::Provider>, String> {
        match provider_config {
            config::ProviderConfig::Test(_) => {
                Err("Test provider does not support chat completions for re-ranking".into())
            }
            _ => create_provider_instance(provider_config, provider_name, circuit_breakers),
        }
    }

    /// Initialize the document processor for RAG file processing.
    ///
    /// The document processor is responsible for:
    /// - Chunking uploaded files into semantically meaningful segments
    /// - Generating embeddings for each chunk
    /// - Storing chunks in the vector store
    ///
    /// It reuses the embedding service and vector store from the file search service
    /// to ensure consistency in how documents are processed and searched.
    #[cfg(any(
        feature = "document-extraction-basic",
        feature = "document-extraction-full"
    ))]
    fn init_document_processor(
        config: &config::GatewayConfig,
        db: Option<&Arc<db::DbPool>>,
        services: Option<&services::Services>,
        file_search_service: Option<&Arc<services::FileSearchService>>,
    ) -> Option<Arc<services::DocumentProcessor>> {
        // Document processor requires database and vector stores service
        let db = db?.clone();
        let vector_stores_service = Arc::new(services?.vector_stores.clone());

        // Get embedding service and vector store from file search service (if available)
        let (embedding_service, vector_store) = file_search_service
            .map(|fs| (Some(fs.embedding_service()), Some(fs.vector_store())))
            .unwrap_or((None, None));

        // Build document processor config from file_processing config
        let processor_config: services::DocumentProcessorConfig =
            (&config.features.file_processing).into();

        // Log processing mode
        match processor_config.processing_mode {
            services::document_processor::ProcessingMode::Inline => {
                tracing::info!(
                    max_file_size_mb = processor_config.max_file_size / (1024 * 1024),
                    max_concurrent_tasks = processor_config.max_concurrent_tasks,
                    default_chunk_tokens = processor_config.default_max_chunk_tokens,
                    has_embedding_service = embedding_service.is_some(),
                    has_vector_store = vector_store.is_some(),
                    "Document processor initialized (inline mode)"
                );
            }
            services::document_processor::ProcessingMode::Queue => {
                tracing::info!(
                    max_file_size_mb = processor_config.max_file_size / (1024 * 1024),
                    has_queue_backend = processor_config.queue_backend.is_some(),
                    "Document processor initialized (queue mode)"
                );
            }
        }

        match services::DocumentProcessor::new(
            db,
            vector_stores_service,
            embedding_service,
            vector_store,
            processor_config,
        ) {
            Ok(processor) => Some(Arc::new(processor)),
            Err(e) => {
                tracing::error!(error = %e, "Failed to initialize document processor");
                None
            }
        }
    }
}

/// Create a provider instance from a ProviderConfig.
///
/// This is a general-purpose helper for instantiating providers, used by:
/// - Re-ranker initialization (via `AppState::create_reranker_provider`)
/// - Provider health checker
///
/// Returns an error message if the provider type is not supported.
fn create_provider_instance(
    provider_config: &config::ProviderConfig,
    provider_name: &str,
    circuit_breakers: &providers::CircuitBreakerRegistry,
) -> Result<Arc<dyn providers::Provider>, String> {
    let provider: Arc<dyn providers::Provider> = match provider_config {
        config::ProviderConfig::OpenAi(cfg) => Arc::new(
            providers::open_ai::OpenAICompatibleProvider::from_config_with_registry(
                cfg,
                provider_name,
                circuit_breakers,
            ),
        ),
        config::ProviderConfig::Anthropic(cfg) => Arc::new(
            providers::anthropic::AnthropicProvider::from_config_with_registry(
                cfg,
                provider_name,
                circuit_breakers,
            ),
        ),
        #[cfg(feature = "provider-azure")]
        config::ProviderConfig::AzureOpenAi(cfg) => Arc::new(
            providers::azure_openai::AzureOpenAIProvider::from_config_with_registry(
                cfg,
                provider_name,
                circuit_breakers,
            ),
        ),
        #[cfg(feature = "provider-vertex")]
        config::ProviderConfig::Vertex(cfg) => Arc::new(
            providers::vertex::VertexProvider::from_config_with_registry(
                cfg,
                provider_name,
                circuit_breakers,
            ),
        ),
        #[cfg(feature = "provider-bedrock")]
        config::ProviderConfig::Bedrock(cfg) => Arc::new(
            providers::bedrock::BedrockProvider::from_config_with_registry(
                cfg,
                provider_name,
                circuit_breakers,
            ),
        ),
        config::ProviderConfig::Test(cfg) => {
            Arc::new(providers::test::TestProvider::from_config(cfg))
        }
    };

    Ok(provider)
}

/// CLI arguments for Hadrian Gateway
#[derive(Parser, Debug)]
#[command(version, about = "Hadrian AI Gateway", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Path to config file (defaults to ~/.config/hadrian/hadrian.toml if it exists,
    /// otherwise creates a default config)
    #[arg(short, long, global = true)]
    config: Option<String>,

    /// Disable automatic browser opening on startup
    #[arg(long, global = true)]
    no_browser: bool,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Start the gateway server (default)
    Serve,
    /// Export the OpenAPI specification (JSON format)
    Openapi {
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Export the JSON schema for the configuration file
    Schema {
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Initialize a new configuration file
    Init {
        /// Path to create the config file (defaults to ~/.config/hadrian/hadrian.toml)
        #[arg(short, long)]
        output: Option<String>,
        /// Overwrite existing config file
        #[arg(long)]
        force: bool,
        /// Run interactive configuration wizard
        #[arg(short, long)]
        wizard: bool,
    },
    /// Run the file processing worker (for queue-based file processing)
    #[cfg(any(
        feature = "document-extraction-basic",
        feature = "document-extraction-full"
    ))]
    Worker {
        /// Unique consumer name for this worker instance (defaults to random UUID)
        #[arg(long)]
        consumer_name: Option<String>,
        /// Number of jobs to process per batch (default: 10)
        #[arg(long, default_value = "10")]
        batch_size: usize,
        /// Block timeout in milliseconds when waiting for jobs (default: 5000)
        #[arg(long, default_value = "5000")]
        block_timeout_ms: u64,
        /// Whether to claim pending messages from other consumers (default: true)
        #[arg(long, default_value = "true")]
        claim_pending: bool,
        /// Max idle time in ms before a pending message can be claimed (default: 60000)
        #[arg(long, default_value = "60000")]
        pending_timeout_ms: u64,
    },
    /// Run database migrations and exit
    ///
    /// Useful for Kubernetes init containers or CI/CD pipelines.
    /// Connects to the database, runs any pending migrations, and exits.
    Migrate,
    /// Show enabled compile-time features
    Features,
}

/// Default configuration for zero-config startup.
/// Uses SQLite for storage and in-memory cache for simplicity.
fn default_config_toml() -> &'static str {
    r#"# Hadrian AI Gateway Configuration
# Generated automatically for local development

[server]
host = "127.0.0.1"
port = 8080

# CORS: Allow local development origins
[server.cors]
enabled = true
allowed_origins = ["http://localhost:8080", "http://127.0.0.1:8080"]
allow_credentials = true

# SQLite database for persistent storage
[database]
type = "sqlite"
path = "~/.local/share/hadrian/hadrian.db"

# In-memory cache for rate limiting and sessions
[cache]
type = "memory"

# Web UI enabled and served from embedded assets
[ui]
enabled = true

# Example provider configuration (uncomment and add your API key)
# [providers.openai]
# type = "open_ai"
# api_key = "${OPENAI_API_KEY}"
#
# [providers.anthropic]
# type = "anthropic"
# api_key = "${ANTHROPIC_API_KEY}"
"#
}

/// Get the default config directory path.
#[cfg(feature = "wizard")]
fn default_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("hadrian"))
}

/// Get the default config directory path.
#[cfg(not(feature = "wizard"))]
fn default_config_dir() -> Option<PathBuf> {
    None
}

/// Get the default config file path.
fn default_config_path() -> Option<PathBuf> {
    default_config_dir().map(|p| p.join("hadrian.toml"))
}

/// Get the default data directory path.
#[cfg(feature = "wizard")]
fn default_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|p| p.join("hadrian"))
}

/// Get the default data directory path.
#[cfg(not(feature = "wizard"))]
fn default_data_dir() -> Option<PathBuf> {
    None
}

/// Resolve the config path, creating default config if necessary.
/// Returns the config path and whether it was newly created.
fn resolve_config_path(explicit_path: Option<&str>) -> Result<(PathBuf, bool), String> {
    // If explicit path is provided, use it
    if let Some(path) = explicit_path {
        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(format!("Config file not found: {}", path.display()));
        }
        return Ok((path, false));
    }

    // Check for hadrian.toml in current directory
    let cwd_config = PathBuf::from("hadrian.toml");
    if cwd_config.exists() {
        return Ok((cwd_config, false));
    }

    // Check for config in default location
    if let Some(default_path) = default_config_path()
        && default_path.exists()
    {
        return Ok((default_path, false));
    }

    // No config found - create default config
    create_default_config()
}

/// Create the default configuration file and data directory.
fn create_default_config() -> Result<(PathBuf, bool), String> {
    let config_dir = default_config_dir().ok_or("Could not determine config directory")?;
    let config_path = config_dir.join("hadrian.toml");
    let data_dir = default_data_dir().ok_or("Could not determine data directory")?;

    // Create directories
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create data directory: {}", e))?;

    // Write default config with expanded path
    let config_content = default_config_toml().replace(
        "~/.local/share/hadrian/hadrian.db",
        &data_dir.join("hadrian.db").to_string_lossy(),
    );
    std::fs::write(&config_path, config_content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok((config_path, true))
}

pub fn build_app(config: &config::GatewayConfig, state: AppState) -> Router {
    let mut app = Router::new()
        // Health check endpoint
        .route("/health", get(routes::health::health_check))
        .route("/health/live", get(routes::health::liveness))
        .route("/health/ready", get(routes::health::readiness));

    // OpenAPI spec and Scalar docs UI (optional)
    #[cfg(feature = "utoipa")]
    {
        app = app
            .route("/openapi.json", get(openapi_json))
            .merge(Scalar::with_url("/api/docs", openapi::ApiDoc::build()));
    }

    // Add Prometheus metrics endpoint if enabled
    if config.observability.metrics.enabled {
        let metrics_path = config
            .observability
            .metrics
            .prometheus
            .as_ref()
            .map(|p| p.path.clone())
            .unwrap_or_else(|| "/metrics".to_string());

        app = app.route(&metrics_path, get(routes::health::metrics));
    }

    app = app.nest("/api", routes::get_api_routes(state.clone()));

    // Only mount admin routes if database is configured
    if !config.database.is_none() {
        // Mount public admin routes first (no auth required)
        // These are needed for frontend bootstrap before the user logs in
        let public_admin_routes = routes::admin::get_public_admin_routes().route_layer(
            axum::middleware::from_fn_with_state(state.clone(), middleware::rate_limit_middleware),
        );
        app = app.nest("/admin", public_admin_routes);

        // Use protected routes if UI auth is configured, otherwise unprotected
        // (for development or when using external auth proxy)
        if config.auth.admin.is_some() {
            // Apply middleware in order: admin_auth_middleware runs first,
            // then authz_middleware runs second (layers are applied in reverse order)
            // IP rate limiting runs before auth for defense in depth
            let admin_routes = routes::admin::get_protected_admin_routes()
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::authz_middleware,
                ))
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::admin_auth_middleware,
                ))
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::rate_limit_middleware,
                ));
            app = app.merge(Router::new().nest("/admin", admin_routes));
        } else {
            tracing::warn!(
                "Admin routes are UNPROTECTED - configure auth.admin for Zero Trust or OIDC authentication"
            );
            // Apply permissive authz middleware so handlers can still require AuthzContext
            // (fail-closed pattern) but authorization checks will always pass
            // IP rate limiting still applied for DoS protection
            let admin_routes = routes::admin::get_admin_routes()
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::permissive_authz_middleware,
                ))
                .route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::rate_limit_middleware,
                ));
            app = app.merge(Router::new().nest("/admin", admin_routes));
        }
    }

    // Add auth routes
    // SSO routes are added when Session auth is configured or per-org SSO registries exist
    #[cfg(feature = "sso")]
    {
        let has_session_auth = matches!(
            &config.auth.admin,
            Some(config::AdminAuthConfig::Session(_))
        );
        let has_oidc_registry = state.oidc_registry.is_some();
        #[cfg(feature = "saml")]
        let has_saml = state.saml_registry.is_some();
        #[cfg(not(feature = "saml"))]
        let has_saml = false;

        // When auth is fully disabled (no UI auth, API auth is none), always use permissive
        // middleware for /auth/me. The OIDC registry is always created when a database exists
        // (to support lazy loading), so has_oidc_registry alone doesn't mean SSO is configured.
        let auth_disabled = config.auth.admin.is_none() && !config.auth.gateway.is_enabled();

        if !auth_disabled && (has_session_auth || has_oidc_registry || has_saml) {
            // When SSO is configured, /auth/me uses admin middleware
            let me_route =
                get(routes::auth_routes::me).route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::admin_auth_middleware,
                ));

            if has_session_auth || has_oidc_registry {
                // Build OIDC auth routes with IP rate limiting
                let auth_routes = Router::new()
                    .route("/login", get(routes::auth_routes::login))
                    .route("/callback", get(routes::auth_routes::callback))
                    .route("/logout", post(routes::auth_routes::logout))
                    .route_layer(axum::middleware::from_fn_with_state(
                        state.clone(),
                        middleware::rate_limit_middleware,
                    ));

                app = app.nest("/auth", auth_routes).route("/auth/me", me_route);
            } else {
                // SAML-only: just add /auth/me with admin middleware
                app = app.route("/auth/me", me_route);
            }

            // Add SSO discovery endpoint if database is configured (for per-org SSO)
            // This is needed for both OIDC and SAML per-org configurations
            if !config.database.is_none() {
                let discover_route = get(routes::auth_routes::discover).route_layer(
                    axum::middleware::from_fn_with_state(
                        state.clone(),
                        middleware::rate_limit_middleware,
                    ),
                );
                app = app.route("/auth/discover", discover_route);
            }
        } else if !config.database.is_none() {
            // When SSO feature is enabled but auth is disabled and database is available,
            // add /auth/me with permissive middleware
            let me_route =
                get(routes::auth_routes::me).route_layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::permissive_authz_middleware,
                ));
            app = app.route("/auth/me", me_route);
        }
    }

    // Add SAML routes if database is configured (SAML uses per-org SSO configs from database)
    // These routes are separate from OIDC since they use HTTP-POST binding and different flows
    #[cfg(feature = "saml")]
    if !config.database.is_none() {
        let saml_routes = Router::new()
            .route("/login", get(routes::auth_routes::saml_login))
            .route("/acs", post(routes::auth_routes::saml_acs))
            .route(
                "/slo",
                get(routes::auth_routes::saml_slo).post(routes::auth_routes::saml_slo),
            )
            .route("/metadata", get(routes::auth_routes::saml_metadata))
            .route_layer(axum::middleware::from_fn_with_state(
                state.clone(),
                middleware::rate_limit_middleware,
            ));

        app = app.nest("/auth/saml", saml_routes);
        tracing::debug!("SAML 2.0 authentication routes enabled at /auth/saml/");
    }

    // Add SCIM routes for automated user provisioning from IdPs
    // SCIM requires database to be configured (for token storage and user/group mappings)
    #[cfg(feature = "sso")]
    if !config.database.is_none() {
        app = app.nest("/scim", routes::scim_routes(state.clone()));
        tracing::info!("SCIM 2.0 provisioning endpoints enabled at /scim/v2/");
    }

    // Add WebSocket route for real-time event subscriptions if enabled
    if config.features.websocket.enabled {
        app = app.route("/ws/events", get(routes::ws_handler));
        tracing::info!("WebSocket event subscriptions enabled at /ws/events");
    }

    // Serve documentation site if enabled (must be before UI to avoid fallback conflicts)
    if config.docs.enabled {
        app = add_docs_routes(app, config);
    }

    // Serve static UI files if enabled
    if config.ui.enabled {
        app = add_ui_routes(app, config);
    }

    // Add request ID middleware first, then cookies layer for session management
    // Security headers are added to all responses
    app = app
        .layer(axum::middleware::from_fn(middleware::request_id_middleware))
        .layer(tower_cookies::CookieManagerLayer::new())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::security_headers_middleware,
        ));

    // Apply CORS layer if enabled (layers are applied in reverse order, so this runs first)
    if let Some(cors_layer) = config.server.cors.clone().into_layer() {
        app = app.layer(cors_layer);
    }

    app.layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(config.server.body_limit_bytes))
        .with_state(state)
}

/// Returns the OpenAPI spec as JSON
#[cfg(feature = "utoipa")]
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi::ApiDoc::build())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.command {
        Some(Command::Openapi { output }) => {
            #[cfg(feature = "utoipa")]
            run_openapi_export(output);
            #[cfg(not(feature = "utoipa"))]
            {
                let _ = output;
                eprintln!("Error: OpenAPI export requires the 'utoipa' feature to be enabled");
                std::process::exit(1);
            }
        }
        Some(Command::Schema { output }) => {
            #[cfg(feature = "json-schema")]
            run_schema_export(output);
            #[cfg(not(feature = "json-schema"))]
            {
                let _ = output;
                eprintln!("Error: JSON schema export requires the 'json-schema' feature");
                std::process::exit(1);
            }
        }
        Some(Command::Init {
            output,
            force,
            wizard,
        }) => {
            run_init(output, force, wizard);
        }
        #[cfg(any(
            feature = "document-extraction-basic",
            feature = "document-extraction-full"
        ))]
        Some(Command::Worker {
            consumer_name,
            batch_size,
            block_timeout_ms,
            claim_pending,
            pending_timeout_ms,
        }) => {
            run_worker(
                args.config.as_deref(),
                consumer_name,
                batch_size,
                block_timeout_ms,
                claim_pending,
                pending_timeout_ms,
            )
            .await;
        }
        Some(Command::Migrate) => {
            run_migrate(args.config.as_deref()).await;
        }
        Some(Command::Features) => {
            run_features();
        }
        Some(Command::Serve) | None => {
            run_server(args.config.as_deref(), args.no_browser).await;
        }
    }
}

/// Initialize a new configuration file
fn run_init(output: Option<String>, force: bool, use_wizard: bool) {
    if use_wizard {
        #[cfg(feature = "wizard")]
        run_init_wizard(output, force);
        #[cfg(not(feature = "wizard"))]
        {
            let _ = (output, force);
            eprintln!("Error: The interactive wizard requires the 'wizard' feature to be enabled.");
            eprintln!("Rebuild with: cargo build --features wizard");
            eprintln!("Or use 'gateway init' without --wizard for a default config.");
            std::process::exit(1);
        }
    } else {
        run_init_default(output, force);
    }
}

/// Run the interactive configuration wizard.
#[cfg(feature = "wizard")]
fn run_init_wizard(output: Option<String>, force: bool) {
    match wizard::run() {
        Ok(result) => {
            // Use the wizard's suggested path or override with --output
            let output_path = output.map(PathBuf::from).unwrap_or(result.path);

            if output_path.exists() && !force {
                eprintln!(
                    "Config file already exists: {}\nUse --force to overwrite.",
                    output_path.display()
                );
                std::process::exit(1);
            }

            // Create parent directories if needed
            if let Some(parent) = output_path.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                eprintln!("Failed to create directory {}: {}", parent.display(), e);
                std::process::exit(1);
            }

            // Create data directory if needed
            if let Some(data_dir) = default_data_dir()
                && let Err(e) = std::fs::create_dir_all(&data_dir)
            {
                eprintln!(
                    "Warning: Failed to create data directory {}: {}",
                    data_dir.display(),
                    e
                );
            }

            if let Err(e) = std::fs::write(&output_path, &result.config) {
                eprintln!("Failed to write config file: {}", e);
                std::process::exit(1);
            }

            println!();
            println!("Created config file: {}", output_path.display());
            println!();
            println!("To start the gateway, run:");
            println!("  gateway serve --config {}", output_path.display());
        }
        Err(wizard::WizardError::Cancelled) => {
            println!("Wizard cancelled.");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Wizard error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Create a default configuration file (non-interactive).
fn run_init_default(output: Option<String>, force: bool) {
    let Some(output_path) = output.map(PathBuf::from).or_else(default_config_path) else {
        eprintln!("Could not determine default config path. Please specify one with --output.");
        std::process::exit(1);
    };

    if output_path.exists() && !force {
        eprintln!(
            "Config file already exists: {}\nUse --force to overwrite.",
            output_path.display()
        );
        std::process::exit(1);
    }

    // Create parent directories if needed
    if let Some(parent) = output_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("Failed to create directory {}: {}", parent.display(), e);
        std::process::exit(1);
    }

    // Determine data directory and expand paths
    let data_dir = default_data_dir().unwrap_or_else(|| PathBuf::from("."));
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        eprintln!(
            "Failed to create data directory {}: {}",
            data_dir.display(),
            e
        );
        std::process::exit(1);
    }

    let config_content = default_config_toml().replace(
        "~/.local/share/hadrian/hadrian.db",
        &data_dir.join("hadrian.db").to_string_lossy(),
    );

    if let Err(e) = std::fs::write(&output_path, config_content) {
        eprintln!("Failed to write config file: {}", e);
        std::process::exit(1);
    }

    println!("Created config file: {}", output_path.display());
    println!("Database will be stored at: {}", data_dir.display());
    println!();
    println!("To start the gateway, run:");
    println!("  gateway serve");
    println!();
    println!("For interactive configuration, use:");
    println!("  gateway init --wizard");
}

/// Export JSON schema for the configuration file to file or stdout
#[cfg(feature = "json-schema")]
fn run_schema_export(output: Option<String>) {
    let content = config::GatewayConfig::json_schema_string();

    match output {
        Some(path) => {
            std::fs::write(&path, &content)
                .unwrap_or_else(|e| panic!("Failed to write to {}: {}", path, e));
            eprintln!("Config JSON schema written to {}", path);
        }
        None => {
            println!("{}", content);
        }
    }
}

/// Export OpenAPI specification to file or stdout (JSON format)
#[cfg(feature = "utoipa")]
fn run_openapi_export(output: Option<String>) {
    let spec = openapi::ApiDoc::build();
    let content =
        serde_json::to_string_pretty(&spec).expect("Failed to serialize OpenAPI spec to JSON");

    match output {
        Some(path) => {
            std::fs::write(&path, &content)
                .unwrap_or_else(|e| panic!("Failed to write to {}: {}", path, e));
            eprintln!("OpenAPI spec written to {}", path);
        }
        None => {
            println!("{}", content);
        }
    }
}

/// Print enabled compile-time features and build profile.
fn run_features() {
    let version = env!("CARGO_PKG_VERSION");

    // Check each feature at compile time
    let features: &[(&str, &str, bool)] = &[
        // Providers
        (
            "provider-openai",
            "Providers",
            cfg!(feature = "provider-openai"),
        ),
        (
            "provider-anthropic",
            "Providers",
            cfg!(feature = "provider-anthropic"),
        ),
        (
            "provider-test",
            "Providers",
            cfg!(feature = "provider-test"),
        ),
        (
            "provider-bedrock",
            "Providers",
            cfg!(feature = "provider-bedrock"),
        ),
        (
            "provider-vertex",
            "Providers",
            cfg!(feature = "provider-vertex"),
        ),
        (
            "provider-azure",
            "Providers",
            cfg!(feature = "provider-azure"),
        ),
        // Assets
        ("embed-ui", "Assets", cfg!(feature = "embed-ui")),
        ("embed-docs", "Assets", cfg!(feature = "embed-docs")),
        ("embed-catalog", "Assets", cfg!(feature = "embed-catalog")),
        // Databases
        (
            "database-sqlite",
            "Databases",
            cfg!(feature = "database-sqlite"),
        ),
        (
            "database-postgres",
            "Databases",
            cfg!(feature = "database-postgres"),
        ),
        // Infrastructure
        ("redis", "Infrastructure", cfg!(feature = "redis")),
        ("otlp", "Infrastructure", cfg!(feature = "otlp")),
        ("sso", "Infrastructure", cfg!(feature = "sso")),
        ("saml", "Infrastructure", cfg!(feature = "saml")),
        ("cel", "Infrastructure", cfg!(feature = "cel")),
        ("prometheus", "Infrastructure", cfg!(feature = "prometheus")),
        // Secrets
        ("vault", "Secrets", cfg!(feature = "vault")),
        ("secrets-aws", "Secrets", cfg!(feature = "secrets-aws")),
        ("secrets-azure", "Secrets", cfg!(feature = "secrets-azure")),
        ("secrets-gcp", "Secrets", cfg!(feature = "secrets-gcp")),
        // Storage & Processing
        (
            "s3-storage",
            "Storage & Processing",
            cfg!(feature = "s3-storage"),
        ),
        (
            "document-extraction-basic",
            "Storage & Processing",
            cfg!(feature = "document-extraction-basic"),
        ),
        (
            "document-extraction-full",
            "Storage & Processing",
            cfg!(feature = "document-extraction-full"),
        ),
        (
            "virus-scan",
            "Storage & Processing",
            cfg!(feature = "virus-scan"),
        ),
        // Validation & Export
        (
            "json-schema",
            "Validation & Export",
            cfg!(feature = "json-schema"),
        ),
        (
            "response-validation",
            "Validation & Export",
            cfg!(feature = "response-validation"),
        ),
        (
            "csv-export",
            "Validation & Export",
            cfg!(feature = "csv-export"),
        ),
        // Tools
        ("forecasting", "Tools", cfg!(feature = "forecasting")),
        ("wizard", "Tools", cfg!(feature = "wizard")),
        // Documentation
        ("utoipa", "Documentation", cfg!(feature = "utoipa")),
    ];

    // Infer build profile from enabled features
    let profile = if cfg!(feature = "full") {
        "full"
    } else if cfg!(feature = "headless") {
        "headless"
    } else if cfg!(feature = "standard") {
        "standard"
    } else if cfg!(feature = "minimal") {
        "minimal"
    } else if cfg!(feature = "tiny") {
        "tiny"
    } else {
        "custom"
    };

    println!("Hadrian Gateway v{version}\n");
    println!("Build profile: {profile}");
    match profile {
        "full" => println!("  (full = standard + saml, doc-extraction-full, virus-scan)\n"),
        "headless" => {
            println!("  (headless = full features without embedded assets — UI, docs, catalog)\n")
        }
        "standard" => println!(
            "  (standard = minimal + redis, otlp, doc-extraction-basic, postgres, embed-docs, prometheus, cel, utoipa, sso, forecasting, json-schema, response-validation, csv-export)\n"
        ),
        "minimal" => {
            println!("  (minimal = tiny + sqlite, embed-catalog, embed-ui, wizard)\n")
        }
        "tiny" => {
            println!(
                "  (tiny = openai, anthropic, test providers only, no database, no embedded assets)\n"
            )
        }
        _ => println!(),
    }

    println!("Compile-time features:");

    let mut current_group = "";
    for &(name, group, enabled) in features {
        if group != current_group {
            if !current_group.is_empty() {
                println!();
            }
            println!("  {group}:");
            current_group = group;
        }
        let status = if enabled { "enabled" } else { "disabled" };
        println!("    {name:<32} {status}");
    }
}

/// Open the UI in the system browser.
#[cfg(feature = "wizard")]
fn open_ui(url: &str) {
    match open::that(url) {
        Ok(()) => tracing::info!(url = %url, "Opened browser"),
        Err(e) => tracing::warn!(error = %e, url = %url, "Failed to open browser"),
    }
}

/// Run the gateway server
async fn run_server(explicit_config_path: Option<&str>, no_browser: bool) {
    // Resolve config path, creating default if necessary
    let (config_path, is_new_config) = match resolve_config_path(explicit_config_path) {
        Ok((path, is_new)) => (path, is_new),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if is_new_config {
        println!(
            "Created default configuration at: {}",
            config_path.display()
        );
        println!();
    }

    let config = match config::GatewayConfig::from_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "Failed to load config from {}: {}",
                config_path.display(),
                e
            );
            std::process::exit(1);
        }
    };

    // Initialize observability (tracing, metrics)
    // Keep the guard alive to ensure proper OpenTelemetry shutdown
    let _tracing_guard =
        observability::init_tracing(&config.observability).expect("Failed to initialize tracing");

    if let Err(e) = observability::metrics::init_metrics(&config.observability.metrics) {
        tracing::warn!(error = %e, "Failed to initialize metrics: {e}");
    }

    tracing::info!(
        config_file = %config_path.display(),
        "Starting AI Gateway"
    );

    // Emit startup security warnings for insecure configurations
    if let Some(crate::config::AdminAuthConfig::ProxyAuth(_)) = &config.auth.admin
        && !config.server.trusted_proxies.is_configured()
    {
        tracing::warn!(
            "SECURITY RISK: Proxy auth is enabled but no trusted_proxies are configured. \
             Anyone can spoof identity headers by connecting directly to the gateway. \
             Configure [server.trusted_proxies] with your reverse proxy's CIDR ranges."
        );
    }
    if config.auth.admin.is_none() {
        tracing::warn!(
            "No authentication configured for admin routes — admin routes use permissive \
             authorization. Configure auth.admin in hadrian.toml for production deployments."
        );
        if !config.server.host.is_loopback() {
            tracing::error!(
                bind_address = %config.server.host,
                "Gateway is bound to a non-localhost address without admin authentication. \
                 Admin routes are accessible to anyone who can reach this address. \
                 Configure auth.admin in hadrian.toml or bind to 127.0.0.1 for local-only access."
            );
        }
    }
    if !config.auth.rbac.enabled {
        tracing::warn!("RBAC disabled — all authorization checks will pass");
    }

    // Show welcome message for new configs
    if is_new_config {
        tracing::info!(
            "First-time setup complete! Configure providers in: {}",
            config_path.display()
        );
    }

    let state = AppState::new(config.clone())
        .await
        .expect("Failed to initialize application state");

    // Check for RBAC configuration mismatches with database state
    if !config.auth.rbac.enabled
        && let Some(db) = state.db.as_ref()
    {
        match db.org_rbac_policies().count_all().await {
            Ok(count) if count > 0 => {
                tracing::warn!(
                    policy_count = count,
                    "RBAC is disabled but organization RBAC policies exist in the database. \
                     These policies will not be evaluated."
                );
            }
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    "Failed to check for org RBAC policies at startup"
                );
            }
            _ => {}
        }
    }

    // Start DLQ retry worker if configured
    if let (Some(dlq), Some(db), Some(dlq_config)) = (
        state.dlq.clone(),
        state.db.clone(),
        config.observability.dead_letter_queue.as_ref(),
    ) {
        let retry_config = dlq_config.retry().clone();
        let ttl_secs = dlq_config.ttl_secs();

        tokio::spawn(async move {
            dlq::start_dlq_worker(dlq, db, retry_config, ttl_secs).await;
        });
    }

    // Start retention worker if configured and database is available
    if let Some(db) = state.db.clone() {
        let retention_config = config.retention.clone();
        tokio::spawn(async move {
            retention::start_retention_worker(db, retention_config).await;
        });
    }

    // Start vector store cleanup worker if configured and database is available
    if let Some(db) = state.db.clone() {
        let cleanup_config = config.features.vector_store_cleanup.clone();
        let vector_store = state
            .file_search_service
            .as_ref()
            .map(|fs| fs.vector_store());

        tokio::spawn(async move {
            jobs::start_vector_store_cleanup_worker(db, vector_store, cleanup_config).await;
        });
    }

    // Start model catalog sync worker if enabled
    {
        let catalog_config = config.features.model_catalog.clone();
        let registry = state.model_catalog.clone();
        let http_client = state.http_client.clone();

        tokio::spawn(async move {
            jobs::start_model_catalog_sync_worker(registry, catalog_config, http_client).await;
        });
    }

    // Start provider health checker for providers with health checks enabled
    {
        let mut health_checker = jobs::ProviderHealthChecker::with_registry(
            state.http_client.clone(),
            Some(state.event_bus.clone()),
            state.circuit_breakers.clone(),
            state.provider_health.clone(),
        );

        // Register providers with health checks enabled
        for (name, provider_config) in config.providers.iter() {
            let health_config = provider_config.health_check_config();
            if health_config.enabled {
                match create_provider_instance(provider_config, name, &state.circuit_breakers) {
                    Ok(provider) => {
                        health_checker.register(name, provider, health_config.clone());
                    }
                    Err(e) => {
                        tracing::warn!(
                            provider = %name,
                            error = %e,
                            "Failed to create provider for health checking"
                        );
                    }
                }
            }
        }

        // Spawn health checker if we have any providers registered
        if !health_checker.is_empty() {
            tracing::info!(
                provider_count = health_checker.provider_count(),
                "Starting provider health checker"
            );
            tokio::spawn(async move {
                health_checker.start().await;
            });
        }
    }

    // Start usage log buffer worker with configured sinks
    let usage_buffer_handle = if let Some(buffer) = state.usage_buffer.clone() {
        // Build usage sinks based on configuration
        let mut sinks: Vec<Arc<dyn usage_sink::UsageSink>> = Vec::new();

        // Add database sink if enabled and database is configured
        if config.observability.usage.database
            && let Some(db) = state.db.clone()
        {
            let db_sink = Arc::new(usage_sink::DatabaseSink::new(db, state.dlq.clone()));
            sinks.push(db_sink);
            tracing::info!("Usage logging to database enabled");
        }

        // Add OTLP sink if configured
        #[cfg(feature = "otlp")]
        if let Some(otlp_config) = &config.observability.usage.otlp
            && otlp_config.enabled
        {
            match usage_sink::OtlpSink::new(otlp_config, &config.observability.tracing) {
                Ok(otlp_sink) => {
                    sinks.push(Arc::new(otlp_sink));
                    tracing::info!("Usage logging to OTLP enabled");
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to initialize OTLP usage sink");
                }
            }
        }
        #[cfg(not(feature = "otlp"))]
        if let Some(otlp_config) = &config.observability.usage.otlp
            && otlp_config.enabled
        {
            tracing::warn!(
                "OTLP usage sink is enabled in config but the 'otlp' feature is not compiled. \
                Rebuild with: cargo build --features otlp"
            );
        }

        // Start worker if we have at least one sink
        if sinks.is_empty() {
            tracing::warn!("No usage sinks configured, usage data will be discarded");
            None
        } else {
            let composite_sink = Arc::new(usage_sink::CompositeSink::new(sinks));
            let handle = buffer.start_worker(composite_sink);
            tracing::info!("Usage log buffer worker started");
            Some((buffer, handle))
        }
    } else {
        None
    };

    let task_tracker = state.task_tracker.clone();
    let app = build_app(&config, state);

    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    //tracing::info!(address = %bind_addr, "Server listening");

    // Format to prepend with http://
    tracing::info!("Server listening on http://{}", bind_addr);

    // Open UI if enabled and not disabled via CLI
    #[cfg(feature = "wizard")]
    if config.ui.enabled && !no_browser && is_new_config {
        // Build URL using localhost for 0.0.0.0 bindings
        let host = if config.server.host.is_unspecified() {
            "127.0.0.1"
        } else {
            &config.server.host.to_string()
        };
        let url = format!("http://{}:{}", host, config.server.port);

        // Small delay to ensure server is ready before opening UI
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            open_ui(&url);
        });
    }
    #[cfg(not(feature = "wizard"))]
    let _ = no_browser;

    // Graceful shutdown: wait for SIGINT/SIGTERM, then wait for all background tasks
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(task_tracker, usage_buffer_handle))
        .await
        .unwrap();
}

async fn shutdown_signal(
    task_tracker: TaskTracker,
    usage_buffer_handle: Option<(
        Arc<usage_buffer::UsageLogBuffer>,
        tokio::task::JoinHandle<()>,
    )>,
) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, waiting for background tasks to complete...");

    // Close the task tracker to prevent new tasks from being spawned
    task_tracker.close();

    // Shutdown usage buffer worker and wait for it to flush
    if let Some((buffer, handle)) = usage_buffer_handle {
        buffer.shutdown();
        if let Err(e) = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
            tracing::warn!(error = %e, "Timeout waiting for usage buffer to flush");
        } else {
            tracing::info!("Usage buffer flushed successfully");
        }
    }

    // Wait for all in-flight tasks to complete (with timeout)
    let wait_result =
        tokio::time::timeout(std::time::Duration::from_secs(30), task_tracker.wait()).await;

    match wait_result {
        Ok(()) => tracing::info!("All background tasks completed"),
        Err(_) => {
            tracing::warn!("Timeout waiting for background tasks, some may not have completed")
        }
    }

    tracing::info!("Shutdown complete");
}

#[cfg(any(
    feature = "document-extraction-basic",
    feature = "document-extraction-full"
))]
/// Run the file processing worker.
///
/// This worker consumes jobs from a message queue (Redis Streams) and processes
/// files by chunking them and generating embeddings for vector search.
///
/// # Requirements
/// - Queue mode must be configured: `[features.file_processing] mode = "queue"`
/// - Queue backend must be configured: `[features.file_processing.queue]`
/// - Database must be configured for file metadata and chunk storage
async fn run_worker(
    explicit_config_path: Option<&str>,
    consumer_name: Option<String>,
    batch_size: usize,
    block_timeout_ms: u64,
    claim_pending: bool,
    pending_timeout_ms: u64,
) {
    // Resolve config path
    let (config_path, _) = match resolve_config_path(explicit_config_path) {
        Ok((path, is_new)) => (path, is_new),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let config = match config::GatewayConfig::from_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "Failed to load config from {}: {}",
                config_path.display(),
                e
            );
            std::process::exit(1);
        }
    };

    // Initialize observability
    let _tracing_guard =
        observability::init_tracing(&config.observability).expect("Failed to initialize tracing");

    tracing::info!(
        config_file = %config_path.display(),
        "Starting File Processing Worker"
    );

    // Validate that queue mode is configured
    if config.features.file_processing.mode != config::FileProcessingMode::Queue {
        eprintln!(
            "Error: File processing must be configured in queue mode.\n\
             Set [features.file_processing] mode = \"queue\" in your config file."
        );
        std::process::exit(1);
    }

    if config.features.file_processing.queue.is_none() {
        eprintln!(
            "Error: Queue backend not configured.\n\
             Add [features.file_processing.queue] section to your config file."
        );
        std::process::exit(1);
    }

    // Initialize database
    #[allow(unreachable_patterns, unreachable_code)]
    let db = match &config.database {
        config::DatabaseConfig::None => {
            eprintln!("Error: Database is required for file processing worker.");
            std::process::exit(1);
        }
        _ => {
            let pool = db::DbPool::from_config(&config.database)
                .await
                .expect("Failed to connect to database");
            pool.run_migrations()
                .await
                .expect("Failed to run migrations");
            Arc::new(pool)
        }
    };

    // Create file storage backend
    let file_storage = services::create_file_storage(&config.storage.files, db.clone())
        .await
        .expect("Failed to initialize file storage");

    // Create services
    let services = services::Services::new(
        db.clone(),
        file_storage,
        config.auth.rbac.max_expression_length,
    );
    let vector_stores_service = Arc::new(services.vector_stores.clone());

    // Initialize embedding service and vector store (similar to init_file_search_service)
    let (embedding_service, vector_store) =
        init_worker_embedding_service(&config, db.clone()).await;

    // Build document processor config
    let processor_config: services::DocumentProcessorConfig =
        (&config.features.file_processing).into();

    // Create document processor
    let processor = match services::DocumentProcessor::new(
        db,
        vector_stores_service,
        embedding_service,
        vector_store,
        processor_config,
    ) {
        Ok(p) => Arc::new(p),
        Err(e) => {
            eprintln!("Failed to initialize document processor: {}", e);
            std::process::exit(1);
        }
    };

    // Build worker config
    let worker_config = services::WorkerConfig {
        consumer_name: consumer_name.unwrap_or_else(|| format!("worker-{}", uuid::Uuid::new_v4())),
        batch_size,
        block_timeout_ms,
        idle_interval_secs: 1,
        claim_pending,
        pending_timeout_ms,
    };

    tracing::info!(
        consumer_name = %worker_config.consumer_name,
        batch_size = worker_config.batch_size,
        block_timeout_ms = worker_config.block_timeout_ms,
        claim_pending = worker_config.claim_pending,
        "Worker configuration"
    );

    // Run the worker (blocks until shutdown)
    services::start_file_processing_worker(processor, worker_config).await;
}

/// Run database migrations and exit.
///
/// This is useful for:
/// - Kubernetes init containers (run migrations before main container starts)
/// - CI/CD pipelines (run migrations as a separate step)
/// - Manual migration runs
///
/// Exits with code 0 on success, 1 on failure.
async fn run_migrate(explicit_config_path: Option<&str>) {
    // Resolve config path
    let (config_path, _) = match resolve_config_path(explicit_config_path) {
        Ok((path, is_new)) => (path, is_new),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let config = match config::GatewayConfig::from_file(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "Failed to load config from {}: {}",
                config_path.display(),
                e
            );
            std::process::exit(1);
        }
    };

    // Initialize minimal observability for migration logging
    let _tracing_guard =
        observability::init_tracing(&config.observability).expect("Failed to initialize tracing");

    tracing::info!(
        config_file = %config_path.display(),
        "Running database migrations"
    );

    // Validate database is configured
    if config.database.is_none() {
        eprintln!("Error: Database is not configured. Nothing to migrate.");
        std::process::exit(1);
    }

    // Connect to database and run migrations
    match db::DbPool::from_config(&config.database).await {
        Ok(pool) => match pool.run_migrations().await {
            Ok(()) => {
                tracing::info!("Database migrations completed successfully");
                std::process::exit(0);
            }
            Err(e) => {
                tracing::error!(error = %e, "Database migrations failed");
                eprintln!("Error: Database migrations failed: {}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to connect to database");
            eprintln!("Error: Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(any(
    feature = "document-extraction-basic",
    feature = "document-extraction-full"
))]
/// Initialize embedding service and vector store for the worker.
async fn init_worker_embedding_service(
    config: &config::GatewayConfig,
    db: Arc<db::DbPool>,
) -> (
    Option<Arc<cache::EmbeddingService>>,
    Option<Arc<dyn cache::vector_store::VectorBackend>>,
) {
    #[cfg(not(feature = "database-postgres"))]
    let _ = &db;
    // Get embedding configuration (same priority as init_file_search_service)
    let file_search_config = match &config.features.file_search {
        Some(cfg) if cfg.enabled => cfg,
        _ => {
            tracing::warn!("File search not configured, chunks will not have embeddings");
            return (None, None);
        }
    };

    let embedding_config = file_search_config
        .embedding
        .as_ref()
        .or_else(|| {
            config
                .features
                .response_caching
                .as_ref()
                .and_then(|rc| rc.semantic.as_ref())
                .map(|sc| &sc.embedding)
        })
        .or_else(|| {
            config
                .features
                .vector_search
                .as_ref()
                .map(|vs| &vs.embedding)
        });

    let embedding_config = match embedding_config {
        Some(cfg) => cfg,
        None => {
            tracing::warn!("No embedding configuration found, chunks will not have embeddings");
            return (None, None);
        }
    };

    // Get the embedding provider configuration
    let provider_config = match config.providers.get(&embedding_config.provider) {
        Some(cfg) => cfg,
        None => {
            tracing::error!(
                provider = %embedding_config.provider,
                "Embedding provider '{}' not configured",
                embedding_config.provider
            );
            return (None, None);
        }
    };

    // Create circuit breaker registry (empty for worker)
    let circuit_breakers = providers::CircuitBreakerRegistry::new();

    // Create HTTP client
    let http_client = reqwest::Client::new();

    // Create embedding service
    let embedding_service = match cache::EmbeddingService::new(
        embedding_config,
        provider_config,
        &circuit_breakers,
        http_client,
    ) {
        Ok(service) => Arc::new(service),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create embedding service");
            return (None, None);
        }
    };

    // Create vector store
    let vector_store: Arc<dyn cache::vector_store::VectorBackend> = if let Some(rag_backend) =
        &file_search_config.vector_backend
    {
        match rag_backend {
            #[cfg(feature = "database-postgres")]
            config::RagVectorBackend::Pgvector {
                table_name,
                index_type,
                distance_metric,
            } => {
                let pg_pool = match db.pg_write_pool() {
                    Some(pool) => pool.clone(),
                    None => {
                        tracing::error!("pgvector requires PostgreSQL database");
                        return (Some(embedding_service), None);
                    }
                };

                let store = cache::vector_store::PgvectorStore::new(
                    pg_pool,
                    format!("{}_semantic", table_name.trim_end_matches("_chunks")),
                    embedding_config.dimensions,
                    index_type.clone(),
                    *distance_metric,
                );

                if let Err(e) = store.initialize().await {
                    tracing::error!(error = %e, "Failed to initialize pgvector store");
                    return (Some(embedding_service), None);
                }

                Arc::new(store)
            }
            #[cfg(not(feature = "database-postgres"))]
            config::RagVectorBackend::Pgvector { .. } => {
                tracing::error!(
                    "pgvector requires the 'database-postgres' feature. \
                         Rebuild with --features database-postgres or use a different vector backend."
                );
                return (Some(embedding_service), None);
            }
            config::RagVectorBackend::Qdrant {
                url,
                api_key,
                qdrant_collection_name,
                distance_metric,
            } => {
                let store = cache::vector_store::QdrantStore::new(
                    url.clone(),
                    api_key.clone(),
                    qdrant_collection_name.clone(),
                    embedding_config.dimensions,
                    *distance_metric,
                );

                if let Err(e) = store.initialize().await {
                    tracing::error!(error = %e, "Failed to initialize Qdrant store");
                    return (Some(embedding_service), None);
                }

                Arc::new(store)
            }
        }
    } else {
        // Default to pgvector
        #[cfg(not(feature = "database-postgres"))]
        {
            tracing::warn!(
                "No vector store configured and the 'database-postgres' feature is not enabled. \
                     Configure [features.file_search.vector_backend] or rebuild with --features database-postgres."
            );
            return (Some(embedding_service), None);
        }

        #[cfg(feature = "database-postgres")]
        {
            let pg_pool = match db.pg_write_pool() {
                Some(pool) => pool.clone(),
                None => {
                    tracing::warn!("No vector store configured and not using PostgreSQL");
                    return (Some(embedding_service), None);
                }
            };

            let store = cache::vector_store::PgvectorStore::new(
                pg_pool,
                "rag".to_string(),
                embedding_config.dimensions,
                config::PgvectorIndexType::IvfFlat,
                config::DistanceMetric::default(), // Cosine (default)
            );

            if let Err(e) = store.initialize().await {
                tracing::error!(error = %e, "Failed to initialize pgvector store");
                return (Some(embedding_service), None);
            }

            Arc::new(store)
        }
    };

    (Some(embedding_service), Some(vector_store))
}
