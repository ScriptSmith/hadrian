use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use axum_valid::Valid;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::{AuditActor, error::AdminError, organizations::ListQuery};
use crate::{
    AppState,
    cache::CacheKeys,
    config::GatewayAuthConfig,
    middleware::{AdminAuth, AuthzContext},
    models::{
        ApiKey, ApiKeyScope, CreateApiKey, CreateAuditLog, CreatedApiKey, DEFAULT_API_KEY_PREFIX,
        validate_ip_allowlist, validate_model_patterns, validate_scopes,
    },
    openapi::PaginationMeta,
    services::Services,
};

/// Extract IP address and user agent from request headers for audit logging.
///
/// Uses trusted-proxy-aware IP extraction to prevent spoofing via `X-Forwarded-For`.
fn extract_audit_context(
    headers: &HeaderMap,
    trusted_proxies: &crate::config::TrustedProxiesConfig,
) -> (Option<String>, Option<String>) {
    // Note: ConnectInfo (direct TCP connection IP) is not available in route handlers;
    // the server does not use `into_make_service_with_connect_info`. Most production
    // deployments use reverse proxies, so X-Forwarded-For is the primary source.
    let ip = crate::middleware::extract_client_ip_from_parts(headers, None, trusted_proxies)
        .map(|ip: std::net::IpAddr| ip.to_string());
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());
    (ip, ua)
}

