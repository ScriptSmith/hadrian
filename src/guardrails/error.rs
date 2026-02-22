//! Error types for guardrails evaluation.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

use super::{Category, ContentSource, Severity, Violation};

/// Result type for guardrails operations.
pub type GuardrailsResult<T> = Result<T, GuardrailsError>;

/// Errors that can occur during guardrails evaluation.
#[derive(Debug, Error)]
pub enum GuardrailsError {
    /// Content was blocked by guardrails policy.
    #[error("Content blocked: {reason}")]
    Blocked {
        /// Reason for blocking.
        reason: String,
        /// Violations that triggered the block.
        violations: Vec<Violation>,
        /// Where the blocked content came from (input or output).
        content_source: ContentSource,
    },

    /// Guardrails evaluation timed out.
    #[error("Guardrails evaluation timed out after {timeout_ms}ms")]
    Timeout {
        /// Timeout duration in milliseconds.
        timeout_ms: u64,
        /// Provider that timed out.
        provider: String,
    },

    /// Error communicating with guardrails provider.
    #[error("Provider error: {message}")]
    ProviderError {
        /// Error message.
        message: String,
        /// Provider that failed.
        provider: String,
        /// Whether this is a retryable error.
        retryable: bool,
        /// Underlying error (if any).
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Authentication error with guardrails provider.
    #[error("Authentication failed for provider {provider}: {message}")]
    AuthError {
        /// Error message.
        message: String,
        /// Provider that failed authentication.
        provider: String,
    },

    /// Rate limit exceeded on guardrails provider.
    #[error("Rate limit exceeded for provider {provider}")]
    RateLimited {
        /// Provider that rate limited.
        provider: String,
        /// Retry after (seconds), if provided.
        retry_after: Option<u64>,
    },

    /// Invalid configuration.
    #[error("Configuration error: {message}")]
    ConfigError {
        /// Error message.
        message: String,
    },

    /// Content parsing error (e.g., extracting text from messages).
    #[error("Failed to parse content: {message}")]
    ParseError {
        /// Error message.
        message: String,
    },

    /// Internal error.
    #[error("Internal error: {message}")]
    Internal {
        /// Error message.
        message: String,
    },
}

impl GuardrailsError {
    /// Creates a blocked error with a single violation.
    pub fn blocked(
        content_source: ContentSource,
        category: Category,
        severity: Severity,
        reason: impl Into<String>,
    ) -> Self {
        let reason = reason.into();
        Self::Blocked {
            reason: reason.clone(),
            violations: vec![Violation::new(category, severity, 1.0).with_message(reason)],
            content_source,
        }
    }

    /// Creates a blocked error with multiple violations.
    pub fn blocked_with_violations(
        content_source: ContentSource,
        reason: impl Into<String>,
        violations: Vec<Violation>,
    ) -> Self {
        Self::Blocked {
            reason: reason.into(),
            violations,
            content_source,
        }
    }

    /// Creates a timeout error.
    pub fn timeout(provider: impl Into<String>, timeout_ms: u64) -> Self {
        Self::Timeout {
            timeout_ms,
            provider: provider.into(),
        }
    }

    /// Creates a provider error.
    pub fn provider_error(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ProviderError {
            message: message.into(),
            provider: provider.into(),
            retryable: false,
            source: None,
        }
    }

    /// Creates a retryable provider error.
    pub fn retryable_error(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ProviderError {
            message: message.into(),
            provider: provider.into(),
            retryable: true,
            source: None,
        }
    }

    /// Creates a provider error from a reqwest error.
    pub fn from_reqwest(provider: impl Into<String>, err: reqwest::Error) -> Self {
        let retryable = err.is_timeout() || err.is_connect();
        Self::ProviderError {
            message: err.to_string(),
            provider: provider.into(),
            retryable,
            source: Some(Box::new(err)),
        }
    }

    /// Creates an authentication error.
    pub fn auth_error(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self::AuthError {
            message: message.into(),
            provider: provider.into(),
        }
    }

    /// Creates a rate limit error.
    pub fn rate_limited(provider: impl Into<String>, retry_after: Option<u64>) -> Self {
        Self::RateLimited {
            provider: provider.into(),
            retry_after,
        }
    }

