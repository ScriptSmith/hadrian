//! SCIM Bearer Token Authentication Middleware
//!
//! This middleware authenticates SCIM API requests using Bearer tokens.
//! Each organization has a unique SCIM bearer token stored hashed in the database.

use axum::{
    body::Body,
    extract::State,
    http::{Request, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::{AppState, models::OrgScimConfig, scim::ScimErrorResponse};

/// Authenticated SCIM context injected into request extensions.
///
/// Used by SCIM resource endpoints (Phase 4-5) to access the authenticated
/// organization's configuration and ID.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used by User/Group endpoints in Phase 4-5
pub struct ScimAuth {
    /// The organization's SCIM configuration
    pub config: OrgScimConfig,
    /// The organization ID (convenience field, same as config.org_id)
    pub org_id: Uuid,
}

/// SCIM bearer token authentication middleware.
///
/// Extracts the bearer token from the Authorization header, validates it
/// against the database, and injects `ScimAuth` into request extensions.
///
/// Returns RFC 7644 compliant error responses on authentication failure.
pub async fn scim_auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    // Extract bearer token from Authorization header
    let token = match extract_bearer_token(&request) {
        Some(token) => token,
        None => {
            return ScimErrorResponse::unauthorized(
                "Missing or invalid Authorization header. Expected: Bearer <token>",
            )
            .into_response();
        }
    };

    // Get services - SCIM requires database to be configured
    let services = match &state.services {
        Some(s) => s,
        None => {
            tracing::error!("SCIM request received but database is not configured");
            return ScimErrorResponse::internal("SCIM service is not available").into_response();
        }
    };

    // Authenticate the token
    let config_with_hash = match services.scim_configs.authenticate_token(token).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::debug!("SCIM authentication failed: invalid token");
            return ScimErrorResponse::unauthorized("Invalid SCIM bearer token").into_response();
        }
        Err(e) => {
            tracing::error!("SCIM authentication error: {}", e);
            return ScimErrorResponse::internal("Authentication service error").into_response();
        }
    };

    // Check if SCIM is enabled for this organization
    if !config_with_hash.config.enabled {
        tracing::debug!(
            org_id = %config_with_hash.config.org_id,
            "SCIM authentication failed: SCIM is disabled for organization"
        );
        return ScimErrorResponse::forbidden("SCIM is disabled for this organization")
            .into_response();
    }

    // Inject ScimAuth into request extensions
    let scim_auth = ScimAuth {
        org_id: config_with_hash.config.org_id,
        config: config_with_hash.config,
    };

    request.extensions_mut().insert(scim_auth);

    next.run(request).await
}

/// Extract bearer token from the Authorization header.
///
/// Expects format: `Authorization: Bearer <token>`
/// Returns the token portion (may be empty if no token provided after "Bearer ").
/// Token validation happens in `authenticate_token()`.
fn extract_bearer_token(request: &Request<Body>) -> Option<&str> {
    let auth_header = request.headers().get(header::AUTHORIZATION)?;
    let auth_str = auth_header.to_str().ok()?;

    // Case-insensitive "Bearer " prefix check (7 chars for "Bearer ")
    if auth_str.len() >= 7 && auth_str[..7].eq_ignore_ascii_case("Bearer ") {
        Some(&auth_str[7..])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bearer_token_valid() {
        let request = Request::builder()
            .header(header::AUTHORIZATION, "Bearer scim_abc123")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_bearer_token(&request), Some("scim_abc123"));
    }

    #[test]
    fn test_extract_bearer_token_case_insensitive() {
        let request = Request::builder()
            .header(header::AUTHORIZATION, "bearer scim_abc123")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_bearer_token(&request), Some("scim_abc123"));

        let request2 = Request::builder()
            .header(header::AUTHORIZATION, "BEARER scim_abc123")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_bearer_token(&request2), Some("scim_abc123"));
    }

    #[test]
    fn test_extract_bearer_token_missing_header() {
        let request = Request::builder().body(Body::empty()).unwrap();

        assert_eq!(extract_bearer_token(&request), None);
    }

    #[test]
    fn test_extract_bearer_token_wrong_scheme() {
        let request = Request::builder()
            .header(header::AUTHORIZATION, "Basic dXNlcjpwYXNz")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_bearer_token(&request), None);
    }

    #[test]
    fn test_extract_bearer_token_empty_token() {
        let request = Request::builder()
            .header(header::AUTHORIZATION, "Bearer ")
            .body(Body::empty())
            .unwrap();

        // Returns empty string (valid extraction, validation happens elsewhere)
        assert_eq!(extract_bearer_token(&request), Some(""));
    }
}
