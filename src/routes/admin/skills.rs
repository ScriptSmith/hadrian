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
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    models::{CreateAuditLog, CreateSkill, Skill, SkillOwnerType, UpdateSkill},
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of skills.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SkillListResponse {
    /// List of skills (file contents omitted; see `files_manifest` on each).
    pub data: Vec<Skill>,
    /// Pagination metadata.
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Map a skill's owner to (org_id, project_id) for audit log correlation.
fn audit_owner(skill: &Skill) -> (Option<Uuid>, Option<Uuid>) {
    match skill.owner_type {
        SkillOwnerType::Organization => (Some(skill.owner_id), None),
        SkillOwnerType::Project => (None, Some(skill.owner_id)),
        SkillOwnerType::Team | SkillOwnerType::User => (None, None),
    }
}

/// Create a skill.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/skills",
    tag = "skills",
    operation_id = "skill_create",
    request_body = CreateSkill,
    responses(
        (status = 201, description = "Skill created", body = Skill),
        (status = 400, description = "Invalid skill (missing SKILL.md, duplicate path, or size limit exceeded)", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Owner not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Skill with this name already exists for this owner", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.skills.create", skip(state, admin_auth, authz, input))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Valid(Json(input)): Valid<Json<CreateSkill>>,
) -> Result<(StatusCode, Json<Skill>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    authz.require("skill", "create", None, None, None, None)?;

    // Enforce per-owner skill count limit.
    let max = state.config.limits.resource_limits.max_skills_per_owner;
    if max > 0 {
        let count = services
            .skills
            .count_by_owner(input.owner.owner_type(), input.owner.owner_id(), false)
            .await?;
        if count >= max as i64 {
            return Err(AdminError::Conflict(format!(
                "Owner has reached the maximum number of skills ({max})"
            )));
        }
    }

    let skill = services.skills.create(input).await?;

    let (org_id, project_id) = audit_owner(&skill);
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "skill.create".to_string(),
            resource_type: "skill".to_string(),
            resource_id: skill.id,
            org_id,
            project_id,
            details: json!({
                "name": skill.name,
                "owner_type": skill.owner_type,
                "owner_id": skill.owner_id,
                "file_count": skill.files.len(),
                "total_bytes": skill.total_bytes,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(skill)))
}

/// Get a skill by ID (with full file contents).
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/skills/{id}",
    tag = "skills",
    operation_id = "skill_get",
    params(("id" = Uuid, Path, description = "Skill ID")),
    responses(
        (status = 200, description = "Skill found", body = Skill),
        (status = 404, description = "Skill not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.skills.get", skip(state, authz), fields(%id))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Skill>, AdminError> {
    let services = get_services(&state)?;

    authz.require("skill", "read", None, None, None, None)?;

    let skill = services
        .skills
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Skill not found".to_string()))?;

    Ok(Json(skill))
}

/// Update a skill.
///
/// When `files` is provided, the full file set is replaced.
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/skills/{id}",
    tag = "skills",
    operation_id = "skill_update",
    params(("id" = Uuid, Path, description = "Skill ID")),
    request_body = UpdateSkill,
    responses(
        (status = 200, description = "Skill updated", body = Skill),
        (status = 400, description = "Invalid skill (missing SKILL.md, duplicate path, or size limit exceeded)", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Skill not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Skill with this name already exists for this owner", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.skills.update", skip(state, admin_auth, authz, input), fields(%id))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(id): Path<Uuid>,
    Valid(Json(input)): Valid<Json<UpdateSkill>>,
) -> Result<Json<Skill>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    authz.require("skill", "update", None, None, None, None)?;

    // Capture a redacted change summary for the audit log (avoids logging
    // full file contents).
    let changes = json!({
        "name": input.name,
        "description": input.description,
        "files": input.files.as_ref().map(|fs| json!({
            "count": fs.len(),
            "total_bytes": fs.iter().map(|f| f.content.len() as i64).sum::<i64>(),
            "paths": fs.iter().map(|f| &f.path).collect::<Vec<_>>(),
        })),
        "user_invocable": input.user_invocable,
        "disable_model_invocation": input.disable_model_invocation,
        "allowed_tools": input.allowed_tools,
        "argument_hint": input.argument_hint,
        "source_url": input.source_url,
        "source_ref": input.source_ref,
    });

    let skill = services.skills.update(id, input).await?;

    let (org_id, project_id) = audit_owner(&skill);
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "skill.update".to_string(),
            resource_type: "skill".to_string(),
            resource_id: skill.id,
            org_id,
            project_id,
            details: json!({
                "name": skill.name,
                "changes": changes,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(skill))
}

/// Soft-delete a skill.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/skills/{id}",
    tag = "skills",
    operation_id = "skill_delete",
    params(("id" = Uuid, Path, description = "Skill ID")),
    responses(
        (status = 200, description = "Skill deleted"),
        (status = 404, description = "Skill not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.skills.delete", skip(state, admin_auth, authz), fields(%id))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(id): Path<Uuid>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    authz.require("skill", "delete", None, None, None, None)?;

    // Capture details before deletion for the audit log.
    let skill = services
        .skills
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Skill not found".to_string()))?;

    let (org_id, project_id) = audit_owner(&skill);
    let name = skill.name.clone();
    let owner_type = skill.owner_type;
    let owner_id = skill.owner_id;

    services.skills.delete(id).await?;

    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "skill.delete".to_string(),
            resource_type: "skill".to_string(),
            resource_id: id,
            org_id,
            project_id,
            details: json!({
                "name": name,
                "owner_type": owner_type,
                "owner_id": owner_id,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}

/// List skills by organization.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/skills",
    tag = "skills",
    operation_id = "skill_list_by_org",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of skills", body = SkillListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.skills.list_by_org", skip(state, authz, query), fields(%org_slug))]
pub async fn list_by_org(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<SkillListResponse>, AdminError> {
    let services = get_services(&state)?;

    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    authz.require(
        "skill",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.skills.list_by_org(org.id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(SkillListResponse {
        data: result.items,
        pagination,
    }))
}

/// List skills by team.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/teams/{team_slug}/skills",
    tag = "skills",
    operation_id = "skill_list_by_team",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("team_slug" = String, Path, description = "Team slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of skills", body = SkillListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or team not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.skills.list_by_team", skip(state, authz, query), fields(%org_slug, %team_slug))]
pub async fn list_by_team(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, team_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<SkillListResponse>, AdminError> {
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

    authz.require(
        "skill",
        "list",
        None,
        Some(&org.id.to_string()),
        Some(&team.id.to_string()),
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .skills
        .list_by_owner(SkillOwnerType::Team, team.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(SkillListResponse {
        data: result.items,
        pagination,
    }))
}

/// List skills by project.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}/skills",
    tag = "skills",
    operation_id = "skill_list_by_project",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of skills", body = SkillListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.skills.list_by_project", skip(state, authz, query), fields(%org_slug, %project_slug))]
pub async fn list_by_project(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<SkillListResponse>, AdminError> {
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
        "skill",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        Some(&project.id.to_string()),
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .skills
        .list_by_owner(SkillOwnerType::Project, project.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(SkillListResponse {
        data: result.items,
        pagination,
    }))
}

/// List skills by user.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}/skills",
    tag = "skills",
    operation_id = "skill_list_by_user",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of skills", body = SkillListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.skills.list_by_user", skip(state, authz, query), fields(%user_id))]
pub async fn list_by_user(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<SkillListResponse>, AdminError> {
    let services = get_services(&state)?;

    authz.require("skill", "list", None, None, None, None)?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .skills
        .list_by_owner(SkillOwnerType::User, user_id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(SkillListResponse {
        data: result.items,
        pagination,
    }))
}
