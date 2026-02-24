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
    models::{CreateAuditLog, CreatePrompt, Prompt, PromptOwnerType, UpdatePrompt},
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of prompts
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PromptListResponse {
    /// List of prompts
    pub data: Vec<Prompt>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Create a prompt template
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/prompts",
    tag = "prompts",
    operation_id = "prompt_create",
    request_body = CreatePrompt,
    responses(
        (status = 201, description = "Prompt created", body = Prompt),
        (status = 404, description = "Owner not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Prompt with this name already exists", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.prompts.create", skip(state, admin_auth, authz, input))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Valid(Json(input)): Valid<Json<CreatePrompt>>,
) -> Result<(StatusCode, Json<Prompt>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    authz.require("prompt", "create", None, None, None, None)?;

    let prompt = services.prompts.create(input).await?;

    // Extract org_id and project_id from owner for audit log
    let (org_id, project_id) = match prompt.owner_type {
        PromptOwnerType::Organization => (Some(prompt.owner_id), None),
        PromptOwnerType::Project => (None, Some(prompt.owner_id)),
        PromptOwnerType::Team | PromptOwnerType::User => (None, None),
    };

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "prompt.create".to_string(),
            resource_type: "prompt".to_string(),
            resource_id: prompt.id,
            org_id,
            project_id,
            details: json!({
                "name": prompt.name,
                "owner_type": prompt.owner_type,
                "owner_id": prompt.owner_id,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(prompt)))
}

/// Get a prompt by ID
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/prompts/{id}",
    tag = "prompts",
    operation_id = "prompt_get",
    params(("id" = Uuid, Path, description = "Prompt ID")),
    responses(
        (status = 200, description = "Prompt found", body = Prompt),
        (status = 404, description = "Prompt not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.prompts.get", skip(state, authz), fields(%id))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Prompt>, AdminError> {
    let services = get_services(&state)?;

    authz.require("prompt", "read", None, None, None, None)?;

    let prompt = services
        .prompts
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Prompt not found".to_string()))?;

    Ok(Json(prompt))
}

/// Update a prompt
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/prompts/{id}",
    tag = "prompts",
    operation_id = "prompt_update",
    params(("id" = Uuid, Path, description = "Prompt ID")),
    request_body = UpdatePrompt,
    responses(
        (status = 200, description = "Prompt updated", body = Prompt),
        (status = 404, description = "Prompt not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Prompt with this name already exists", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.prompts.update", skip(state, admin_auth, authz, input), fields(%id))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(id): Path<Uuid>,
    Valid(Json(input)): Valid<Json<UpdatePrompt>>,
) -> Result<Json<Prompt>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    authz.require("prompt", "update", None, None, None, None)?;

    // Capture changes for audit log
    let changes = json!({
        "name": input.name,
        "description": input.description,
        "content": input.content.as_ref().map(|_| "<updated>"),
        "metadata": input.metadata,
    });

    let prompt = services.prompts.update(id, input).await?;

    // Extract org_id and project_id from owner for audit log
    let (org_id, project_id) = match prompt.owner_type {
        PromptOwnerType::Organization => (Some(prompt.owner_id), None),
        PromptOwnerType::Project => (None, Some(prompt.owner_id)),
        PromptOwnerType::Team | PromptOwnerType::User => (None, None),
    };

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "prompt.update".to_string(),
            resource_type: "prompt".to_string(),
            resource_id: prompt.id,
            org_id,
            project_id,
            details: json!({
                "name": prompt.name,
                "changes": changes,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(prompt))
}

/// Delete a prompt
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/prompts/{id}",
    tag = "prompts",
    operation_id = "prompt_delete",
    params(("id" = Uuid, Path, description = "Prompt ID")),
    responses(
        (status = 200, description = "Prompt deleted"),
        (status = 404, description = "Prompt not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.prompts.delete", skip(state, admin_auth, authz), fields(%id))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(id): Path<Uuid>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    authz.require("prompt", "delete", None, None, None, None)?;

    // Get prompt details before deletion for audit log
    let prompt = services
        .prompts
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Prompt not found".to_string()))?;

    // Extract org_id and project_id from owner for audit log
    let (org_id, project_id) = match prompt.owner_type {
        PromptOwnerType::Organization => (Some(prompt.owner_id), None),
        PromptOwnerType::Project => (None, Some(prompt.owner_id)),
        PromptOwnerType::Team | PromptOwnerType::User => (None, None),
    };

    // Capture details for audit log before deletion
    let prompt_name = prompt.name.clone();
    let prompt_owner_type = prompt.owner_type;
    let prompt_owner_id = prompt.owner_id;

    services.prompts.delete(id).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "prompt.delete".to_string(),
            resource_type: "prompt".to_string(),
            resource_id: id,
            org_id,
            project_id,
            details: json!({
                "name": prompt_name,
                "owner_type": prompt_owner_type,
                "owner_id": prompt_owner_id,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}

/// List prompts by organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/prompts",
    tag = "prompts",
    operation_id = "prompt_list_by_org",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of prompts", body = PromptListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.prompts.list_by_org", skip(state, authz, query), fields(%org_slug))]
pub async fn list_by_org(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<PromptListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    authz.require(
        "prompt",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .prompts
        .list_by_owner(PromptOwnerType::Organization, org.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(PromptListResponse {
        data: result.items,
        pagination,
    }))
}

/// List prompts by team
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/teams/{team_slug}/prompts",
    tag = "prompts",
    operation_id = "prompt_list_by_team",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("team_slug" = String, Path, description = "Team slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of prompts", body = PromptListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or team not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.prompts.list_by_team", skip(state, authz, query), fields(%org_slug, %team_slug))]
pub async fn list_by_team(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, team_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<PromptListResponse>, AdminError> {
    let services = get_services(&state)?;

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

    authz.require(
        "prompt",
        "list",
        None,
        Some(&org.id.to_string()),
        Some(&team.id.to_string()),
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .prompts
        .list_by_owner(PromptOwnerType::Team, team.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(PromptListResponse {
        data: result.items,
        pagination,
    }))
}

/// List prompts by project
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}/prompts",
    tag = "prompts",
    operation_id = "prompt_list_by_project",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of prompts", body = PromptListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.prompts.list_by_project", skip(state, authz, query), fields(%org_slug, %project_slug))]
pub async fn list_by_project(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<PromptListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get project by slug
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
        "prompt",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        Some(&project.id.to_string()),
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .prompts
        .list_by_owner(PromptOwnerType::Project, project.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(PromptListResponse {
        data: result.items,
        pagination,
    }))
}

/// List prompts by user
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}/prompts",
    tag = "prompts",
    operation_id = "prompt_list_by_user",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of prompts", body = PromptListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.prompts.list_by_user", skip(state, authz, query), fields(%user_id))]
pub async fn list_by_user(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<PromptListResponse>, AdminError> {
    let services = get_services(&state)?;

    authz.require("prompt", "list", None, None, None, None)?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .prompts
        .list_by_owner(PromptOwnerType::User, user_id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(PromptListResponse {
        data: result.items,
        pagination,
    }))
}
