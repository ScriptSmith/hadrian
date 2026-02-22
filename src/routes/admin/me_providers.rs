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
    middleware::{AdminAuth, AuthzContext},
    models::{
        ConnectivityTestResponse, CreateAuditLog, CreateDynamicProvider, CreateSelfServiceProvider,
        DynamicProvider, DynamicProviderResponse, ProviderOwner, UpdateDynamicProvider,
    },
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of dynamic providers
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SelfServiceProviderListResponse {
    pub data: Vec<DynamicProviderResponse>,
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

fn get_user_id(admin_auth: &AdminAuth) -> Result<Uuid, AdminError> {
    admin_auth
        .identity
        .user_id
        .ok_or(AdminError::Forbidden("User account required".to_string()))
}

/// Verify that a provider is owned by the given user.
/// Returns 404 (not 403) to prevent provider ID enumeration.
async fn verify_user_owns_provider(
    services: &Services,
    user_id: Uuid,
    provider_id: Uuid,
) -> Result<crate::models::DynamicProvider, AdminError> {
    let provider = services
        .providers
        .get_by_id(provider_id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Provider not found".to_string()))?;

    match &provider.owner {
        ProviderOwner::User {
            user_id: owner_id, ..
        } if *owner_id == user_id => Ok(provider),
        _ => Err(AdminError::NotFound("Provider not found".to_string())),
    }
}

/// List current user's dynamic providers
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/me/providers",
    tag = "me",
    operation_id = "me_providers_list",
    params(ListQuery),
    responses(
        (status = 200, description = "List of user's dynamic providers", body = SelfServiceProviderListResponse),
        (status = 401, description = "User not identified from session", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Query(query): Query<ListQuery>,
) -> Result<Json<SelfServiceProviderListResponse>, AdminError> {
    authz.require("dynamic_provider", "self_list", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.providers.list_by_user(user_id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(SelfServiceProviderListResponse {
        data: result.items.into_iter().map(Into::into).collect(),
        pagination,
    }))
}

/// Create a dynamic provider for the current user
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/me/providers",
    tag = "me",
    operation_id = "me_providers_create",
    request_body = CreateSelfServiceProvider,
    responses(
        (status = 201, description = "Dynamic provider created", body = DynamicProviderResponse),
        (status = 401, description = "User not identified from session", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Provider with this name already exists", body = crate::openapi::ErrorResponse),
        (status = 422, description = "Provider limit exceeded", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Valid(Json(input)): Valid<Json<CreateSelfServiceProvider>>,
) -> Result<(StatusCode, Json<DynamicProviderResponse>), AdminError> {
    authz.require("dynamic_provider", "self_create", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;
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

    // Check per-user provider limit
    let max = state.config.limits.resource_limits.max_providers_per_user;
    if max > 0 {
        let count = services.providers.count_by_user(user_id).await?;
        if count >= max as i64 {
            return Err(AdminError::Validation(format!(
                "Provider limit reached ({max}). Delete an existing provider before creating a new one."
            )));
        }
    }

    let create_input = CreateDynamicProvider {
        name: input.name,
        owner: ProviderOwner::User { user_id },
        provider_type: input.provider_type,
        base_url: input.base_url,
        api_key: input.api_key,
        config: input.config,
        models: input.models,
    };

    let provider = services
        .providers
        .create(create_input, state.secrets.as_ref())
        .await?;

    // Audit log (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "dynamic_provider.self_create".to_string(),
            resource_type: "dynamic_provider".to_string(),
            resource_id: provider.id,
            org_id: None,
            project_id: None,
            details: json!({
                "name": provider.name,
                "provider_type": provider.provider_type,
                "base_url": provider.base_url,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok((StatusCode::CREATED, Json(provider.into())))
}

/// Get a dynamic provider by ID (current user only)
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/me/providers/{id}",
    tag = "me",
    operation_id = "me_providers_get",
    params(("id" = Uuid, Path, description = "Dynamic provider ID")),
    responses(
        (status = 200, description = "Dynamic provider found", body = DynamicProviderResponse),
        (status = 404, description = "Provider not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<DynamicProviderResponse>, AdminError> {
    authz.require("dynamic_provider", "self_read", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;

    let provider = verify_user_owns_provider(services, user_id, id).await?;

    Ok(Json(provider.into()))
}

/// Update a dynamic provider (current user only)
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/me/providers/{id}",
    tag = "me",
    operation_id = "me_providers_update",
    params(("id" = Uuid, Path, description = "Dynamic provider ID")),
    request_body = UpdateDynamicProvider,
    responses(
        (status = 200, description = "Dynamic provider updated", body = DynamicProviderResponse),
        (status = 404, description = "Provider not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
    Valid(Json(input)): Valid<Json<UpdateDynamicProvider>>,
) -> Result<Json<DynamicProviderResponse>, AdminError> {
    authz.require("dynamic_provider", "self_update", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Ownership check
    verify_user_owns_provider(services, user_id, id).await?;

    // Validate base URL against SSRF if being updated
    if let Some(ref base_url) = input.base_url
        && !base_url.is_empty()
    {
        crate::validation::validate_base_url(base_url, state.config.server.allow_loopback_urls)
            .map_err(|e| AdminError::Validation(format!("Invalid base URL: {e}")))?;
    }

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

    // Audit log (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "dynamic_provider.self_update".to_string(),
            resource_type: "dynamic_provider".to_string(),
            resource_id: provider.id,
            org_id: None,
            project_id: None,
            details: json!({
                "name": provider.name,
                "changes": changes,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(provider.into()))
}

/// Delete a dynamic provider (current user only)
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/me/providers/{id}",
    tag = "me",
    operation_id = "me_providers_delete",
    params(("id" = Uuid, Path, description = "Dynamic provider ID")),
    responses(
        (status = 200, description = "Provider deleted"),
        (status = 404, description = "Provider not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<()>, AdminError> {
    authz.require("dynamic_provider", "self_delete", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let provider = verify_user_owns_provider(services, user_id, id).await?;

    let provider_name = provider.name.clone();
    let provider_type = provider.provider_type.clone();

    services
        .providers
        .delete(id, state.secrets.as_ref())
        .await?;

    // Audit log (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "dynamic_provider.self_delete".to_string(),
            resource_type: "dynamic_provider".to_string(),
            resource_id: id,
            org_id: None,
            project_id: None,
            details: json!({
                "name": provider_name,
                "provider_type": provider_type,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(()))
}

/// Test connectivity for a dynamic provider (current user only)
///
/// Sends a lightweight request to the provider to verify the API key and endpoint are working.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/me/providers/{id}/test",
    tag = "me",
    operation_id = "me_providers_test",
    params(("id" = Uuid, Path, description = "Dynamic provider ID")),
    responses(
        (status = 200, description = "Connectivity test result", body = ConnectivityTestResponse),
        (status = 404, description = "Provider not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn test_connectivity(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConnectivityTestResponse>, AdminError> {
    authz.require("dynamic_provider", "self_read", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;

    let provider = verify_user_owns_provider(services, user_id, id).await?;

    let result = crate::services::DynamicProviderService::run_connectivity_test(
        &provider,
        &state,
        state.secrets.as_ref(),
    )
    .await;
    Ok(Json(result))
}

/// Test credentials before creating a provider
///
/// Accepts the same body as the create endpoint but does not persist anything.
/// Useful for validating API keys and endpoints before committing.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/me/providers/test-credentials",
    tag = "me",
    operation_id = "me_providers_test_credentials",
    request_body = CreateSelfServiceProvider,
    responses(
        (status = 200, description = "Connectivity test result", body = ConnectivityTestResponse),
        (status = 422, description = "Validation error", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn test_credentials(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Valid(Json(input)): Valid<Json<CreateSelfServiceProvider>>,
) -> Result<Json<ConnectivityTestResponse>, AdminError> {
    authz.require("dynamic_provider", "self_create", None, None, None, None)?;
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

/// Built-in provider summary
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BuiltInProvider {
    /// Provider name (as configured in hadrian.toml)
    pub name: String,
    /// Provider type (e.g., "open_ai", "anthropic")
    pub provider_type: String,
    /// Base URL (if applicable)
    pub base_url: Option<String>,
}

/// List of built-in providers from the gateway configuration
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct BuiltInProvidersResponse {
    pub data: Vec<BuiltInProvider>,
}

/// List built-in providers from the gateway configuration
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/me/built-in-providers",
    tag = "me",
    operation_id = "me_built_in_providers_list",
    responses(
        (status = 200, description = "List of built-in providers", body = BuiltInProvidersResponse),
    )
))]
pub async fn built_in_providers(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<BuiltInProvidersResponse>, AdminError> {
    authz.require("provider", "read", None, None, None, None)?;

    let data: Vec<BuiltInProvider> = state
        .config
        .providers
        .iter()
        .map(|(name, config)| BuiltInProvider {
            name: name.to_string(),
            provider_type: config.provider_type_name().to_string(),
            base_url: config.base_url().map(|s| s.to_string()),
        })
        .collect();

    Ok(Json(BuiltInProvidersResponse { data }))
}
