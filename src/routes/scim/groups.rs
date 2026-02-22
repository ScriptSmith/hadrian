//! SCIM 2.0 Group Resource Endpoints
//!
//! Implements RFC 7644 Section 3 CRUD operations for Group resources:
//! - POST /Groups: Create group
//! - GET /Groups: List/search groups
//! - GET /Groups/{id}: Get group by ID
//! - PUT /Groups/{id}: Replace group (full update)
//! - PATCH /Groups/{id}: Partial update
//! - DELETE /Groups/{id}: Delete group

use axum::{
    Extension,
    body::Body,
    extract::{Path, Query, State},
    http::{Request, StatusCode, header},
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use super::{middleware::ScimAuth, users::ScimJsonWithStatus};
use crate::{
    AppState,
    scim::{PatchRequest, ScimErrorResponse, ScimGroup, ScimListParams},
    services::ScimProvisioningService,
};

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
// Group Endpoints
// =============================================================================

/// List groups with optional filter and pagination.
///
/// `GET /scim/v2/Groups`
///
/// Query parameters:
/// - `filter`: SCIM filter expression (e.g., `displayName eq "Engineering"`)
/// - `startIndex`: 1-based pagination start (default: 1)
/// - `count`: Results per page (default: 100, max: 200)
#[tracing::instrument(
    name = "scim.groups.list",
    skip_all,
    fields(org_id = %scim_auth.org_id)
)]
pub async fn list_groups(
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
        .list_groups(scim_auth.org_id, &params, &base_url)
        .await
    {
        Ok(response) => ScimJsonWithStatus::ok(response).into_response(),
        Err(e) => e.into_response(),
    }
}

/// Create a new group.
///
/// `POST /scim/v2/Groups`
///
/// Creates a new group (team) in the organization with the provided attributes.
/// Returns 201 Created with the full group resource on success.
#[tracing::instrument(name = "scim.groups.create", skip_all, fields(org_id = %scim_auth.org_id))]
pub async fn create_group(
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

    let scim_group: ScimGroup = match serde_json::from_slice(&bytes) {
        Ok(g) => g,
        Err(e) => {
            return ScimErrorResponse::invalid_syntax(format!("Invalid JSON: {}", e))
                .into_response();
        }
    };

    match service
        .create_group(scim_auth.org_id, &scim_auth.config, &scim_group, &base_url)
        .await
    {
        Ok(created) => ScimJsonWithStatus::created(created).into_response(),
        Err(e) => ScimErrorResponse::from(e).into_response(),
    }
}

/// Get a group by ID.
///
/// `GET /scim/v2/Groups/{id}`
#[tracing::instrument(
    name = "scim.groups.get",
    skip_all,
    fields(org_id = %scim_auth.org_id, %id)
)]
pub async fn get_group(
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

    match service.get_group(scim_auth.org_id, id, &base_url).await {
        Ok(group) => ScimJsonWithStatus::ok(group).into_response(),
        Err(e) => ScimErrorResponse::from(e).into_response(),
    }
}

/// Replace a group (full update).
///
/// `PUT /scim/v2/Groups/{id}`
///
/// Replaces all group attributes with the provided values.
/// Group membership is fully synced to match the provided members list.
#[tracing::instrument(
    name = "scim.groups.replace",
    skip_all,
    fields(org_id = %scim_auth.org_id, %id)
)]
pub async fn replace_group(
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

    let scim_group: ScimGroup = match serde_json::from_slice(&bytes) {
        Ok(g) => g,
        Err(e) => {
            return ScimErrorResponse::invalid_syntax(format!("Invalid JSON: {}", e))
                .into_response();
        }
    };

    match service
        .replace_group(
            scim_auth.org_id,
            &scim_auth.config,
            id,
            &scim_group,
            &base_url,
        )
        .await
    {
        Ok(updated) => ScimJsonWithStatus::ok(updated).into_response(),
        Err(e) => ScimErrorResponse::from(e).into_response(),
    }
}

/// Partially update a group.
///
/// `PATCH /scim/v2/Groups/{id}`
///
/// Applies one or more patch operations to the group resource.
/// Supports add, replace, and remove operations per RFC 7644.
#[tracing::instrument(
    name = "scim.groups.patch",
    skip_all,
    fields(org_id = %scim_auth.org_id, %id)
)]
pub async fn patch_group(
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
        .patch_group(
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

/// Delete a group.
///
/// `DELETE /scim/v2/Groups/{id}`
///
/// Removes the SCIM group mapping. The underlying team is preserved
/// (soft delete semantics) to maintain data integrity for existing resources.
#[tracing::instrument(
    name = "scim.groups.delete",
    skip_all,
    fields(org_id = %scim_auth.org_id, %id)
)]
pub async fn delete_group(
    State(state): State<AppState>,
    Extension(scim_auth): Extension<ScimAuth>,
    Path(id): Path<Uuid>,
) -> Response {
    let service = match get_provisioning_service(&state) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    match service.delete_group(scim_auth.org_id, id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => ScimErrorResponse::from(e).into_response(),
    }
}