    /// Creates a configuration error.
    pub fn config_error(message: impl Into<String>) -> Self {
        Self::ConfigError {
            message: message.into(),
        }
    }

    /// Creates a parse error.
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
        }
    }

    /// Creates an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Returns true if this error should result in blocking the request.
    pub fn is_blocking(&self) -> bool {
        matches!(self, GuardrailsError::Blocked { .. })
    }

    /// Returns true if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            GuardrailsError::Timeout { .. } => true,
            GuardrailsError::ProviderError { retryable, .. } => *retryable,
            GuardrailsError::RateLimited { .. } => true,
            _ => false,
        }
    }

    /// Returns the provider name if this error is provider-specific.
    pub fn provider(&self) -> Option<&str> {
        match self {
            GuardrailsError::Timeout { provider, .. } => Some(provider),
            GuardrailsError::ProviderError { provider, .. } => Some(provider),
            GuardrailsError::AuthError { provider, .. } => Some(provider),
            GuardrailsError::RateLimited { provider, .. } => Some(provider),
            _ => None,
        }
    }

    /// Returns the violations if this is a blocked error.
    pub fn violations(&self) -> Option<&[Violation]> {
        match self {
            GuardrailsError::Blocked { violations, .. } => Some(violations),
            _ => None,
        }
    }

    /// Returns an error code string for this error type.
    pub fn error_code(&self) -> &'static str {
        match self {
            GuardrailsError::Blocked { .. } => "guardrails_blocked",
            GuardrailsError::Timeout { .. } => "guardrails_timeout",
            GuardrailsError::ProviderError { .. } => "guardrails_provider_error",
            GuardrailsError::AuthError { .. } => "guardrails_auth_error",
            GuardrailsError::RateLimited { .. } => "guardrails_rate_limited",
            GuardrailsError::ConfigError { .. } => "guardrails_config_error",
            GuardrailsError::ParseError { .. } => "guardrails_parse_error",
            GuardrailsError::Internal { .. } => "guardrails_internal_error",
        }
    }

    /// Returns a short error type string for metrics.
    pub fn error_type_for_metrics(&self) -> &'static str {
        match self {
            GuardrailsError::Blocked { .. } => "blocked",
            GuardrailsError::Timeout { .. } => "timeout",
            GuardrailsError::ProviderError { .. } => "provider_error",
            GuardrailsError::AuthError { .. } => "auth",
            GuardrailsError::RateLimited { .. } => "rate_limited",
            GuardrailsError::ConfigError { .. } => "config",
            GuardrailsError::ParseError { .. } => "parse",
            GuardrailsError::Internal { .. } => "internal",
        }
    }
}

/// Error response body for guardrails errors.
#[derive(Debug, Serialize)]
struct GuardrailsErrorResponse {
    error: GuardrailsErrorBody,
}

#[derive(Debug, Serialize)]
struct GuardrailsErrorBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    violations: Option<Vec<ViolationSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_after: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ViolationSummary {
    category: String,
    severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl IntoResponse for GuardrailsError {
    fn into_response(self) -> Response {
        let (status, code, message, violations, provider, retry_after) = match &self {
            GuardrailsError::Blocked {
                reason, violations, ..
            } => {
                let violation_summaries: Vec<_> = violations
                    .iter()
                    .map(|v| ViolationSummary {
                        category: v.category.to_string(),
                        severity: v.severity.to_string(),
                        message: v.message.clone(),
                    })
                    .collect();

                (
                    StatusCode::BAD_REQUEST,
                    self.error_code(),
                    reason.clone(),
                    Some(violation_summaries),
                    None,
                    None,
                )
            }
            GuardrailsError::Timeout { provider, .. } => (
                StatusCode::GATEWAY_TIMEOUT,
                self.error_code(),
                self.to_string(),
                None,
                Some(provider.clone()),
                None,
            ),
            GuardrailsError::ProviderError { provider, .. } => (
                StatusCode::BAD_GATEWAY,
                self.error_code(),
                self.to_string(),
                None,
                Some(provider.clone()),
                None,
            ),
            GuardrailsError::AuthError { provider, .. } => (
                StatusCode::BAD_GATEWAY,
                self.error_code(),
                self.to_string(),
                None,
                Some(provider.clone()),
                None,
            ),
            GuardrailsError::RateLimited {
                provider,
                retry_after,
            } => (
                StatusCode::TOO_MANY_REQUESTS,
                self.error_code(),
                self.to_string(),
                None,
                Some(provider.clone()),
                *retry_after,
            ),
            GuardrailsError::ConfigError { .. } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                self.error_code(),
                self.to_string(),
                None,
                None,
                None,
            ),
            GuardrailsError::ParseError { .. } => (
                StatusCode::BAD_REQUEST,
                self.error_code(),
                self.to_string(),
                None,
                None,
                None,
            ),
            GuardrailsError::Internal { .. } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                self.error_code(),
                self.to_string(),
                None,
                None,
                None,
            ),
        };

