//! OAuth-style PKCE flow for issuing user-scoped API keys to external apps.
//!
//! - `POST /admin/v1/oauth/authorize` — authenticated; called by the
//!   in-browser consent page after the user clicks "Allow". Stores a
//!   short-lived authorization code bound to the user and PKCE challenge,
//!   then returns the redirect URL the page should send the browser to.
//! - The public counterpart `POST /oauth/token` lives in
//!   [`crate::routes::oauth_public`] and exchanges the code for an API key.

use axum::{Extension, Json, extract::State, http::StatusCode};
use axum_valid::Valid;
use serde_json::json;

use super::{
    AuditActor,
    api_keys::{check_owner_create_authz, check_owner_create_limits, validate_api_key_input},
    error::AdminError,
};
use crate::{
    AppState,
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    models::{
        ApiKeyOwner, AuthorizationCodeResponse, CreateAuditLog, CreateAuthorizationCode,
        PkceCodeChallengeMethod,
    },
    services::{Services, oauth_pkce::IssueCodeInput},
};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Append `?code=...` (or `&code=...`) to a callback URL. Validates that the
/// callback URL has an HTTPS scheme (or HTTP for loopback) and a recognised
/// host before returning, so we never redirect users to malformed targets.
fn build_redirect_url(callback_url: &str, code: &str) -> Result<(String, String), AdminError> {
    let parsed = url::Url::parse(callback_url)
        .map_err(|_| AdminError::Validation("callback_url must be a valid URL".to_string()))?;

    let scheme = parsed.scheme();
    let host = parsed
        .host_str()
        .ok_or_else(|| AdminError::Validation("callback_url must include a host".to_string()))?;

    let is_loopback = matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "::1");
    if scheme != "https" && !(scheme == "http" && is_loopback) {
        return Err(AdminError::Validation(
            "callback_url must use https (http is allowed only for loopback hosts)".to_string(),
        ));
    }

    let mut redirect = parsed.clone();
    redirect.query_pairs_mut().append_pair("code", code);
    Ok((redirect.to_string(), host.to_ascii_lowercase()))
}

/// Issue an authorization code after explicit user consent.
///
/// The browser calls this endpoint from the consent page when the user clicks
/// "Allow". The endpoint:
///
/// 1. Validates the requested scopes and key name shape (mirroring the
///    self-service API key endpoint), so we reject obvious mistakes before
///    issuing the code.
/// 2. Validates the callback URL scheme and host against
///    `auth.oauth_pkce.allowed_domains` / `denied_domains`.
/// 3. Persists a single-use authorization code bound to the current user
///    and the supplied PKCE challenge.
/// 4. Returns the redirect URL the page should send the browser to.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/oauth/authorize",
    tag = "oauth",
    operation_id = "oauth_authorize",
    request_body = CreateAuthorizationCode,
    responses(
        (status = 201, description = "Authorization code issued", body = AuthorizationCodeResponse),
        (status = 400, description = "Invalid PKCE request", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Callback domain not permitted", body = crate::openapi::ErrorResponse),
        (status = 404, description = "OAuth PKCE flow disabled", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn authorize(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Valid(Json(input)): Valid<Json<CreateAuthorizationCode>>,
) -> Result<(StatusCode, Json<AuthorizationCodeResponse>), AdminError> {
    let pkce = &state.config.auth.oauth_pkce;
    if !pkce.enabled {
        return Err(AdminError::NotFound(
            "OAuth PKCE flow is disabled".to_string(),
        ));
    }

    let user_id = admin_auth
        .identity
        .user_id
        .ok_or_else(|| AdminError::Forbidden("User account required".to_string()))?;

    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Resolve the owner the user picked. `None` defaults to a user-owned
    // key (the OpenRouter-style "personal" choice). When the user picked a
    // different scope (org/team/project/SA) we re-use the same RBAC checks
    // and per-scope limits as the admin "Create API Key" endpoint.
    let owner = input
        .key_options
        .owner
        .clone()
        .unwrap_or(ApiKeyOwner::User { user_id });

    // If the picked owner is *this* user, gate via the self-service permission
    // so OAuth consent doesn't require admin-level "create" scope. For any
    // other owner type, use the full owner-scoped check.
    match &owner {
        ApiKeyOwner::User { user_id: chosen } => {
            if *chosen != user_id {
                return Err(AdminError::Forbidden(
                    "OAuth-issued user keys must be owned by the consenting user".to_string(),
                ));
            }
            authz.require("api_key", "self_create", None, None, None, None)?;
        }
        _ => {
            check_owner_create_authz(services, &authz, &owner).await?;
        }
    }
    check_owner_create_limits(services, &owner, &state.config.limits.resource_limits).await?;

    if !pkce.allow_plain_method
        && matches!(input.code_challenge_method, PkceCodeChallengeMethod::Plain)
    {
        return Err(AdminError::Validation(
            "code_challenge_method must be 'S256'".to_string(),
        ));
    }

    let (redirect_url, callback_host) = build_redirect_url(&input.callback_url, "placeholder")?;
    if !pkce.is_callback_host_allowed(&callback_host) {
        return Err(AdminError::Forbidden(format!(
            "callback host '{callback_host}' is not permitted by server policy"
        )));
    }
    // We computed redirect_url with a placeholder so the URL parser ran. We
    // always rebuild it below with the real code, so drop the placeholder.
    drop(redirect_url);

    // Reuse the same validation rules the self-service "Create API Key"
    // endpoint applies, so the consent page can't smuggle in invalid scopes,
    // model patterns, IP allowlist entries, or rate-limit overrides.
    validate_api_key_input(
        input.key_options.scopes.as_ref(),
        input.key_options.allowed_models.as_ref(),
        input.key_options.ip_allowlist.as_ref(),
        input.key_options.rate_limit_rpm,
        input.key_options.rate_limit_tpm,
        &state.config.limits.rate_limits,
    )?;

    if input
        .key_options
        .budget_limit_cents
        .is_some_and(|cents| cents > 0)
        && input.key_options.budget_period.is_none()
    {
        return Err(AdminError::Validation(
            "key_options.budget_period is required when key_options.budget_limit_cents is set"
                .to_string(),
        ));
    }

    // Persist the *resolved* owner so the token endpoint doesn't have to
    // re-default. (Defaults are user-bound and consent-time, not redeem-time.)
    let mut key_options = input.key_options.clone();
    key_options.owner = Some(owner.clone());

    let stored = services
        .oauth_pkce
        .issue_code(IssueCodeInput {
            user_id,
            callback_url: input.callback_url.clone(),
            code_challenge: input.code_challenge,
            code_challenge_method: input.code_challenge_method,
            app_name: input.app_name.clone(),
            key_options,
            ttl_seconds: pkce.code_ttl_seconds,
        })
        .await?;

    let (redirect_url, _) = build_redirect_url(&input.callback_url, &stored.code)?;

    // Audit log (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "api_key.oauth_authorize".to_string(),
            resource_type: "oauth_authorization_code".to_string(),
            resource_id: stored.id,
            org_id: None,
            project_id: None,
            details: json!({
                "callback_host": callback_host,
                "app_name": stored.app_name,
                "key_options": stored.key_options,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((
        StatusCode::CREATED,
        Json(AuthorizationCodeResponse {
            code: stored.code,
            redirect_url,
            expires_at: stored.expires_at,
        }),
    ))
}
