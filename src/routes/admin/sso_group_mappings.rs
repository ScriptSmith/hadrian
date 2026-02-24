use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_valid::Valid;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use super::{AuditActor, error::AdminError, organizations::ListQuery};
use crate::{
    AppState,
    db::ListParams,
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    models::{CreateAuditLog, CreateSsoGroupMapping, SsoGroupMapping, UpdateSsoGroupMapping},
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of SSO group mappings
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SsoGroupMappingListResponse {
    /// List of SSO group mappings
    pub data: Vec<SsoGroupMapping>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

/// Request to test SSO group mapping resolution
#[derive(Debug, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TestMappingRequest {
    /// SSO connection name (defaults to 'default')
    #[serde(default = "default_connection_name")]
    pub sso_connection_name: String,
    /// List of IdP group names to test
    #[validate(length(min = 1, message = "At least one IdP group is required"))]
    pub idp_groups: Vec<String>,
    /// Default role to use for mappings without a role (defaults to 'member')
    #[serde(default = "default_role")]
    pub default_role: String,
}

fn default_connection_name() -> String {
    "default".to_string()
}

fn default_role() -> String {
    "member".to_string()
}

/// A resolved team membership from the test
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TestMappingResult {
    /// The IdP group that matched
    pub idp_group: String,
    /// Team ID the user would be added to
    pub team_id: Uuid,
    /// Team name (for display)
    pub team_name: String,
    /// Role that would be assigned
    pub role: String,
}

/// Response from testing SSO group mapping resolution
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TestMappingResponse {
    /// Groups that matched mappings and their resolved teams/roles
    pub resolved: Vec<TestMappingResult>,
    /// Groups that did not match any mapping
    pub unmapped_groups: Vec<String>,
}

/// Export format for SSO group mappings
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// JSON format (default)
    #[default]
    Json,
    /// CSV format
    Csv,
}

/// Query parameters for exporting SSO group mappings
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams))]
pub struct ExportQuery {
    /// Export format (json or csv, defaults to json)
    #[serde(default)]
    pub format: ExportFormat,
    /// Filter by SSO connection name (optional)
    pub sso_connection_name: Option<String>,
}

/// A single mapping entry in the export
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ExportMappingEntry {
    /// The IdP group name
    pub idp_group: String,
    /// Team ID (if assigned to a team)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<Uuid>,
    /// Team name for reference (if assigned to a team)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_name: Option<String>,
    /// Role to assign
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Priority for role precedence (higher = wins when multiple mappings target same team)
    pub priority: i32,
    /// SSO connection name
    pub sso_connection_name: String,
}

/// Response for JSON export of SSO group mappings
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ExportResponse {
    /// Organization slug
    pub organization: String,
    /// Export timestamp
    pub exported_at: chrono::DateTime<chrono::Utc>,
    /// List of mappings
    pub mappings: Vec<ExportMappingEntry>,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Validate that a role is not a reserved system role.
///
/// Roles starting with `_` are reserved for internal use (e.g., `_system_bootstrap`
/// for bootstrap authentication). IdPs should never be able to assign these roles
/// to prevent privilege escalation.
fn validate_role_not_reserved(role: Option<&str>) -> Result<(), AdminError> {
    if let Some(r) = role
        && r.starts_with('_')
    {
        return Err(AdminError::BadRequest(format!(
            "Role '{}' is reserved for internal use. Roles starting with '_' cannot be assigned via SSO mappings.",
            r
        )));
    }
    Ok(())
}

// ============================================================================
// SSO Group Mapping CRUD endpoints
// ============================================================================

/// List SSO group mappings for an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/sso-group-mappings",
    tag = "sso",
    operation_id = "sso_group_mapping_list",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of SSO group mappings", body = SsoGroupMappingListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sso_group_mappings.list", skip(state, authz, query), fields(%org_slug))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<SsoGroupMappingListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on org SSO settings
    authz.require(
        "sso_group_mapping",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .sso_group_mappings
        .list_by_org(org.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(SsoGroupMappingListResponse {
        data: result.items,
        pagination,
    }))
}