/// Paginated list of API keys
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ApiKeyListResponse {
    /// List of API keys
    pub data: Vec<ApiKey>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Validate API key input fields (scopes, models, IPs, rate limits).
/// Shared by both admin and self-service create endpoints.
pub(super) fn validate_api_key_input(
    scopes: Option<&Vec<String>>,
    allowed_models: Option<&Vec<String>>,
    ip_allowlist: Option<&Vec<String>>,
    rate_limit_rpm: Option<i32>,
    rate_limit_tpm: Option<i32>,
    rate_limits_config: &crate::config::RateLimitDefaults,
) -> Result<(), AdminError> {
    if let Some(scopes) = scopes
        && let Err(invalid_scopes) = validate_scopes(scopes)
    {
        return Err(AdminError::Validation(format!(
            "Invalid scopes: {}. Valid scopes: {}",
            invalid_scopes.join(", "),
            ApiKeyScope::all_names().join(", ")
        )));
    }

    if let Some(patterns) = allowed_models
        && let Err(invalid_patterns) = validate_model_patterns(patterns)
    {
        return Err(AdminError::Validation(format!(
            "Invalid model patterns: {}. Patterns must be non-empty and only support trailing wildcards (e.g., 'gpt-4*').",
            invalid_patterns.join(", ")
        )));
    }

    if let Some(allowlist) = ip_allowlist
        && let Err(invalid_entries) = validate_ip_allowlist(allowlist)
    {
        return Err(AdminError::Validation(format!(
            "Invalid IP allowlist entries: {}. Entries must be valid IPs or CIDR notation (e.g., '192.168.1.0/24', '10.0.0.1').",
            invalid_entries.join(", ")
        )));
    }

    if let Some(rpm) = rate_limit_rpm {
        if rpm <= 0 {
            return Err(AdminError::Validation(
                "rate_limit_rpm must be a positive integer".to_string(),
            ));
        }
        if !rate_limits_config.allow_per_key_above_global
            && (rpm as u32) > rate_limits_config.requests_per_minute
        {
            return Err(AdminError::Validation(format!(
                "rate_limit_rpm ({}) cannot exceed global limit ({}). Set allow_per_key_above_global = true in config to override.",
                rpm, rate_limits_config.requests_per_minute
            )));
        }
    }
    if let Some(tpm) = rate_limit_tpm {
        if tpm <= 0 {
            return Err(AdminError::Validation(
                "rate_limit_tpm must be a positive integer".to_string(),
            ));
        }
        if !rate_limits_config.allow_per_key_above_global
            && (tpm as u32) > rate_limits_config.tokens_per_minute
        {
            return Err(AdminError::Validation(format!(
                "rate_limit_tpm ({}) cannot exceed global limit ({}). Set allow_per_key_above_global = true in config to override.",
                tpm, rate_limits_config.tokens_per_minute
            )));
        }
    }

    Ok(())
}

/// Invalidate all cache entries for an API key.
/// Shared by revoke and rotate endpoints (both admin and self-service).
pub(super) async fn invalidate_api_key_cache(cache: &dyn crate::cache::Cache, key_id: uuid::Uuid) {
    use crate::models::BudgetPeriod;

    let id_cache_key = CacheKeys::api_key_by_id(key_id);
    let _ = cache.delete(&id_cache_key).await;

    let reverse_key = CacheKeys::api_key_reverse(key_id);
    if let Ok(Some(hash_bytes)) = cache.get_bytes(&reverse_key).await
        && let Ok(hash) = String::from_utf8(hash_bytes)
    {
        let hash_cache_key = CacheKeys::api_key(&hash);
        let _ = cache.delete(&hash_cache_key).await;
    }
    let _ = cache.delete(&reverse_key).await;

    let _ = cache.delete(&CacheKeys::rate_limit(key_id, "minute")).await;
    let _ = cache.delete(&CacheKeys::rate_limit(key_id, "day")).await;
    let _ = cache
        .delete(&CacheKeys::rate_limit_tokens(key_id, "minute"))
        .await;
    let _ = cache
        .delete(&CacheKeys::rate_limit_tokens(key_id, "day"))
        .await;
    let _ = cache.delete(&CacheKeys::concurrent_requests(key_id)).await;

    let _ = cache
        .delete(&CacheKeys::spend(key_id, BudgetPeriod::Daily))
        .await;
    let _ = cache
        .delete(&CacheKeys::spend(key_id, BudgetPeriod::Monthly))
        .await;
}

/// Create an API key
///
/// Creates a new API key scoped to an organization, project, or user. The raw API key value
/// is returned only once in this response and cannot be retrieved later. Store it securely.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/api-keys",
    tag = "api-keys",
    operation_id = "api_key_create",
    request_body(
        content = CreateApiKey,
        examples(
            ("Organization key" = (
                summary = "Create an org-scoped API key",
                value = json!({
                    "name": "Production API Key",
                    "owner": {
                        "type": "organization",
                        "org_id": "550e8400-e29b-41d4-a716-446655440000"
                    }
                })
            )),
            ("Project key with budget" = (
                summary = "Create a project-scoped key with budget limits",
                value = json!({
                    "name": "Dev Team Key",
                    "owner": {
                        "type": "project",
                        "project_id": "123e4567-e89b-12d3-a456-426614174000"
                    },
                    "budget_limit_cents": 10000,
                    "budget_period": "monthly"
                })
            )),
            ("User key with expiration" = (
                summary = "Create a user-scoped key with expiration",
                value = json!({
                    "name": "Personal Key",
                    "owner": {
                        "type": "user",
                        "user_id": "7c9e6679-7425-40de-944b-e07fc1f90ae7"
                    },
                    "expires_at": "2025-12-31T23:59:59Z"
                })
            )),
            ("Service account key" = (
                summary = "Create a service account-scoped API key",
                value = json!({
                    "name": "CI/CD Pipeline Key",
                    "owner": {
                        "type": "service_account",
                        "service_account_id": "8d0e7891-3456-78ef-9012-345678901234"
                    }
                })
            ))
        )
    ),
    responses(
        (status = 201, description = "API key created successfully. Store the key value securely - it won't be shown again.",
            body = CreatedApiKey,
            example = json!({
                "api_key": {
                    "id": "550e8400-e29b-41d4-a716-446655440001",
                    "name": "Production API Key",
                    "key_prefix": "gw_live_abc",
                    "owner": {
                        "type": "organization",
                        "org_id": "550e8400-e29b-41d4-a716-446655440000"
                    },
                    "budget_limit": null,
                    "budget_period": null,
                    "created_at": "2025-01-15T10:30:00Z",
                    "expires_at": null,
                    "revoked_at": null
                },
                "key": "gw_live_abc123def456ghi789jkl012mno345pqr678"
            })
        ),
        (status = 404, description = "Owner (organization, project, or user) not found",
            body = crate::openapi::ErrorResponse,
            example = json!({
                "error": {
                    "code": "not_found",
                    "message": "Organization '550e8400-e29b-41d4-a716-446655440000' not found"
                }
            })
        ),
    )
))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<crate::middleware::AuthzContext>,
    headers: HeaderMap,
    Valid(Json(input)): Valid<Json<CreateApiKey>>,
) -> Result<(StatusCode, Json<CreatedApiKey>), AdminError> {
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

    // Authorization check for API key creation.
    // Each owner type requires permission scoped to the appropriate org/team/project.
    match &input.owner {
        crate::models::ApiKeyOwner::Organization { org_id } => {
            let org_id_str = org_id.to_string();
            authz.require("api_key", "create", None, Some(&org_id_str), None, None)?;
        }
        crate::models::ApiKeyOwner::Team { team_id } => {
            let team = services
                .teams
                .get_by_id(*team_id)
                .await?
                .ok_or_else(|| AdminError::NotFound(format!("Team '{}' not found", team_id)))?;
            let org_id_str = team.org_id.to_string();
            let team_id_str = team_id.to_string();
            authz.require(
                "api_key",
                "create",
                None,
                Some(&org_id_str),
                Some(&team_id_str),
                None,
            )?;
        }
        crate::models::ApiKeyOwner::Project { project_id } => {
            let project = services
                .projects
                .get_by_id(*project_id)
                .await?
                .ok_or_else(|| {
                    AdminError::NotFound(format!("Project '{}' not found", project_id))
                })?;
            let org_id_str = project.org_id.to_string();
            let project_id_str = project_id.to_string();
            authz.require(
                "api_key",
                "create",
                None,
                Some(&org_id_str),
                None,
                Some(&project_id_str),
            )?;
        }
        crate::models::ApiKeyOwner::User { .. } => {
            authz.require("api_key", "create", None, None, None, None)?;
        }
        crate::models::ApiKeyOwner::ServiceAccount { service_account_id } => {
            let sa = services
                .service_accounts
                .get_by_id(*service_account_id)
                .await?
                .ok_or_else(|| {
                    AdminError::NotFound(format!(
                        "Service account '{}' not found",
                        service_account_id
                    ))
                })?;
            authz.require(
                "api_key",
                "create",
                None,
                Some(&sa.org_id.to_string()),
                None,
                None,
            )?;
        }
    }

    // Get the key generation prefix from config
    let prefix = match &state.config.auth.gateway {
        GatewayAuthConfig::ApiKey(config) => config.generation_prefix(),
        GatewayAuthConfig::Multi(config) => config.api_key.generation_prefix(),
        _ => DEFAULT_API_KEY_PREFIX.to_string(),
    };

    // Capture owner info for audit log before consuming input
    let (org_id, project_id) = match &input.owner {
        crate::models::ApiKeyOwner::Organization { org_id } => (Some(*org_id), None),
        crate::models::ApiKeyOwner::Team { .. } => (None, None), // Team org resolved separately if needed
        crate::models::ApiKeyOwner::Project { project_id } => (None, Some(*project_id)),
        crate::models::ApiKeyOwner::User { .. } => (None, None),
        crate::models::ApiKeyOwner::ServiceAccount { .. } => (None, None), // SA org resolved separately if needed
    };

    // Create API key through service
    let created = services.api_keys.create(input, &prefix).await?;

    let (ip_address, user_agent) =
        extract_audit_context(&headers, &state.config.server.trusted_proxies);

    // Log audit event (fire-and-forget, don't fail the request if logging fails)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "api_key.create".to_string(),
            resource_type: "api_key".to_string(),
            resource_id: created.api_key.id,
            org_id,
            project_id,
            details: json!({
                "name": created.api_key.name,
                "key_prefix": created.api_key.key_prefix,
            }),
            ip_address,
            user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(created)))
}

