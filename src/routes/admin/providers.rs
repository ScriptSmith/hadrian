//! Provider administration endpoints.
//!
//! Provides endpoints for monitoring provider health, circuit breaker status,
//! and metrics-based statistics.

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::AdminError;
use crate::{
    AppState,
    jobs::ProviderHealthState,
    middleware::AuthzContext,
    providers::CircuitBreakerStatus,
    services::{ProviderStats, ProviderStatsHistorical, StatsGranularity},
};

/// Response for circuit breaker status endpoint.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CircuitBreakersResponse {
    /// List of circuit breaker statuses for all providers.
    pub circuit_breakers: Vec<CircuitBreakerStatus>,
}

/// Response for a single provider's circuit breaker status.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProviderCircuitBreakerResponse {
    /// Provider name.
    pub provider: String,
    /// Circuit breaker state (closed, open, half_open).
    pub state: String,
    /// Number of consecutive failures (only relevant in Closed state).
    pub failure_count: u32,
}

/// Get circuit breaker status for all providers.
///
/// Returns the current state and failure count for all configured
/// circuit breakers. Useful for monitoring provider health.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/providers/circuit-breakers",
    tag = "providers",
    responses(
        (status = 200, description = "Circuit breaker status for all providers", body = CircuitBreakersResponse),
    )
))]
pub async fn list_circuit_breakers(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<CircuitBreakersResponse>, AdminError> {
    authz.require("provider", "list", None, None, None, None)?;

    let circuit_breakers = state.circuit_breakers.status();

    Ok(Json(CircuitBreakersResponse { circuit_breakers }))
}

/// Get circuit breaker status for a specific provider.
///
/// Returns the current state and failure count for the specified provider's
/// circuit breaker.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/providers/{provider_name}/circuit-breaker",
    tag = "providers",
    params(
        ("provider_name" = String, Path, description = "Provider name")
    ),
    responses(
        (status = 200, description = "Circuit breaker status for the provider", body = ProviderCircuitBreakerResponse),
        (status = 404, description = "Provider not found or circuit breaker not enabled"),
    )
))]
pub async fn get_circuit_breaker(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    axum::extract::Path(provider_name): axum::extract::Path<String>,
) -> Result<Json<ProviderCircuitBreakerResponse>, AdminError> {
    authz.require("provider", "read", None, None, None, None)?;

    let status = state
        .circuit_breakers
        .status_for(&provider_name)
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Circuit breaker not found for provider '{}' (not configured or disabled)",
                provider_name
            ))
        })?;

    Ok(Json(ProviderCircuitBreakerResponse {
        provider: status.provider,
        state: format!("{:?}", status.state).to_lowercase(),
        failure_count: status.failure_count,
    }))
}

/// Response for provider health status endpoint.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProviderHealthResponse {
    /// List of health states for all providers with health checks enabled.
    pub providers: Vec<ProviderHealthState>,
}

/// Get health status for all providers.
///
/// Returns the current health status for all providers that have health checks
/// enabled. Includes status, latency, last check time, and consecutive
/// success/failure counts.
///
/// Note: Only providers with `health_check.enabled = true` in their config
/// will appear in this list. Providers without health checks rely solely
/// on circuit breaker status.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/providers/health",
    tag = "providers",
    responses(
        (status = 200, description = "Health status for all providers", body = ProviderHealthResponse),
    )
))]
pub async fn list_provider_health(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<ProviderHealthResponse>, AdminError> {
    authz.require("provider", "list", None, None, None, None)?;

    let providers = state.provider_health.get_all();
    Ok(Json(ProviderHealthResponse { providers }))
}

/// Get health status for a specific provider.
///
/// Returns the current health status for the specified provider. Only works
/// for providers that have health checks enabled.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/providers/{provider_name}/health",
    tag = "providers",
    params(
        ("provider_name" = String, Path, description = "Provider name")
    ),
    responses(
        (status = 200, description = "Health status for the provider", body = ProviderHealthState),
        (status = 404, description = "Provider not found or health checks not enabled"),
    )
))]
pub async fn get_provider_health(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    axum::extract::Path(provider_name): axum::extract::Path<String>,
) -> Result<Json<ProviderHealthState>, AdminError> {
    authz.require("provider", "read", None, None, None, None)?;

    let health = state.provider_health.get(&provider_name).ok_or_else(|| {
        AdminError::NotFound(format!(
            "Health status not found for provider '{}' (health checks not enabled or provider not configured)",
            provider_name
        ))
    })?;

    Ok(Json(health))
}

// ─────────────────────────────────────────────────────────────────────────────
// Provider Stats Endpoints
// ─────────────────────────────────────────────────────────────────────────────

/// Response for provider stats endpoint.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProviderStatsResponse {
    /// List of stats for all providers.
    pub stats: Vec<ProviderStats>,
}

