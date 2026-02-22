use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_valid::Valid;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::{AuditActor, error::AdminError, organizations::ListQuery};
use crate::{
    AppState,
    cache::CacheKeys,
    middleware::{AdminAuth, AuthzContext},
    models::{CreateAuditLog, CreateUser, UpdateUser, User, UserDataExport, UserDeletionResponse},
    openapi::PaginationMeta,
    services::Services,
};

fn default_member_role() -> String {
    "member".to_string()
}

/// Request to add a member to an organization or project
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AddMemberRequest {
    /// User ID to add as member
    pub user_id: Uuid,
    /// Role to assign (defaults to 'member')
    #[serde(default = "default_member_role")]
    pub role: String,
}

/// Request to update a member's role
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateMemberRequest {
    /// New role to assign
    pub role: String,
}

/// Paginated list of users
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserListResponse {
    /// List of users
    pub data: Vec<User>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Create a user
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/users",
    tag = "users",
    operation_id = "user_create",
    request_body = CreateUser,
    responses(
        (status = 201, description = "User created", body = User),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Conflict (external_id already exists)", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.create", skip(state, admin_auth, authz, input))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Valid(Json(input)): Valid<Json<CreateUser>>,
) -> Result<(StatusCode, Json<User>), AdminError> {
    authz.require("user", "create", None, None, None, None)?;

    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);
    let user = services.users.create(input).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "user.create".to_string(),
            resource_type: "user".to_string(),
            resource_id: user.id,
            org_id: None,
            project_id: None,
            details: json!({
                "email": user.email,
                "name": user.name,
                "external_id": user.external_id,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok((StatusCode::CREATED, Json(user)))
}

/// Get a user by ID
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}",
    tag = "users",
    operation_id = "user_get",
    params(("user_id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "User found", body = User),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.get", skip(state, authz), fields(%user_id))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<User>, AdminError> {
    authz.require("user", "read", Some(&user_id.to_string()), None, None, None)?;

    let services = get_services(&state)?;
    let user = services
        .users
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;
    Ok(Json(user))
}

/// List all users
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users",
    tag = "users",
    operation_id = "user_list",
    params(ListQuery),
    responses(
        (status = 200, description = "List of users", body = UserListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.list", skip(state, authz, query))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Query(query): Query<ListQuery>,
) -> Result<Json<UserListResponse>, AdminError> {
    authz.require("user", "list", None, None, None, None)?;

    let services = get_services(&state)?;
    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.users.list(params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(UserListResponse {
        data: result.items,
        pagination,
    }))
}

/// Update a user
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/users/{user_id}",
    tag = "users",
    operation_id = "user_update",
    params(("user_id" = Uuid, Path, description = "User ID")),
    request_body = UpdateUser,
    responses(
        (status = 200, description = "User updated", body = User),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.update", skip(state, admin_auth, authz, input), fields(%user_id))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
    Valid(Json(input)): Valid<Json<UpdateUser>>,
) -> Result<Json<User>, AdminError> {
    authz.require(
        "user",
        "update",
        Some(&user_id.to_string()),
        None,
        None,
        None,
    )?;

    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Capture changes for audit log
    let changes = json!({
        "email": input.email,
        "name": input.name,
    });

    let updated = services.users.update(user_id, input).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "user.update".to_string(),
            resource_type: "user".to_string(),
            resource_id: user_id,
            org_id: None,
            project_id: None,
            details: changes,
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(updated))
}

/// Export all user data (GDPR Article 15 - Right of Access)
///
/// Returns all personal data associated with the user including:
/// - User profile
/// - Organization and project memberships
/// - API keys (excluding sensitive hash)
/// - Conversations
/// - Usage summary
/// - Audit logs where user was the actor
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}/export",
    tag = "users",
    operation_id = "user_export",
    params(("user_id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "User data export", body = UserDataExport),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.export", skip(state, admin_auth, authz), fields(%user_id))]