/// List API keys by organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/api-keys",
    tag = "api-keys",
    operation_id = "api_key_list_by_org",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of API keys", body = ApiKeyListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list_by_org(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ApiKeyListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    authz.require(
        "api_key",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services.api_keys.list_by_org(org.id, params).await?;

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

/// List API keys by project
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/projects/{project_slug}/api-keys",
    tag = "api-keys",
    operation_id = "api_key_list_by_project",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("project_slug" = String, Path, description = "Project slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of API keys", body = ApiKeyListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or project not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list_by_project(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, project_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ApiKeyListResponse>, AdminError> {
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
        "api_key",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        Some(&project.id.to_string()),
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .api_keys
        .list_by_project(project.id, params)
        .await?;

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

/// List API keys by user
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}/api-keys",
    tag = "api-keys",
    operation_id = "api_key_list_by_user",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of API keys", body = ApiKeyListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list_by_user(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ApiKeyListResponse>, AdminError> {
    authz.require("api_key", "list", None, None, None, None)?;
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

/// List API keys by service account
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/service-accounts/{sa_slug}/api-keys",
    tag = "api-keys",
    operation_id = "api_key_list_by_service_account",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("sa_slug" = String, Path, description = "Service account slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of API keys", body = ApiKeyListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or service account not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list_by_service_account(
    State(state): State<AppState>,
    Extension(authz): Extension<crate::middleware::AuthzContext>,
    Path((org_slug, sa_slug)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ApiKeyListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get service account by slug
    let sa = services
        .service_accounts
        .get_by_slug(org.id, &sa_slug)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "Service account '{}' not found in organization '{}'",
                sa_slug, org_slug
            ))
        })?;

    // Require read permission on service account's API keys
    authz.require(
        "api_key",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;

    let result = services
        .api_keys
        .list_by_service_account(sa.id, params)
        .await?;

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

/// Revoke an API key
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/api-keys/{key_id}",
    tag = "api-keys",
    operation_id = "api_key_revoke",
    params(("key_id" = Uuid, Path, description = "API key ID")),
    responses(
        (status = 200, description = "API key revoked"),
        (status = 404, description = "API key not found", body = crate::openapi::ErrorResponse),
    )
))]
#[allow(clippy::collapsible_if)]
pub async fn revoke(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    headers: HeaderMap,
    Path(key_id): Path<Uuid>,
) -> Result<Json<()>, AdminError> {
    authz.require(
        "api_key",
        "delete",
        Some(&key_id.to_string()),
        None,
        None,
        None,
    )?;

    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get API key info for audit log before revoking
    let key_info = services.api_keys.get_by_id(key_id).await?;

    services.api_keys.revoke(key_id).await?;

    let (ip_address, user_agent) =
        extract_audit_context(&headers, &state.config.server.trusted_proxies);

    // Log audit event
    if let Some(key) = key_info {
        let (org_id, project_id) = match &key.owner {
            crate::models::ApiKeyOwner::Organization { org_id } => (Some(*org_id), None),
            crate::models::ApiKeyOwner::Team { .. } => (None, None), // Team org resolved separately if needed
            crate::models::ApiKeyOwner::Project { project_id } => (None, Some(*project_id)),
            crate::models::ApiKeyOwner::User { .. } => (None, None),
            crate::models::ApiKeyOwner::ServiceAccount { .. } => (None, None), // SA org resolved separately if needed
        };

        let _ = services
            .audit_logs
            .create(CreateAuditLog {
                actor_type: actor.actor_type,
                actor_id: actor.actor_id,
                action: "api_key.revoke".to_string(),
                resource_type: "api_key".to_string(),
                resource_id: key_id,
                org_id,
                project_id,
                details: json!({
                    "name": key.name,
                    "key_prefix": key.key_prefix,
                }),
                ip_address,
                user_agent,
            })
            .await;
    }

    // Invalidate all cache entries for this API key
    if let Some(cache) = &state.cache {
        invalidate_api_key_cache(cache.as_ref(), key_id).await;
        tracing::info!(
            api_key_id = %key_id,
            "Invalidated all cache entries for revoked API key"
        );
    }

    Ok(Json(()))
}

