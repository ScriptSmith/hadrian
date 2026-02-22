//! Health check endpoints for Kubernetes probes and monitoring.

use axum::{Json, extract::State, response::IntoResponse};
use http::StatusCode;
use serde::Serialize;

use crate::AppState;
#[cfg(feature = "prometheus")]
use crate::observability::metrics::get_prometheus_handle;

/// Detailed health status response.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct HealthStatus {
    /// Overall status: "healthy", "degraded", or "unhealthy"
    #[cfg_attr(feature = "utoipa", schema(example = "healthy"))]
    pub status: String,
    /// Service version
    #[cfg_attr(feature = "utoipa", schema(example = "0.1.0"))]
    pub version: String,
    /// Individual subsystem statuses
    pub subsystems: SubsystemStatus,
}

/// Status of individual subsystems.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SubsystemStatus {
    /// Database connection status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<ComponentStatus>,
    /// Cache connection status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<ComponentStatus>,
    /// Secrets manager status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<ComponentStatus>,
}

/// Status of a single component.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ComponentStatus {
    /// Whether the component is healthy
    #[cfg_attr(feature = "utoipa", schema(example = true))]
    pub healthy: bool,
    /// Optional message with details
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(example = json!(null)))]
    pub message: Option<String>,
    /// Latency of the health check in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(example = 5))]
    pub latency_ms: Option<u64>,
}

/// Full health check with subsystem status.
///
/// Returns detailed status of all subsystems including database, cache, and secrets manager.
/// Use this endpoint for comprehensive health monitoring and dashboards.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/health",
    tag = "health",
    operation_id = "health_check",
    responses(
        (status = 200, description = "Service is healthy", body = HealthStatus),
        (status = 503, description = "Service is unhealthy", body = HealthStatus),
    )
))]
#[tracing::instrument(name = "health.check", skip(state))]
pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let mut overall_healthy = true;
    let mut subsystems = SubsystemStatus {
        database: None,
        cache: None,
        secrets: None,
    };

    // Check database
    if let Some(db) = &state.db {
        let start = std::time::Instant::now();
        let db_healthy = db.health_check().await.is_ok();
        let latency_ms = start.elapsed().as_millis() as u64;

        if !db_healthy {
            overall_healthy = false;
        }

        subsystems.database = Some(ComponentStatus {
            healthy: db_healthy,
            message: if db_healthy {
                None
            } else {
                Some("Database connection failed".to_string())
            },
            latency_ms: Some(latency_ms),
        });
    }

    // Check cache
    if let Some(cache) = &state.cache {
        let start = std::time::Instant::now();
        // Try a simple operation to verify cache connectivity
        let cache_healthy = cache.get_bytes("__health_check__").await.is_ok();
        let latency_ms = start.elapsed().as_millis() as u64;

        // Cache being unhealthy is degraded, not unhealthy
        // (the system can still function without cache)

        subsystems.cache = Some(ComponentStatus {
            healthy: cache_healthy,
            message: if cache_healthy {
                None
            } else {
                Some("Cache connection failed".to_string())
            },
            latency_ms: Some(latency_ms),
        });
    }

    // Check secrets manager
    if let Some(secrets) = &state.secrets {
        let start = std::time::Instant::now();
        let secrets_healthy = secrets.health_check().await.is_ok();
        let latency_ms = start.elapsed().as_millis() as u64;

        if !secrets_healthy {
            overall_healthy = false;
        }

        subsystems.secrets = Some(ComponentStatus {
            healthy: secrets_healthy,
            message: if secrets_healthy {
                None
            } else {
                Some("Secrets manager unavailable".to_string())
            },
            latency_ms: Some(latency_ms),
        });
    }

    let status = if overall_healthy {
        "healthy"
    } else {
        "unhealthy"
    };

    let health = HealthStatus {
        status: status.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        subsystems,
    };

    let status_code = if overall_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (status_code, Json(health))
}

/// Kubernetes liveness probe.
///
/// Returns 200 if the service is running. This endpoint should always succeed
/// unless the service process is completely broken. Use this for Kubernetes
/// liveness probes to detect and restart unhealthy pods.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/health/live",
    tag = "health",
    operation_id = "health_liveness",
    responses(
        (status = 200, description = "Service is alive"),
    )
))]
#[tracing::instrument(name = "health.liveness")]
pub async fn liveness() -> impl IntoResponse {
    StatusCode::OK
}

/// Kubernetes readiness probe.
///
/// Returns 200 if the service is ready to accept traffic. Checks that critical
/// dependencies (database) are available. Use this for Kubernetes readiness
/// probes to control traffic routing to pods.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/health/ready",
    tag = "health",
    operation_id = "health_readiness",
    responses(
        (status = 200, description = "Service is ready to accept traffic"),
        (status = 503, description = "Service is not ready (database unavailable)"),
    )
))]
#[tracing::instrument(name = "health.readiness", skip(state))]
pub async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    // In minimal mode (no database), always ready
    if state.db.is_none() {
        return StatusCode::OK;
    }

    // Check database connectivity
    if let Some(db) = &state.db
        && db.health_check().await.is_err()
    {
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    StatusCode::OK
}

