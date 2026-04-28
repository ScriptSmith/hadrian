use std::sync::Arc;

use tokio_util::task::TaskTracker;

use super::resolve_config_path;
use crate::{
    app::{AppState, build_app},
    config, dlq,
    init::create_provider_instance,
    jobs, observability, retention, usage_buffer, usage_sink,
};

/// Open the UI in the system browser.
#[cfg(feature = "wizard")]
fn open_ui(url: &str) {
    match open::that(url) {
        Ok(()) => tracing::info!(url = %url, "Opened browser"),
        Err(e) => tracing::warn!(error = %e, url = %url, "Failed to open browser"),
    }
}

/// Run the gateway server
pub(crate) async fn run_server(explicit_config_path: Option<&str>, no_browser: bool) {
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
    let _tracing_guard = match observability::init_tracing(&config.observability) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Failed to initialize tracing: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = observability::metrics::init_metrics(&config.observability.metrics) {
        tracing::warn!(error = %e, "Failed to initialize metrics: {e}");
    }

    tracing::info!(
        config_file = %config_path.display(),
        "Starting AI Gateway"
    );

    // Emit startup security warnings for insecure configurations
    if matches!(config.auth.mode, crate::config::AuthMode::Iap(_))
        && !config.server.trusted_proxies.is_configured()
    {
        tracing::warn!(
            "SECURITY RISK: IAP auth is enabled but no trusted_proxies are configured. \
             Anyone can spoof identity headers by connecting directly to the gateway. \
             Configure [server.trusted_proxies] with your reverse proxy's CIDR ranges."
        );
    }
    if !config.auth.is_auth_enabled() {
        tracing::warn!(
            "No authentication configured — all routes use permissive authorization. \
             Configure [auth.mode] in hadrian.toml for production deployments."
        );
        if !config.server.host.is_loopback() {
            tracing::error!(
                bind_address = %config.server.host,
                "Gateway is bound to a non-localhost address without authentication. \
                 All routes are accessible to anyone who can reach this address. \
                 Configure [auth.mode] in hadrian.toml or bind to 127.0.0.1 for local-only access."
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

    if let Some(tls) = config.server.tls.as_ref() {
        if !tls.acknowledge_unsupported {
            tracing::error!(
                "[server.tls] is set but the gateway does not yet terminate TLS \
                 itself. Refusing to start to avoid serving the gateway on plain \
                 HTTP while the operator believes TLS is active. Terminate TLS \
                 upstream (reverse proxy / load balancer) and remove the \
                 [server.tls] section, or set \
                 `[server.tls].acknowledge_unsupported = true` to opt in to the \
                 plaintext-listener behaviour while native TLS support is built out."
            );
            std::process::exit(1);
        }
        tracing::warn!(
            "[server.tls] is set with acknowledge_unsupported = true; the \
             gateway will continue to listen on plain HTTP because native \
             TLS is not yet implemented. Terminate TLS upstream."
        );
    }

    let state = match AppState::new(config.clone()).await {
        Ok(state) => state,
        Err(e) => {
            tracing::error!(error = %e, "Failed to initialize application state");
            std::process::exit(1);
        }
    };

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

    // Pre-load per-org gateway JWT validators in the background.
    // Each org requires an HTTP round-trip to its IdP's discovery endpoint, so this
    // runs concurrently after server start instead of blocking startup.
    #[cfg(feature = "sso")]
    if let (Some(registry), Some(db)) = (state.gateway_jwt_registry.clone(), state.db.clone()) {
        let http_client = state.http_client.clone();
        let allow_loopback = config.server.allow_loopback_urls;
        let allow_private = config.server.allow_private_urls;
        let jwt_loader_concurrency = config.server.jwt_loader_concurrency;
        state.task_tracker.spawn(async move {
            let configs = match db.org_sso_configs().list_enabled().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Failed to load SSO configs for gateway JWT registry \
                         (will lazy-load on first request)"
                    );
                    return;
                }
            };

            let oidc_configs: Vec<_> = configs
                .into_iter()
                .filter(|c| c.config.provider_type == crate::models::SsoProviderType::Oidc)
                .collect();

            if oidc_configs.is_empty() {
                return;
            }

            // Load concurrently with bounded parallelism to avoid overwhelming IdPs
            use futures::stream::{self, StreamExt};
            let results: Vec<_> = stream::iter(oidc_configs)
                .map(|cfg| {
                    let registry = &registry;
                    let http_client = &http_client;
                    async move {
                        if let Err(e) = registry
                            .register_from_sso_config(
                                &cfg.config,
                                http_client,
                                allow_loopback,
                                allow_private,
                            )
                            .await
                        {
                            tracing::warn!(
                                org_id = %cfg.config.org_id,
                                issuer = ?cfg.config.issuer,
                                error = %e,
                                "Failed to register gateway JWT validator for org \
                                 (will lazy-load on first request)"
                            );
                            false
                        } else {
                            true
                        }
                    }
                })
                .buffer_unordered(jwt_loader_concurrency)
                .collect()
                .await;

            let loaded = results.iter().filter(|ok| **ok).count();
            if loaded > 0 {
                tracing::info!(count = loaded, "Gateway JWT validator registry initialized");
            }
        });
    }

    // Start retention worker if configured and database is available
    if let Some(db) = state.db.clone() {
        let retention_config = config.retention.clone();
        tokio::spawn(async move {
            retention::start_retention_worker(db, retention_config).await;
        });
    }

    // Start OAuth PKCE authorization code cleanup worker. Always runs when
    // the database is available — codes are short-lived housekeeping data,
    // not subject to retention policy.
    if let Some(db) = state.db.clone()
        && config.auth.oauth_pkce.enabled
    {
        tokio::spawn(async move {
            jobs::start_oauth_code_cleanup_worker(db).await;
        });
    }

    // Start vector store cleanup worker if configured and database is available
    if let Some(db) = state.db.clone() {
        let cleanup_config = config.features.vector_store_cleanup.clone();
        let vector_store = state
            .file_search_service
            .as_ref()
            .map(|fs| fs.vector_store());
        let file_storage = state.services.as_ref().map(|s| s.files.storage());

        tokio::spawn(async move {
            jobs::start_vector_store_cleanup_worker(db, vector_store, file_storage, cleanup_config)
                .await;
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

        // Add OTLP sinks if configured
        #[cfg(feature = "otlp")]
        use usage_sink::UsageSink as _;
        #[cfg(feature = "otlp")]
        for otlp_config in &config.observability.usage.otlp {
            if !otlp_config.enabled {
                continue;
            }
            match usage_sink::OtlpSink::new(otlp_config, &config.observability.tracing) {
                Ok(otlp_sink) => {
                    tracing::info!(name = otlp_sink.name(), "Usage logging to OTLP enabled");
                    sinks.push(Arc::new(otlp_sink));
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to initialize OTLP usage sink");
                }
            }
        }
        #[cfg(not(feature = "otlp"))]
        if config.observability.usage.otlp.iter().any(|c| c.enabled) {
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

    // Refresh the static models cache periodically in the background
    // (initial warming already happened in AppState::new)
    if config.features.static_models_cache.enabled() {
        let interval = config.features.static_models_cache.refresh_interval();
        let state_ref = state.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.tick().await; // skip the immediate first tick (already warmed)
            loop {
                ticker.tick().await;
                state_ref.warm_static_models_cache().await;
            }
        });
    }

    let task_tracker = state.task_tracker.clone();
    let static_cache_enabled = state.config.features.static_models_cache.enabled();
    let warm_state = if static_cache_enabled {
        Some(state.clone())
    } else {
        None
    };
    let app = build_app(&config, state);

    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!(error = %e, bind_addr = %bind_addr, "Failed to bind to address");
            std::process::exit(1);
        }
    };

    tracing::info!("Server listening on http://{}", bind_addr);

    // Warm the static models cache on a background task. With many providers
    // (including slow/dead ones holding open connections until they time out)
    // the warm can take tens of seconds; doing it inline would delay the
    // listener bind, the readiness probe, and any rolling deploy gated on
    // `/health/ready`.
    if let Some(warm_state) = warm_state {
        task_tracker.spawn(async move {
            warm_state.warm_static_models_cache().await;
        });
    }

    if config.server.allow_loopback_urls || config.server.allow_private_urls {
        tracing::info!(
            allow_loopback = config.server.allow_loopback_urls,
            allow_private = config.server.allow_private_urls,
            "SSRF validation relaxed for development/Docker"
        );
    }

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

    let shutdown_config = config.server.shutdown.clone();

    // Graceful shutdown: wait for SIGINT/SIGTERM, then wait for all background tasks.
    // `into_make_service_with_connect_info` is required so middleware can read the
    // connecting peer address via `ConnectInfo<SocketAddr>` for IP-based rate limits,
    // API-key IP allowlists, and audit logging.
    if let Err(e) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(
        task_tracker,
        usage_buffer_handle,
        shutdown_config,
    ))
    .await
    {
        tracing::error!(error = %e, "Server error");
        std::process::exit(1);
    }
}

async fn shutdown_signal(
    task_tracker: TaskTracker,
    usage_buffer_handle: Option<(
        Arc<usage_buffer::UsageLogBuffer>,
        tokio::task::JoinHandle<()>,
    )>,
    shutdown_config: crate::config::ShutdownConfig,
) {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "Failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
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
        if let Err(e) = tokio::time::timeout(
            std::time::Duration::from_secs(shutdown_config.usage_buffer_flush_secs),
            handle,
        )
        .await
        {
            tracing::warn!(error = %e, "Timeout waiting for usage buffer to flush");
        } else {
            tracing::info!("Usage buffer flushed successfully");
        }
    }

    // Wait for all in-flight tasks to complete (with timeout)
    let wait_result = tokio::time::timeout(
        std::time::Duration::from_secs(shutdown_config.drain_secs),
        task_tracker.wait(),
    )
    .await;

    match wait_result {
        Ok(()) => tracing::info!("All background tasks completed"),
        Err(_) => {
            tracing::warn!("Timeout waiting for background tasks, some may not have completed")
        }
    }

    tracing::info!("Shutdown complete");
}