/// Default grace period for key rotation: 24 hours
pub(super) const DEFAULT_GRACE_PERIOD_SECONDS: u64 = 86400;
/// Maximum grace period: 7 days
pub(super) const MAX_GRACE_PERIOD_SECONDS: u64 = 604800;

/// Request body for rotating an API key
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RotateApiKeyRequest {
    /// Grace period in seconds during which both old and new keys are valid.
    /// Default: 86400 (24 hours). Maximum: 604800 (7 days).
    pub grace_period_seconds: Option<u64>,
}

/// Rotate an API key
///
/// Creates a new API key with the same settings as the old key and sets a grace period
/// on the old key. During the grace period, both keys are valid. After the grace period
/// expires, only the new key works.
///
/// The raw API key value for the new key is returned only once in this response.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/api-keys/{key_id}/rotate",
    tag = "api-keys",
    operation_id = "api_key_rotate",
    params(("key_id" = Uuid, Path, description = "API key ID to rotate")),
    request_body(
        content = RotateApiKeyRequest,
        examples(
            ("Default grace period" = (
                summary = "Rotate with default 24-hour grace period",
                value = json!({})
            )),
            ("Custom grace period" = (
                summary = "Rotate with 1-hour grace period",
                value = json!({
                    "grace_period_seconds": 3600
                })
            ))
        )
    ),
    responses(
        (status = 201, description = "API key rotated successfully. Store the new key value securely - it won't be shown again.",
            body = CreatedApiKey,
            example = json!({
                "api_key": {
                    "id": "550e8400-e29b-41d4-a716-446655440002",
                    "name": "Production API Key (rotated)",
                    "key_prefix": "gw_live_xyz",
                    "owner": {
                        "type": "organization",
                        "org_id": "550e8400-e29b-41d4-a716-446655440000"
                    },
                    "rotated_from_key_id": "550e8400-e29b-41d4-a716-446655440001",
                    "created_at": "2025-01-15T10:30:00Z"
                },
                "key": "gw_live_xyz123abc456def789ghi012jkl345mno678"
            })
        ),
        (status = 400, description = "Invalid grace period",
            body = crate::openapi::ErrorResponse,
            example = json!({
                "error": {
                    "code": "validation_error",
                    "message": "Grace period cannot exceed 604800 seconds (7 days)"
                }
            })
        ),
        (status = 404, description = "API key not found",
            body = crate::openapi::ErrorResponse,
            example = json!({
                "error": {
                    "code": "not_found",
                    "message": "API key not found"
                }
            })
        ),
        (status = 409, description = "API key cannot be rotated (already revoked or being rotated)",
            body = crate::openapi::ErrorResponse,
            example = json!({
                "error": {
                    "code": "conflict",
                    "message": "API key is already being rotated"
                }
            })
        ),
    )
))]
pub async fn rotate(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    headers: HeaderMap,
    Path(key_id): Path<Uuid>,
    Json(request): Json<RotateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreatedApiKey>), AdminError> {
    authz.require(
        "api_key",
        "update",
        Some(&key_id.to_string()),
        None,
        None,
        None,
    )?;

    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

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

    // Get old key info for audit log before rotating
    let old_key = services.api_keys.get_by_id(key_id).await?;

    // Perform the rotation
    let created = services
        .api_keys
        .rotate(key_id, grace_period_seconds, &prefix)
        .await?;

    let (ip_address, user_agent) =
        extract_audit_context(&headers, &state.config.server.trusted_proxies);

    // Log audit event
    if let Some(key) = old_key {
        let (org_id, project_id) = match &key.owner {
            crate::models::ApiKeyOwner::Organization { org_id } => (Some(*org_id), None),
            crate::models::ApiKeyOwner::Team { .. } => (None, None),
            crate::models::ApiKeyOwner::Project { project_id } => (None, Some(*project_id)),
            crate::models::ApiKeyOwner::User { .. } => (None, None),
            crate::models::ApiKeyOwner::ServiceAccount { .. } => (None, None), // SA org resolved separately if needed
        };

        let _ = services
            .audit_logs
            .create(CreateAuditLog {
                actor_type: actor.actor_type,
                actor_id: actor.actor_id,
                action: "api_key.rotate".to_string(),
                resource_type: "api_key".to_string(),
                resource_id: key_id,
                org_id,
                project_id,
                details: json!({
                    "old_key_id": key_id,
                    "old_key_name": key.name,
                    "old_key_prefix": key.key_prefix,
                    "new_key_id": created.api_key.id,
                    "new_key_prefix": created.api_key.key_prefix,
                    "grace_period_seconds": grace_period_seconds,
                }),
                ip_address,
                user_agent,
            })
            .await;
    }

    // Invalidate cache for old key
    if let Some(cache) = &state.cache {
        invalidate_api_key_cache(cache.as_ref(), key_id).await;
        tracing::info!(
            old_key_id = %key_id,
            new_key_id = %created.api_key.id,
            grace_period_seconds = grace_period_seconds,
            "Rotated API key and invalidated cache for old key"
        );
    }

    Ok((StatusCode::CREATED, Json(created)))
}
