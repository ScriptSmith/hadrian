use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_valid::Valid;
use serde_json::json;
use uuid::Uuid;

use super::{
    AuditActor,
    api_keys::{
        ApiKeyListResponse, RotateApiKeyRequest, invalidate_api_key_cache, validate_api_key_input,
    },
    error::AdminError,
    organizations::ListQuery,
};
use crate::{
    AppState,
    config::GatewayAuthConfig,
    middleware::{AdminAuth, AuthzContext},
    models::{
        ApiKey, ApiKeyOwner, CreateApiKey, CreateAuditLog, CreateSelfServiceApiKey, CreatedApiKey,
        DEFAULT_API_KEY_PREFIX,
    },
    openapi::PaginationMeta,
    services::Services,
};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

fn get_user_id(admin_auth: &AdminAuth) -> Result<Uuid, AdminError> {
    admin_auth
        .identity
        .user_id
        .ok_or(AdminError::Forbidden("User account required".to_string()))
}

/// Verify that an API key is owned by the given user.
/// Returns 404 (not 403) to prevent key ID enumeration.
async fn verify_user_owns_key(
    services: &Services,
    user_id: Uuid,
    key_id: Uuid,
) -> Result<ApiKey, AdminError> {
    let key = services
        .api_keys
        .get_by_id(key_id)
        .await?
        .ok_or_else(|| AdminError::NotFound("API key not found".to_string()))?;

    match &key.owner {
        ApiKeyOwner::User { user_id: owner_id } if *owner_id == user_id => Ok(key),
        _ => Err(AdminError::NotFound("API key not found".to_string())),
    }
}

