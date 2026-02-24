use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_valid::Valid;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use super::{AuditActor, error::AdminError, organizations::ListQuery};
use crate::{
    AppState,
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    models::{
        AddTeamMember, CreateAuditLog, CreateTeam, Team, TeamMember, UpdateTeam, UpdateTeamMember,
    },
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of teams
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TeamListResponse {
    /// List of teams
    pub data: Vec<Team>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

/// Paginated list of team members
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TeamMemberListResponse {
    /// List of team members
    pub data: Vec<TeamMember>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

// ============================================================================
// Team CRUD endpoints
// ============================================================================

/// Create a team in an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/teams",
    tag = "teams",
    operation_id = "team_create",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = CreateTeam,
    responses(
        (status = 201, description = "Team created", body = Team),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Conflict (slug already exists)", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.teams.create", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<CreateTeam>>,
) -> Result<(StatusCode, Json<Team>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug to get its ID
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require org admin permission to create teams
    authz.require(
        "team",
        "create",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let team = services.teams.create(org.id, input).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "team.create".to_string(),
            resource_type: "team".to_string(),
            resource_id: team.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "name": team.name,
                "slug": team.slug,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(team)))
}

/// Get a team by slug
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/teams/{team_slug}",
    tag = "teams",
    operation_id = "team_get",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("team_slug" = String, Path, description = "Team slug"),
    ),
    responses(
        (status = 200, description = "Team found", body = Team),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or team not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.teams.get", skip(state, authz), fields(%org_slug, %team_slug))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, team_slug)): Path<(String, String)>,
) -> Result<Json<Team>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    let team = services
        .teams
        .get_by_slug(org.id, &team_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Team '{}' not found in organization '{}'",
                team_slug, org_slug
            ))
        })?;

    // Require read permission on the team
    authz.require(
        "team",
        "read",
        Some(&team.id.to_string()),
        Some(&org.id.to_string()),
        Some(&team.id.to_string()),
        None,
    )?;

    Ok(Json(team))
}

/// List teams in an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/teams",
    tag = "teams",
    operation_id = "team_list",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of teams", body = TeamListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.teams.list", skip(state, authz, query), fields(%org_slug))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<TeamListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on the org to list teams
    authz.require("team", "list", None, Some(&org.id.to_string()), None, None)?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.teams.list_by_org(org.id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(TeamListResponse {
        data: result.items,
        pagination,
    }))
}

/// Update a team
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{org_slug}/teams/{team_slug}",
    tag = "teams",
    operation_id = "team_update",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("team_slug" = String, Path, description = "Team slug"),
    ),
    request_body = UpdateTeam,
    responses(
        (status = 200, description = "Team updated", body = Team),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or team not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.teams.update", skip(state, admin_auth, authz, input), fields(%org_slug, %team_slug))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, team_slug)): Path<(String, String)>,
    Valid(Json(input)): Valid<Json<UpdateTeam>>,
) -> Result<Json<Team>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get team by slug
    let team = services
        .teams
        .get_by_slug(org.id, &team_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Team '{}' not found in organization '{}'",
                team_slug, org_slug
            ))
        })?;

    // Require update permission on the team
    authz.require(
        "team",
        "update",
        Some(&team.id.to_string()),
        Some(&org.id.to_string()),
        Some(&team.id.to_string()),
        None,
    )?;

    // Capture changes for audit log
    let changes = json!({
        "name": input.name,
    });

    let updated = services.teams.update(team.id, input).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "team.update".to_string(),
            resource_type: "team".to_string(),
            resource_id: team.id,
            org_id: Some(org.id),
            project_id: None,
            details: changes,
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(updated))
}

/// Delete a team
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/teams/{team_slug}",
    tag = "teams",
    operation_id = "team_delete",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("team_slug" = String, Path, description = "Team slug"),
    ),
    responses(
        (status = 200, description = "Team deleted"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or team not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.teams.delete", skip(state, admin_auth, authz), fields(%org_slug, %team_slug))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, team_slug)): Path<(String, String)>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get team by slug
    let team = services
        .teams
        .get_by_slug(org.id, &team_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Team '{}' not found in organization '{}'",
                team_slug, org_slug
            ))
        })?;

    // Require delete permission on the team (typically org admin)
    authz.require(
        "team",
        "delete",
        Some(&team.id.to_string()),
        Some(&org.id.to_string()),
        Some(&team.id.to_string()),
        None,
    )?;

    // Capture details for audit log before deletion
    let team_id = team.id;
    let team_name = team.name.clone();
    let team_slug_val = team.slug.clone();

    services.teams.delete(team_id).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "team.delete".to_string(),
            resource_type: "team".to_string(),
            resource_id: team_id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "name": team_name,
                "slug": team_slug_val,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}

// ============================================================================
// Team membership endpoints
// ============================================================================

