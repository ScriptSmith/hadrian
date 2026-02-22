//! Provider health check types and utilities.
//!
//! Health checks allow proactive monitoring of provider availability,
//! rather than only reacting to failures via circuit breakers.
//!
//! Configuration types (`ProviderHealthCheckConfig`, `ProviderHealthCheckMode`) are defined in
//! `crate::config::providers` and re-exported here for convenience.

use serde::{Deserialize, Serialize};

// Re-export configuration types from the config module
pub use crate::config::{ProviderHealthCheckConfig, ProviderHealthCheckMode};

/// Status of a provider's health.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// Provider is healthy and responding.
    Healthy,
    /// Provider is unhealthy (failed health check).
    Unhealthy,
    /// Health status is unknown (not yet checked or checks disabled).
    #[default]
    Unknown,
}

/// Result of a health check operation.
#[derive(Debug, Clone, Serialize)]
pub struct HealthCheckResult {
    /// Health status.
    pub status: HealthStatus,

    /// Latency of the health check in milliseconds.
    pub latency_ms: u64,

    /// Error message if the health check failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// HTTP status code from the health check request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

impl HealthCheckResult {
    /// Create a healthy result.
    pub fn healthy(latency_ms: u64, status_code: u16) -> Self {
        Self {
            status: HealthStatus::Healthy,
            latency_ms,
            error: None,
            status_code: Some(status_code),
        }
    }

    /// Create an unhealthy result with an error message.
    pub fn unhealthy(latency_ms: u64, error: impl Into<String>, status_code: Option<u16>) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            latency_ms,
            error: Some(error.into()),
            status_code,
        }
    }

    /// Create an unhealthy result from an error.
    pub fn from_error(latency_ms: u64, error: &impl std::error::Error) -> Self {
        Self::unhealthy(latency_ms, error.to_string(), None)
    }

    /// Check if the result indicates a healthy provider.
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_result_healthy() {
        let result = HealthCheckResult::healthy(150, 200);
        assert!(result.is_healthy());
        assert_eq!(result.latency_ms, 150);
        assert!(result.error.is_none());
        assert_eq!(result.status_code, Some(200));
    }

    #[test]
    fn test_health_check_result_unhealthy() {
        let result = HealthCheckResult::unhealthy(500, "Connection refused", None);
        assert!(!result.is_healthy());
        assert_eq!(result.latency_ms, 500);
        assert_eq!(result.error, Some("Connection refused".to_string()));
        assert!(result.status_code.is_none());
    }

    #[test]
    fn test_health_status_serde() {
        let status: HealthStatus = serde_json::from_str("\"healthy\"").unwrap();
        assert_eq!(status, HealthStatus::Healthy);

        let status: HealthStatus = serde_json::from_str("\"unhealthy\"").unwrap();
        assert_eq!(status, HealthStatus::Unhealthy);

        let json = serde_json::to_string(&HealthStatus::Unknown).unwrap();
        assert_eq!(json, "\"unknown\"");
    }
}
