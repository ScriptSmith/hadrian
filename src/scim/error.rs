//! SCIM 2.0 Error Types
//!
//! This module defines SCIM-specific error responses per RFC 7644 Section 3.12.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use super::types::SCHEMA_ERROR;

/// SCIM error response per RFC 7644.
///
/// All SCIM errors are returned in this format with appropriate HTTP status codes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimErrorResponse {
    /// SCIM schema URIs (always contains the Error schema)
    pub schemas: Vec<String>,

    /// HTTP status code as a string (e.g., "400", "404")
    pub status: String,

    /// SCIM-specific error type (optional, per RFC 7644)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scim_type: Option<ScimErrorType>,

    /// Human-readable error detail
    pub detail: String,
}

impl ScimErrorResponse {
    /// Create a new SCIM error
    fn new(
        status: StatusCode,
        scim_type: Option<ScimErrorType>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            schemas: vec![SCHEMA_ERROR.to_string()],
            status: status.as_u16().to_string(),
            scim_type,
            detail: detail.into(),
        }
    }

    /// Invalid filter syntax error (400)
    pub fn invalid_filter(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            Some(ScimErrorType::InvalidFilter),
            detail,
        )
    }

    /// Invalid JSON syntax error (400)
    pub fn invalid_syntax(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            Some(ScimErrorType::InvalidSyntax),
            detail,
        )
    }

    /// No target for PATCH remove operation (400)
    pub fn no_target(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            Some(ScimErrorType::NoTarget),
            detail,
        )
    }

    /// Attempt to modify immutable or read-only attribute (400)
    pub fn mutability(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            Some(ScimErrorType::Mutability),
            detail,
        )
    }

    /// Invalid attribute value (400)
    pub fn invalid_value(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            Some(ScimErrorType::InvalidValue),
            detail,
        )
    }

    /// Generic bad request without specific scimType (400)
    pub fn bad_request(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, None, detail)
    }

    /// Authentication required (401)
    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, None, detail)
    }

    /// Permission denied (403)
    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, None, detail)
    }

    /// Resource not found (404)
    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, None, detail)
    }

    /// Uniqueness constraint violation (409)
    pub fn uniqueness(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::CONFLICT,
            Some(ScimErrorType::Uniqueness),
            detail,
        )
    }

    /// Request too large (413)
    pub fn too_many(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            Some(ScimErrorType::TooMany),
            detail,
        )
    }

    /// Internal server error (500)
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, None, detail)
    }

    /// Get the HTTP status code
    pub fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status.parse().unwrap_or(500))
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl IntoResponse for ScimErrorResponse {
    fn into_response(self) -> Response {
        let status = self.status_code();
        (status, Json(self)).into_response()
    }
}

/// SCIM error types per RFC 7644 Section 3.12.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum ScimErrorType {
    /// Filter syntax is invalid or unsupported
    InvalidFilter,

    /// Request body has invalid JSON syntax
    InvalidSyntax,

    /// PATCH remove operation missing required path
    NoTarget,

    /// Attempt to modify read-only or immutable attribute
    Mutability,

    /// Uniqueness constraint violated (e.g., duplicate userName)
    Uniqueness,

    /// Attribute value is invalid for its type
    InvalidValue,

    /// Request payload too large
    TooMany,
}

impl std::fmt::Display for ScimErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScimErrorType::InvalidFilter => write!(f, "invalidFilter"),
            ScimErrorType::InvalidSyntax => write!(f, "invalidSyntax"),
            ScimErrorType::NoTarget => write!(f, "noTarget"),
            ScimErrorType::Mutability => write!(f, "mutability"),
            ScimErrorType::Uniqueness => write!(f, "uniqueness"),
            ScimErrorType::InvalidValue => write!(f, "invalidValue"),
            ScimErrorType::TooMany => write!(f, "tooMany"),
        }
    }
}

/// Result type for SCIM operations
pub type ScimResult<T> = Result<T, ScimErrorResponse>;

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scim_error_invalid_filter() {
        let err = ScimErrorResponse::invalid_filter("The filter syntax is invalid");

        assert_eq!(err.status, "400");
        assert_eq!(err.scim_type, Some(ScimErrorType::InvalidFilter));
        assert!(err.detail.contains("filter"));

        let json = serde_json::to_string_pretty(&err).unwrap();
        assert!(json.contains("\"scimType\": \"invalidFilter\""));
        assert!(json.contains("\"status\": \"400\""));
    }

    #[test]
    fn test_scim_error_not_found() {
        let err = ScimErrorResponse::not_found("User with id '12345' not found");

        assert_eq!(err.status, "404");
        assert_eq!(err.scim_type, None);

        let json = serde_json::to_string_pretty(&err).unwrap();
        assert!(!json.contains("scimType")); // Should be omitted
    }

    #[test]
    fn test_scim_error_uniqueness() {
        let err =
            ScimErrorResponse::uniqueness("User with userName 'john@example.com' already exists");

        assert_eq!(err.status, "409");
        assert_eq!(err.scim_type, Some(ScimErrorType::Uniqueness));
    }

    #[test]
    fn test_scim_error_status_code() {
        assert_eq!(
            ScimErrorResponse::bad_request("test").status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ScimErrorResponse::not_found("test").status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ScimErrorResponse::uniqueness("test").status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            ScimErrorResponse::internal("test").status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_scim_error_type_display() {
        assert_eq!(format!("{}", ScimErrorType::InvalidFilter), "invalidFilter");
        assert_eq!(format!("{}", ScimErrorType::Uniqueness), "uniqueness");
    }
}