/// Create a new SSO group mapping
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/sso-group-mappings",
    tag = "sso",
    operation_id = "sso_group_mapping_create",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = CreateSsoGroupMapping,
    responses(
        (status = 201, description = "SSO group mapping created", body = SsoGroupMapping),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sso_group_mappings.create", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<CreateSsoGroupMapping>>,
) -> Result<(StatusCode, Json<SsoGroupMapping>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require create permission on org SSO settings
    authz.require(
        "sso_group_mapping",
        "create",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Validate team_id belongs to the org if provided
    if let Some(team_id) = input.team_id {
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

    // Validate role is not a reserved system role
    validate_role_not_reserved(input.role.as_deref())?;

    let mapping = services
        .sso_group_mappings
        .create(org.id, input.clone())
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "sso_group_mapping.create".to_string(),
            resource_type: "sso_group_mapping".to_string(),
            resource_id: mapping.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "idp_group": input.idp_group,
                "team_id": input.team_id,
                "role": input.role,
                "priority": input.priority,
                "sso_connection_name": input.sso_connection_name,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(mapping)))
}

/// Get an SSO group mapping by ID
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/sso-group-mappings/{mapping_id}",
    tag = "sso",
    operation_id = "sso_group_mapping_get",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("mapping_id" = Uuid, Path, description = "Mapping ID"),
    ),
    responses(
        (status = 200, description = "SSO group mapping found", body = SsoGroupMapping),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or mapping not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sso_group_mappings.get", skip(state, authz), fields(%org_slug, %mapping_id))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, mapping_id)): Path<(String, Uuid)>,
) -> Result<Json<SsoGroupMapping>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get mapping
    let mapping = services
        .sso_group_mappings
        .get_by_id(mapping_id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!("SSO group mapping '{}' not found", mapping_id))
        })?;

    // Verify mapping belongs to this org
    if mapping.org_id != org.id {
        return Err(AdminError::NotFound(format!(
            "SSO group mapping '{}' not found in organization '{}'",
            mapping_id, org_slug
        )));
    }

    // Require read permission
    authz.require(
        "sso_group_mapping",
        "read",
        Some(&mapping.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    Ok(Json(mapping))
}

/// Update an SSO group mapping
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{org_slug}/sso-group-mappings/{mapping_id}",
    tag = "sso",
    operation_id = "sso_group_mapping_update",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("mapping_id" = Uuid, Path, description = "Mapping ID"),
    ),
    request_body = UpdateSsoGroupMapping,
    responses(
        (status = 200, description = "SSO group mapping updated", body = SsoGroupMapping),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or mapping not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sso_group_mappings.update", skip(state, admin_auth, authz, input), fields(%org_slug, %mapping_id))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, mapping_id)): Path<(String, Uuid)>,
    Valid(Json(input)): Valid<Json<UpdateSsoGroupMapping>>,
) -> Result<Json<SsoGroupMapping>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get existing mapping
    let mapping = services
        .sso_group_mappings
        .get_by_id(mapping_id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!("SSO group mapping '{}' not found", mapping_id))
        })?;

    // Verify mapping belongs to this org
    if mapping.org_id != org.id {
        return Err(AdminError::NotFound(format!(
            "SSO group mapping '{}' not found in organization '{}'",
            mapping_id, org_slug
        )));
    }

    // Require update permission
    authz.require(
        "sso_group_mapping",
        "update",
        Some(&mapping.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Validate team_id belongs to the org if being updated
    if let Some(Some(team_id)) = input.team_id {
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

    // Validate role is not a reserved system role (if being updated)
    if let Some(role) = &input.role {
        validate_role_not_reserved(role.as_deref())?;
    }

    let updated = services
        .sso_group_mappings
        .update(mapping_id, input.clone())
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "sso_group_mapping.update".to_string(),
            resource_type: "sso_group_mapping".to_string(),
            resource_id: mapping_id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "idp_group": input.idp_group,
                "team_id": input.team_id,
                "role": input.role,
                "priority": input.priority,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(updated))
}

/// Delete an SSO group mapping
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/sso-group-mappings/{mapping_id}",
    tag = "sso",
    operation_id = "sso_group_mapping_delete",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("mapping_id" = Uuid, Path, description = "Mapping ID"),
    ),
    responses(
        (status = 200, description = "SSO group mapping deleted"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or mapping not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sso_group_mappings.delete", skip(state, admin_auth, authz), fields(%org_slug, %mapping_id))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, mapping_id)): Path<(String, Uuid)>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get existing mapping
    let mapping = services
        .sso_group_mappings
        .get_by_id(mapping_id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!("SSO group mapping '{}' not found", mapping_id))
        })?;

    // Verify mapping belongs to this org
    if mapping.org_id != org.id {
        return Err(AdminError::NotFound(format!(
            "SSO group mapping '{}' not found in organization '{}'",
            mapping_id, org_slug
        )));
    }

    // Require delete permission
    authz.require(
        "sso_group_mapping",
        "delete",
        Some(&mapping.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Capture details for audit log before deletion
    let idp_group = mapping.idp_group.clone();
    let team_id = mapping.team_id;

    services.sso_group_mappings.delete(mapping_id).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "sso_group_mapping.delete".to_string(),
            resource_type: "sso_group_mapping".to_string(),
            resource_id: mapping_id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "idp_group": idp_group,
                "team_id": team_id,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}

/// Test SSO group mapping resolution
///
/// Given a list of IdP group names, returns what teams/roles a user with those
/// groups would be resolved to. This is useful for debugging and verifying
/// mapping configuration.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/sso-group-mappings/test",
    tag = "sso",
    operation_id = "sso_group_mapping_test",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = TestMappingRequest,
    responses(
        (status = 200, description = "Mapping test results", body = TestMappingResponse),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sso_group_mappings.test", skip(state, authz, input), fields(%org_slug))]
pub async fn test(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<TestMappingRequest>>,
) -> Result<Json<TestMappingResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on org SSO settings (same as list)
    authz.require(
        "sso_group_mapping",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Resolve memberships using the service
    let memberships = services
        .sso_group_mappings
        .resolve_memberships(
            &input.sso_connection_name,
            org.id,
            &input.idp_groups,
            &input.default_role,
        )
        .await?;

    // Collect unique team IDs and fetch team names in a single batch query
    let unique_team_ids: Vec<Uuid> = memberships
        .iter()
        .map(|m| m.team_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let teams = services.teams.get_by_ids(&unique_team_ids).await?;
    let team_names: std::collections::HashMap<Uuid, String> =
        teams.into_iter().map(|t| (t.id, t.name)).collect();

    // Build resolved results
    let resolved: Vec<TestMappingResult> = memberships
        .into_iter()
        .map(|m| TestMappingResult {
            idp_group: m.from_idp_group,
            team_id: m.team_id,
            team_name: team_names
                .get(&m.team_id)
                .cloned()
                .unwrap_or_else(|| m.team_id.to_string()),
            role: m.role,
        })
        .collect();

    // Determine which groups were not mapped
    let mapped_groups: std::collections::HashSet<&str> =
        resolved.iter().map(|r| r.idp_group.as_str()).collect();
    let unmapped_groups: Vec<String> = input
        .idp_groups
        .iter()
        .filter(|g| !mapped_groups.contains(g.as_str()))
        .cloned()
        .collect();

    Ok(Json(TestMappingResponse {
        resolved,
        unmapped_groups,
    }))
}

/// Export SSO group mappings
///
/// Exports all SSO group mappings for an organization in JSON or CSV format.
/// Useful for backup, migration, or bulk editing in a spreadsheet.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/sso-group-mappings/export",
    tag = "sso",
    operation_id = "sso_group_mapping_export",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ExportQuery,
    ),
    responses(
        (status = 200, description = "Export of SSO group mappings", body = ExportResponse, content_type = "application/json"),
        (status = 200, description = "Export of SSO group mappings as CSV", content_type = "text/csv"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sso_group_mappings.export", skip(state, authz, query), fields(%org_slug))]
pub async fn export(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on org SSO settings (same as list)
    authz.require(
        "sso_group_mapping",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Fetch all mappings (use a high limit to get everything)
    let mappings = if let Some(ref conn_name) = query.sso_connection_name {
        services
            .sso_group_mappings
            .list_by_connection(
                conn_name,
                org.id,
                ListParams {
                    limit: Some(10000),
                    ..Default::default()
                },
            )
            .await?
            .items
    } else {
        services
            .sso_group_mappings
            .list_by_org(
                org.id,
                ListParams {
                    limit: Some(10000),
                    ..Default::default()
                },
            )
            .await?
            .items
    };

    // Collect unique team IDs and fetch team names in a single batch query
    let unique_team_ids: Vec<Uuid> = mappings
        .iter()
        .filter_map(|m| m.team_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let teams = services.teams.get_by_ids(&unique_team_ids).await?;
    let team_names: std::collections::HashMap<Uuid, String> =
        teams.into_iter().map(|t| (t.id, t.name)).collect();

    // Convert to export entries
    let entries: Vec<ExportMappingEntry> = mappings
        .into_iter()
        .map(|m| ExportMappingEntry {
            idp_group: m.idp_group,
            team_id: m.team_id,
            team_name: m.team_id.and_then(|id| team_names.get(&id).cloned()),
            role: m.role,
            priority: m.priority,
            sso_connection_name: m.sso_connection_name,
        })
        .collect();

    match query.format {
        ExportFormat::Json => {
            let response = ExportResponse {
                organization: org_slug,
                exported_at: chrono::Utc::now(),
                mappings: entries,
            };
            Ok(Json(response).into_response())
        }
        ExportFormat::Csv => {
            let mut csv = String::new();
            // Header row
            csv.push_str("idp_group,team_id,team_name,role,priority,sso_connection_name\n");
            // Data rows
            for entry in entries {
                csv.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    escape_csv(&entry.idp_group),
                    entry.team_id.map(|id| id.to_string()).unwrap_or_default(),
                    escape_csv(&entry.team_name.unwrap_or_default()),
                    escape_csv(&entry.role.unwrap_or_default()),
                    entry.priority,
                    escape_csv(&entry.sso_connection_name),
                ));
            }

            Ok((
                [
                    (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8"),
                    (
                        axum::http::header::CONTENT_DISPOSITION,
                        &format!(
                            "attachment; filename=\"sso-group-mappings-{}.csv\"",
                            org_slug
                        ),
                    ),
                ],
                csv,
            )
                .into_response())
        }
    }
}

/// Escape a string for CSV format (RFC 4180 compliant)
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Conflict resolution strategy for import
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ImportConflictStrategy {
    /// Skip mappings that already exist (default)
    #[default]
    Skip,
    /// Overwrite existing mappings with imported values
    Overwrite,
    /// Return an error if any mapping already exists
    Error,
}

/// A single mapping entry to import
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ImportMappingEntry {
    /// The IdP group name exactly as it appears in the groups claim
    #[validate(length(min = 1, max = 512))]
    pub idp_group: String,
    /// Team ID to add users to (optional, can use team_name instead)
    pub team_id: Option<Uuid>,
    /// Role to assign
    #[validate(length(min = 1, max = 32))]
    pub role: Option<String>,
    /// Priority for role precedence (higher = wins when multiple mappings target same team)
    /// Defaults to 0 if not specified.
    #[serde(default)]
    pub priority: i32,
    /// SSO connection name (defaults to 'default')
    #[serde(default = "default_connection_name")]
    pub sso_connection_name: String,
}

/// Request to import SSO group mappings
#[derive(Debug, Serialize, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ImportRequest {
    /// List of mappings to import
    #[validate(length(
        min = 1,
        max = 1000,
        message = "Must import between 1 and 1000 mappings"
    ))]
    #[validate(nested)]
    pub mappings: Vec<ImportMappingEntry>,
    /// How to handle conflicts with existing mappings
    #[serde(default)]
    pub on_conflict: ImportConflictStrategy,
}

/// Details about an import error
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ImportError {
    /// Index of the mapping in the input array (0-based)
    pub index: usize,
    /// The IdP group that caused the error
    pub idp_group: String,
    /// Error message
    pub error: String,
}

/// Response from importing SSO group mappings
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ImportResponse {
    /// Number of mappings created
    pub created: usize,
    /// Number of mappings updated (overwritten)
    pub updated: usize,
    /// Number of mappings skipped (already existed)
    pub skipped: usize,
    /// List of errors encountered
    pub errors: Vec<ImportError>,
}

/// Import SSO group mappings
///
/// Bulk import SSO group mappings from a JSON payload. Useful for migrating
/// mappings between environments or restoring from a backup.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/sso-group-mappings/import",
    tag = "sso",
    operation_id = "sso_group_mapping_import",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = ImportRequest,
    responses(
        (status = 200, description = "Import completed", body = ImportResponse),
        (status = 400, description = "Invalid request or conflict error", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sso_group_mappings.import", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn import(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<ImportRequest>>,
) -> Result<Json<ImportResponse>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require create permission on org SSO settings
    authz.require(
        "sso_group_mapping",
        "create",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Pre-validate all entries:
    // - team_ids exist in this org
    // - roles are not reserved system roles
    let mut team_cache: std::collections::HashMap<Uuid, bool> = std::collections::HashMap::new();
    let mut validation_errors: Vec<ImportError> = Vec::new();

    for (index, entry) in input.mappings.iter().enumerate() {
        // Validate team_id
        if let Some(team_id) = entry.team_id {
            if let std::collections::hash_map::Entry::Vacant(e) = team_cache.entry(team_id) {
                let valid = if let Some(team) = services.teams.get_by_id(team_id).await? {
                    team.org_id == org.id
                } else {
                    false
                };
                e.insert(valid);
            }
            if !team_cache.get(&team_id).copied().unwrap_or(false) {
                validation_errors.push(ImportError {
                    index,
                    idp_group: entry.idp_group.clone(),
                    error: format!("Team '{}' not found in organization", team_id),
                });
            }
        }

        // Validate role is not a reserved system role
        if let Some(ref role) = entry.role
            && role.starts_with('_')
        {
            validation_errors.push(ImportError {
                index,
                idp_group: entry.idp_group.clone(),
                error: format!(
                    "Role '{}' is reserved for internal use. Roles starting with '_' cannot be assigned via SSO mappings.",
                    role
                ),
            });
        }
    }

    // If there are validation errors, return them immediately
    if !validation_errors.is_empty() {
        return Ok(Json(ImportResponse {
            created: 0,
            updated: 0,
            skipped: 0,
            errors: validation_errors,
        }));
    }

    // Fetch existing mappings to check for conflicts
    let existing = services
        .sso_group_mappings
        .list_by_org(
            org.id,
            ListParams {
                limit: Some(10000),
                ..Default::default()
            },
        )
        .await?
        .items;

    // Build a map of existing mappings: (sso_connection_name, idp_group, team_id) -> mapping_id
    let existing_map: std::collections::HashMap<(String, String, Option<Uuid>), Uuid> = existing
        .iter()
        .map(|m| {
            (
                (
                    m.sso_connection_name.clone(),
                    m.idp_group.clone(),
                    m.team_id,
                ),
                m.id,
            )
        })
        .collect();

    let mut created = 0;
    let mut updated = 0;
    let mut skipped = 0;
    let mut errors: Vec<ImportError> = Vec::new();

    for (index, entry) in input.mappings.into_iter().enumerate() {
        let key = (
            entry.sso_connection_name.clone(),
            entry.idp_group.clone(),
            entry.team_id,
        );

        if let Some(&existing_id) = existing_map.get(&key) {
            // Mapping already exists
            match input.on_conflict {
                ImportConflictStrategy::Skip => {
                    skipped += 1;
                }
                ImportConflictStrategy::Overwrite => {
                    // Update the existing mapping
                    let update = UpdateSsoGroupMapping {
                        idp_group: Some(entry.idp_group.clone()),
                        team_id: Some(entry.team_id),
                        role: Some(entry.role.clone()),
                        priority: Some(entry.priority),
                    };
                    if let Err(e) = services
                        .sso_group_mappings
                        .update(existing_id, update)
                        .await
                    {
                        errors.push(ImportError {
                            index,
                            idp_group: entry.idp_group,
                            error: e.to_string(),
                        });
                    } else {
                        updated += 1;
                    }
                }
                ImportConflictStrategy::Error => {
                    errors.push(ImportError {
                        index,
                        idp_group: entry.idp_group,
                        error: "Mapping already exists".to_string(),
                    });
                }
            }
        } else {
            // Create new mapping
            let create = CreateSsoGroupMapping {
                sso_connection_name: entry.sso_connection_name,
                idp_group: entry.idp_group.clone(),
                team_id: entry.team_id,
                role: entry.role,
                priority: entry.priority,
            };
            match services.sso_group_mappings.create(org.id, create).await {
                Ok(_) => created += 1,
                Err(e) => {
                    errors.push(ImportError {
                        index,
                        idp_group: entry.idp_group,
                        error: e.to_string(),
                    });
                }
            }
        }
    }

    // Log audit event for the import (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "sso_group_mapping.import".to_string(),
            resource_type: "sso_group_mapping".to_string(),
            resource_id: org.id, // Use org ID as the resource
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "created": created,
                "updated": updated,
                "skipped": skipped,
                "errors": errors.len(),
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(ImportResponse {
        created,
        updated,
        skipped,
        errors,
    }))
}
