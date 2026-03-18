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
    models::{CreateAuditLog, CreateTemplate, Template, TemplateOwnerType, UpdateTemplate},
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of templates
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TemplateListResponse {
    /// List of templates
    pub data: Vec<Template>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Create a template
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/templates",
    tag = "templates",
    operation_id = "template_create",
    request_body = CreateTemplate,
    responses(
        (status = 201, description = "Template created", body = Template),
        (status = 404, description = "Owner not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Template with this name already exists", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.templates.create", skip(state, admin_auth, authz, input))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Valid(Json(input)): Valid<Json<CreateTemplate>>,
) -> Result<(StatusCode, Json<Template>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    authz.require("template", "create", None, None, None, None)?;

    // Check template limit
    let max = state.config.limits.resource_limits.max_templates_per_owner;
    if max > 0 {
        let count = services
            .templates
            .count_by_owner(input.owner.owner_type(), input.owner.owner_id(), false)
            .await?;
        if count >= max as i64 {
            return Err(AdminError::Conflict(format!(
                "Owner has reached the maximum number of templates ({max})"
            )));
        }
    }

    let template = services.templates.create(input).await?;

    // Extract org_id and project_id from owner for audit log
    let (org_id, project_id) = match template.owner_type {
        TemplateOwnerType::Organization => (Some(template.owner_id), None),
        TemplateOwnerType::Project => (None, Some(template.owner_id)),
        TemplateOwnerType::Team | TemplateOwnerType::User => (None, None),
    };

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "template.create".to_string(),
            resource_type: "template".to_string(),
            resource_id: template.id,
            org_id,
            project_id,
            details: json!({
                "name": template.name,
                "owner_type": template.owner_type,
                "owner_id": template.owner_id,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(template)))
}

/// Get a template by ID
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/templates/{id}",
    tag = "templates",
    operation_id = "template_get",
    params(("id" = Uuid, Path, description = "Template ID")),
    responses(
        (status = 200, description = "Template found", body = Template),
        (status = 404, description = "Template not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.templates.get", skip(state, authz), fields(%id))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Template>, AdminError> {
    let services = get_services(&state)?;

    authz.require("template", "read", None, None, None, None)?;

    let template = services
        .templates
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Template not found".to_string()))?;

    Ok(Json(template))
}

/// Update a template
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/templates/{id}",
    tag = "templates",
    operation_id = "template_update",
    params(("id" = Uuid, Path, description = "Template ID")),
    request_body = UpdateTemplate,
    responses(
        (status = 200, description = "Template updated", body = Template),
        (status = 404, description = "Template not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Template with this name already exists", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.templates.update", skip(state, admin_auth, authz, input), fields(%id))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(id): Path<Uuid>,
    Valid(Json(input)): Valid<Json<UpdateTemplate>>,
) -> Result<Json<Template>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    authz.require("template", "update", None, None, None, None)?;

    // Capture changes for audit log
    let changes = json!({
        "name": input.name,
        "description": input.description,
        "content": input.content.as_ref().map(|_| "<updated>"),
        "metadata": input.metadata,
    });

    let template = services.templates.update(id, input).await?;

    // Extract org_id and project_id from owner for audit log
    let (org_id, project_id) = match template.owner_type {
        TemplateOwnerType::Organization => (Some(template.owner_id), None),
        TemplateOwnerType::Project => (None, Some(template.owner_id)),
        TemplateOwnerType::Team | TemplateOwnerType::User => (None, None),
    };

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "template.update".to_string(),
            resource_type: "template".to_string(),
            resource_id: template.id,
            org_id,
            project_id,
            details: json!({
                "name": template.name,
                "changes": changes,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(template))
}

/// Delete a template
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/templates/{id}",
    tag = "templates",
    operation_id = "template_delete",
    params(("id" = Uuid, Path, description = "Template ID")),
    responses(
        (status = 200, description = "Template deleted"),
        (status = 404, description = "Template not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.templates.delete", skip(state, admin_auth, authz), fields(%id))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(id): Path<Uuid>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    authz.require("template", "delete", None, None, None, None)?;

    // Get template details before deletion for audit log
    let template = services
        .templates
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Template not found".to_string()))?;

    // Extract org_id and project_id from owner for audit log
    let (org_id, project_id) = match template.owner_type {
        TemplateOwnerType::Organization => (Some(template.owner_id), None),
        TemplateOwnerType::Project => (None, Some(template.owner_id)),
        TemplateOwnerType::Team | TemplateOwnerType::User => (None, None),
    };

    // Capture details for audit log before deletion
    let template_name = template.name.clone();
    let template_owner_type = template.owner_type;
    let template_owner_id = template.owner_id;

    services.templates.delete(id).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "template.delete".to_string(),
            resource_type: "template".to_string(),
            resource_id: id,
            org_id,
            project_id,
            details: json!({
                "name": template_name,
                "owner_type": template_owner_type,
                "owner_id": template_owner_id,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}

/// List templates by organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/templates",
    tag = "templates",
    operation_id = "template_list_by_org",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of templates", body = TemplateListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.templates.list_by_org", skip(state, authz, query), fields(%org_slug))]
pub async fn list_by_org(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<TemplateListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    authz.require(
        "template",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.templates.list_by_org(org.id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(TemplateListResponse {
        data: result.items,
        pagination,
    }))
}

/// List templates by team
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/teams/{team_slug}/templates",
    tag = "templates",
    operation_id = "template_list_by_team",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("team_slug" = String, Path, description = "Team slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of templates", body = TemplateListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or team not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.templates.list_by_team", skip(state, authz, query), fields(%org_slug, %team_slug))]
pub async fn list_by_team(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, team_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<TemplateListResponse>, AdminError> {
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
        "template",
        "list",
        None,
        Some(&org.id.to_string()),
        Some(&team.id.to_string()),
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .templates
        .list_by_owner(TemplateOwnerType::Team, team.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(TemplateListResponse {
        data: result.items,
        pagination,
    }))
}

/// List templates by project
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}/templates",
    tag = "templates",
    operation_id = "template_list_by_project",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of templates", body = TemplateListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.templates.list_by_project", skip(state, authz, query), fields(%org_slug, %project_slug))]
pub async fn list_by_project(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<TemplateListResponse>, AdminError> {
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
        "template",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        Some(&project.id.to_string()),
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .templates
        .list_by_owner(TemplateOwnerType::Project, project.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(TemplateListResponse {
        data: result.items,
        pagination,
    }))
}

/// List templates by user
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}/templates",
    tag = "templates",
    operation_id = "template_list_by_user",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of templates", body = TemplateListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.templates.list_by_user", skip(state, authz, query), fields(%user_id))]
pub async fn list_by_user(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<TemplateListResponse>, AdminError> {
    let services = get_services(&state)?;

    authz.require("template", "list", None, None, None, None)?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .templates
        .list_by_owner(TemplateOwnerType::User, user_id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(TemplateListResponse {
        data: result.items,
        pagination,
    }))
}