/// Query parameters for historical stats.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams, utoipa::ToSchema))]
pub struct ProviderStatsHistoryQuery {
    /// Start of the time range (defaults to 24 hours ago).
    pub start: Option<DateTime<Utc>>,
    /// End of the time range (defaults to now).
    pub end: Option<DateTime<Utc>>,
    /// Granularity of the data points: "hour" or "day" (defaults to "hour").
    pub granularity: Option<String>,
}

/// Get aggregated statistics for all providers.
///
/// Returns current metrics including latency percentiles, error counts,
/// token usage, and costs for all providers.
///
/// In single-node deployments, stats are derived from the local /metrics endpoint.
/// In multi-node deployments with Prometheus configured, stats are aggregated
/// from Prometheus queries.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/providers/stats",
    tag = "providers",
    responses(
        (status = 200, description = "Stats for all providers", body = ProviderStatsResponse),
        (status = 500, description = "Failed to fetch stats"),
    )
))]
pub async fn list_provider_stats(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<ProviderStatsResponse>, AdminError> {
    authz.require("provider", "list", None, None, None, None)?;

    let stats = state
        .provider_metrics
        .get_all_stats()
        .await
        .map_err(|e| AdminError::Internal(format!("Failed to get provider stats: {}", e)))?;

    Ok(Json(ProviderStatsResponse { stats }))
}

/// Get aggregated statistics for a specific provider.
///
/// Returns current metrics including latency percentiles, error counts,
/// token usage, and costs for the specified provider.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/providers/{provider_name}/stats",
    tag = "providers",
    params(
        ("provider_name" = String, Path, description = "Provider name")
    ),
    responses(
        (status = 200, description = "Stats for the provider", body = ProviderStats),
        (status = 404, description = "Provider not found or no stats available"),
    )
))]
pub async fn get_provider_stats(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(provider_name): Path<String>,
) -> Result<Json<ProviderStats>, AdminError> {
    authz.require("provider", "read", None, None, None, None)?;

    let stats = state
        .provider_metrics
        .get_stats(&provider_name)
        .await
        .map_err(|e| AdminError::Internal(format!("Failed to get provider stats: {}", e)))?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "No stats found for provider '{}' (no requests recorded or provider not configured)",
                provider_name
            ))
        })?;

    Ok(Json(stats))
}

/// Get historical statistics for a specific provider.
///
/// Returns time series data for the specified provider within the given
/// time range. Data is returned as hourly or daily buckets depending
/// on the granularity parameter.
///
/// **Note:** Historical stats require Prometheus to be configured via
/// `observability.metrics.prometheus_query_url`. In single-node deployments
/// without Prometheus, this endpoint returns an error.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/providers/{provider_name}/stats/history",
    tag = "providers",
    params(
        ("provider_name" = String, Path, description = "Provider name"),
        ProviderStatsHistoryQuery,
    ),
    responses(
        (status = 200, description = "Historical stats for the provider", body = ProviderStatsHistorical),
        (status = 400, description = "Invalid query parameters or Prometheus not configured"),
    )
))]
pub async fn get_provider_stats_history(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(provider_name): Path<String>,
    Query(query): Query<ProviderStatsHistoryQuery>,
) -> Result<Json<ProviderStatsHistorical>, AdminError> {
    authz.require("provider", "read", None, None, None, None)?;

    // Parse granularity (default to hour)
    let granularity = match query.granularity.as_deref() {
        Some("day") | Some("daily") => StatsGranularity::Day,
        Some("hour") | Some("hourly") | None => StatsGranularity::Hour,
        Some(other) => {
            return Err(AdminError::BadRequest(format!(
                "Invalid granularity '{}'. Must be 'hour' or 'day'",
                other
            )));
        }
    };

    // Default time range: last 24 hours
    let end = query.end.unwrap_or_else(Utc::now);
    let start = query.start.unwrap_or_else(|| end - Duration::hours(24));

    // Validate time range
    if start >= end {
        return Err(AdminError::BadRequest(
            "Start time must be before end time".to_string(),
        ));
    }

    // Validate max time range based on granularity to prevent expensive queries
    let range_duration = end - start;
    let (max_range, max_description) = match granularity {
        StatsGranularity::Hour => (Duration::days(30), "30 days"),
        StatsGranularity::Day => (Duration::days(365), "365 days"),
    };

    if range_duration > max_range {
        return Err(AdminError::BadRequest(format!(
            "Time range exceeds maximum for {} granularity ({}). \
             Use daily granularity for longer ranges.",
            granularity.prometheus_step(),
            max_description
        )));
    }

    // Return empty data if Prometheus is not configured (let frontend handle display)
    if !state.provider_metrics.has_prometheus() {
        return Ok(Json(ProviderStatsHistorical {
            provider: provider_name,
            granularity,
            data: vec![],
            prometheus_configured: false,
        }));
    }

    let historical = state
        .provider_metrics
        .get_historical(&provider_name, start, end, granularity)
        .await
        .map_err(|e| AdminError::Internal(format!("Failed to get historical stats: {}", e)))?;

    Ok(Json(historical))
}
