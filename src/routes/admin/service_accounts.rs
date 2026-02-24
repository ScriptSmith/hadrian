use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_valid::Valid;
use serde::Serialize;
use serde_json::json;

use super::{AuditActor, error::AdminError, organizations::ListQuery};
use crate::{
    AppState,
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    models::{CreateAuditLog, CreateServiceAccount, ServiceAccount, UpdateServiceAccount},
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of service accounts
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ServiceAccountListResponse {
    /// List of service accounts
    pub data: Vec<ServiceAccount>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

// ============================================================================
// Service Account CRUD endpoints
// ============================================================================

/// Create a service account in an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/service-accounts",
    tag = "service_accounts",
    operation_id = "service_account_create",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = CreateServiceAccount,
    responses(
        (status = 201, description = "Service account created", body = ServiceAccount),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Conflict (slug already exists)", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.service_accounts.create", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<CreateServiceAccount>>,
) -> Result<(StatusCode, Json<ServiceAccount>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug to get its ID
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require org admin permission to create service accounts
    authz.require(
        "service_account",
        "create",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let sa = services
        .service_accounts
        .create(org.id, input.clone())
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "service_account.create".to_string(),
            resource_type: "service_account".to_string(),
            resource_id: sa.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "name": sa.name,
                "slug": sa.slug,
                "roles": input.roles,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(sa)))
}

/// Get a service account by slug
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/service-accounts/{sa_slug}",
    tag = "service_accounts",
    operation_id = "service_account_get",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("sa_slug" = String, Path, description = "Service account slug"),
    ),
    responses(
        (status = 200, description = "Service account found", body = ServiceAccount),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or service account not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.service_accounts.get", skip(state, authz), fields(%org_slug, %sa_slug))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, sa_slug)): Path<(String, String)>,
) -> Result<Json<ServiceAccount>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    let sa = services
        .service_accounts
        .get_by_slug(org.id, &sa_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Service account '{}' not found in organization '{}'",
                sa_slug, org_slug
            ))
        })?;

    // Require read permission on the service account
    authz.require(
        "service_account",
        "read",
        Some(&sa.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    Ok(Json(sa))
}

/// List service accounts in an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/service-accounts",
    tag = "service_accounts",
    operation_id = "service_account_list",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of service accounts", body = ServiceAccountListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.service_accounts.list", skip(state, authz, query), fields(%org_slug))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ServiceAccountListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on the org to list service accounts
    authz.require(
        "service_account",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .service_accounts
        .list_by_org(org.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(ServiceAccountListResponse {
        data: result.items,
        pagination,
    }))
}

/// Update a service account
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{org_slug}/service-accounts/{sa_slug}",
    tag = "service_accounts",
    operation_id = "service_account_update",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("sa_slug" = String, Path, description = "Service account slug"),
    ),
    request_body = UpdateServiceAccount,
    responses(
        (status = 200, description = "Service account updated", body = ServiceAccount),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or service account not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.service_accounts.update", skip(state, admin_auth, authz, input), fields(%org_slug, %sa_slug))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, sa_slug)): Path<(String, String)>,
    Valid(Json(input)): Valid<Json<UpdateServiceAccount>>,
) -> Result<Json<ServiceAccount>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get service account by slug
    let sa = services
        .service_accounts
        .get_by_slug(org.id, &sa_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Service account '{}' not found in organization '{}'",
                sa_slug, org_slug
            ))
        })?;

    // Require update permission on the service account
    authz.require(
        "service_account",
        "update",
        Some(&sa.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Capture changes for audit log
    let changes = json!({
        "name": input.name,
        "description": input.description,
        "roles": input.roles,
    });

    // Track whether roles are being updated for cache invalidation
    let roles_updated = input.roles.is_some();

    let updated = services.service_accounts.update(sa.id, input).await?;

    // If roles were updated, invalidate API key caches for this service account.
    // API keys cache the service account's roles, so stale caches could grant
    // incorrect permissions.
    if roles_updated && let Some(cache) = &state.cache {
        match services
            .api_keys
            .get_key_hashes_by_service_account(sa.id)
            .await
        {
            Ok(key_hashes) => {
                for hash in key_hashes {
                    let cache_key = crate::cache::CacheKeys::api_key(&hash);
                    let _ = cache.delete(&cache_key).await;
                }
                tracing::debug!(
                    service_account_id = %sa.id,
                    "Invalidated API key caches after service account role update"
                );
            }
            Err(e) => {
                // Log but don't fail the request - cache will expire eventually
                tracing::warn!(
                    error = %e,
                    service_account_id = %sa.id,
                    "Failed to invalidate API key caches after role update"
                );
            }
        }
    }

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "service_account.update".to_string(),
            resource_type: "service_account".to_string(),
            resource_id: sa.id,
            org_id: Some(org.id),
            project_id: None,
            details: changes,
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(updated))
}

/// Delete a service account
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/service-accounts/{sa_slug}",
    tag = "service_accounts",
    operation_id = "service_account_delete",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("sa_slug" = String, Path, description = "Service account slug"),
    ),
    responses(
        (status = 200, description = "Service account deleted"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or service account not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.service_accounts.delete", skip(state, admin_auth, authz), fields(%org_slug, %sa_slug))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, sa_slug)): Path<(String, String)>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get service account by slug
    let sa = services
        .service_accounts
        .get_by_slug(org.id, &sa_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Service account '{}' not found in organization '{}'",
                sa_slug, org_slug
            ))
        })?;

    // Require delete permission on the service account (typically org admin)
    authz.require(
        "service_account",
        "delete",
        Some(&sa.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Capture details for audit log before deletion
    let sa_id = sa.id;
    let sa_name = sa.name.clone();
    let sa_slug_val = sa.slug.clone();

    // Delete service account and revoke all its API keys atomically.
    // Uses row locking to prevent race conditions.
    let revoked_api_key_ids = services
        .service_accounts
        .delete_with_api_key_revocation(sa_id)
        .await?;

    tracing::info!(
        service_account_id = %sa_id,
        revoked_count = revoked_api_key_ids.len(),
        ?revoked_api_key_ids,
        "Deleted service account and revoked API keys"
    );

    // Log audit event with full list of revoked API key IDs for forensics
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "service_account.delete".to_string(),
            resource_type: "service_account".to_string(),
            resource_id: sa_id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "name": sa_name,
                "slug": sa_slug_val,
                "revoked_api_key_count": revoked_api_key_ids.len(),
                "revoked_api_key_ids": revoked_api_key_ids,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}
