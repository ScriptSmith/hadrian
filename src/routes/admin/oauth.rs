//! OAuth-style PKCE flow for issuing user-scoped API keys to external apps.
//!
//! - `POST /admin/v1/oauth/authorize` — authenticated; called by the
//!   in-browser consent page after the user clicks "Allow". Stores a
//!   short-lived authorization code bound to the user and PKCE challenge,
//!   then returns the redirect URL the page should send the browser to.
//! - The public counterpart `POST /oauth/token` lives in
//!   [`crate::routes::oauth_public`] and exchanges the code for an API key.

use axum::{
    Extension, Json,
    extract::{Query, State},
    http::StatusCode,
};
use axum_valid::Valid;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    AuditActor,
    api_keys::{check_owner_create_authz, check_owner_create_limits, validate_api_key_input},
    error::AdminError,
};
use crate::{
    AppState,
    config::OAuthPkceConfig,
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

/// Validate a `callback_url` against scheme rules and the operator's
/// allow/deny lists. Returns the lowercase host on success. Used by both
/// the authorize endpoint (allow leg) and the preflight endpoint that
/// gates the consent UI (so the deny leg can't bypass denied_domains by
/// just redirecting client-side).
fn validate_callback_url(callback_url: &str, pkce: &OAuthPkceConfig) -> Result<String, AdminError> {
    let parsed = url::Url::parse(callback_url)
        .map_err(|_| AdminError::Validation("callback_url must be a valid URL".to_string()))?;

    let scheme = parsed.scheme();
    let host = parsed
        .host_str()
        .ok_or_else(|| AdminError::Validation("callback_url must include a host".to_string()))?
        .to_ascii_lowercase();

    // Treat the entire 127.0.0.0/8 IPv4 loopback range and IPv6 loopback (incl.
    // IPv4-mapped form) as loopback. `host` is already lowercased; `url::Host`
    // gives us a parsed view that handles bracketed IPv6 correctly.
    let is_loopback = match parsed.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => {
            ip.is_loopback()
                || ip
                    .to_ipv4_mapped()
                    .map(|v4| v4.is_loopback())
                    .unwrap_or(false)
        }
        None => false,
    };
    if scheme != "https" && !(scheme == "http" && is_loopback) {
        return Err(AdminError::Validation(
            "callback_url must use https (http is allowed only for loopback hosts)".to_string(),
        ));
    }

    if !pkce.is_callback_host_allowed(&host) {
        return Err(AdminError::Forbidden(format!(
            "callback host '{host}' is not permitted by server policy"
        )));
    }

    Ok(host)
}

/// Append `?code=...` (or `&code=...`) to a callback URL. The URL is
/// assumed to have already been through [`validate_callback_url`]. Any
/// pre-existing `code` query parameter is removed first to prevent an
/// attacker who controls the registered callback from pre-seeding a code
/// the OAuth client would then submit on exchange.
fn build_redirect_url(callback_url: &str, code: &str) -> Result<String, AdminError> {
    let mut redirect = url::Url::parse(callback_url)
        .map_err(|_| AdminError::Validation("callback_url must be a valid URL".to_string()))?;
    let preserved: Vec<(String, String)> = redirect
        .query_pairs()
        .filter(|(k, _)| k != "code")
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    {
        let mut pairs = redirect.query_pairs_mut();
        pairs.clear();
        for (k, v) in &preserved {
            pairs.append_pair(k, v);
        }
        pairs.append_pair("code", code);
    }
    Ok(redirect.to_string())
}

/// Query parameters for the preflight endpoint.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams))]
pub struct PreflightQuery {
    /// The callback URL the external app intends to use.
    pub callback_url: String,
}

/// Result of the preflight check.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PreflightResponse {
    /// The validated host (lowercased) — useful for surfacing in the UI.
    pub callback_host: String,
}

/// Validate a `callback_url` against the deployment's OAuth PKCE policy
/// without issuing a code. The consent UI calls this on mount and refuses
/// to render if the URL is rejected — closing the gap where a "Deny"
/// click could otherwise redirect to a host the operator denied.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/oauth/preflight",
    tag = "oauth",
    operation_id = "oauth_preflight",
    params(PreflightQuery),
    responses(
        (status = 200, description = "Callback URL passes policy", body = PreflightResponse),
        (status = 400, description = "Invalid URL or scheme", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Callback host denied by policy", body = crate::openapi::ErrorResponse),
        (status = 404, description = "OAuth PKCE flow disabled", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn preflight(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Query(query): Query<PreflightQuery>,
) -> Result<Json<PreflightResponse>, AdminError> {
    let pkce = &state.config.auth.oauth_pkce;
    if !pkce.enabled {
        return Err(AdminError::NotFound(
            "OAuth PKCE flow is disabled".to_string(),
        ));
    }
    // Same authz scope the authorize endpoint uses — only authenticated
    // users with self-service key permission can probe the policy.
    authz.require("api_key", "self_create", None, None, None, None)?;
    let callback_host = validate_callback_url(&query.callback_url, pkce)?;
    Ok(Json(PreflightResponse { callback_host }))
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

    let callback_host = validate_callback_url(&input.callback_url, pkce)?;

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

    let redirect_url = build_redirect_url(&input.callback_url, &stored.code)?;

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
