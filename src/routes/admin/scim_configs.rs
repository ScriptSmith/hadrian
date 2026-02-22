//! Admin API endpoints for per-organization SCIM configuration.
//!
//! Each organization can have at most one SCIM configuration, enabling IT admins
//! to configure SCIM 2.0 provisioning from identity providers (Okta, Azure AD, etc.)
//! via the Admin UI.

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use axum_valid::Valid;
use serde_json::json;

use super::{AuditActor, error::AdminError};
use crate::{
    AppState,
    middleware::{AdminAuth, AuthzContext},
    models::{CreateAuditLog, CreateOrgScimConfig, CreatedOrgScimConfig, UpdateOrgScimConfig},
    services::Services,
};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

// ============================================================================
// Organization SCIM Config CRUD endpoints
// ============================================================================

/// Get the SCIM configuration for an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/scim-config",
    tag = "scim",
    operation_id = "org_scim_config_get",
    params(("org_slug" = String, Path, description = "Organization slug")),
    responses(
        (status = 200, description = "SCIM config found", body = crate::models::OrgScimConfig),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or SCIM config not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.scim_configs.get", skip(state, authz), fields(%org_slug))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
) -> Result<Json<crate::models::OrgScimConfig>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on org SCIM config
    authz.require(
        "org_scim_config",
        "read",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Get the SCIM config for this org
    let config = services
        .scim_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SCIM config not found for organization '{}'",
                org_slug
            ))
        })?;

    Ok(Json(config))
}

/// Create a new SCIM configuration for an organization
///
/// Each organization can have at most one SCIM configuration. Creating a config
/// for an organization that already has one will result in a 409 Conflict error.
///
/// The response includes the SCIM bearer token which is only shown once.
/// Store it securely - it cannot be retrieved again.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/scim-config",
    tag = "scim",
    operation_id = "org_scim_config_create",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = CreateOrgScimConfig,
    responses(
        (status = 201, description = "SCIM config created with bearer token", body = CreatedOrgScimConfig),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Organization already has a SCIM config", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.scim_configs.create", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<CreateOrgScimConfig>>,
) -> Result<(StatusCode, Json<CreatedOrgScimConfig>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require create permission on org SCIM config
    authz.require(
        "org_scim_config",
        "create",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Check if org already has a SCIM config
    if services.scim_configs.get_by_org_id(org.id).await?.is_some() {
        return Err(AdminError::Conflict(format!(
            "Organization '{}' already has a SCIM configuration",
            org_slug
        )));
    }

    // Validate default_team_id belongs to the org if provided
    if let Some(team_id) = input.default_team_id {
        let team = services
            .teams
            .get_by_id(team_id)
            .await?
            .ok_or_else(|| AdminError::NotFound(format!("Team '{}' not found", team_id)))?;
        if team.org_id != org.id {
            return Err(AdminError::BadRequest(
                "Team does not belong to this organization".to_string(),
            ));
        }
    }

    // Create the SCIM config
    let created = services.scim_configs.create(org.id, input.clone()).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "org_scim_config.create".to_string(),
            resource_type: "org_scim_config".to_string(),
            resource_id: created.config.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "enabled": created.config.enabled,
                "create_users": created.config.create_users,
                "default_team_id": created.config.default_team_id,
                "sync_display_name": created.config.sync_display_name,
                "deprovisioning": {
                    "deactivate_deletes_user": created.config.deactivate_deletes_user,
                    "revoke_api_keys_on_deactivate": created.config.revoke_api_keys_on_deactivate,
                }
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok((StatusCode::CREATED, Json(created)))
}

/// Update the SCIM configuration for an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{org_slug}/scim-config",
    tag = "scim",
    operation_id = "org_scim_config_update",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = UpdateOrgScimConfig,
    responses(
        (status = 200, description = "SCIM config updated", body = crate::models::OrgScimConfig),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or SCIM config not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.scim_configs.update", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<UpdateOrgScimConfig>>,
) -> Result<Json<crate::models::OrgScimConfig>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get existing config
    let existing = services
        .scim_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SCIM config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Require update permission
    authz.require(
        "org_scim_config",
        "update",
        Some(&existing.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Validate default_team_id belongs to the org if being updated
    if let Some(Some(team_id)) = input.default_team_id {
        let team = services
            .teams
            .get_by_id(team_id)
            .await?
            .ok_or_else(|| AdminError::NotFound(format!("Team '{}' not found", team_id)))?;
        if team.org_id != org.id {
            return Err(AdminError::BadRequest(
                "Team does not belong to this organization".to_string(),
            ));
        }
    }

    // Update the SCIM config
    let updated = services
        .scim_configs
        .update(existing.id, input.clone())
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "org_scim_config.update".to_string(),
            resource_type: "org_scim_config".to_string(),
            resource_id: existing.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "enabled": input.enabled,
                "create_users": input.create_users,
                "default_team_id": input.default_team_id,
                "sync_display_name": input.sync_display_name,
                "deactivate_deletes_user": input.deactivate_deletes_user,
                "revoke_api_keys_on_deactivate": input.revoke_api_keys_on_deactivate,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(updated))
}

/// Delete the SCIM configuration for an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/scim-config",
    tag = "scim",
    operation_id = "org_scim_config_delete",
    params(("org_slug" = String, Path, description = "Organization slug")),
    responses(
        (status = 200, description = "SCIM config deleted"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or SCIM config not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.scim_configs.delete", skip(state, admin_auth, authz), fields(%org_slug))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get existing config
    let existing = services
        .scim_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SCIM config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Require delete permission
    authz.require(
        "org_scim_config",
        "delete",
        Some(&existing.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Capture details for audit log before deletion
    let token_prefix = existing.token_prefix.clone();
    let config_id = existing.id;

    // Delete the SCIM config
    services.scim_configs.delete(existing.id).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "org_scim_config.delete".to_string(),
            resource_type: "org_scim_config".to_string(),
            resource_id: config_id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "token_prefix": token_prefix,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(()))
}

/// Rotate the SCIM bearer token for an organization
///
/// Generates a new bearer token and invalidates the old one immediately.
/// The new token is returned only once - store it securely.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/scim-config/rotate-token",
    tag = "scim",
    operation_id = "org_scim_config_rotate_token",
    params(("org_slug" = String, Path, description = "Organization slug")),
    responses(
        (status = 200, description = "Token rotated successfully", body = CreatedOrgScimConfig),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or SCIM config not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.scim_configs.rotate_token", skip(state, admin_auth, authz), fields(%org_slug))]
pub async fn rotate_token(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
) -> Result<Json<CreatedOrgScimConfig>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get existing config
    let existing = services
        .scim_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SCIM config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Require update permission (token rotation is a form of update)
    authz.require(
        "org_scim_config",
        "update",
        Some(&existing.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Capture old token prefix for audit log
    let old_token_prefix = existing.token_prefix.clone();

    // Rotate the token
    let rotated = services.scim_configs.rotate_token(existing.id).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "org_scim_config.rotate_token".to_string(),
            resource_type: "org_scim_config".to_string(),
            resource_id: existing.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "old_token_prefix": old_token_prefix,
                "new_token_prefix": rotated.config.token_prefix,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(rotated))
}
