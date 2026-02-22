//! SCIM 2.0 User Resource Endpoints
//!
//! Implements RFC 7644 Section 3 CRUD operations for User resources:
//! - POST /Users: Create user
//! - GET /Users: List/search users
//! - GET /Users/{id}: Get user by ID
//! - PUT /Users/{id}: Replace user (full update)
//! - PATCH /Users/{id}: Partial update
//! - DELETE /Users/{id}: Delete/deactivate user

use axum::{
    Extension,
    body::Body,
    extract::{Path, Query, State},
    http::{Request, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use uuid::Uuid;

use super::middleware::ScimAuth;
use crate::{
    AppState,
    scim::{PatchRequest, ScimErrorResponse, ScimListParams, ScimUser},
    services::ScimProvisioningService,
};

// =============================================================================
// Custom Response Type for SCIM Content-Type
// =============================================================================

/// SCIM JSON response with correct Content-Type and status code.
pub struct ScimJsonWithStatus<T> {
    body: T,
    status: StatusCode,
}

impl<T: Serialize> ScimJsonWithStatus<T> {
    pub fn ok(body: T) -> Self {
        Self {
            body,
            status: StatusCode::OK,
        }
    }

    pub fn created(body: T) -> Self {
        Self {
            body,
            status: StatusCode::CREATED,
        }
    }
}

impl<T: Serialize> IntoResponse for ScimJsonWithStatus<T> {
    fn into_response(self) -> Response {
        match serde_json::to_vec(&self.body) {
            Ok(body) => Response::builder()
                .status(self.status)
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
fn get_base_url(request: &Request<Body>) -> String {
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

/// Get the provisioning service or return an error.
fn get_provisioning_service(
    state: &AppState,
) -> Result<&ScimProvisioningService, ScimErrorResponse> {
    state
        .services
        .as_ref()
        .map(|s| &s.scim_provisioning)
        .ok_or_else(|| ScimErrorResponse::internal("SCIM service is not available"))
}

// =============================================================================
// User Endpoints
// =============================================================================

/// List users with optional filter and pagination.
///
/// `GET /scim/v2/Users`
///
/// Query parameters:
/// - `filter`: SCIM filter expression (e.g., `userName eq "john@example.com"`)
/// - `startIndex`: 1-based pagination start (default: 1)
/// - `count`: Results per page (default: 100, max: 200)
#[tracing::instrument(
    name = "scim.users.list",
    skip_all,
    fields(org_id = %scim_auth.org_id)
)]
pub async fn list_users(
    State(state): State<AppState>,
    Extension(scim_auth): Extension<ScimAuth>,
    Query(params): Query<ScimListParams>,
    request: Request<Body>,
) -> Response {
    let base_url = get_base_url(&request);
    let service = match get_provisioning_service(&state) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    match service
        .list_users(scim_auth.org_id, &params, &base_url)
        .await
    {
        Ok(response) => ScimJsonWithStatus::ok(response).into_response(),
        Err(e) => e.into_response(),
    }
}

/// Create a new user.
///
/// `POST /scim/v2/Users`
///
/// Creates a new user in the organization with the provided attributes.
/// Returns 201 Created with the full user resource on success.
#[tracing::instrument(name = "scim.users.create", skip_all, fields(org_id = %scim_auth.org_id))]
pub async fn create_user(
    State(state): State<AppState>,
    Extension(scim_auth): Extension<ScimAuth>,
    request: Request<Body>,
) -> Response {
    let base_url = get_base_url(&request);
    let service = match get_provisioning_service(&state) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    // Parse the request body
    let bytes = match axum::body::to_bytes(request.into_body(), 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return ScimErrorResponse::invalid_syntax(format!(
                "Failed to read request body: {}",
                e
            ))
            .into_response();
        }
    };

    let scim_user: ScimUser = match serde_json::from_slice(&bytes) {
        Ok(u) => u,
        Err(e) => {
            return ScimErrorResponse::invalid_syntax(format!("Invalid JSON: {}", e))
                .into_response();
        }
    };

    match service
        .create_user(scim_auth.org_id, &scim_auth.config, &scim_user, &base_url)
        .await
    {
        Ok(created) => ScimJsonWithStatus::created(created).into_response(),
        Err(e) => ScimErrorResponse::from(e).into_response(),
    }
}

/// Get a user by ID.
///
/// `GET /scim/v2/Users/{id}`
#[tracing::instrument(
    name = "scim.users.get",
    skip_all,
    fields(org_id = %scim_auth.org_id, %id)
)]
pub async fn get_user(
    State(state): State<AppState>,
    Extension(scim_auth): Extension<ScimAuth>,
    Path(id): Path<Uuid>,
    request: Request<Body>,
) -> Response {
    let base_url = get_base_url(&request);
    let service = match get_provisioning_service(&state) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    match service.get_user(scim_auth.org_id, id, &base_url).await {
        Ok(user) => ScimJsonWithStatus::ok(user).into_response(),
        Err(e) => ScimErrorResponse::from(e).into_response(),
    }
}

/// Replace a user (full update).
///
/// `PUT /scim/v2/Users/{id}`
///
/// Replaces all user attributes with the provided values.
/// Attributes not included in the request are set to their default values.
#[tracing::instrument(
    name = "scim.users.replace",
    skip_all,
    fields(org_id = %scim_auth.org_id, %id)
)]
pub async fn replace_user(
    State(state): State<AppState>,
    Extension(scim_auth): Extension<ScimAuth>,
    Path(id): Path<Uuid>,
    request: Request<Body>,
) -> Response {
    let base_url = get_base_url(&request);
    let service = match get_provisioning_service(&state) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    // Parse the request body
    let bytes = match axum::body::to_bytes(request.into_body(), 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return ScimErrorResponse::invalid_syntax(format!(
                "Failed to read request body: {}",
                e
            ))
            .into_response();
        }
    };

    let scim_user: ScimUser = match serde_json::from_slice(&bytes) {
        Ok(u) => u,
        Err(e) => {
            return ScimErrorResponse::invalid_syntax(format!("Invalid JSON: {}", e))
                .into_response();
        }
    };

    match service
        .replace_user(
            scim_auth.org_id,
            &scim_auth.config,
            id,
            &scim_user,
            &base_url,
        )
        .await
    {
        Ok(updated) => ScimJsonWithStatus::ok(updated).into_response(),
        Err(e) => ScimErrorResponse::from(e).into_response(),
    }
}

