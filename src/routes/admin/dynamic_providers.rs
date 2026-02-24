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
    models::{
        ConnectivityTestResponse, CreateAuditLog, CreateDynamicProvider, DynamicProvider,
        DynamicProviderResponse, ProviderOwner, UpdateDynamicProvider,
    },
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of dynamic providers
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DynamicProviderListResponse {
    /// List of dynamic providers
    pub data: Vec<DynamicProviderResponse>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Extract authz scope parameters (org_id, team_id, project_id) from a provider owner.
async fn owner_authz_scope(
    owner: &ProviderOwner,
    services: &Services,
) -> (Option<String>, Option<String>, Option<String>) {
    match owner {
        ProviderOwner::Organization { org_id } => (Some(org_id.to_string()), None, None),
        ProviderOwner::Project { project_id } => (None, None, Some(project_id.to_string())),
        ProviderOwner::Team { team_id } => {
            let org_id = services
                .teams
                .get_by_id(*team_id)
                .await
                .ok()
                .flatten()
                .map(|t| t.org_id.to_string());
            (org_id, Some(team_id.to_string()), None)
        }
        ProviderOwner::User { .. } => (None, None, None),
    }
}

/// Extract org_id and project_id from owner for audit logging.
/// For team-owned providers, resolves the team's parent organization.
async fn owner_audit_scope(
    owner: &ProviderOwner,
    services: &Services,
) -> (Option<Uuid>, Option<Uuid>) {
    match owner {
        ProviderOwner::Organization { org_id } => (Some(*org_id), None),
        ProviderOwner::Project { project_id } => (None, Some(*project_id)),
        ProviderOwner::Team { team_id } => {
            // Resolve team's parent org for audit completeness
            let org_id = services
                .teams
                .get_by_id(*team_id)
                .await
                .ok()
                .flatten()
                .map(|t| t.org_id);
            (org_id, None)
        }
        ProviderOwner::User { .. } => (None, None),
    }
}

/// Create a dynamic provider
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/dynamic-providers",
    tag = "dynamic-providers",
    operation_id = "dynamic_provider_create",
    request_body = CreateDynamicProvider,
    responses(
        (status = 201, description = "Dynamic provider created", body = DynamicProviderResponse),
        (status = 404, description = "Owner not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Valid(Json(input)): Valid<Json<CreateDynamicProvider>>,
) -> Result<(StatusCode, Json<DynamicProviderResponse>), AdminError> {
    let services = get_services(&state)?;
    let (authz_org, authz_team, authz_project) = owner_authz_scope(&input.owner, services).await;
    authz.require(
        "dynamic_provider",
        "create",
        None,
        authz_org.as_deref(),
        authz_team.as_deref(),
        authz_project.as_deref(),
    )?;
    let actor = AuditActor::from(&admin_auth);

    // Validate provider type
    crate::services::validate_provider_type(&input.provider_type)?;

    // Validate provider-specific config and base URL (SSRF protection)
    crate::services::validate_provider_config_with_url(
        &input.provider_type,
        &input.base_url,
        input.config.as_ref(),
        input.api_key.as_deref(),
        state.config.server.allow_loopback_urls,
    )?;

    let provider = services
        .providers
        .create(input, state.secrets.as_ref())
        .await?;

    let (org_id, project_id) = owner_audit_scope(&provider.owner, services).await;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "dynamic_provider.create".to_string(),
            resource_type: "dynamic_provider".to_string(),
            resource_id: provider.id,
            org_id,
            project_id,
            details: json!({
                "name": provider.name,
                "provider_type": provider.provider_type,
                "base_url": provider.base_url,
                "owner": provider.owner,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(provider.into())))
}

/// Get a dynamic provider by ID
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/dynamic-providers/{id}",
    tag = "dynamic-providers",
    operation_id = "dynamic_provider_get",
    params(("id" = Uuid, Path, description = "Dynamic provider ID")),
    responses(
        (status = 200, description = "Dynamic provider found", body = DynamicProviderResponse),
        (status = 404, description = "Dynamic provider not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<DynamicProviderResponse>, AdminError> {
    let services = get_services(&state)?;

    let provider = services
        .providers
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Dynamic provider not found".to_string()))?;

    let (authz_org, authz_team, authz_project) = owner_authz_scope(&provider.owner, services).await;
    authz.require(
        "dynamic_provider",
        "read",
        Some(&id.to_string()),
        authz_org.as_deref(),
        authz_team.as_deref(),
        authz_project.as_deref(),
    )?;

    Ok(Json(provider.into()))
}

/// Update a dynamic provider
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/dynamic-providers/{id}",
    tag = "dynamic-providers",
    operation_id = "dynamic_provider_update",
    params(("id" = Uuid, Path, description = "Dynamic provider ID")),
    request_body = UpdateDynamicProvider,
    responses(
        (status = 200, description = "Dynamic provider updated", body = DynamicProviderResponse),
        (status = 404, description = "Dynamic provider not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(id): Path<Uuid>,
    Valid(Json(input)): Valid<Json<UpdateDynamicProvider>>,
) -> Result<Json<DynamicProviderResponse>, AdminError> {
    let services = get_services(&state)?;

    // Fetch existing provider first to check scope-aware authorization
    let existing = services
        .providers
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Dynamic provider not found".to_string()))?;

    let (authz_org, authz_team, authz_project) = owner_authz_scope(&existing.owner, services).await;
    authz.require(
        "dynamic_provider",
        "update",
        Some(&id.to_string()),
        authz_org.as_deref(),
        authz_team.as_deref(),
        authz_project.as_deref(),
    )?;

    let actor = AuditActor::from(&admin_auth);

    // Validate base URL against SSRF if being updated
    if let Some(ref base_url) = input.base_url
        && !base_url.is_empty()
    {
        crate::validation::validate_base_url(base_url, state.config.server.allow_loopback_urls)
            .map_err(|e| AdminError::Validation(format!("Invalid base URL: {e}")))?;
    }

    // Capture changes for audit log
    let changes = json!({
        "base_url": input.base_url,
        "api_key": input.api_key.as_ref().map(|_| "****"),
        "models": input.models,
        "is_enabled": input.is_enabled,
    });

    let provider = services
        .providers
        .update(id, input, state.secrets.as_ref())
        .await?;

    let (org_id, project_id) = owner_audit_scope(&provider.owner, services).await;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "dynamic_provider.update".to_string(),
            resource_type: "dynamic_provider".to_string(),
            resource_id: provider.id,
            org_id,
            project_id,
            details: json!({
                "name": provider.name,
                "changes": changes,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(provider.into()))
}

/// Delete a dynamic provider
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/dynamic-providers/{id}",
    tag = "dynamic-providers",
    operation_id = "dynamic_provider_delete",
    params(("id" = Uuid, Path, description = "Dynamic provider ID")),
    responses(
        (status = 200, description = "Dynamic provider deleted"),
        (status = 404, description = "Dynamic provider not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(id): Path<Uuid>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;

    // Get provider details before deletion for authz and audit log
    let provider = services
        .providers
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Dynamic provider not found".to_string()))?;

    let (authz_org, authz_team, authz_project) = owner_authz_scope(&provider.owner, services).await;
    authz.require(
        "dynamic_provider",
        "delete",
        Some(&id.to_string()),
        authz_org.as_deref(),
        authz_team.as_deref(),
        authz_project.as_deref(),
    )?;

    let actor = AuditActor::from(&admin_auth);
    let (org_id, project_id) = owner_audit_scope(&provider.owner, services).await;

    // Capture details for audit log before deletion
    let provider_name = provider.name.clone();
    let provider_type = provider.provider_type.clone();
    let provider_owner = provider.owner.clone();

    services
        .providers
        .delete(id, state.secrets.as_ref())
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "dynamic_provider.delete".to_string(),
            resource_type: "dynamic_provider".to_string(),
            resource_id: id,
            org_id,
            project_id,
            details: json!({
                "name": provider_name,
                "provider_type": provider_type,
                "owner": provider_owner,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}

/// List dynamic providers by organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/dynamic-providers",
    tag = "dynamic-providers",
    operation_id = "dynamic_provider_list_by_org",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of dynamic providers", body = DynamicProviderListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list_by_org(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<DynamicProviderListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    authz.require(
        "dynamic_provider",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.providers.list_by_org(org.id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(DynamicProviderListResponse {
        data: result.items.into_iter().map(Into::into).collect(),
        pagination,
    }))
}

/// List dynamic providers by project
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}/dynamic-providers",
    tag = "dynamic-providers",
    operation_id = "dynamic_provider_list_by_project",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of dynamic providers", body = DynamicProviderListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list_by_project(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<DynamicProviderListResponse>, AdminError> {
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
        "dynamic_provider",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        Some(&project.id.to_string()),
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .providers
        .list_by_project(project.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(DynamicProviderListResponse {
        data: result.items.into_iter().map(Into::into).collect(),
        pagination,
    }))
}

/// List dynamic providers by user
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}/dynamic-providers",
    tag = "dynamic-providers",
    operation_id = "dynamic_provider_list_by_user",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of dynamic providers", body = DynamicProviderListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list_by_user(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<DynamicProviderListResponse>, AdminError> {
    let services = get_services(&state)?;
    // Resolve user's org for scope context (best-effort)
    let org_id = services
        .users
        .get_org_memberships_for_user(user_id)
        .await
        .ok()
        .and_then(|m| m.into_iter().next())
        .map(|m| m.org_id.to_string());
    authz.require(
        "dynamic_provider",
        "list",
        None,
        org_id.as_deref(),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.providers.list_by_user(user_id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(DynamicProviderListResponse {
        data: result.items.into_iter().map(Into::into).collect(),
        pagination,
    }))
}

/// Test connectivity for an existing dynamic provider
///
/// Sends a lightweight request to the provider to verify the API key and endpoint are working.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/dynamic-providers/{id}/test",
    tag = "dynamic-providers",
    operation_id = "dynamic_provider_test",
    params(("id" = Uuid, Path, description = "Dynamic provider ID")),
    responses(
        (status = 200, description = "Connectivity test result", body = ConnectivityTestResponse),
        (status = 404, description = "Provider not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn test_connectivity(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConnectivityTestResponse>, AdminError> {
    let services = get_services(&state)?;

    let provider = services
        .providers
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Dynamic provider not found".to_string()))?;

    let (authz_org, authz_team, authz_project) = owner_authz_scope(&provider.owner, services).await;
    authz.require(
        "dynamic_provider",
        "read",
        Some(&id.to_string()),
        authz_org.as_deref(),
        authz_team.as_deref(),
        authz_project.as_deref(),
    )?;

    let result = crate::services::DynamicProviderService::run_connectivity_test(
        &provider,
        &state,
        state.secrets.as_ref(),
    )
    .await;
    Ok(Json(result))
}

/// Test credentials before creating a dynamic provider
///
/// Accepts the same body as the create endpoint but does not persist anything.
/// Useful for validating API keys and endpoints before committing.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/dynamic-providers/test-credentials",
    tag = "dynamic-providers",
    operation_id = "dynamic_provider_test_credentials",
    request_body = CreateDynamicProvider,
    responses(
        (status = 200, description = "Connectivity test result", body = ConnectivityTestResponse),
        (status = 422, description = "Validation error", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn test_credentials(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Valid(Json(input)): Valid<Json<CreateDynamicProvider>>,
) -> Result<Json<ConnectivityTestResponse>, AdminError> {
    let services = get_services(&state)?;
    let (authz_org, authz_team, authz_project) = owner_authz_scope(&input.owner, services).await;
    authz.require(
        "dynamic_provider",
        "create",
        None,
        authz_org.as_deref(),
        authz_team.as_deref(),
        authz_project.as_deref(),
    )?;

    // Validate provider type
    crate::services::validate_provider_type(&input.provider_type)?;

    // Validate provider-specific config and base URL (SSRF protection)
    crate::services::validate_provider_config_with_url(
        &input.provider_type,
        &input.base_url,
        input.config.as_ref(),
        input.api_key.as_deref(),
        state.config.server.allow_loopback_urls,
    )?;

    // Build a transient DynamicProvider (not persisted) — use raw key directly
    let provider = DynamicProvider {
        id: Uuid::nil(),
        name: input.name,
        owner: ProviderOwner::User {
            user_id: Uuid::nil(),
        },
        provider_type: input.provider_type,
        base_url: input.base_url,
        api_key_secret_ref: input.api_key,
        config: input.config,
        models: input.models.unwrap_or_default(),
        is_enabled: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    // Pass None for secrets so the raw key is used as-is by the resolver
    let result =
        crate::services::DynamicProviderService::run_connectivity_test(&provider, &state, None)
            .await;
    Ok(Json(result))
}