pub async fn export(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserDataExport>, AdminError> {
    // Requires explicit export permission (more restrictive than read)
    authz.require(
        "user",
        "export",
        Some(&user_id.to_string()),
        None,
        None,
        None,
    )?;

    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);
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

    // Log audit event for GDPR compliance (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "user.export".to_string(),
            resource_type: "user".to_string(),
            resource_id: user_id,
            org_id: None,
            project_id: None,
            details: json!({
                "email": export.user.email,
                "reason": "GDPR Article 15 - Right of Access",
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(export))
}

/// Delete a user and all associated data (GDPR Article 17 - Right to Erasure)
///
/// Permanently deletes the user and all associated data including:
/// - User record
/// - Organization and project memberships
/// - API keys owned by the user
/// - Conversations owned by the user
/// - Dynamic providers owned by the user
/// - Usage records for user's API keys
///
/// This operation is irreversible.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/users/{user_id}",
    tag = "users",
    operation_id = "user_delete",
    params(("user_id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "User deleted", body = UserDeletionResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.delete", skip(state, admin_auth, authz), fields(%user_id))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserDeletionResponse>, AdminError> {
    // Requires explicit delete permission
    authz.require(
        "user",
        "delete",
        Some(&user_id.to_string()),
        None,
        None,
        None,
    )?;

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

    let result = services.users.delete_user(user_id).await?;

    // Log audit event for GDPR compliance (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "user.delete".to_string(),
            resource_type: "user".to_string(),
            resource_id: user_id,
            org_id: None,
            project_id: None,
            details: json!({
                "email": user_email,
                "name": user_name,
                "external_id": user_external_id,
                "reason": "GDPR Article 17 - Right to Erasure",
                "api_keys_deleted": result.api_keys_deleted,
                "conversations_deleted": result.conversations_deleted,
                "dynamic_providers_deleted": result.dynamic_providers_deleted,
                "usage_records_deleted": result.usage_records_deleted,
            }),
            ip_address: None,
            user_agent: None,
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

/// List organization members
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/members",
    tag = "users",
    operation_id = "org_member_list",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of organization members", body = UserListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.list_org_members", skip(state, authz, query), fields(%org_slug))]
pub async fn list_org_members(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<UserListResponse>, AdminError> {
    let services = get_services(&state)?;

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    authz.require(
        "organization",
        "read",
        Some(&org.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.users.list_org_members(org.id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(UserListResponse {
        data: result.items,
        pagination,
    }))
}

/// Add a member to an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/members",
    tag = "users",
    operation_id = "org_member_add",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = AddMemberRequest,
    responses(
        (status = 201, description = "Member added"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or user not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.add_org_member", skip(state, admin_auth, authz, req), fields(%org_slug))]
pub async fn add_org_member(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Json(req): Json<AddMemberRequest>,
) -> Result<(StatusCode, Json<()>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    authz.require(
        "organization",
        "update",
        Some(&org.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    services
        .users
        .add_to_org(
            req.user_id,
            org.id,
            &req.role,
            crate::models::MembershipSource::Manual,
        )
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "membership.add_org".to_string(),
            resource_type: "organization".to_string(),
            resource_id: org.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "user_id": req.user_id,
                "org_slug": org_slug,
                "org_name": org.name,
                "role": req.role,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok((StatusCode::CREATED, Json(())))
}

/// Update a member's role in an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{org_slug}/members/{user_id}",
    tag = "users",
    operation_id = "org_member_update",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    request_body = UpdateMemberRequest,
    responses(
        (status = 200, description = "Member role updated"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or membership not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.update_org_member", skip(state, admin_auth, authz, req), fields(%org_slug, %user_id))]
pub async fn update_org_member(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, user_id)): Path<(String, Uuid)>,
    Json(req): Json<UpdateMemberRequest>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    authz.require(
        "organization",
        "update",
        Some(&org.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    services
        .users
        .update_org_member_role(user_id, org.id, &req.role)
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "membership.update_org".to_string(),
            resource_type: "organization".to_string(),
            resource_id: org.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "user_id": user_id,
                "org_slug": org_slug,
                "new_role": req.role,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(()))
}

/// Remove a member from an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/members/{user_id}",
    tag = "users",
    operation_id = "org_member_remove",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "Member removed"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.remove_org_member", skip(state, admin_auth, authz), fields(%org_slug, %user_id))]
pub async fn remove_org_member(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, user_id)): Path<(String, Uuid)>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    authz.require(
        "organization",
        "update",
        Some(&org.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    services.users.remove_from_org(user_id, org.id).await?;

    // Invalidate cached API keys for the removed user so stale auth data is not used
    invalidate_user_api_key_cache(services, &state, user_id).await;

    // Invalidate user sessions so they cannot continue accessing the org via SSO
    #[cfg(feature = "sso")]
    invalidate_user_sessions(services, &state, user_id).await;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "membership.remove_org".to_string(),
            resource_type: "organization".to_string(),
            resource_id: org.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "user_id": user_id,
                "org_slug": org_slug,
                "org_name": org.name,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(()))
}

/// List project members
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}/members",
    tag = "users",
    operation_id = "project_member_list",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of project members", body = UserListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.list_project_members", skip(state, authz, query), fields(%org_slug, %project_slug))]
pub async fn list_project_members(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<UserListResponse>, AdminError> {
    let services = get_services(&state)?;

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    let project = services
        .projects
        .get_by_slug(org.id, &project_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Project '{}' not found in organization '{}'",
                project_slug, org_slug
            ))
        })?;

    authz.require(
        "project",
        "read",
        Some(&project.id.to_string()),
        Some(&org.id.to_string()),
        None,
        Some(&project.id.to_string()),
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .users
        .list_project_members(project.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(UserListResponse {
        data: result.items,
        pagination,
    }))
}

/// Add a member to a project
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}/members",
    tag = "users",
    operation_id = "project_member_add",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
    ),
    request_body = AddMemberRequest,
    responses(
        (status = 201, description = "Member added"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization, project, or user not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.add_project_member", skip(state, admin_auth, authz, req), fields(%org_slug, %project_slug))]
pub async fn add_project_member(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Json(req): Json<AddMemberRequest>,
) -> Result<(StatusCode, Json<()>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    let project = services
        .projects
        .get_by_slug(org.id, &project_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Project '{}' not found in organization '{}'",
                project_slug, org_slug
            ))
        })?;

    authz.require(
        "project",
        "update",
        Some(&project.id.to_string()),
        Some(&org.id.to_string()),
        None,
        Some(&project.id.to_string()),
    )?;

    services
        .users
        .add_to_project(
            req.user_id,
            project.id,
            &req.role,
            crate::models::MembershipSource::Manual,
        )
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "membership.add_project".to_string(),
            resource_type: "project".to_string(),
            resource_id: project.id,
            org_id: Some(org.id),
            project_id: Some(project.id),
            details: json!({
                "user_id": req.user_id,
                "org_slug": org_slug,
                "project_slug": project_slug,
                "project_name": project.name,
                "role": req.role,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok((StatusCode::CREATED, Json(())))
}

/// Update a member's role in a project
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}/members/{user_id}",
    tag = "users",
    operation_id = "project_member_update",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    request_body = UpdateMemberRequest,
    responses(
        (status = 200, description = "Member role updated"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization, project, or membership not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.update_project_member", skip(state, admin_auth, authz, req), fields(%org_slug, %project_slug, %user_id))]
pub async fn update_project_member(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug, user_id)): Path<(String, String, Uuid)>,
    Json(req): Json<UpdateMemberRequest>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    let project = services
        .projects
        .get_by_slug(org.id, &project_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Project '{}' not found in organization '{}'",
                project_slug, org_slug
            ))
        })?;

    authz.require(
        "project",
        "update",
        Some(&project.id.to_string()),
        Some(&org.id.to_string()),
        None,
        Some(&project.id.to_string()),
    )?;

    services
        .users
        .update_project_member_role(user_id, project.id, &req.role)
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "membership.update_project".to_string(),
            resource_type: "project".to_string(),
            resource_id: project.id,
            org_id: Some(org.id),
            project_id: Some(project.id),
            details: json!({
                "user_id": user_id,
                "org_slug": org_slug,
                "project_slug": project_slug,
                "new_role": req.role,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(()))
}