/// List team members
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/teams/{team_slug}/members",
    tag = "teams",
    operation_id = "team_member_list",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("team_slug" = String, Path, description = "Team slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of team members", body = TeamMemberListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or team not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.teams.list_members", skip(state, authz, query), fields(%org_slug, %team_slug))]
pub async fn list_members(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, team_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<TeamMemberListResponse>, AdminError> {
    let services = get_services(&state)?;

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    let team = services
        .teams
        .get_by_slug(org.id, &team_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Team '{}' not found in organization '{}'",
                team_slug, org_slug
            ))
        })?;

    // Require read permission on the team to list members
    authz.require(
        "team",
        "read",
        Some(&team.id.to_string()),
        Some(&org.id.to_string()),
        Some(&team.id.to_string()),
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.teams.list_members(team.id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(TeamMemberListResponse {
        data: result.items,
        pagination,
    }))
}

/// Add a member to a team
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/teams/{team_slug}/members",
    tag = "teams",
    operation_id = "team_member_add",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("team_slug" = String, Path, description = "Team slug"),
    ),
    request_body = AddTeamMember,
    responses(
        (status = 201, description = "Member added", body = TeamMember),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization, team, or user not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "User is already a member", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.teams.add_member", skip(state, admin_auth, authz, input), fields(%org_slug, %team_slug))]
pub async fn add_member(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, team_slug)): Path<(String, String)>,
    Valid(Json(input)): Valid<Json<AddTeamMember>>,
) -> Result<(StatusCode, Json<TeamMember>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    let team = services
        .teams
        .get_by_slug(org.id, &team_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Team '{}' not found in organization '{}'",
                team_slug, org_slug
            ))
        })?;

    // Require manage_members permission on the team
    authz.require(
        "team",
        "manage_members",
        Some(&team.id.to_string()),
        Some(&org.id.to_string()),
        Some(&team.id.to_string()),
        None,
    )?;

    let user_id = input.user_id;
    let role = input.role.clone();
    let member = services.teams.add_member(team.id, input).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "team.add_member".to_string(),
            resource_type: "team".to_string(),
            resource_id: team.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "user_id": user_id,
                "role": role,
                "team_slug": team_slug,
                "team_name": team.name,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(member)))
}

/// Remove a member from a team
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/teams/{team_slug}/members/{user_id}",
    tag = "teams",
    operation_id = "team_member_remove",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("team_slug" = String, Path, description = "Team slug"),
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "Member removed"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization, team, or membership not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.teams.remove_member", skip(state, admin_auth, authz), fields(%org_slug, %team_slug, %user_id))]
pub async fn remove_member(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, team_slug, user_id)): Path<(String, String, Uuid)>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    let team = services
        .teams
        .get_by_slug(org.id, &team_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Team '{}' not found in organization '{}'",
                team_slug, org_slug
            ))
        })?;

    // Require manage_members permission on the team
    authz.require(
        "team",
        "manage_members",
        Some(&team.id.to_string()),
        Some(&org.id.to_string()),
        Some(&team.id.to_string()),
        None,
    )?;

    services.teams.remove_member(team.id, user_id).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "team.remove_member".to_string(),
            resource_type: "team".to_string(),
            resource_id: team.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "user_id": user_id,
                "team_slug": team_slug,
                "team_name": team.name,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}

/// Update a team member's role
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{org_slug}/teams/{team_slug}/members/{user_id}",
    tag = "teams",
    operation_id = "team_member_update",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("team_slug" = String, Path, description = "Team slug"),
        ("user_id" = Uuid, Path, description = "User ID"),
    ),
    request_body = UpdateTeamMember,
    responses(
        (status = 200, description = "Member role updated", body = TeamMember),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization, team, or membership not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.teams.update_member", skip(state, admin_auth, authz, input), fields(%org_slug, %team_slug, %user_id))]
pub async fn update_member(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, team_slug, user_id)): Path<(String, String, Uuid)>,
    Valid(Json(input)): Valid<Json<UpdateTeamMember>>,
) -> Result<Json<TeamMember>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    let team = services
        .teams
        .get_by_slug(org.id, &team_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Team '{}' not found in organization '{}'",
                team_slug, org_slug
            ))
        })?;

    // Require manage_members permission on the team
    authz.require(
        "team",
        "manage_members",
        Some(&team.id.to_string()),
        Some(&org.id.to_string()),
        Some(&team.id.to_string()),
        None,
    )?;

    let new_role = input.role.clone();
    let member = services
        .teams
        .update_member_role(team.id, user_id, input)
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "team.update_member".to_string(),
            resource_type: "team".to_string(),
            resource_id: team.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "user_id": user_id,
                "new_role": new_role,
                "team_slug": team_slug,
                "team_name": team.name,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(member))
}
