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
    models::{CreateAuditLog, CreateProject, MembershipSource, Project, UpdateProject},
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of projects
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProjectListResponse {
    /// List of projects
    pub data: Vec<Project>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Create a project in an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/projects",
    tag = "projects",
    operation_id = "project_create",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = CreateProject,
    responses(
        (status = 201, description = "Project created", body = Project),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Conflict (slug already exists)", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.projects.create", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<CreateProject>>,
) -> Result<(StatusCode, Json<Project>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug to get its ID
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require permission to create projects in this org (and optionally team)
    authz.require(
        "project",
        "create",
        None,
        Some(&org.id.to_string()),
        input.team_id.as_ref().map(|t| t.to_string()).as_deref(),
        None,
    )?;

    let project = services.projects.create(org.id, input).await?;

    // Auto-add the creator as an admin member of the project
    if let Some(user_id) = admin_auth.identity.user_id {
        let _ = services
            .users
            .add_to_project(user_id, project.id, "admin", MembershipSource::Manual)
            .await;
    }

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "project.create".to_string(),
            resource_type: "project".to_string(),
            resource_id: project.id,
            org_id: Some(org.id),
            project_id: Some(project.id),
            details: json!({
                "name": project.name,
                "slug": project.slug,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(project)))
}

/// Get a project by slug
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}",
    tag = "projects",
    operation_id = "project_get",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
    ),
    responses(
        (status = 200, description = "Project found", body = Project),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.projects.get", skip(state, authz), fields(%org_slug, %project_slug))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug)): Path<(String, String)>,
) -> Result<Json<Project>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
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

    // Require read permission on the project
    authz.require(
        "project",
        "read",
        Some(&project.id.to_string()),
        Some(&org.id.to_string()),
        project.team_id.as_ref().map(|t| t.to_string()).as_deref(),
        Some(&project.id.to_string()),
    )?;

    Ok(Json(project))
}

/// List projects in an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/projects",
    tag = "projects",
    operation_id = "project_list",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of projects", body = ProjectListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.projects.list", skip(state, authz, query), fields(%org_slug))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ProjectListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on the org to list projects
    authz.require(
        "project",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.projects.list_by_org(org.id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(ProjectListResponse {
        data: result.items,
        pagination,
    }))
}

/// Update a project
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}",
    tag = "projects",
    operation_id = "project_update",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
    ),
    request_body = UpdateProject,
    responses(
        (status = 200, description = "Project updated", body = Project),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.projects.update", skip(state, admin_auth, authz, input), fields(%org_slug, %project_slug))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Valid(Json(input)): Valid<Json<UpdateProject>>,
) -> Result<Json<Project>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

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

    // Require update permission on the project
    authz.require(
        "project",
        "update",
        Some(&project.id.to_string()),
        Some(&org.id.to_string()),
        project.team_id.as_ref().map(|t| t.to_string()).as_deref(),
        Some(&project.id.to_string()),
    )?;

    // Capture changes for audit log
    let changes = json!({
        "name": input.name,
    });

    let updated = services.projects.update(project.id, input).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "project.update".to_string(),
            resource_type: "project".to_string(),
            resource_id: project.id,
            org_id: Some(org.id),
            project_id: Some(project.id),
            details: changes,
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(updated))
}

/// Delete a project
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}",
    tag = "projects",
    operation_id = "project_delete",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
    ),
    responses(
        (status = 200, description = "Project deleted"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.projects.delete", skip(state, admin_auth, authz), fields(%org_slug, %project_slug))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, project_slug)): Path<(String, String)>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

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

    // Require delete permission on the project (typically org admin or team admin)
    authz.require(
        "project",
        "delete",
        Some(&project.id.to_string()),
        Some(&org.id.to_string()),
        project.team_id.as_ref().map(|t| t.to_string()).as_deref(),
        Some(&project.id.to_string()),
    )?;

    // Capture details for audit log before deletion
    let project_id = project.id;
    let project_name = project.name.clone();
    let project_slug_val = project.slug.clone();

    services.projects.delete(project_id).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "project.delete".to_string(),
            resource_type: "project".to_string(),
            resource_id: project_id,
            org_id: Some(org.id),
            project_id: Some(project_id),
            details: json!({
                "name": project_name,
                "slug": project_slug_val,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}
