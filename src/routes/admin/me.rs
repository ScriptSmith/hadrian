use axum::{Extension, Json, extract::State};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use super::{AuditActor, error::AdminError};
use crate::{
    AppState,
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    models::{CreateAuditLog, UserDataExport, UserDeletionResponse},
    services::Services,
};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Export current user's data (GDPR Article 15 - Right of Access)
///
/// Exports all personal data associated with the authenticated user including:
/// - User profile
/// - Organization and project memberships
/// - API keys (excluding sensitive hash)
/// - Conversations
/// - Usage summary
/// - Audit logs where user was the actor
///
/// This endpoint allows users to export their own data without admin permissions.
/// The user is identified from the authenticated session.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/me/export",
    tag = "me",
    operation_id = "me_export",
    responses(
        (status = 200, description = "User data export", body = UserDataExport),
        (status = 401, description = "User not identified from session", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn export(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<UserDataExport>, AdminError> {
    authz.require("me", "read", None, None, None, None)?;

    // Get the user ID from the authenticated session
    // User must exist in database to export data
    let user_id = admin_auth.identity.user_id.ok_or(AdminError::NotFound(
        "User not found in database. Session identity may not be linked to a user record."
            .to_string(),
    ))?;

    let services = get_services(&state)?;

    // Export the user's data
    #[cfg(feature = "sso")]
    let export = {
        let session_store = super::sessions::get_session_store(&state).ok();
        services
            .users
            .export_user_data(user_id, session_store.as_ref())
            .await?
    };
    #[cfg(not(feature = "sso"))]
    let export = services.users.export_user_data(user_id).await?;

    Ok(Json(export))
}

/// Delete current user and all associated data (GDPR Article 17 - Right to Erasure)
///
/// Permanently deletes the authenticated user and all associated data including:
/// - User record
/// - Organization and project memberships
/// - API keys owned by the user
/// - Conversations owned by the user
/// - Dynamic providers owned by the user
/// - Usage records for user's API keys
///
/// This endpoint allows users to delete their own account without admin permissions.
/// The user is identified from the authenticated session.
///
/// **Warning:** This operation is irreversible.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/me",
    tag = "me",
    operation_id = "me_delete",
    responses(
        (status = 200, description = "User deleted", body = UserDeletionResponse),
        (status = 401, description = "User not identified from session", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.me.delete", skip(state, admin_auth, authz))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
) -> Result<Json<UserDeletionResponse>, AdminError> {
    authz.require("me", "delete", None, None, None, None)?;

    // Get the user ID from the authenticated session
    let user_id = admin_auth.identity.user_id.ok_or(AdminError::NotFound(
        "User not found in database. Session identity may not be linked to a user record."
            .to_string(),
    ))?;

    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Capture user details before deletion for audit log
    let user = services
        .users
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;
    let user_email = user.email.clone();
    let user_name = user.name.clone();
    let user_external_id = user.external_id.clone();

    // Delete the user and all associated data
    let result = services.users.delete_user(user_id).await?;

    // Log audit event for GDPR compliance (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "user.self_delete".to_string(),
            resource_type: "user".to_string(),
            resource_id: user_id,
            org_id: None,
            project_id: None,
            details: json!({
                "email": user_email,
                "name": user_name,
                "external_id": user_external_id,
                "reason": "GDPR Article 17 - Right to Erasure (self-service)",
                "api_keys_deleted": result.api_keys_deleted,
                "conversations_deleted": result.conversations_deleted,
                "dynamic_providers_deleted": result.dynamic_providers_deleted,
                "usage_records_deleted": result.usage_records_deleted,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(UserDeletionResponse {
        deleted: result.user_deleted,
        user_id,
        api_keys_deleted: result.api_keys_deleted,
        conversations_deleted: result.conversations_deleted,
        dynamic_providers_deleted: result.dynamic_providers_deleted,
        usage_records_deleted: result.usage_records_deleted,
    }))
}

/// One owner the current user could pick on the OAuth consent page.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EligibleOwner {
    /// Owner ID (uuid).
    pub id: Uuid,
    /// Slug the user sees in URLs.
    pub slug: String,
    /// Display name shown in the picker.
    pub name: String,
    /// Parent organization id, if applicable. `None` for orgs themselves
    /// and the user's own identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_id: Option<Uuid>,
    /// Parent organization slug, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_slug: Option<String>,
    /// User's role in the owner (member, admin, etc.). `None` for the user
    /// entry, where role is implicit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

/// Response from `GET /admin/v1/me/eligible-owners`.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct EligibleOwnersResponse {
    /// The current user as an owner choice — always present.
    pub user: EligibleOwner,
    /// Organizations the user belongs to.
    pub organizations: Vec<EligibleOwner>,
    /// Teams the user belongs to.
    pub teams: Vec<EligibleOwner>,
    /// Projects the user belongs to.
    pub projects: Vec<EligibleOwner>,
}

/// List the owners the current user can plausibly issue an API key for.
///
/// This is a hint for the OAuth consent UI; the actual create-permission
/// check still runs server-side at consent time, so an owner appearing
/// here doesn't guarantee the create will succeed if RBAC denies it.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/me/eligible-owners",
    tag = "me",
    operation_id = "me_eligible_owners",
    responses(
        (status = 200, description = "Owners the user can pick from", body = EligibleOwnersResponse),
        (status = 401, description = "User not identified from session", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn eligible_owners(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<EligibleOwnersResponse>, AdminError> {
    authz.require("me", "read", None, None, None, None)?;

    let user_id = admin_auth
        .identity
        .user_id
        .ok_or_else(|| AdminError::Forbidden("User account required".to_string()))?;

    let services = get_services(&state)?;

    let user_record = services
        .users
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;

    let users_repo = state
        .db
        .as_ref()
        .ok_or(AdminError::DatabaseRequired)?
        .users();

    let org_memberships = users_repo.get_org_memberships_for_user(user_id).await?;
    let team_memberships = users_repo.get_team_memberships_for_user(user_id).await?;
    let project_memberships = users_repo.get_project_memberships_for_user(user_id).await?;

    // Project memberships only include the org_id, not slug. Pre-fetch the
    // user's orgs once so we can look up org slugs without N+1 queries.
    let org_slug_by_id: std::collections::HashMap<Uuid, String> = org_memberships
        .iter()
        .map(|m| (m.org_id, m.org_slug.clone()))
        .collect();

    let user = EligibleOwner {
        id: user_id,
        slug: user_record
            .email
            .clone()
            .unwrap_or_else(|| user_record.external_id.clone()),
        name: user_record
            .name
            .clone()
            .or_else(|| user_record.email.clone())
            .unwrap_or_else(|| "You".to_string()),
        org_id: None,
        org_slug: None,
        role: None,
    };

    let organizations = org_memberships
        .iter()
        .map(|m| EligibleOwner {
            id: m.org_id,
            slug: m.org_slug.clone(),
            name: m.org_name.clone(),
            org_id: None,
            org_slug: None,
            role: Some(m.role.clone()),
        })
        .collect();

    let teams = team_memberships
        .iter()
        .map(|m| EligibleOwner {
            id: m.team_id,
            slug: m.team_slug.clone(),
            name: m.team_name.clone(),
            org_id: Some(m.org_id),
            org_slug: org_slug_by_id.get(&m.org_id).cloned(),
            role: Some(m.role.clone()),
        })
        .collect();

    let projects = project_memberships
        .iter()
        .map(|m| EligibleOwner {
            id: m.project_id,
            slug: m.project_slug.clone(),
            name: m.project_name.clone(),
            org_id: Some(m.org_id),
            org_slug: org_slug_by_id.get(&m.org_id).cloned(),
            role: Some(m.role.clone()),
        })
        .collect();

    Ok(Json(EligibleOwnersResponse {
        user,
        organizations,
        teams,
        projects,
    }))
}