/// Get an API key by ID (current user only)
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/me/api-keys/{key_id}",
    tag = "me",
    operation_id = "me_api_keys_get",
    params(("key_id" = Uuid, Path, description = "API key ID")),
    responses(
        (status = 200, description = "API key found", body = ApiKey),
        (status = 404, description = "API key not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(key_id): Path<Uuid>,
) -> Result<Json<ApiKey>, AdminError> {
    authz.require("api_key", "self_read", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;

    let key = verify_user_owns_key(services, user_id, key_id).await?;
    Ok(Json(key))
}

/// List current user's API keys
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/me/api-keys",
    tag = "me",
    operation_id = "me_api_keys_list",
    params(ListQuery),
    responses(
        (status = 200, description = "List of user's API keys", body = ApiKeyListResponse),
        (status = 401, description = "User not identified from session", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ApiKeyListResponse>, AdminError> {
    authz.require("api_key", "self_list", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.api_keys.list_by_user(user_id, params).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(ApiKeyListResponse {
        data: result.items,
        pagination,
    }))
}

/// Create an API key for the current user
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/me/api-keys",
    tag = "me",
    operation_id = "me_api_keys_create",
    request_body = CreateSelfServiceApiKey,
    responses(
        (status = 201, description = "API key created. Store the key value securely - it won't be shown again.", body = CreatedApiKey),
        (status = 401, description = "User not identified from session", body = crate::openapi::ErrorResponse),
        (status = 422, description = "API key limit exceeded or validation error", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Valid(Json(input)): Valid<Json<CreateSelfServiceApiKey>>,
) -> Result<(StatusCode, Json<CreatedApiKey>), AdminError> {
    authz.require("api_key", "self_create", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Validate input fields
    validate_api_key_input(
        input.scopes.as_ref(),
        input.allowed_models.as_ref(),
        input.ip_allowlist.as_ref(),
        input.rate_limit_rpm,
        input.rate_limit_tpm,
        &state.config.limits.rate_limits,
    )?;

    // Check per-user API key limit
    let max = state.config.limits.resource_limits.max_api_keys_per_user;
    if max > 0 {
        let count = services.api_keys.count_by_user(user_id, false).await?;
        if count >= max as i64 {
            return Err(AdminError::Validation(format!(
                "API key limit reached ({max}). Revoke an existing key before creating a new one."
            )));
        }
    }

    // Get the key generation prefix from config
    let prefix = match &state.config.auth.gateway {
        GatewayAuthConfig::ApiKey(config) => config.generation_prefix(),
        GatewayAuthConfig::Multi(config) => config.api_key.generation_prefix(),
        _ => DEFAULT_API_KEY_PREFIX.to_string(),
    };

    let create_input = CreateApiKey {
        name: input.name,
        owner: ApiKeyOwner::User { user_id },
        budget_limit_cents: input.budget_limit_cents,
        budget_period: input.budget_period,
        expires_at: input.expires_at,
        scopes: input.scopes,
        allowed_models: input.allowed_models,
        ip_allowlist: input.ip_allowlist,
        rate_limit_rpm: input.rate_limit_rpm,
        rate_limit_tpm: input.rate_limit_tpm,
    };

    let created = services.api_keys.create(create_input, &prefix).await?;

    // Audit log (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "api_key.self_create".to_string(),
            resource_type: "api_key".to_string(),
            resource_id: created.api_key.id,
            org_id: None,
            project_id: None,
            details: json!({
                "name": created.api_key.name,
                "key_prefix": created.api_key.key_prefix,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok((StatusCode::CREATED, Json(created)))
}

/// Revoke an API key (current user only)
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/me/api-keys/{key_id}",
    tag = "me",
    operation_id = "me_api_keys_revoke",
    params(("key_id" = Uuid, Path, description = "API key ID")),
    responses(
        (status = 200, description = "API key revoked"),
        (status = 404, description = "API key not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn revoke(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(key_id): Path<Uuid>,
) -> Result<Json<()>, AdminError> {
    authz.require("api_key", "self_delete", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let key = verify_user_owns_key(services, user_id, key_id).await?;

    services.api_keys.revoke(key_id).await?;

    // Audit log (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "api_key.self_revoke".to_string(),
            resource_type: "api_key".to_string(),
            resource_id: key_id,
            org_id: None,
            project_id: None,
            details: json!({
                "name": key.name,
                "key_prefix": key.key_prefix,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    // Invalidate cache
    if let Some(cache) = &state.cache {
        invalidate_api_key_cache(cache.as_ref(), key_id).await;
        tracing::info!(
            api_key_id = %key_id,
            "Invalidated all cache entries for self-service revoked API key"
        );
    }

    Ok(Json(()))
}

use super::api_keys::{DEFAULT_GRACE_PERIOD_SECONDS, MAX_GRACE_PERIOD_SECONDS};

/// Rotate an API key (current user only)
///
/// Creates a new API key with the same settings as the old key and sets a grace period
/// on the old key. During the grace period, both keys are valid.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/me/api-keys/{key_id}/rotate",
    tag = "me",
    operation_id = "me_api_keys_rotate",
    params(("key_id" = Uuid, Path, description = "API key ID to rotate")),
    request_body = RotateApiKeyRequest,
    responses(
        (status = 201, description = "API key rotated. Store the new key value securely.", body = CreatedApiKey),
        (status = 400, description = "Invalid grace period", body = crate::openapi::ErrorResponse),
        (status = 404, description = "API key not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "API key cannot be rotated", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn rotate(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(key_id): Path<Uuid>,
    Json(request): Json<RotateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreatedApiKey>), AdminError> {
    authz.require("api_key", "self_update", None, None, None, None)?;
    let user_id = get_user_id(&admin_auth)?;
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Ownership check
    let key = verify_user_owns_key(services, user_id, key_id).await?;

    // Validate grace period
    let grace_period_seconds = request
        .grace_period_seconds
        .unwrap_or(DEFAULT_GRACE_PERIOD_SECONDS);

    if grace_period_seconds > MAX_GRACE_PERIOD_SECONDS {
        return Err(AdminError::Validation(format!(
            "Grace period cannot exceed {} seconds (7 days)",
            MAX_GRACE_PERIOD_SECONDS
        )));
    }

    if grace_period_seconds == 0 {
        return Err(AdminError::Validation(
            "Grace period must be at least 1 second".to_string(),
        ));
    }

    // Get the key generation prefix from config
    let prefix = match &state.config.auth.gateway {
        GatewayAuthConfig::ApiKey(config) => config.generation_prefix(),
        GatewayAuthConfig::Multi(config) => config.api_key.generation_prefix(),
        _ => DEFAULT_API_KEY_PREFIX.to_string(),
    };

    // Perform the rotation
    let created = services
        .api_keys
        .rotate(key_id, grace_period_seconds, &prefix)
        .await?;

    // Audit log (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "api_key.self_rotate".to_string(),
            resource_type: "api_key".to_string(),
            resource_id: key_id,
            org_id: None,
            project_id: None,
            details: json!({
                "old_key_id": key_id,
                "old_key_name": key.name,
                "old_key_prefix": key.key_prefix,
                "new_key_id": created.api_key.id,
                "new_key_prefix": created.api_key.key_prefix,
                "grace_period_seconds": grace_period_seconds,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    // Invalidate cache for old key
    if let Some(cache) = &state.cache {
        invalidate_api_key_cache(cache.as_ref(), key_id).await;
        tracing::info!(
            old_key_id = %key_id,
            new_key_id = %created.api_key.id,
            grace_period_seconds = grace_period_seconds,
            "Rotated API key (self-service) and invalidated cache for old key"
        );
    }

    Ok((StatusCode::CREATED, Json(created)))
}