/// Partially update a user.
///
/// `PATCH /scim/v2/Users/{id}`
///
/// Applies one or more patch operations to the user resource.
/// Supports add, replace, and remove operations per RFC 7644.
#[tracing::instrument(
    name = "scim.users.patch",
    skip_all,
    fields(org_id = %scim_auth.org_id, %id)
)]
pub async fn patch_user(
    State(state): State<AppState>,
    Extension(scim_auth): Extension<ScimAuth>,
    Path(id): Path<Uuid>,
    request: Request<Body>,
) -> Response {
    let base_url = get_base_url(&request);
    let service = match get_provisioning_service(&state) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    // Parse the request body
    let bytes = match axum::body::to_bytes(request.into_body(), 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return ScimErrorResponse::invalid_syntax(format!(
                "Failed to read request body: {}",
                e
            ))
            .into_response();
        }
    };

    let patch_request: PatchRequest = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(e) => {
            return ScimErrorResponse::invalid_syntax(format!("Invalid JSON: {}", e))
                .into_response();
        }
    };

    match service
        .patch_user(
            scim_auth.org_id,
            &scim_auth.config,
            id,
            &patch_request,
            &base_url,
        )
        .await
    {
        Ok(updated) => ScimJsonWithStatus::ok(updated).into_response(),
        Err(e) => ScimErrorResponse::from(e).into_response(),
    }
}

/// Delete a user.
///
/// `DELETE /scim/v2/Users/{id}`
///
/// Behavior depends on organization SCIM configuration:
/// - `deactivate_deletes_user: true`: Hard deletes the user
/// - `deactivate_deletes_user: false`: Sets user as inactive (soft delete)
///
/// If `revoke_api_keys_on_deactivate` is enabled, also revokes all user API keys.
#[tracing::instrument(
    name = "scim.users.delete",
    skip_all,
    fields(org_id = %scim_auth.org_id, %id)
)]
pub async fn delete_user(
    State(state): State<AppState>,
    Extension(scim_auth): Extension<ScimAuth>,
    Path(id): Path<Uuid>,
) -> Response {
    let service = match get_provisioning_service(&state) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    match service
        .delete_user(scim_auth.org_id, &scim_auth.config, id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => ScimErrorResponse::from(e).into_response(),
    }
}