/// Remove a member from a project
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}/members/{user_id}",
    tag = "users",
    operation_id = "project_member_remove",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "Member removed"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.users.remove_project_member", skip(state, admin_auth, authz), fields(%org_slug, %project_slug, %user_id))]
pub async fn remove_project_member(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug, user_id)): Path<(String, String, Uuid)>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    let project = services
        .projects
        .get_by_slug(org.id, &project_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Project '{}' not found in organization '{}'",
                project_slug, org_slug
            ))
        })?;

    authz.require(
        "project",
        "update",
        Some(&project.id.to_string()),
        Some(&org.id.to_string()),
        None,
        Some(&project.id.to_string()),
    )?;

    services
        .users
        .remove_from_project(user_id, project.id)
        .await?;

    // Invalidate cached API keys for the removed user so stale auth data is not used
    invalidate_user_api_key_cache(services, &state, user_id).await;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "membership.remove_project".to_string(),
            resource_type: "project".to_string(),
            resource_id: project.id,
            org_id: Some(org.id),
            project_id: Some(project.id),
            details: json!({
                "user_id": user_id,
                "org_slug": org_slug,
                "project_slug": project_slug,
                "project_name": project.name,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(()))
}

/// Invalidate all cached API keys owned by a user.
///
/// When a user is removed from an organization or project, their cached API key
/// entries may contain stale auth/membership data. This function fetches the key
/// hashes for all active user-owned API keys and deletes them from the cache,
/// forcing a fresh DB lookup on the next request.
async fn invalidate_user_api_key_cache(services: &Services, state: &AppState, user_id: Uuid) {
    let Some(cache) = &state.cache else {
        return;
    };

    match services.api_keys.get_key_hashes_by_user(user_id).await {
        Ok(key_hashes) => {
            for hash in &key_hashes {
                let cache_key = CacheKeys::api_key(hash);
                let _ = cache.delete(&cache_key).await;
            }
            if !key_hashes.is_empty() {
                tracing::debug!(
                    user_id = %user_id,
                    keys_invalidated = key_hashes.len(),
                    "Invalidated cached API keys after membership removal"
                );
            }
        }
        Err(e) => {
            // Log but don't fail the request — cache entries will expire eventually
            tracing::warn!(
                error = %e,
                user_id = %user_id,
                "Failed to invalidate API key caches after membership removal"
            );
        }
    }
}

/// Invalidate all SSO sessions for a user after org membership removal.
///
/// This forces the user to re-authenticate, preventing continued access to
/// the organization through stale browser sessions.
#[cfg(feature = "sso")]
async fn invalidate_user_sessions(services: &Services, state: &AppState, user_id: Uuid) {
    // Look up the user to get their external_id (needed for session store)
    let user = match services.users.get_by_id(user_id).await {
        Ok(Some(user)) if !user.external_id.is_empty() => user,
        _ => return,
    };

    let session_store = match super::sessions::get_session_store(state) {
        Ok(store) => store,
        Err(_) => return,
    };

    match session_store.delete_user_sessions(&user.external_id).await {
        Ok(count) if count > 0 => {
            tracing::info!(
                user_id = %user_id,
                sessions_revoked = count,
                "Revoked user sessions after org membership removal"
            );
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                user_id = %user_id,
                "Failed to revoke user sessions after membership removal"
            );
        }
        _ => {}
    }
}
