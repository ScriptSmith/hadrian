use std::fmt;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{observability::metrics, openapi::ErrorResponse};

#[derive(Debug)]
pub enum AuthError {
    /// No authentication credentials provided
    MissingCredentials,

    /// Credentials were provided but are invalid (generic — prevents enumeration)
    InvalidCredentials,

    /// Ambiguous credentials: both X-API-Key and Authorization headers provided
    AmbiguousCredentials,

    /// Invalid API key format
    InvalidApiKeyFormat,

    /// API key not found or revoked
    InvalidApiKey,

    /// API key has expired
    ExpiredApiKey,

    /// Required identity header missing
    #[allow(dead_code)] // Error variant for proxy auth
    MissingIdentity,

    /// Invalid JWT token
    InvalidToken,

    /// JWT token has expired
    ExpiredToken,

    /// Session not found or expired
    SessionNotFound,

    /// Session has expired
    SessionExpired,

    /// OIDC authentication required (redirect to IdP)
    OidcAuthRequired { redirect_url: String },

    /// Access forbidden (e.g., email domain not allowed)
    Forbidden(String),

    /// API key lacks required scope
    InsufficientScope {
        required: String,
        available: Vec<String>,
    },

    /// API key does not allow access to the requested model
    ModelNotAllowed {
        model: String,
        allowed_patterns: Vec<String>,
    },

    /// API key does not allow requests from this IP address
    IPNotAllowed { ip: String, allowlist: Vec<String> },

    /// Internal error during authentication
    Internal(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AuthError::MissingCredentials => (
                StatusCode::UNAUTHORIZED,
                "missing_credentials",
                "Authentication credentials required",
            ),
            AuthError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid authentication credentials",
            ),
            AuthError::AmbiguousCredentials => (
                StatusCode::BAD_REQUEST,
                "ambiguous_credentials",
                "Ambiguous credentials: provide either X-API-Key or Authorization header, not both",
            ),
            AuthError::InvalidApiKeyFormat => (
                StatusCode::UNAUTHORIZED,
                "invalid_api_key_format",
                "Invalid API key format",
            ),
            AuthError::InvalidApiKey | AuthError::ExpiredApiKey => (
                StatusCode::UNAUTHORIZED,
                "invalid_api_key",
                "Invalid or expired API key",
            ),
            AuthError::MissingIdentity => (
                StatusCode::UNAUTHORIZED,
                "missing_identity",
                "Identity header required",
            ),
            AuthError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "invalid_token",
                "Invalid authentication token",
            ),
            AuthError::ExpiredToken => (
                StatusCode::UNAUTHORIZED,
                "expired_token",
                "Authentication token has expired",
            ),
            AuthError::SessionNotFound => (
                StatusCode::UNAUTHORIZED,
                "session_not_found",
                "Session not found",
            ),
            AuthError::SessionExpired => (
                StatusCode::UNAUTHORIZED,
                "session_expired",
                "Session has expired",
            ),
            AuthError::OidcAuthRequired { redirect_url } => {
                // Return a 302 redirect to the IdP (not an error, just a redirect)
                return Response::builder()
                    .status(StatusCode::FOUND)
                    .header("Location", redirect_url.as_str())
                    .body(axum::body::Body::empty())
                    .unwrap();
            }
            AuthError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg.as_str()),
            AuthError::InsufficientScope {
                required,
                available: _,
            } => {
                metrics::record_gateway_error("auth_failure", "insufficient_scope", None);
                // Don't expose available scopes to clients (security: prevents enumeration)
                let message = format!("API key lacks required scope '{}'", required);
                let body =
                    ErrorResponse::with_type("permission_error", "insufficient_scope", message);
                return (StatusCode::FORBIDDEN, Json(body)).into_response();
            }
            AuthError::ModelNotAllowed {
                model,
                allowed_patterns: _,
            } => {
                metrics::record_gateway_error("auth_failure", "model_not_allowed", None);
                // Don't expose allowed patterns to clients (security: prevents enumeration)
                let message = format!("API key does not allow access to model '{}'", model);
                let body =
                    ErrorResponse::with_type("permission_error", "model_not_allowed", message);
                return (StatusCode::FORBIDDEN, Json(body)).into_response();
            }
            AuthError::IPNotAllowed { ip, allowlist: _ } => {
                metrics::record_gateway_error("auth_failure", "ip_not_allowed", None);
                // Don't expose IP allowlist to clients (security: reveals network infrastructure)
                let message = format!("API key does not allow requests from IP '{}'", ip);
                let body = ErrorResponse::with_type("permission_error", "ip_not_allowed", message);
                return (StatusCode::FORBIDDEN, Json(body)).into_response();
            }
            AuthError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                msg.as_str(),
            ),
        };

        // Record auth failure metric
        metrics::record_gateway_error("auth_failure", code, None);

        let body = ErrorResponse::with_type("authentication_error", code, message);
        (status, Json(body)).into_response()
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::MissingCredentials => write!(f, "Authentication credentials required"),
            AuthError::InvalidCredentials => {
                write!(f, "Invalid authentication credentials")
            }
            AuthError::AmbiguousCredentials => write!(
                f,
                "Ambiguous credentials: provide either X-API-Key or Authorization header, not both"
            ),
            AuthError::InvalidApiKeyFormat => write!(f, "Invalid API key format"),
            AuthError::InvalidApiKey | AuthError::ExpiredApiKey => {
                write!(f, "Invalid or expired API key")
            }
            AuthError::MissingIdentity => write!(f, "Identity header required"),
            AuthError::InvalidToken => write!(f, "Invalid authentication token"),
            AuthError::ExpiredToken => write!(f, "Authentication token has expired"),
            AuthError::SessionNotFound => write!(f, "Session not found"),
            AuthError::SessionExpired => write!(f, "Session has expired"),
            AuthError::OidcAuthRequired { redirect_url } => {
                write!(f, "OIDC authentication required: {}", redirect_url)
            }
            AuthError::Forbidden(msg) => write!(f, "Access forbidden: {}", msg),
            AuthError::InsufficientScope {
                required,
                available,
            } => {
                write!(
                    f,
                    "Insufficient scope: required '{}', available [{}]",
                    required,
                    available.join(", ")
                )
            }
            AuthError::ModelNotAllowed {
                model,
                allowed_patterns,
            } => {
                write!(
                    f,
                    "Model not allowed: '{}', allowed patterns [{}]",
                    model,
                    allowed_patterns.join(", ")
                )
            }
            AuthError::IPNotAllowed { ip, allowlist } => {
                write!(
                    f,
                    "IP not allowed: '{}', allowed [{}]",
                    ip,
                    allowlist.join(", ")
                )
            }
            AuthError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AuthError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ambiguous_credentials_error_code() {
        let error = AuthError::AmbiguousCredentials;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_ambiguous_credentials_display() {
        let error = AuthError::AmbiguousCredentials;
        let display = format!("{}", error);
        assert!(display.contains("Ambiguous credentials"));
        assert!(display.contains("X-API-Key"));
        assert!(display.contains("Authorization"));
    }

    #[test]
    fn test_missing_credentials_is_401() {
        let error = AuthError::MissingCredentials;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_invalid_api_key_format_is_401() {
        let error = AuthError::InvalidApiKeyFormat;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