        let body = GuardrailsErrorResponse {
            error: GuardrailsErrorBody {
                code,
                message,
                violations,
                provider,
                retry_after,
            },
        };

        let mut response = (status, Json(body)).into_response();

        // Add Retry-After header for rate limiting
        if let Some(retry_after) = retry_after
            && let Ok(value) = http::HeaderValue::try_from(retry_after.to_string())
        {
            response.headers_mut().insert("Retry-After", value);
        }

        // Add guardrails-specific headers
        response.headers_mut().insert(
            "X-Guardrails-Result",
            http::HeaderValue::from_static("blocked"),
        );

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocked_error() {
        let err = GuardrailsError::blocked(
            ContentSource::UserInput,
            Category::Hate,
            Severity::High,
            "Hate speech detected",
        );

        assert!(err.is_blocking());
        assert!(!err.is_retryable());
        assert!(err.violations().is_some());
        assert_eq!(err.violations().unwrap().len(), 1);
        assert_eq!(err.error_code(), "guardrails_blocked");
    }

    #[test]
    fn test_timeout_error() {
        let err = GuardrailsError::timeout("openai", 5000);

        assert!(!err.is_blocking());
        assert!(err.is_retryable());
        assert_eq!(err.provider(), Some("openai"));
        assert_eq!(err.error_code(), "guardrails_timeout");
    }

    #[test]
    fn test_provider_error_retryable() {
        let err = GuardrailsError::retryable_error("bedrock", "Connection failed");

        assert!(err.is_retryable());
        assert_eq!(err.provider(), Some("bedrock"));
    }

    #[test]
    fn test_provider_error_non_retryable() {
        let err = GuardrailsError::provider_error("azure", "Invalid response format");

        assert!(!err.is_retryable());
        assert_eq!(err.provider(), Some("azure"));
    }

    #[test]
    fn test_rate_limited_error() {
        let err = GuardrailsError::rate_limited("openai", Some(60));

        assert!(err.is_retryable());
        assert_eq!(err.provider(), Some("openai"));
        assert_eq!(err.error_code(), "guardrails_rate_limited");
    }

    #[test]
    fn test_config_error() {
        let err = GuardrailsError::config_error("Invalid guardrail_id");

        assert!(!err.is_blocking());
        assert!(!err.is_retryable());
        assert!(err.provider().is_none());
        assert_eq!(err.error_code(), "guardrails_config_error");
    }

    #[test]
    fn test_blocked_with_violations() {
        let violations = vec![
            Violation::new(Category::Hate, Severity::High, 0.95),
            Violation::new(Category::Violence, Severity::Medium, 0.7),
        ];

        let err = GuardrailsError::blocked_with_violations(
            ContentSource::UserInput,
            "Multiple violations detected",
            violations,
        );

        assert!(err.is_blocking());
        assert_eq!(err.violations().unwrap().len(), 2);
    }

    #[test]
    fn test_error_display() {
        let err = GuardrailsError::blocked(
            ContentSource::UserInput,
            Category::Hate,
            Severity::High,
            "Hate speech detected",
        );

        assert_eq!(err.to_string(), "Content blocked: Hate speech detected");
    }

    #[test]
    fn test_auth_error() {
        let err = GuardrailsError::auth_error("bedrock", "Invalid AWS credentials");

        assert!(!err.is_retryable());
        assert_eq!(err.provider(), Some("bedrock"));
        assert_eq!(err.error_code(), "guardrails_auth_error");
    }
}
