//! SCIM 2.0 Discovery Endpoints
//!
//! Implements RFC 7644 Section 4 discovery endpoints:
//! - ServiceProviderConfig: Advertises service capabilities
//! - ResourceTypes: Lists supported resource types (User, Group)
//! - Schemas: Lists and retrieves schema definitions

use axum::{
    Extension,
    body::Body,
    extract::Path,
    http::{Request, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::middleware::ScimAuth;
use crate::scim::{
    ResourceType, SCHEMA_GROUP, SCHEMA_USER, ScimErrorResponse, ScimListResponse, ScimSchema,
    ServiceProviderConfig,
};

// =============================================================================
// Custom Response Type for SCIM Content-Type
// =============================================================================

/// SCIM JSON response with correct Content-Type.
///
/// All SCIM responses should use `application/scim+json` per RFC 7644.
pub struct ScimJson<T>(pub T);

impl<T: Serialize> IntoResponse for ScimJson<T> {
    fn into_response(self) -> Response {
        match serde_json::to_vec(&self.0) {
            Ok(body) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/scim+json")
                .body(Body::from(body))
                .unwrap(),
            Err(e) => {
                tracing::error!("Failed to serialize SCIM response: {}", e);
                ScimErrorResponse::internal("Failed to serialize response").into_response()
            }
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Extract the SCIM base URL from the request.
///
/// Uses X-Forwarded-* headers if present (for reverse proxy scenarios),
/// otherwise constructs from the Host header.
fn get_base_url(request: &Request<Body>) -> String {
    // Check for forwarded proto/host first (common in reverse proxy setups)
    let scheme = request
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("https");

    let host = request
        .headers()
        .get("x-forwarded-host")
        .or_else(|| request.headers().get(header::HOST))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");

    format!("{}://{}/scim/v2", scheme, host)
}

// =============================================================================
// ServiceProviderConfig Endpoint
// =============================================================================

/// Get SCIM Service Provider Configuration.
///
/// Returns the service's capabilities including supported features,
/// authentication schemes, and operational limits.
///
/// `GET /scim/v2/ServiceProviderConfig`
#[tracing::instrument(name = "scim.discovery.service_provider_config", skip_all)]
pub async fn service_provider_config(
    Extension(_scim_auth): Extension<ScimAuth>,
    request: Request<Body>,
) -> impl IntoResponse {
    let base_url = get_base_url(&request);

    let config = ServiceProviderConfig {
        documentation_uri: Some(format!("{}/../../docs/features/scim", base_url)),
        ..Default::default()
    };

    ScimJson(config)
}

// =============================================================================
// ResourceTypes Endpoints
// =============================================================================

/// List all supported resource types.
///
/// `GET /scim/v2/ResourceTypes`
#[tracing::instrument(name = "scim.discovery.resource_types", skip_all)]
pub async fn resource_types(
    Extension(_scim_auth): Extension<ScimAuth>,
    request: Request<Body>,
) -> impl IntoResponse {
    let base_url = get_base_url(&request);

    let resource_types = vec![
        ResourceType::user(&base_url),
        ResourceType::group(&base_url),
    ];

    ScimJson(ScimListResponse::new(resource_types, 2, 1))
}

/// Get a specific resource type by ID.
///
/// `GET /scim/v2/ResourceTypes/{id}`
#[tracing::instrument(name = "scim.discovery.resource_type", skip_all, fields(%id))]
pub async fn resource_type(
    Extension(_scim_auth): Extension<ScimAuth>,
    Path(id): Path<String>,
    request: Request<Body>,
) -> Response {
    let base_url = get_base_url(&request);

    match id.as_str() {
        "User" => ScimJson(ResourceType::user(&base_url)).into_response(),
        "Group" => ScimJson(ResourceType::group(&base_url)).into_response(),
        _ => {
            ScimErrorResponse::not_found(format!("ResourceType '{}' not found", id)).into_response()
        }
    }
}

// =============================================================================
// Schemas Endpoints
// =============================================================================

/// List all supported schemas.
///
/// `GET /scim/v2/Schemas`
#[tracing::instrument(name = "scim.discovery.schemas", skip_all)]
pub async fn schemas(
    Extension(_scim_auth): Extension<ScimAuth>,
    request: Request<Body>,
) -> impl IntoResponse {
    let base_url = get_base_url(&request);

    let schemas = vec![ScimSchema::user(&base_url), ScimSchema::group(&base_url)];

    ScimJson(ScimListResponse::new(schemas, 2, 1))
}

/// Get a specific schema by ID (URI).
///
/// `GET /scim/v2/Schemas/{id}`
///
/// Note: The schema ID is a URI. Axum's Path extractor automatically URL-decodes
/// the path segment, so clients can use either:
/// - `/Schemas/urn:ietf:params:scim:schemas:core:2.0:User` (raw)
/// - `/Schemas/urn%3Aietf%3Aparams%3Ascim%3Aschemas%3Acore%3A2.0%3AUser` (encoded)
#[tracing::instrument(name = "scim.discovery.schema", skip_all, fields(%id))]
pub async fn schema(
    Extension(_scim_auth): Extension<ScimAuth>,
    Path(id): Path<String>,
    request: Request<Body>,
) -> Response {
    let base_url = get_base_url(&request);

    // Axum's Path extractor automatically URL-decodes the path segment
    match id.as_str() {
        s if s == SCHEMA_USER => ScimJson(ScimSchema::user(&base_url)).into_response(),
        s if s == SCHEMA_GROUP => ScimJson(ScimSchema::group(&base_url)).into_response(),
        _ => ScimErrorResponse::not_found(format!("Schema '{}' not found", id)).into_response(),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_base_url_with_forwarded_headers() {
        let request = Request::builder()
            .header("x-forwarded-proto", "https")
            .header("x-forwarded-host", "api.example.com")
            .body(Body::empty())
            .unwrap();

        assert_eq!(get_base_url(&request), "https://api.example.com/scim/v2");
    }

    #[test]
    fn test_get_base_url_with_host_header() {
        let request = Request::builder()
            .header(header::HOST, "localhost:8080")
            .body(Body::empty())
            .unwrap();

        assert_eq!(get_base_url(&request), "https://localhost:8080/scim/v2");
    }

    #[test]
    fn test_get_base_url_default() {
        let request = Request::builder().body(Body::empty()).unwrap();

        assert_eq!(get_base_url(&request), "https://localhost/scim/v2");
    }

    #[test]
    fn test_service_provider_config_default() {
        let config = ServiceProviderConfig::default();

        // Verify key features are advertised correctly
        assert!(config.patch.supported);
        assert!(config.filter.supported);
        assert!(config.sort.supported);
        assert!(!config.bulk.supported);
        assert!(!config.change_password.supported);
        assert_eq!(config.authentication_schemes.len(), 1);
        assert_eq!(
            config.authentication_schemes[0].scheme_type,
            "oauthbearertoken"
        );
    }

    #[test]
    fn test_resource_type_user() {
        let rt = ResourceType::user("https://example.com/scim/v2");

        assert_eq!(rt.id, "User");
        assert_eq!(rt.name, "User");
        assert_eq!(rt.endpoint, "/Users");
        assert_eq!(rt.schema, SCHEMA_USER);
    }

    #[test]
    fn test_resource_type_group() {
        let rt = ResourceType::group("https://example.com/scim/v2");

        assert_eq!(rt.id, "Group");
        assert_eq!(rt.name, "Group");
        assert_eq!(rt.endpoint, "/Groups");
        assert_eq!(rt.schema, SCHEMA_GROUP);
    }

    #[test]
    fn test_schema_user() {
        let schema = ScimSchema::user("https://example.com/scim/v2");

        assert_eq!(schema.id, SCHEMA_USER);
        assert_eq!(schema.name, "User");
        assert!(!schema.attributes.is_empty());

        // Check that userName is required
        let user_name_attr = schema.attributes.iter().find(|a| a.name == "userName");
        assert!(user_name_attr.is_some());
        assert!(user_name_attr.unwrap().required);
    }

    #[test]
    fn test_schema_group() {
        let schema = ScimSchema::group("https://example.com/scim/v2");

        assert_eq!(schema.id, SCHEMA_GROUP);
        assert_eq!(schema.name, "Group");
        assert!(!schema.attributes.is_empty());

        // Check that displayName is required
        let display_name_attr = schema.attributes.iter().find(|a| a.name == "displayName");
        assert!(display_name_attr.is_some());
        assert!(display_name_attr.unwrap().required);
    }
}