/// Prometheus metrics endpoint.
///
/// Returns metrics in Prometheus text format.
#[tracing::instrument(name = "health.metrics")]
pub async fn metrics() -> impl IntoResponse {
    #[cfg(feature = "prometheus")]
    {
        return match get_prometheus_handle() {
            Some(handle) => {
                let metrics: String = handle.render();
                (
                    StatusCode::OK,
                    [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
                    metrics,
                )
            }
            None => (
                StatusCode::SERVICE_UNAVAILABLE,
                [("content-type", "text/plain")],
                "Metrics not initialized".to_string(),
            ),
        };
    }
    #[cfg(not(feature = "prometheus"))]
    (
        StatusCode::NOT_FOUND,
        [("content-type", "text/plain")],
        "Prometheus metrics not enabled".to_string(),
    )
}

#[cfg(all(test, feature = "database-sqlite"))]
mod tests {
    use axum::{Router, body::Body};
    use http::Request;
    use serde_json::Value;
    use tower::ServiceExt;

    use super::*;

    /// Create a test application with database configured
    async fn test_app_with_db() -> Router {
        use std::sync::atomic::{AtomicU64, Ordering};

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let db_id = COUNTER.fetch_add(1, Ordering::SeqCst);

        let config_str = format!(
            r#"
[database]
type = "sqlite"
path = "file:test_health_db_{}?mode=memory&cache=shared"
create_if_missing = true
run_migrations = true
wal_mode = false
busy_timeout_ms = 5000

[providers.test-openai]
type = "open_ai"
api_key = "sk-test-key"
"#,
            db_id
        );

        let config = crate::config::GatewayConfig::from_str(&config_str)
            .expect("Failed to parse test config");
        let state = crate::AppState::new(config.clone())
            .await
            .expect("Failed to create AppState");
        crate::build_app(&config, state)
    }

    /// Create a minimal test application without database
    async fn test_app_no_db() -> Router {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        let config_str = r#"
[providers.test-openai]
type = "open_ai"
api_key = "sk-test-key"
"#;

        let config = crate::config::GatewayConfig::from_str(config_str)
            .expect("Failed to parse test config");
        let state = crate::AppState::new(config.clone())
            .await
            .expect("Failed to create AppState");
        crate::build_app(&config, state)
    }

    /// Helper to make a GET request and parse JSON response
    async fn get_json(app: &Router, uri: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        (status, json)
    }

    /// Helper to make a GET request and return raw response
    async fn get_raw(app: &Router, uri: &str) -> (StatusCode, String) {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&body).to_string();
        (status, text)
    }

    // ============================================================================
    // Health Check Tests (/health)
    // ============================================================================

    #[tokio::test]
    async fn test_health_check_with_db_healthy() {
        let app = test_app_with_db().await;

        let (status, body) = get_json(&app, "/health").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "healthy");
        assert!(body["version"].is_string());
        assert!(!body["version"].as_str().unwrap().is_empty());

        // Database should be reported
        assert!(body["subsystems"]["database"].is_object());
        assert_eq!(body["subsystems"]["database"]["healthy"], true);
        assert!(body["subsystems"]["database"]["latency_ms"].is_number());
    }

    #[tokio::test]
    async fn test_health_check_no_db_healthy() {
        let app = test_app_no_db().await;

        let (status, body) = get_json(&app, "/health").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "healthy");
        assert!(body["version"].is_string());

        // No database configured - should not be in response
        assert!(body["subsystems"]["database"].is_null());
    }

    #[tokio::test]
    async fn test_health_check_returns_version() {
        let app = test_app_no_db().await;

        let (status, body) = get_json(&app, "/health").await;

        assert_eq!(status, StatusCode::OK);
        // Version should match Cargo.toml version
        let version = body["version"].as_str().unwrap();
        assert!(!version.is_empty());
        // Should be a valid semver-ish format (at least major.minor)
        assert!(version.contains('.'));
    }

    // ============================================================================
    // Liveness Probe Tests (/health/live)
    // ============================================================================

    #[tokio::test]
    async fn test_liveness_always_ok() {
        let app = test_app_no_db().await;

        let (status, _) = get_raw(&app, "/health/live").await;

        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_liveness_ok_with_db() {
        let app = test_app_with_db().await;

        let (status, _) = get_raw(&app, "/health/live").await;

        assert_eq!(status, StatusCode::OK);
    }

    // ============================================================================
    // Readiness Probe Tests (/health/ready)
    // ============================================================================

    #[tokio::test]
    async fn test_readiness_no_db_always_ready() {
        let app = test_app_no_db().await;

        let (status, _) = get_raw(&app, "/health/ready").await;

        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_readiness_with_healthy_db() {
        let app = test_app_with_db().await;

        let (status, _) = get_raw(&app, "/health/ready").await;

        assert_eq!(status, StatusCode::OK);
    }

    // ============================================================================
    // Metrics Endpoint Tests (/metrics)
    // ============================================================================

    #[cfg(feature = "prometheus")]
    #[tokio::test]
    async fn test_metrics_not_initialized() {
        // Without metrics initialization, should return 503
        let app = test_app_no_db().await;

        let (status, body) = get_raw(&app, "/metrics").await;

        // Metrics are typically not initialized in test mode
        // This test documents the expected behavior
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains("not initialized"));
    }

    // ============================================================================
    // Response Structure Tests
    // ============================================================================

    #[tokio::test]
    async fn test_health_response_structure() {
        let app = test_app_with_db().await;

        let (status, body) = get_json(&app, "/health").await;

        assert_eq!(status, StatusCode::OK);

        // Verify expected fields exist
        assert!(body.get("status").is_some());
        assert!(body.get("version").is_some());
        assert!(body.get("subsystems").is_some());

        // Subsystems should be an object
        assert!(body["subsystems"].is_object());
    }

    #[tokio::test]
    async fn test_health_database_component_structure() {
        let app = test_app_with_db().await;

        let (status, body) = get_json(&app, "/health").await;

        assert_eq!(status, StatusCode::OK);

        let db_status = &body["subsystems"]["database"];
        assert!(db_status.is_object());
        assert!(db_status.get("healthy").is_some());
        assert!(db_status["healthy"].is_boolean());
        assert!(db_status.get("latency_ms").is_some());
        assert!(db_status["latency_ms"].is_number());
    }
}
