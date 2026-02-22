use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_valid::Valid;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{AuditActor, error::AdminError};
use crate::{
    AppState,
    db::{Cursor, CursorDirection, ListParams},
    middleware::{AdminAuth, AuthzContext},
    models::{CreateAuditLog, CreateOrganization, Organization, UpdateOrganization},
    openapi::PaginationMeta,
    services::{OrganizationService, Services},
};

/// Query parameters for list operations with cursor-based pagination.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
pub struct ListQuery {
    /// Maximum number of results to return
    pub limit: Option<i64>,
    /// Cursor for keyset pagination. Encoded as base64 string.
    #[cfg_attr(
        feature = "utoipa",
        schema(example = "MTczMzU4MDgwMDAwMDphYmMxMjM0NS02Nzg5LTAxMjMtNDU2Ny0wMTIzNDU2Nzg5YWI")
    )]
    pub cursor: Option<String>,
    /// Pagination direction: "forward" (default) or "backward".
    #[serde(default)]
    pub direction: Option<String>,
    /// Include soft-deleted records in results
    #[serde(default)]
    pub include_deleted: Option<bool>,
}

/// Simple conversion that requires using try_into_with_cursor() for cursor validation.
impl From<ListQuery> for ListParams {
    fn from(q: ListQuery) -> Self {
        ListParams {
            limit: q.limit,
            cursor: None,
            direction: CursorDirection::Forward,
            sort_order: Default::default(),
            include_deleted: q.include_deleted.unwrap_or(false),
        }
    }
}

impl ListQuery {
    /// Convert to ListParams with cursor support, returning an error for invalid cursors.
    pub fn try_into_with_cursor(self) -> Result<ListParams, AdminError> {
        let cursor = match &self.cursor {
            Some(c) => Some(
                Cursor::decode(c)
                    .map_err(|e| AdminError::BadRequest(format!("Invalid cursor: {}", e)))?,
            ),
            None => None,
        };

        let direction = match self.direction.as_deref() {
            Some("backward") => CursorDirection::Backward,
            Some("forward") | None => CursorDirection::Forward,
            Some(other) => {
                return Err(AdminError::BadRequest(format!(
                    "Invalid direction '{}': must be 'forward' or 'backward'",
                    other
                )));
            }
        };

        Ok(ListParams {
            limit: self.limit,
            cursor,
            direction,
            sort_order: Default::default(),
            include_deleted: self.include_deleted.unwrap_or(false),
        })
    }
}

/// Paginated list of organizations
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrganizationListResponse {
    /// List of organizations
    pub data: Vec<Organization>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_service(state: &AppState) -> Result<&OrganizationService, AdminError> {
    state
        .services
        .as_ref()
        .map(|s| &s.organizations)
        .ok_or(AdminError::ServicesRequired)
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Create an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations",
    tag = "organizations",
    operation_id = "organization_create",
    request_body = CreateOrganization,
    responses(
        (status = 201, description = "Organization created", body = Organization),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Conflict (slug already exists)", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Valid(Json(input)): Valid<Json<CreateOrganization>>,
) -> Result<(StatusCode, Json<Organization>), AdminError> {
    authz.require("organization", "create", None, None, None, None)?;

    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);
    let org = services.organizations.create(input).await?;

    // Log audit event
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "organization.create".to_string(),
            resource_type: "organization".to_string(),
            resource_id: org.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "slug": org.slug,
                "name": org.name,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok((StatusCode::CREATED, Json(org)))
}

/// Get an organization by slug
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{slug}",
    tag = "organizations",
    operation_id = "organization_get",
    params(("slug" = String, Path, description = "Organization slug")),
    responses(
        (status = 200, description = "Organization found", body = Organization),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(slug): Path<String>,
) -> Result<Json<Organization>, AdminError> {
    let service = get_service(&state)?;
    let org = service
        .get_by_slug(&slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", slug)))?;

    authz.require(
        "organization",
        "read",
        Some(&org.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    Ok(Json(org))
}

/// List all organizations
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations",
    tag = "organizations",
    operation_id = "organization_list",
    params(ListQuery),
    responses(
        (status = 200, description = "List of organizations", body = OrganizationListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Query(query): Query<ListQuery>,
) -> Result<Json<OrganizationListResponse>, AdminError> {
    authz.require("organization", "list", None, None, None, None)?;

    let service = get_service(&state)?;
    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = service.list(params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(OrganizationListResponse {
        data: result.items,
        pagination,
    }))
}

/// Update an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{slug}",
    tag = "organizations",
    operation_id = "organization_update",
    params(("slug" = String, Path, description = "Organization slug")),
    request_body = UpdateOrganization,
    responses(
        (status = 200, description = "Organization updated", body = Organization),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(slug): Path<String>,
    Valid(Json(input)): Valid<Json<UpdateOrganization>>,
) -> Result<Json<Organization>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // First get the org by slug to get its ID
    let org = services
        .organizations
        .get_by_slug(&slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", slug)))?;

    authz.require(
        "organization",
        "update",
        Some(&org.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Capture changes for audit log
    let changes = json!({
        "name": input.name,
    });

    let updated = services.organizations.update(org.id, input).await?;

    // Log audit event
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "organization.update".to_string(),
            resource_type: "organization".to_string(),
            resource_id: org.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "slug": org.slug,
                "changes": changes,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(updated))
}

/// Delete an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{slug}",
    tag = "organizations",
    operation_id = "organization_delete",
    params(("slug" = String, Path, description = "Organization slug")),
    responses(
        (status = 200, description = "Organization deleted"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(slug): Path<String>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // First get the org by slug to get its ID
    let org = services
        .organizations
        .get_by_slug(&slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", slug)))?;

    authz.require(
        "organization",
        "delete",
        Some(&org.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    services.organizations.delete(org.id).await?;

    // Clean up registry caches for this organization to prevent memory leaks
    if let Some(registry) = &state.policy_registry {
        registry.remove_org(org.id).await;
    }
    #[cfg(feature = "sso")]
    if let Some(registry) = &state.oidc_registry {
        registry.remove(org.id).await;
    }
    #[cfg(feature = "saml")]
    if let Some(registry) = &state.saml_registry {
        registry.remove(org.id).await;
    }

    // Log audit event
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "organization.delete".to_string(),
            resource_type: "organization".to_string(),
            resource_id: org.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "slug": org.slug,
                "name": org.name,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(()))
}
