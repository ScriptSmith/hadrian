//! Authentication routes for OIDC and SAML 2.0.
//!
//! These routes handle the OIDC authorization code flow and SAML 2.0 SSO:
//!
//! ## OIDC Routes
//! - `/auth/login` - Redirects to the IdP for authentication (auto-dispatches SAML vs OIDC)
//! - `/auth/callback` - Handles the callback from the OIDC IdP
//! - `/auth/logout` - Logs out and optionally redirects to IdP logout
//! - `/auth/me` - Returns the current user's identity
//! - `/auth/discover` - Discovers SSO configuration for an email domain
//!
//! ## SAML Routes
//! - `/auth/saml/login` - Generates AuthnRequest and redirects to SAML IdP
//! - `/auth/saml/acs` - Assertion Consumer Service endpoint (handles SAML Response)
//! - `/auth/saml/slo` - Single Logout (stub - local logout only)

#[cfg(feature = "saml")]
use axum::Form;
use axum::{
    Extension, Json,
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tower_cookies::{
    Cookie, Cookies,
    cookie::{SameSite as CookieSameSite, time::Duration as CookieDuration},
};
use uuid::Uuid;
use validator::ValidateEmail;

use crate::{
    AppState,
    auth::AuthError,
    config::{AdminAuthConfig, SameSite, TrustedProxiesConfig},
    middleware::AdminAuth,
    models::{DomainVerificationStatus, SsoEnforcementMode, SsoProviderType},
    services::audit_logs::{AuthEventParams, auth_events},
};

/// Build a session removal cookie with the same security attributes as the login cookie.
fn build_removal_cookie(session_config: &crate::config::SessionConfig) -> Cookie<'static> {
    let same_site = match session_config.same_site {
        SameSite::Strict => CookieSameSite::Strict,
        SameSite::Lax => CookieSameSite::Lax,
        SameSite::None => CookieSameSite::None,
    };
    Cookie::build(session_config.cookie_name.clone())
        .path("/")
        .http_only(true)
        .secure(session_config.secure)
        .same_site(same_site)
        .max_age(CookieDuration::ZERO)
        .build()
}

/// Extract client IP address from request headers and connection info.
///
/// Delegates to [`crate::middleware::rate_limit::extract_client_ip_from_parts`].
pub(crate) fn extract_client_ip_from_parts(
    headers: &axum::http::HeaderMap,
    connecting_addr: Option<std::net::SocketAddr>,
    trusted_proxies: &TrustedProxiesConfig,
) -> Option<std::net::IpAddr> {
    crate::middleware::extract_client_ip_from_parts(headers, connecting_addr, trusted_proxies)
}

/// Query parameters for the login endpoint.
#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    /// URL to redirect to after successful login
    #[serde(default)]
    pub return_to: Option<String>,
    /// Organization ID or slug for per-organization SSO
    #[serde(default)]
    pub org: Option<String>,
}

/// Query parameters for the callback endpoint.
#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    /// Authorization code from the IdP
    pub code: String,
    /// State parameter for CSRF protection
    pub state: String,
    /// Error from the IdP (if any)
    #[serde(default)]
    pub error: Option<String>,
    /// Error description from the IdP
    #[serde(default)]
    pub error_description: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// SAML Types
// ─────────────────────────────────────────────────────────────────────────────

/// Query parameters for the SAML login endpoint.
///
/// Unlike OIDC login, SAML requires `org` since there's no global SAML config.
#[cfg(feature = "saml")]
#[derive(Debug, Deserialize)]
pub struct SamlLoginQuery {
    /// Organization ID or slug (required for SAML - no global SAML config)
    pub org: String,
    /// URL to redirect to after successful login
    #[serde(default)]
    pub return_to: Option<String>,
}

/// Form data from SAML IdP (HTTP-POST binding).
///
/// The IdP sends the SAML Response as a base64-encoded string along with
/// the RelayState we provided during the AuthnRequest.
#[cfg(feature = "saml")]
#[derive(Debug, Deserialize)]
pub struct SamlAcsForm {
    /// Base64-encoded SAML Response from the IdP
    #[serde(rename = "SAMLResponse")]
    pub saml_response: String,
    /// RelayState we sent with the AuthnRequest (used for CSRF protection and routing)
    #[serde(rename = "RelayState")]
    pub relay_state: String,
}

/// Query parameters for the SAML metadata endpoint.
#[cfg(feature = "saml")]
#[derive(Debug, Deserialize)]
pub struct SamlMetadataQuery {
    /// Organization ID or slug (required for per-org SAML config)
    pub org: String,
}

/// Response for the /auth/me endpoint.
#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub external_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,
    /// User's roles (e.g., super_admin, org_admin, team_admin, user)
    pub roles: Vec<String>,
    /// Raw IdP groups from the authentication source.
    /// These are the exact values from the IdP (e.g., OIDC groups claim),
    /// before any mapping or transformation. Useful for debugging SSO group mappings.
    pub idp_groups: Vec<String>,
}

/// Query parameters for the discover endpoint.
#[derive(Debug, Deserialize)]
pub struct DiscoverQuery {
    /// Email address to discover SSO configuration for
    pub email: String,
}

/// Response for the /auth/discover endpoint.
#[derive(Debug, Serialize)]
pub struct DiscoverResponse {
    /// Organization ID
    pub org_id: Uuid,
    /// Organization slug (URL-friendly identifier)
    pub org_slug: String,
    /// Organization display name
    pub org_name: String,
    /// Whether the organization has SSO configured and the domain is verified.
    /// SSO is only available when both conditions are met.
    pub has_sso: bool,
    /// Whether SSO is required for this organization (enforcement_mode = "required").
    /// Only true if has_sso is also true.
    pub sso_required: bool,
    /// The SSO enforcement mode for this organization.
    /// - "optional": SSO is available but not required
    /// - "required": SSO is required; non-SSO auth will be blocked
    /// - "test": SSO enforcement is being tested; non-SSO auth is logged but allowed
    pub enforcement_mode: SsoEnforcementMode,
    /// The SSO provider type (oidc or saml).
    /// Determines which auth flow the frontend should use.
    pub provider_type: SsoProviderType,
    /// Display name for the identity provider (derived from issuer URL or IdP entity ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idp_name: Option<String>,
    /// Whether the email domain has been verified via DNS TXT record.
    /// SSO is only available for verified domains.
    pub domain_verified: bool,
    /// Current verification status of the domain (pending, verified, failed).
    /// None if no verification record exists for this domain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_verification_status: Option<DomainVerificationStatus>,
    /// When the domain was successfully verified (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<DateTime<Utc>>,
}

/// Discover SSO configuration for an email address.
///
/// This endpoint allows the frontend to determine which IdP to use for login
/// based on the user's email domain. If the email domain matches an organization
/// with SSO configured, the response includes the organization details and IdP info.
///
/// SSO is only available when the email domain has been verified via DNS TXT record.
/// The response includes domain verification status to help users understand why
/// SSO may not be available.
#[tracing::instrument(name = "auth.discover", skip(state))]
pub async fn discover(
    State(state): State<AppState>,
    Query(query): Query<DiscoverQuery>,
) -> Result<Json<DiscoverResponse>, AuthError> {
    // Validate email format using proper email validation
    let email = query.email.trim().to_lowercase();
    if !email.validate_email() {
        return Err(AuthError::Forbidden("Invalid email format".to_string()));
    }

    // Extract domain - safe after validation since validate_email guarantees @ exists
    let domain = email
        .split('@')
        .nth(1)
        .expect("validate_email guarantees @ exists with valid domain");

    // Look up SSO config by email domain
    let services = state
        .services
        .as_ref()
        .ok_or_else(|| AuthError::Internal("Database not configured".to_string()))?;

    let sso_config = services
        .org_sso_configs
        .find_by_email_domain(domain)
        .await
        .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| {
            AuthError::Forbidden(format!("No SSO configuration found for domain: {}", domain))
        })?;

    // Look up organization details
    let org = services
        .organizations
        .get_by_id(sso_config.org_id)
        .await
        .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| AuthError::Internal("Organization not found for SSO config".to_string()))?;

    // Check domain verification status
    // SSO is only available if the domain has been verified via DNS TXT record
    let domain_verification = services
        .domain_verifications
        .get_by_config_and_domain(sso_config.id, domain)
        .await
        .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?;

    // Determine verification status
    let (domain_verified, domain_verification_status, verified_at) = match &domain_verification {
        Some(v) if v.is_verified() => (true, Some(v.status), v.verified_at),
        Some(v) => (false, Some(v.status), None),
        None => (false, None, None),
    };

    // SSO is only available if the config is enabled AND the domain is verified
    let effective_has_sso = sso_config.enabled && domain_verified;
    let effective_sso_required = effective_has_sso
        && sso_config.enforcement_mode == crate::models::SsoEnforcementMode::Required;

    // Extract IdP name based on provider type
    // - OIDC: from issuer URL (e.g., "accounts.google.com" from "https://accounts.google.com")
    // - SAML: from IdP entity ID URL (e.g., "idp.example.com" from "https://idp.example.com")
    let idp_name = match sso_config.provider_type {
        SsoProviderType::Oidc => sso_config
            .issuer
            .as_ref()
            .and_then(|issuer| reqwest::Url::parse(issuer).ok())
            .and_then(|u: reqwest::Url| u.host_str().map(|h| h.to_string())),
        SsoProviderType::Saml => sso_config
            .saml_idp_entity_id
            .as_ref()
            .and_then(|entity_id| reqwest::Url::parse(entity_id).ok())
            .and_then(|u: reqwest::Url| u.host_str().map(|h| h.to_string()))
            .or_else(|| sso_config.saml_idp_entity_id.clone()),
    };

    Ok(Json(DiscoverResponse {
        org_id: org.id,
        org_slug: org.slug,
        org_name: org.name,
        has_sso: effective_has_sso,
        sso_required: effective_sso_required,
        enforcement_mode: sso_config.enforcement_mode,
        provider_type: sso_config.provider_type,
        idp_name,
        domain_verified,
        domain_verification_status,
        verified_at,
    }))
}

/// Login endpoint - redirects to IdP.
///
/// If `org` is provided (either as UUID or slug), uses the org-specific SSO configuration.
/// The handler auto-dispatches to SAML or OIDC based on the org's SSO provider type.
/// Otherwise falls back to the global OIDC configuration.
#[tracing::instrument(name = "auth.login", skip(state))]
pub async fn login(
    State(state): State<AppState>,
    Query(query): Query<LoginQuery>,
) -> Result<Redirect, AuthError> {
    // If org is specified, try to use org-specific SSO
    if let Some(org_param) = &query.org {
        if let Some(services) = &state.services {
            // Try to parse as UUID first, then look up by slug
            let org = if let Ok(org_id) = org_param.parse::<Uuid>() {
                services
                    .organizations
                    .get_by_id(org_id)
                    .await
                    .ok()
                    .flatten()
            } else {
                services
                    .organizations
                    .get_by_slug(org_param)
                    .await
                    .ok()
                    .flatten()
            };

            if let Some(org) = org {
                // Look up the SSO config to determine provider type
                if let Ok(Some(sso_config)) = services.org_sso_configs.get_by_org_id(org.id).await {
                    match sso_config.provider_type {
                        #[cfg(feature = "saml")]
                        SsoProviderType::Saml => {
                            // Use SAML authenticator
                            if let Some(registry) = &state.saml_registry
                                && let Some(authenticator) = registry.get(org.id).await
                            {
                                tracing::info!(
                                    org_id = %org.id,
                                    org_slug = %org.slug,
                                    provider_type = "saml",
                                    "Using org-specific SAML SSO configuration"
                                );
                                let (auth_url, _) = authenticator
                                    .authorization_url_with_org(
                                        query.return_to.clone(),
                                        Some(org.id),
                                    )
                                    .await?;
                                return Ok(Redirect::to(&auth_url));
                            }
                        }
                        #[cfg(not(feature = "saml"))]
                        SsoProviderType::Saml => {
                            tracing::warn!(
                                org_id = %org.id,
                                "SAML SSO configured but 'saml' feature is not enabled"
                            );
                        }
                        SsoProviderType::Oidc => {
                            if let Some(registry) = &state.oidc_registry {
                                // Try to get existing authenticator, or lazy load if not registered
                                let authenticator = match registry.get(org.id).await {
                                    Some(auth) => auth,
                                    None => {
                                        // Lazy load: config exists in DB but not in registry
                                        lazy_load_oidc_authenticator(
                                            registry,
                                            services,
                                            state.secrets.as_deref(),
                                            &org,
                                        )
                                        .await?
                                    }
                                };

                                tracing::info!(
                                    org_id = %org.id,
                                    org_slug = %org.slug,
                                    provider_type = "oidc",
                                    "Using org-specific OIDC SSO configuration"
                                );
                                let (auth_url, _) = authenticator
                                    .authorization_url_with_org(
                                        query.return_to.clone(),
                                        Some(org.id),
                                    )
                                    .await?;
                                return Ok(Redirect::to(&auth_url));
                            }
                        }
                    }
                }
            }
        }
        // If org was specified but not found or no SSO config, log warning and fall through
        tracing::warn!(
            org = %org_param,
            "Org SSO config not found, falling back to global authenticator"
        );
    }

    // Fall back to global OIDC authenticator (deprecated - use per-org SSO)
    let authenticator = state.oidc_authenticator.as_ref().ok_or_else(|| {
        AuthError::Forbidden(
            "OIDC authentication not configured. Use per-org SSO with ?org=<slug> parameter."
                .to_string(),
        )
    })?;

    let (auth_url, _) = authenticator.authorization_url(query.return_to).await?;

    Ok(Redirect::to(&auth_url))
}

/// Callback endpoint - handles IdP response.
///
/// This endpoint handles callbacks from both the global OIDC provider and
/// per-organization SSO providers. The correct authenticator is selected
/// based on the `org_id` stored in the authorization state during login.
#[tracing::instrument(name = "auth.callback", skip(state, cookies, query, headers))]
pub async fn callback(
    State(state): State<AppState>,
    cookies: Cookies,
    headers: axum::http::HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, AuthError> {
    use std::sync::Arc;

    use crate::auth::{OidcAuthenticator, session_store::DeviceInfo};

    // Check for error from IdP
    if let Some(error) = &query.error {
        let description = query
            .error_description
            .as_deref()
            .unwrap_or("Unknown error");
        tracing::error!(error = %error, description = %description, "OIDC callback error");

        // Log failed authentication attempt to audit log
        if let Some(services) = &state.services {
            let user_agent = headers
                .get(axum::http::header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let ip_address =
                extract_client_ip_from_parts(&headers, None, &state.config.server.trusted_proxies)
                    .map(|ip| ip.to_string());

            let _ = services
                .audit_logs
                .log_auth_event(AuthEventParams {
                    action: auth_events::OIDC_LOGIN_FAILED,
                    session_id: Uuid::nil(), // Nil UUID indicates no session was created
                    external_id: None,
                    email: None,
                    org_id: None,
                    ip_address,
                    user_agent,
                    details: serde_json::json!({
                        "provider": "oidc",
                        "error": error,
                        "error_description": description,
                    }),
                })
                .await;
        }

        return Err(AuthError::Internal(format!(
            "Authentication failed: {} - {}",
            error, description
        )));
    }

    // Get session config - use Session config for cookie settings
    let session_config = match &state.config.auth.admin {
        Some(AdminAuthConfig::Session(config)) => config.clone(),
        _ => {
            tracing::warn!(
                "No session config found in auth.admin, using defaults for callback. \
                 This may indicate misconfiguration if sessions are expected."
            );
            crate::config::SessionConfig::default()
        }
    };

    // Determine which authenticator to use by peeking at the auth state.
    // SECURITY: We must fail explicitly if org-specific SSO was requested but
    // the authenticator is unavailable, rather than silently falling back to global.
    let authenticator: Arc<OidcAuthenticator> = if let Some(registry) = &state.oidc_registry {
        match registry.peek_auth_state(&query.state).await {
            Ok(Some(auth_state)) => {
                if let Some(org_id) = auth_state.org_id {
                    // Org-specific SSO was requested - MUST use org authenticator
                    match registry.get(org_id).await {
                        Some(org_auth) => {
                            tracing::info!(
                                org_id = %org_id,
                                "Using org-specific authenticator for callback"
                            );
                            org_auth
                        }
                        None => {
                            // This can happen if the org's SSO config was deleted during the auth flow
                            tracing::error!(
                                org_id = %org_id,
                                "Org SSO config deleted during auth flow"
                            );
                            return Err(AuthError::Internal(format!(
                                "SSO configuration for organization {} is no longer available",
                                org_id
                            )));
                        }
                    }
                } else {
                    // No org_id in state - use global authenticator
                    state.oidc_authenticator.clone().ok_or_else(|| {
                        AuthError::Internal("OIDC authentication not configured".to_string())
                    })?
                }
            }
            Ok(None) => {
                // State not found in registry's session store.
                // With per-org SSO, all auth flows should have state in the registry.
                // If not found, try global authenticator (if configured) or return 401.
                if let Some(global_auth) = state.oidc_authenticator.clone() {
                    global_auth
                } else {
                    tracing::warn!("Invalid or expired authentication state");
                    return Err(AuthError::SessionNotFound);
                }
            }
            Err(e) => {
                // Fail explicitly on state lookup errors rather than silently falling back
                tracing::error!(error = %e, "Failed to peek auth state during callback");
                return Err(AuthError::Internal(format!(
                    "Failed to validate authentication state: {}",
                    e
                )));
            }
        }
    } else {
        // No registry configured - use global authenticator if available
        if let Some(global_auth) = state.oidc_authenticator.clone() {
            global_auth
        } else {
            tracing::warn!("OIDC callback received but no authenticator available");
            return Err(AuthError::SessionNotFound);
        }
    };

    // Build device info if enhanced sessions are enabled
    let device_info = if session_config.enhanced.enabled && session_config.enhanced.track_devices {
        let user_agent = headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Generate device ID from user agent (SHA256 hash, first 16 chars)
        let device_id = user_agent.as_ref().map(|ua| {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(ua.as_bytes());
            let hash = hasher.finalize();
            hex::encode(&hash[..8])
        });

        // Parse user agent for human-readable description
        let device_description = user_agent.as_ref().map(|ua| parse_user_agent(ua));

        // Extract client IP address from headers using trusted proxy configuration
        // Note: ConnectInfo (direct TCP connection IP) is not available in the callback;
        // most production deployments use reverse proxies, so X-Forwarded-For is the primary source
        let ip_address = extract_client_ip_from_parts(
            &headers,
            None, // No ConnectInfo available in this context
            &state.config.server.trusted_proxies,
        )
        .map(|ip| ip.to_string());

        Some(DeviceInfo::new(
            user_agent,
            ip_address,
            device_id,
            device_description,
        ))
    } else {
        None
    };

    // Extract IP and user agent for audit log (before device_info is moved)
    let audit_ip_address = device_info.as_ref().and_then(|d| d.ip_address.clone());
    let audit_user_agent = device_info.as_ref().and_then(|d| d.user_agent.clone());

    // Exchange code for tokens and create session
    let (session, return_to) = authenticator
        .exchange_code_with_device(&query.code, &query.state, device_info)
        .await?;

    // Set session cookie
    let same_site = match session_config.same_site {
        SameSite::Strict => CookieSameSite::Strict,
        SameSite::Lax => CookieSameSite::Lax,
        SameSite::None => CookieSameSite::None,
    };

    let cookie_name = session_config.cookie_name.clone();
    let cookie: Cookie<'static> = Cookie::build((cookie_name, session.id.to_string()))
        .path("/")
        .http_only(true)
        .secure(session_config.secure)
        .same_site(same_site)
        .max_age(CookieDuration::seconds(session_config.duration_secs as i64))
        .build();

    cookies.add(cookie);

    tracing::info!(
        session_id = %session.id,
        external_id = %session.external_id,
        sso_org_id = ?session.sso_org_id,
        return_to = ?return_to,
        "OIDC session created"
    );

    // Log successful authentication to audit log
    if let Some(services) = &state.services {
        let _ = services
            .audit_logs
            .log_auth_event(AuthEventParams {
                action: auth_events::OIDC_LOGIN,
                session_id: session.id,
                external_id: Some(&session.external_id),
                email: session.email.as_deref(),
                org_id: session.sso_org_id,
                ip_address: audit_ip_address,
                user_agent: audit_user_agent,
                details: serde_json::json!({
                    "provider": "oidc",
                }),
            })
            .await;
    }

    // Redirect to the requested URL or home page
    // Only allow relative paths to prevent open redirect vulnerabilities
    let redirect_to = return_to
        .filter(|url| url.starts_with('/') && !url.starts_with("//"))
        .unwrap_or_else(|| "/".to_string());

    Ok(Redirect::to(&redirect_to).into_response())
}

/// Logout endpoint.
#[tracing::instrument(name = "auth.logout", skip(state, headers, cookies))]
pub async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    cookies: Cookies,
) -> Result<Response, AuthError> {
    // Extract IP and user-agent for audit logging
    let ip_address =
        extract_client_ip_from_parts(&headers, None, &state.config.server.trusted_proxies)
            .map(|ip| ip.to_string());
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let session_config = match &state.config.auth.admin {
        Some(AdminAuthConfig::Session(config)) => config.clone(),
        _ => {
            tracing::warn!(
                "No session config found in auth.admin, using defaults for logout. \
                 This may indicate misconfiguration if sessions are expected."
            );
            crate::config::SessionConfig::default()
        }
    };

    // Get session ID from cookie and logout using the shared authenticator
    if let Some(session_cookie) = cookies.get(&session_config.cookie_name)
        && let Ok(session_id) = session_cookie.value().parse::<Uuid>()
        && let Some(authenticator) = &state.oidc_authenticator
    {
        // Get session info before logging out (for audit log)
        let session_info = authenticator.get_session(session_id).await.ok();

        let _ = authenticator.logout(session_id).await;

        // Log logout to audit log
        if let Some(services) = &state.services {
            let _ = services
                .audit_logs
                .log_auth_event(AuthEventParams {
                    action: auth_events::LOGOUT,
                    session_id,
                    external_id: session_info.as_ref().map(|s| s.external_id.as_str()),
                    email: session_info.as_ref().and_then(|s| s.email.as_deref()),
                    org_id: session_info.as_ref().and_then(|s| s.sso_org_id),
                    ip_address: ip_address.clone(),
                    user_agent: user_agent.clone(),
                    details: serde_json::json!({
                        "provider": "oidc",
                    }),
                })
                .await;
        }
    }

    // Remove the session cookie
    cookies.remove(build_removal_cookie(&session_config));

    // Redirect to home or IdP logout
    Ok(Redirect::to("/").into_response())
}

/// Get current user identity.
#[tracing::instrument(name = "auth.me", skip(admin_auth))]
pub async fn me(Extension(admin_auth): Extension<AdminAuth>) -> Json<MeResponse> {
    Json(MeResponse {
        external_id: admin_auth.identity.external_id,
        email: admin_auth.identity.email,
        name: admin_auth.identity.name,
        user_id: admin_auth.identity.user_id,
        roles: admin_auth.identity.roles,
        idp_groups: admin_auth.identity.idp_groups,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// SAML Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "saml")]
/// SAML Login endpoint - generates AuthnRequest and redirects to IdP.
///
/// This endpoint initiates the SAML 2.0 SP-initiated SSO flow by generating
/// an AuthnRequest and redirecting the user to the IdP's SSO URL.
///
/// Unlike OIDC login, SAML requires an organization to be specified since
/// there's no global SAML configuration.
#[tracing::instrument(name = "auth.saml.login", skip(state))]
pub async fn saml_login(
    State(state): State<AppState>,
    Query(query): Query<SamlLoginQuery>,
) -> Result<Redirect, AuthError> {
    let services = state
        .services
        .as_ref()
        .ok_or_else(|| AuthError::Internal("Database not configured".to_string()))?;

    let saml_registry = state
        .saml_registry
        .as_ref()
        .ok_or_else(|| AuthError::Internal("SAML authentication not configured".to_string()))?;

    // Look up the organization by ID or slug
    let org = if let Ok(org_id) = query.org.parse::<Uuid>() {
        services
            .organizations
            .get_by_id(org_id)
            .await
            .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?
    } else {
        services
            .organizations
            .get_by_slug(&query.org)
            .await
            .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?
    };

    let org =
        org.ok_or_else(|| AuthError::Forbidden(format!("Organization not found: {}", query.org)))?;

    // Get the SAML authenticator for this org
    let authenticator = saml_registry.get(org.id).await.ok_or_else(|| {
        AuthError::Forbidden(format!(
            "SAML SSO not configured for organization: {}",
            org.slug
        ))
    })?;

    tracing::info!(
        org_id = %org.id,
        org_slug = %org.slug,
        "Initiating SAML SSO login"
    );

    // Generate AuthnRequest and redirect to IdP
    let (auth_url, _) = authenticator
        .authorization_url_with_org(query.return_to, Some(org.id))
        .await?;

    Ok(Redirect::to(&auth_url))
}

#[cfg(feature = "saml")]
/// SAML Assertion Consumer Service (ACS) endpoint - handles SAML Response from IdP.
///
/// This endpoint receives the SAML Response from the IdP via HTTP-POST binding,
/// validates the assertion, creates a session, and redirects to the return URL.
#[tracing::instrument(name = "auth.saml.acs", skip(state, headers, cookies, form))]
pub async fn saml_acs(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    cookies: Cookies,
    Form(form): Form<SamlAcsForm>,
) -> Result<Response, AuthError> {
    // Extract IP and user-agent for audit logging
    let ip_address =
        extract_client_ip_from_parts(&headers, None, &state.config.server.trusted_proxies)
            .map(|ip| ip.to_string());
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let saml_registry = state
        .saml_registry
        .as_ref()
        .ok_or_else(|| AuthError::Internal("SAML authentication not configured".to_string()))?;

    // Use the session config from the SAML registry (which was initialized at startup
    // with the correct config, including a proper secret for cookie signing)
    let session_config = saml_registry.default_session_config().clone();

    // Peek at the auth state to get the org_id
    let auth_state = saml_registry
        .peek_auth_state(&form.relay_state)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to peek SAML auth state");
            AuthError::Internal(format!("Failed to validate SAML state: {}", e))
        })?
        .ok_or(AuthError::InvalidToken)?;

    let org_id = auth_state.org_id.ok_or_else(|| {
        tracing::error!("SAML auth state missing org_id");
        AuthError::Internal("SAML auth state missing org_id".to_string())
    })?;

    // Get the authenticator for this org
    let authenticator = saml_registry.get(org_id).await.ok_or_else(|| {
        tracing::error!(org_id = %org_id, "SAML authenticator not found during callback");
        AuthError::Internal(format!(
            "SAML SSO configuration for organization {} is no longer available",
            org_id
        ))
    })?;

    // Validate SAML Response and create session
    let (session, return_to) = match authenticator
        .exchange_response(&form.saml_response, &form.relay_state)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            // Log failed authentication attempt to audit log
            if let Some(services) = &state.services {
                let _ = services
                    .audit_logs
                    .log_auth_event(AuthEventParams {
                        action: auth_events::SAML_LOGIN_FAILED,
                        session_id: Uuid::nil(), // Nil UUID indicates no session was created
                        external_id: None,
                        email: None,
                        org_id: Some(org_id),
                        ip_address: ip_address.clone(),
                        user_agent: user_agent.clone(),
                        details: serde_json::json!({
                            "provider": "saml",
                            "error": e.to_string(),
                        }),
                    })
                    .await;
            }
            return Err(e);
        }
    };

    // Set session cookie
    let same_site = match session_config.same_site {
        SameSite::Strict => CookieSameSite::Strict,
        SameSite::Lax => CookieSameSite::Lax,
        SameSite::None => CookieSameSite::None,
    };

    let cookie_name = session_config.cookie_name.clone();
    let cookie: Cookie<'static> = Cookie::build((cookie_name, session.id.to_string()))
        .path("/")
        .http_only(true)
        .secure(session_config.secure)
        .same_site(same_site)
        .max_age(CookieDuration::seconds(session_config.duration_secs as i64))
        .build();

    cookies.add(cookie);

    tracing::info!(
        session_id = %session.id,
        external_id = %session.external_id,
        sso_org_id = ?session.sso_org_id,
        return_to = ?return_to,
        "SAML session created"
    );

    // Log successful authentication to audit log
    if let Some(services) = &state.services {
        let _ = services
            .audit_logs
            .log_auth_event(AuthEventParams {
                action: auth_events::SAML_LOGIN,
                session_id: session.id,
                external_id: Some(&session.external_id),
                email: session.email.as_deref(),
                org_id: session.sso_org_id,
                ip_address,
                user_agent,
                details: serde_json::json!({
                    "provider": "saml",
                }),
            })
            .await;
    }

    // Redirect to the requested URL or home page
    // Only allow relative paths to prevent open redirect vulnerabilities
    let redirect_to = return_to
        .filter(|url| url.starts_with('/') && !url.starts_with("//"))
        .unwrap_or_else(|| "/".to_string());

    Ok(Redirect::to(&redirect_to).into_response())
}

#[cfg(feature = "saml")]
/// SAML Single Logout (SLO) endpoint.
///
/// Performs SP-initiated SAML Single Logout by:
/// 1. Looking up the user's session and extracting their NameID
/// 2. Finding the SAML authenticator for the org used during login
/// 3. Generating a LogoutRequest and redirecting to the IdP's SLO endpoint
/// 4. Clearing the local session
///
/// If the IdP SLO URL is not configured or SAML is not used, falls back to local-only logout.
///
/// Supports both GET and POST methods for compatibility with different IdP behaviors.
#[tracing::instrument(name = "auth.saml.slo", skip(state, headers, cookies))]
pub async fn saml_slo(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    cookies: Cookies,
) -> Result<Response, AuthError> {
    // Extract IP and user-agent for audit logging
    let ip_address =
        extract_client_ip_from_parts(&headers, None, &state.config.server.trusted_proxies)
            .map(|ip| ip.to_string());
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    // For SAML SLO, prioritize SAML registry's session config (initialized at startup
    // with the correct secret for cookie signing). Fall back to OIDC config or default.
    let session_config = state
        .saml_registry
        .as_ref()
        .map(|r| r.default_session_config().clone())
        .or_else(|| {
            if let Some(AdminAuthConfig::Session(config)) = &state.config.auth.admin {
                Some(config.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();

    // Try to get IdP SLO redirect URL before clearing session
    let mut idp_slo_redirect: Option<String> = None;

    // Get session ID from cookie
    if let Some(session_cookie) = cookies.get(&session_config.cookie_name)
        && let Ok(session_id) = session_cookie.value().parse::<Uuid>()
    {
        // Try to get the session to extract NameID and org for SLO
        if let Some(saml_registry) = &state.saml_registry {
            if let Ok(Some(session)) = saml_registry.get_session(session_id).await {
                // Check if this session was authenticated via org-specific SAML
                if let Some(sso_org_id) = session.sso_org_id {
                    // Look up the SAML authenticator for this org
                    if let Some(authenticator) = saml_registry.get(sso_org_id).await {
                        // Generate relay state for the logout redirect
                        let relay_state = Uuid::new_v4().to_string();

                        // Try to generate IdP SLO URL
                        match authenticator.generate_logout_request_url(
                            &session.external_id,
                            session.session_index.as_deref(),
                            &relay_state,
                        ) {
                            Ok(Some(url)) => {
                                tracing::info!(
                                    org_id = %sso_org_id,
                                    name_id = %session.external_id,
                                    "Redirecting to IdP SLO endpoint"
                                );
                                idp_slo_redirect = Some(url);
                            }
                            Ok(None) => {
                                tracing::debug!(
                                    org_id = %sso_org_id,
                                    "IdP SLO URL not configured, using local logout only"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    org_id = %sso_org_id,
                                    error = %e,
                                    "Failed to generate IdP SLO URL, using local logout only"
                                );
                            }
                        }
                    }
                }
            }

            // Get session info for audit log before invalidating
            let session_for_audit = saml_registry.get_session(session_id).await.ok().flatten();

            // Log logout to audit log
            if let Some(services) = &state.services {
                let _ = services
                    .audit_logs
                    .log_auth_event(AuthEventParams {
                        action: auth_events::LOGOUT,
                        session_id,
                        external_id: session_for_audit.as_ref().map(|s| s.external_id.as_str()),
                        email: session_for_audit.as_ref().and_then(|s| s.email.as_deref()),
                        org_id: session_for_audit.as_ref().and_then(|s| s.sso_org_id),
                        ip_address: ip_address.clone(),
                        user_agent: user_agent.clone(),
                        details: serde_json::json!({
                            "provider": "saml",
                        }),
                    })
                    .await;
            }

            // Invalidate session via SAML registry's session store
            let session_store = saml_registry.session_store();
            let _ = session_store.delete_session(session_id).await;
        }

        // Also try OIDC session store (sessions are shared)
        if let Some(oidc_auth) = &state.oidc_authenticator {
            let _ = oidc_auth.logout(session_id).await;
        }
    }

    // Remove the session cookie
    cookies.remove(build_removal_cookie(&session_config));

    // Redirect to IdP SLO if available, otherwise to home
    if let Some(slo_url) = idp_slo_redirect {
        tracing::info!("SAML SP-initiated SLO: redirecting to IdP");
        Ok(Redirect::to(&slo_url).into_response())
    } else {
        tracing::info!("SAML local logout completed");
        Ok(Redirect::to("/").into_response())
    }
}

#[cfg(feature = "saml")]
/// SAML SP metadata endpoint.
///
/// Returns the SP metadata XML document for IdP auto-configuration. This
/// endpoint is unauthenticated since IdPs need to fetch it during initial
/// SAML setup before any trust relationship exists.
///
/// The metadata includes:
/// - SP entity ID
/// - Assertion Consumer Service (ACS) URL with HTTP-POST binding
/// - Signing certificate (if configured)
/// - Supported NameID formats
#[tracing::instrument(name = "auth.saml.metadata", skip(state))]
pub async fn saml_metadata(
    State(state): State<AppState>,
    Query(query): Query<SamlMetadataQuery>,
) -> Result<Response, AuthError> {
    let services = state
        .services
        .as_ref()
        .ok_or_else(|| AuthError::Internal("Database not configured".to_string()))?;

    let saml_registry = state
        .saml_registry
        .as_ref()
        .ok_or_else(|| AuthError::Internal("SAML authentication not configured".to_string()))?;

    // Look up the organization by ID or slug
    let org = if let Ok(org_id) = query.org.parse::<Uuid>() {
        services
            .organizations
            .get_by_id(org_id)
            .await
            .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?
    } else {
        services
            .organizations
            .get_by_slug(&query.org)
            .await
            .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?
    };

    let org =
        org.ok_or_else(|| AuthError::Forbidden(format!("Organization not found: {}", query.org)))?;

    // Get the SAML authenticator for this org
    let authenticator = saml_registry.get(org.id).await.ok_or_else(|| {
        AuthError::Forbidden(format!(
            "SAML SSO not configured for organization: {}",
            org.slug
        ))
    })?;

    tracing::info!(
        org_id = %org.id,
        org_slug = %org.slug,
        "Serving SAML SP metadata"
    );

    // Generate SP metadata XML
    let metadata = authenticator.generate_sp_metadata();

    // Return with SAML metadata content type
    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "application/samlmetadata+xml",
        )],
        metadata,
    )
        .into_response())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a User-Agent string into a human-readable device description.
///
/// This is a simple parser that extracts browser and OS information.
/// Example output: "Chrome 120 on Windows", "Safari on macOS", "Firefox on Linux"
fn parse_user_agent(ua: &str) -> String {
    // Browser detection
    let browser = if ua.contains("Edg/") || ua.contains("Edge/") {
        // Check Edge first since it contains "Chrome"
        extract_version(ua, "Edg/")
            .or_else(|| extract_version(ua, "Edge/"))
            .map(|v| format!("Edge {}", v))
            .unwrap_or_else(|| "Edge".to_string())
    } else if ua.contains("Chrome/") && !ua.contains("Chromium/") {
        extract_version(ua, "Chrome/")
            .map(|v| format!("Chrome {}", v))
            .unwrap_or_else(|| "Chrome".to_string())
    } else if ua.contains("Firefox/") {
        extract_version(ua, "Firefox/")
            .map(|v| format!("Firefox {}", v))
            .unwrap_or_else(|| "Firefox".to_string())
    } else if ua.contains("Safari/") && !ua.contains("Chrome/") {
        extract_version(ua, "Version/")
            .map(|v| format!("Safari {}", v))
            .unwrap_or_else(|| "Safari".to_string())
    } else if ua.contains("MSIE") || ua.contains("Trident/") {
        "Internet Explorer".to_string()
    } else {
        "Unknown Browser".to_string()
    };

    // OS detection
    let os = if ua.contains("Windows") {
        "Windows"
    } else if ua.contains("Mac OS X") || ua.contains("macOS") {
        "macOS"
    } else if ua.contains("iPhone") || ua.contains("iPad") {
        "iOS"
    } else if ua.contains("Android") {
        "Android"
    } else if ua.contains("CrOS") {
        "ChromeOS"
    } else if ua.contains("Linux") {
        "Linux"
    } else {
        "Unknown OS"
    };

    format!("{} on {}", browser, os)
}

/// Extract the major version number from a User-Agent string.
fn extract_version(ua: &str, prefix: &str) -> Option<String> {
    ua.split(prefix).nth(1).and_then(|rest| {
        // Take characters until we hit a non-digit and non-dot
        let version: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();

        if version.is_empty() {
            None
        } else {
            // Return just the major version
            Some(version.split('.').next().unwrap_or(&version).to_string())
        }
    })
}

/// Lazy load an OIDC authenticator for an organization.
///
/// This function loads SSO configuration from the database, resolves secrets,
/// and registers the authenticator with the registry. Returns clear error messages
/// for each failure mode.
async fn lazy_load_oidc_authenticator(
    registry: &crate::auth::OidcAuthenticatorRegistry,
    services: &crate::services::Services,
    secrets: Option<&dyn crate::secrets::SecretManager>,
    org: &crate::models::Organization,
) -> Result<std::sync::Arc<crate::auth::oidc::OidcAuthenticator>, AuthError> {
    // Step 1: Ensure secrets manager is available
    let secrets = secrets.ok_or_else(|| {
        tracing::error!(
            org_id = %org.id,
            org_slug = %org.slug,
            "Secrets manager not configured - cannot load SSO client secret"
        );
        AuthError::Internal(
            "Secrets manager not configured. SSO requires a secrets manager to store client secrets."
                .to_string(),
        )
    })?;

    // Step 2: Load SSO config with secret from database
    let config_with_secret = services
        .org_sso_configs
        .get_with_secret_by_org_id(org.id, secrets)
        .await
        .map_err(|e| {
            tracing::error!(
                org_id = %org.id,
                org_slug = %org.slug,
                error = %e,
                "Failed to load SSO configuration from database"
            );
            AuthError::Internal(format!("Failed to load SSO configuration: {}", e))
        })?
        .ok_or_else(|| {
            tracing::error!(
                org_id = %org.id,
                org_slug = %org.slug,
                "SSO configuration not found in database"
            );
            AuthError::Internal(format!(
                "SSO configuration not found for organization '{}'",
                org.slug
            ))
        })?;

    // Step 3: Register the authenticator
    registry
        .register_from_config(&config_with_secret)
        .await
        .map_err(|e| {
            tracing::error!(
                org_id = %org.id,
                org_slug = %org.slug,
                error = %e,
                "Failed to register OIDC authenticator"
            );
            AuthError::Internal(format!(
                "Failed to initialize OIDC authenticator for '{}': {}",
                org.slug, e
            ))
        })?;

    // Step 4: Retrieve the registered authenticator
    registry.get(org.id).await.ok_or_else(|| {
        tracing::error!(
            org_id = %org.id,
            org_slug = %org.slug,
            "Authenticator not found after successful registration"
        );
        AuthError::Internal(format!(
            "Internal error: authenticator not found after registration for '{}'",
            org.slug
        ))
    })
}

#[cfg(all(test, feature = "database-sqlite"))]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    // =========================================================================
    // Test RSA Keypair (2048-bit, for testing only)
    // Generated with: openssl genrsa 2048
    // =========================================================================

    /// Test RSA private key in PKCS#8 PEM format (DO NOT USE IN PRODUCTION)
    const TEST_RSA_PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDi3r/SjMId89x2
yDQrEgFM/R70bV4Iou7z1fKAPHAAN7X8AGqzh8gyXqDvmWHH78fJPhOfUkJq8TlF
dMRrVAH2LHyALTqS0VTLBuzjKHorPXlAh1ykSu1iCSgZfWhVl1wzsR9qszi93IVl
4Zj4dcHUdL/avUfyO8OcGCOzKO4m/TiGudjmxwQ0cpCMtRAw2otU4yecouBaC1F9
Bnm2GBLennzpSJJD4D8TXsyLUKAqa5rETTJ8dsp6VeRmfdCSl4TadnryPb9onTwn
Z8YUkUKNmQEVTxHDZ5CjRoP+7Sbw/ldoYqE8gbaNHgLTZNeuMfR+D1moZZmjszc8
CDkUUvjjAgMBAAECggEACMiUUf6JIB0U6Am68KqdykadMDFxITx4VpBt9xu1P7eT
ICfpTvzEJM8XxARYOM7GbrrXNPqQ/7r0e1qYpYnMbvosnSR4eWlesw2YQPiMN6ha
+Bia3vGCXKKmHsva15V98we52P5fWq/IVQ11nV5RxtFOVusFIhJrnFuC5lOAr5mu
MU0y/h8qMV/An0/8B7V1LziBGJuSc7qL5wAj0Nos58eL4fUPj5MBiaMzs8syow8c
qZPa2MjKE/sOBP5LXzbBqUMprt7g4FaQdB88yLcfeJfOpzSxsbnoZGvDGk2g26IX
TeceCCIcYMAbEKX3ZMnZILU4xyYpt7hCwNbeISzu4QKBgQDyDIMC10SLPcae0BzX
lmQt+gO3JPzsm07OxlW1bxmvJeTwGrJvrZBFBlXPR9rZ18hpuNEm3kZpzQaSIs3A
oRCif+CNk3VbuPnB3yU+srkTCgbtQBTRbiqUOfqtkIum9uZ/t2sB1dgsKZYr6rU6
vT5oABfL3qfWlTU/ydTgs+W45wKBgQDv8kV4OyWecQbzT5GPq+9YtnK2LGG1ZXIn
41ktGzT2sa8XWZbscbtZf5NHn1ESxibrSqiqKGHc5l5SIAHQ9+dia1FtGQreuHBp
u9j4YzL4halKrxalYrsXNzzRpiJ+Gc/6qxKrLiXKIjzLIRUKTPmtmKKE3zzM0ktn
qbrqVNFUpQKBgQDW+C++7SsOM05cq96Bxiqw/rQgCzSqewDR+ioS2lpISPJ8IGnL
b62K8CZz0pBXGyL+aksvJwgIXTPxxAFSjHm2qLXpZ0Y6sRz4h1OPzLE8bJJcUaZr
nlkojhnJ3m95WRy7302lMqQsDL83v9s3EO4E9dgsk1Ii7R9+yKVM79kdjwKBgQC1
m7ZO2N2RPVUYZTnz9xtyFq1eCtttUzoCzMWbKUN+EGBImQttLGuzwqZziDbxsb6V
Se281FG1wzrSh904D9o2mKmJnHGovwp+TKpc3aAfj/LhTwIh7UdTvAAxYcArl1fe
DwtTOttpUV6YFBL7t+UmKiefz+MR130xGbsaT1Yc7QKBgBUl88mGeuB07Xq60wRB
k29JFDno/rBrJxhoqDWVz+1gZUE8bSRNXyo1zHZ3e8OtByA1ESopO25sNs3JJCkh
SgJNcXVhkDiFNMWWo2ZEoFX61AmRQrMulZGl3X/mXDiDQTtJwj6q2IEqbA4Rr6FI
Q/y/GUsTXi5AiBMUhYFZu4vS
-----END PRIVATE KEY-----"#;

    /// Key ID for the test keypair
    const TEST_KEY_ID: &str = "test-key-1";

    // Pre-computed JWKS values for the test RSA key (base64url encoded)
    // N is the modulus WITHOUT the leading 00 padding byte
    const TEST_RSA_N: &str = "4t6_0ozCHfPcdsg0KxIBTP0e9G1eCKLu89XygDxwADe1_ABqs4fIMl6g75lhx-_HyT4Tn1JCavE5RXTEa1QB9ix8gC06ktFUywbs4yh6Kz15QIdcpErtYgkoGX1oVZdcM7EfarM4vdyFZeGY-HXB1HS_2r1H8jvDnBgjsyjuJv04hrnY5scENHKQjLUQMNqLVOMnnKLgWgtRfQZ5thgS3p586UiSQ-A_E17Mi1CgKmuaxE0yfHbKelXkZn3QkpeE2nZ68j2_aJ08J2fGFJFCjZkBFU8Rw2eQo0aD_u0m8P5XaGKhPIG2jR4C02TXrjH0fg9ZqGWZo7M3PAg5FFL44w";
    const TEST_RSA_E: &str = "AQAB";

    // =========================================================================
    // JWT Generation Utilities
    // =========================================================================

    /// Create a signed JWT for testing
    fn create_test_jwt(
        issuer: &str,
        subject: &str,
        audience: &str,
        email: Option<&str>,
        name: Option<&str>,
    ) -> String {
        create_test_jwt_with_nonce(issuer, subject, audience, email, name, None)
    }

    fn create_test_jwt_with_nonce(
        issuer: &str,
        subject: &str,
        audience: &str,
        email: Option<&str>,
        name: Option<&str>,
        nonce: Option<&str>,
    ) -> String {
        use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

        let now = chrono::Utc::now().timestamp() as u64;
        let exp = now + 3600; // 1 hour from now

        let mut claims = json!({
            "iss": issuer,
            "sub": subject,
            "aud": audience,
            "exp": exp,
            "iat": now,
        });

        if let Some(email) = email {
            claims["email"] = json!(email);
        }
        if let Some(name) = name {
            claims["name"] = json!(name);
        }
        if let Some(nonce) = nonce {
            claims["nonce"] = json!(nonce);
        }

        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(TEST_KEY_ID.to_string());

        let key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes())
            .expect("Failed to create encoding key");

        encode(&header, &claims, &key).expect("Failed to encode JWT")
    }

    /// Create JWKS response containing test public key
    fn create_jwks_response() -> Value {
        json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": TEST_KEY_ID,
                "n": TEST_RSA_N,
                "e": TEST_RSA_E
            }]
        })
    }

    // =========================================================================
    // Test App Setup
    // =========================================================================

    /// Create a test app with per-org SSO configured to use the mock server
    /// Returns (app, org_slug) so tests can use ?org=<slug>
    async fn test_app_with_oidc(mock_server: &MockServer) -> (axum::Router, String) {
        use std::sync::atomic::{AtomicU64, Ordering};

        use crate::models::{
            CreateOrgSsoConfig, CreateOrganization, SsoEnforcementMode, SsoProviderType,
        };

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let db_id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let org_slug = format!("test-org-{}", db_id);

        let config_str = format!(
            r#"
[database]
type = "sqlite"
path = "file:auth_test_db_{}?mode=memory&cache=shared"
create_if_missing = true
run_migrations = true
wal_mode = false
busy_timeout_ms = 5000

[auth.admin]
type = "session"
secure = false
cookie_name = "__test_session"

[providers.test]
type = "test"
"#,
            db_id
        );

        let config = crate::config::GatewayConfig::from_str(&config_str)
            .expect("Failed to parse test config");
        let state = crate::AppState::new(config.clone())
            .await
            .expect("Failed to create AppState");

        // Create org and SSO config in database
        // Use the state's secret manager so lazy loading can decrypt secrets
        let services = state
            .services
            .as_ref()
            .expect("Services should be configured");
        let secret_manager = state
            .secrets
            .as_ref()
            .expect("Secrets should be configured");

        let org = services
            .organizations
            .create(CreateOrganization {
                name: "Test Org".to_string(),
                slug: org_slug.clone(),
            })
            .await
            .expect("Failed to create org");

        services
            .org_sso_configs
            .create(
                org.id,
                CreateOrgSsoConfig {
                    provider_type: SsoProviderType::Oidc,
                    issuer: Some(mock_server.uri()),
                    client_id: Some("test-client".to_string()),
                    client_secret: Some("test-secret".to_string()),
                    allowed_email_domains: vec!["example.com".to_string()],
                    enabled: true,
                    enforcement_mode: SsoEnforcementMode::Optional,
                    ..Default::default()
                },
                secret_manager.as_ref(),
            )
            .await
            .expect("Failed to create SSO config");

        (crate::build_app(&config, state), org_slug)
    }

    /// Mount OIDC discovery endpoint on mock server
    async fn mount_oidc_discovery(mock_server: &MockServer) {
        let discovery = json!({
            "issuer": mock_server.uri(),
            "authorization_endpoint": format!("{}/authorize", mock_server.uri()),
            "token_endpoint": format!("{}/token", mock_server.uri()),
            "userinfo_endpoint": format!("{}/userinfo", mock_server.uri()),
            "jwks_uri": format!("{}/jwks", mock_server.uri()),
            "end_session_endpoint": format!("{}/logout", mock_server.uri()),
            "scopes_supported": ["openid", "email", "profile"],
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "token_endpoint_auth_methods_supported": ["client_secret_post", "client_secret_basic"]
        });

        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&discovery))
            .mount(mock_server)
            .await;
    }

    /// Mount JWKS endpoint on mock server
    async fn mount_jwks(mock_server: &MockServer) {
        let jwks = create_jwks_response();

        Mock::given(method("GET"))
            .and(path("/jwks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&jwks))
            .mount(mock_server)
            .await;
    }

    /// Mount token endpoint on mock server
    async fn mount_token_endpoint(mock_server: &MockServer, id_token: &str) {
        let token_response = json!({
            "access_token": "test_access_token",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "test_refresh_token",
            "id_token": id_token
        });

        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&token_response))
            .mount(mock_server)
            .await;
    }

    /// Extract state and nonce from an OIDC authorization redirect URL.
    fn extract_auth_params(location: &str) -> (String, String) {
        let url = reqwest::Url::parse(location).expect("Invalid redirect URL");
        let state = url
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.to_string())
            .expect("Missing state in redirect");
        let nonce = url
            .query_pairs()
            .find(|(k, _)| k == "nonce")
            .map(|(_, v)| v.to_string())
            .expect("Missing nonce in redirect");
        (state, nonce)
    }

    // =========================================================================
    // Tests
    // =========================================================================

    #[test]
    fn test_jwt_creation_valid() {
        // Test that we can create and decode a JWT with the test keypair
        use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

        let token = create_test_jwt(
            "https://test.example.com",
            "user-123",
            "test-client",
            Some("test@example.com"),
            Some("Test User"),
        );

        // Verify the token can be decoded
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&["https://test.example.com"]);
        validation.set_audience(&["test-client"]);

        // Create decoding key from the JWKS N and E values (base64url encoded strings)
        let decoding_key = DecodingKey::from_rsa_components(TEST_RSA_N, TEST_RSA_E)
            .expect("Failed to create decoding key");

        let token_data = decode::<serde_json::Value>(&token, &decoding_key, &validation)
            .expect("Failed to decode token");

        assert_eq!(token_data.claims["sub"], "user-123");
        assert_eq!(token_data.claims["email"], "test@example.com");
    }

    #[tokio::test]
    async fn test_login_redirects_to_idp() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;

        let (app, org_slug) = test_app_with_oidc(&mock_server).await;

        let request = Request::builder()
            .method("GET")
            .uri(format!("/auth/login?org={}", org_slug))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should redirect to the authorization endpoint
        assert!(
            response.status().is_redirection(),
            "Expected redirect, got {}",
            response.status()
        );

        let location = response
            .headers()
            .get("location")
            .expect("Missing location header")
            .to_str()
            .unwrap();

        // Should redirect to mock server's authorize endpoint
        assert!(
            location.starts_with(&format!("{}/authorize", mock_server.uri())),
            "Expected redirect to {}/authorize, got {}",
            mock_server.uri(),
            location
        );

        // Should include required OIDC parameters
        assert!(
            location.contains("response_type=code"),
            "Missing response_type"
        );
        assert!(
            location.contains("client_id=test-client"),
            "Missing client_id"
        );
        assert!(
            location.contains("code_challenge="),
            "Missing PKCE code_challenge"
        );
        assert!(location.contains("state="), "Missing state parameter");
    }

    #[tokio::test]
    async fn test_login_with_return_to() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;

        let (app, org_slug) = test_app_with_oidc(&mock_server).await;

        let request = Request::builder()
            .method("GET")
            .uri(format!("/auth/login?org={}&return_to=/dashboard", org_slug))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert!(response.status().is_redirection());
        // The return_to is stored in session state, not in the redirect URL
        // Just verify the redirect works
    }

    #[tokio::test]
    async fn test_login_without_oidc_configured() {
        use std::sync::atomic::{AtomicU64, Ordering};

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        static COUNTER: AtomicU64 = AtomicU64::new(1000);
        let db_id = COUNTER.fetch_add(1, Ordering::SeqCst);

        // Create app without OIDC configured
        let config_str = format!(
            r#"
[database]
type = "sqlite"
path = "file:auth_test_no_oidc_{}?mode=memory&cache=shared"
create_if_missing = true
run_migrations = true
wal_mode = false

[providers.test]
type = "test"
"#,
            db_id
        );

        let config = crate::config::GatewayConfig::from_str(&config_str)
            .expect("Failed to parse test config");
        let state = crate::AppState::new(config.clone())
            .await
            .expect("Failed to create AppState");
        let app = crate::build_app(&config, state);

        let request = Request::builder()
            .method("GET")
            .uri("/auth/login")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Auth routes are NOT registered when auth is fully disabled (no auth.admin, no auth.gateway)
        // So /auth/login returns 404 Not Found
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_callback_with_idp_error() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;

        let (app, _org_slug) = test_app_with_oidc(&mock_server).await;

        // Simulate IdP returning an error
        let request = Request::builder()
            .method("GET")
            .uri("/auth/callback?code=test&state=test&error=access_denied&error_description=User%20denied%20access")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return error status
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap_or("")
                .contains("access_denied"),
            "Error should mention the IdP error"
        );
    }

    #[tokio::test]
    async fn test_callback_with_invalid_state() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;
        mount_jwks(&mock_server).await;

        let (app, _org_slug) = test_app_with_oidc(&mock_server).await;

        // Try callback with a state that was never issued
        let request = Request::builder()
            .method("GET")
            .uri("/auth/callback?code=test_code&state=invalid_state")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should fail due to invalid/missing state
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_callback_success() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;
        mount_jwks(&mock_server).await;

        let (app, org_slug) = test_app_with_oidc(&mock_server).await;

        // First, initiate login to get a valid state and nonce
        let login_request = Request::builder()
            .method("GET")
            .uri(format!("/auth/login?org={}", org_slug))
            .body(Body::empty())
            .unwrap();

        let login_response = app.clone().oneshot(login_request).await.unwrap();
        assert!(login_response.status().is_redirection());

        let location = login_response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap();
        let (state, nonce) = extract_auth_params(location);

        // Create JWT with the nonce from the login flow
        let id_token = create_test_jwt_with_nonce(
            &mock_server.uri(),
            "user-123",
            "test-client",
            Some("test@example.com"),
            Some("Test User"),
            Some(&nonce),
        );
        mount_token_endpoint(&mock_server, &id_token).await;

        // Now call callback with the valid state
        let callback_uri = format!("/auth/callback?code=test_auth_code&state={}", state);
        let callback_request = Request::builder()
            .method("GET")
            .uri(&callback_uri)
            .body(Body::empty())
            .unwrap();

        let callback_response = app.oneshot(callback_request).await.unwrap();
        let callback_status = callback_response.status();
        let callback_headers = callback_response.headers().clone();

        // Should redirect to home after successful auth
        if !callback_status.is_redirection() {
            let body = axum::body::to_bytes(callback_response.into_body(), usize::MAX)
                .await
                .unwrap();
            let body_str = String::from_utf8_lossy(&body);
            panic!(
                "Expected redirect after successful callback, got {} with body: {}",
                callback_status, body_str
            );
        }

        let redirect_location = callback_headers
            .get("location")
            .expect("Missing location header")
            .to_str()
            .unwrap();
        assert_eq!(redirect_location, "/");

        // Should set session cookie
        let set_cookie = callback_headers
            .get("set-cookie")
            .expect("Missing set-cookie header")
            .to_str()
            .unwrap();
        assert!(
            set_cookie.contains("__test_session"),
            "Session cookie not set"
        );
    }

    #[tokio::test]
    async fn test_callback_with_return_to() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;
        mount_jwks(&mock_server).await;

        let (app, org_slug) = test_app_with_oidc(&mock_server).await;

        // Login with return_to parameter
        let login_request = Request::builder()
            .method("GET")
            .uri(format!(
                "/auth/login?org={}&return_to=/dashboard/settings",
                org_slug
            ))
            .body(Body::empty())
            .unwrap();

        let login_response = app.clone().oneshot(login_request).await.unwrap();
        assert!(login_response.status().is_redirection());

        let location = login_response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap();
        let (state, nonce) = extract_auth_params(location);

        let id_token = create_test_jwt_with_nonce(
            &mock_server.uri(),
            "user-123",
            "test-client",
            Some("test@example.com"),
            Some("Test User"),
            Some(&nonce),
        );
        mount_token_endpoint(&mock_server, &id_token).await;

        // Complete callback
        let callback_uri = format!("/auth/callback?code=test_auth_code&state={}", state);
        let callback_request = Request::builder()
            .method("GET")
            .uri(&callback_uri)
            .body(Body::empty())
            .unwrap();

        let callback_response = app.oneshot(callback_request).await.unwrap();
        assert!(callback_response.status().is_redirection());

        // Should redirect to the return_to URL, not "/"
        let redirect_location = callback_response
            .headers()
            .get("location")
            .expect("Missing location header")
            .to_str()
            .unwrap();
        assert_eq!(
            redirect_location, "/dashboard/settings",
            "Should redirect to return_to URL"
        );
    }

    #[tokio::test]
    async fn test_callback_rejects_absolute_return_to() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;
        mount_jwks(&mock_server).await;

        let (app, org_slug) = test_app_with_oidc(&mock_server).await;

        // Try to use an absolute URL (open redirect attempt)
        let login_request = Request::builder()
            .method("GET")
            .uri(format!(
                "/auth/login?org={}&return_to=https://evil.com/steal-session",
                org_slug
            ))
            .body(Body::empty())
            .unwrap();

        let login_response = app.clone().oneshot(login_request).await.unwrap();
        let location = login_response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap();
        let (state, nonce) = extract_auth_params(location);

        let id_token = create_test_jwt_with_nonce(
            &mock_server.uri(),
            "user-123",
            "test-client",
            Some("test@example.com"),
            Some("Test User"),
            Some(&nonce),
        );
        mount_token_endpoint(&mock_server, &id_token).await;

        let callback_uri = format!("/auth/callback?code=test_auth_code&state={}", state);
        let callback_request = Request::builder()
            .method("GET")
            .uri(&callback_uri)
            .body(Body::empty())
            .unwrap();

        let callback_response = app.oneshot(callback_request).await.unwrap();

        // Should redirect to "/" (reject the absolute URL)
        let redirect_location = callback_response
            .headers()
            .get("location")
            .expect("Missing location header")
            .to_str()
            .unwrap();
        assert_eq!(
            redirect_location, "/",
            "Should reject absolute URLs and redirect to /"
        );
    }

    #[tokio::test]
    async fn test_callback_rejects_protocol_relative_return_to() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;
        mount_jwks(&mock_server).await;

        let (app, org_slug) = test_app_with_oidc(&mock_server).await;

        // Try to use a protocol-relative URL (open redirect attempt)
        let login_request = Request::builder()
            .method("GET")
            .uri(format!(
                "/auth/login?org={}&return_to=//evil.com/steal-session",
                org_slug
            ))
            .body(Body::empty())
            .unwrap();

        let login_response = app.clone().oneshot(login_request).await.unwrap();
        let location = login_response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap();
        let (state, nonce) = extract_auth_params(location);

        let id_token = create_test_jwt_with_nonce(
            &mock_server.uri(),
            "user-123",
            "test-client",
            Some("test@example.com"),
            Some("Test User"),
            Some(&nonce),
        );
        mount_token_endpoint(&mock_server, &id_token).await;

        let callback_uri = format!("/auth/callback?code=test_auth_code&state={}", state);
        let callback_request = Request::builder()
            .method("GET")
            .uri(&callback_uri)
            .body(Body::empty())
            .unwrap();

        let callback_response = app.oneshot(callback_request).await.unwrap();

        // Should redirect to "/" (reject the protocol-relative URL)
        let redirect_location = callback_response
            .headers()
            .get("location")
            .expect("Missing location header")
            .to_str()
            .unwrap();
        assert_eq!(
            redirect_location, "/",
            "Should reject protocol-relative URLs and redirect to /"
        );
    }

    #[tokio::test]
    async fn test_logout_clears_cookie() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;

        let (app, _org_slug) = test_app_with_oidc(&mock_server).await;

        // Logout is a POST endpoint
        let request = Request::builder()
            .method("POST")
            .uri("/auth/logout")
            .header("cookie", "__test_session=some-session-id")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();

        // Should redirect to home
        assert!(
            status.is_redirection(),
            "Expected redirect, got {} {:?}",
            status,
            response.headers()
        );

        let location = response
            .headers()
            .get("location")
            .expect("Missing location header")
            .to_str()
            .unwrap();
        assert_eq!(location, "/");

        // Should clear the session cookie (max-age=0)
        let set_cookie = response
            .headers()
            .get("set-cookie")
            .expect("Missing set-cookie header")
            .to_str()
            .unwrap();
        assert!(
            set_cookie.contains("Max-Age=0") || set_cookie.contains("max-age=0"),
            "Cookie should be expired, got: {}",
            set_cookie
        );
    }

    #[tokio::test]
    async fn test_logout_without_oidc_configured() {
        use std::sync::atomic::{AtomicU64, Ordering};

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        static COUNTER: AtomicU64 = AtomicU64::new(2000);
        let db_id = COUNTER.fetch_add(1, Ordering::SeqCst);

        let config_str = format!(
            r#"
[database]
type = "sqlite"
path = "file:auth_test_logout_no_oidc_{}?mode=memory&cache=shared"
create_if_missing = true
run_migrations = true
wal_mode = false

[providers.test]
type = "test"
"#,
            db_id
        );

        let config = crate::config::GatewayConfig::from_str(&config_str)
            .expect("Failed to parse test config");
        let state = crate::AppState::new(config.clone())
            .await
            .expect("Failed to create AppState");
        let app = crate::build_app(&config, state);

        let request = Request::builder()
            .method("POST")
            .uri("/auth/logout")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Auth routes are NOT registered when auth is fully disabled (no auth.admin, no auth.gateway)
        // So /auth/logout returns 404 Not Found
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Expected 404, got {}",
            response.status()
        );
    }

    // =========================================================================
    // Discovery Endpoint Tests
    // =========================================================================

    /// Create a test app with database services (for discover endpoint tests)
    async fn test_app_with_db_and_oidc(
        mock_server: &MockServer,
    ) -> (axum::Router, crate::AppState) {
        use std::sync::atomic::{AtomicU64, Ordering};

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
        // mock_server is kept for API compatibility but not used in config anymore
        let _ = mock_server;

        static COUNTER: AtomicU64 = AtomicU64::new(3000);
        let db_id = COUNTER.fetch_add(1, Ordering::SeqCst);

        let config_str = format!(
            r#"
[database]
type = "sqlite"
path = "file:auth_discover_test_db_{}?mode=memory&cache=shared"
create_if_missing = true
run_migrations = true
wal_mode = false
busy_timeout_ms = 5000

[auth.admin]
type = "session"
secure = false
cookie_name = "__test_session"

[providers.test]
type = "test"
"#,
            db_id
        );

        let config = crate::config::GatewayConfig::from_str(&config_str)
            .expect("Failed to parse test config");
        let state = crate::AppState::new(config.clone())
            .await
            .expect("Failed to create AppState");
        let app = crate::build_app(&config, state.clone());
        (app, state)
    }

    #[tokio::test]
    async fn test_discover_no_sso_config_for_domain() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;

        let (app, _state) = test_app_with_db_and_oidc(&mock_server).await;

        // Try to discover SSO for a domain with no config
        let request = Request::builder()
            .method("GET")
            .uri("/auth/discover?email=user@unknown-domain.com")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return 403 Forbidden
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_discover_invalid_email_format() {
        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;

        let (app, _state) = test_app_with_db_and_oidc(&mock_server).await;

        // Try to discover SSO with invalid email
        let request = Request::builder()
            .method("GET")
            .uri("/auth/discover?email=invalid-email")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should return 403 Forbidden (invalid email format)
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_discover_with_verified_domain() {
        use crate::{
            models::{
                CreateDomainVerification, CreateOrgSsoConfig, CreateOrganization,
                DomainVerificationStatus, SsoEnforcementMode, SsoProviderType,
                UpdateDomainVerification,
            },
            secrets::MemorySecretManager,
        };

        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;

        let (app, state) = test_app_with_db_and_oidc(&mock_server).await;

        let services = state
            .services
            .as_ref()
            .expect("Services should be configured");
        let secret_manager: &dyn crate::secrets::SecretManager = &MemorySecretManager::new();

        // Create an organization
        let org = services
            .organizations
            .create(CreateOrganization {
                name: "Test Org".to_string(),
                slug: "test-org".to_string(),
            })
            .await
            .expect("Failed to create org");

        // Create SSO config with allowed_email_domains
        let sso_config = services
            .org_sso_configs
            .create(
                org.id,
                CreateOrgSsoConfig {
                    provider_type: SsoProviderType::Oidc,
                    issuer: Some(mock_server.uri()),
                    client_id: Some("test-client".to_string()),
                    client_secret: Some("test-secret".to_string()),
                    allowed_email_domains: vec!["verified-domain.com".to_string()],
                    enabled: true,
                    enforcement_mode: SsoEnforcementMode::Optional,
                    ..Default::default()
                },
                secret_manager,
            )
            .await
            .expect("Failed to create SSO config");

        // Create a verified domain verification record
        let verification = services
            .domain_verifications
            .create(
                sso_config.id,
                CreateDomainVerification {
                    domain: "verified-domain.com".to_string(),
                },
            )
            .await
            .expect("Failed to create domain verification");

        // Update status directly for testing (DNS lookup will fail in tests)
        // Access db directly since service doesn't expose update
        let db = state.db.as_ref().expect("Database should be configured");
        db.domain_verifications()
            .update(
                verification.id,
                UpdateDomainVerification {
                    status: Some(DomainVerificationStatus::Verified),
                    verified_at: Some(Some(chrono::Utc::now())),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to update verification status");

        // Discover SSO for the verified domain
        let request = Request::builder()
            .method("GET")
            .uri("/auth/discover?email=user@verified-domain.com")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["org_id"], org.id.to_string());
        assert_eq!(json["org_slug"], "test-org");
        assert_eq!(json["org_name"], "Test Org");
        assert_eq!(json["has_sso"], true);
        assert_eq!(json["sso_required"], false);
        assert_eq!(json["enforcement_mode"], "optional"); // Default mode
        assert_eq!(json["provider_type"], "oidc"); // OIDC provider type
        assert_eq!(json["domain_verified"], true);
        assert_eq!(json["domain_verification_status"], "verified");
        assert!(json["verified_at"].is_string());
    }

    #[tokio::test]
    async fn test_discover_with_pending_domain() {
        use crate::{
            models::{
                CreateDomainVerification, CreateOrgSsoConfig, CreateOrganization,
                SsoEnforcementMode, SsoProviderType,
            },
            secrets::MemorySecretManager,
        };

        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;

        let (app, state) = test_app_with_db_and_oidc(&mock_server).await;

        let services = state
            .services
            .as_ref()
            .expect("Services should be configured");
        let secret_manager: &dyn crate::secrets::SecretManager = &MemorySecretManager::new();

        // Create an organization
        let org = services
            .organizations
            .create(CreateOrganization {
                name: "Pending Org".to_string(),
                slug: "pending-org".to_string(),
            })
            .await
            .expect("Failed to create org");

        // Create SSO config with allowed_email_domains
        let sso_config = services
            .org_sso_configs
            .create(
                org.id,
                CreateOrgSsoConfig {
                    provider_type: SsoProviderType::Oidc,
                    issuer: Some(mock_server.uri()),
                    client_id: Some("test-client".to_string()),
                    client_secret: Some("test-secret".to_string()),
                    allowed_email_domains: vec!["pending-domain.com".to_string()],
                    enabled: true,
                    enforcement_mode: SsoEnforcementMode::Required,
                    ..Default::default()
                },
                secret_manager,
            )
            .await
            .expect("Failed to create SSO config");

        // Create a pending domain verification record (not verified)
        services
            .domain_verifications
            .create(
                sso_config.id,
                CreateDomainVerification {
                    domain: "pending-domain.com".to_string(),
                },
            )
            .await
            .expect("Failed to create domain verification");

        // Discover SSO for the pending domain
        let request = Request::builder()
            .method("GET")
            .uri("/auth/discover?email=user@pending-domain.com")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["org_id"], org.id.to_string());
        assert_eq!(json["org_slug"], "pending-org");
        assert_eq!(json["has_sso"], false); // SSO not available - domain not verified
        assert_eq!(json["sso_required"], false); // Not required because has_sso is false
        assert_eq!(json["enforcement_mode"], "required"); // Mode is set but not effective (domain not verified)
        assert_eq!(json["provider_type"], "oidc"); // OIDC provider type
        assert_eq!(json["domain_verified"], false);
        assert_eq!(json["domain_verification_status"], "pending");
        assert!(json["verified_at"].is_null());
    }

    #[tokio::test]
    async fn test_discover_no_verification_record() {
        use crate::{
            models::{CreateOrgSsoConfig, CreateOrganization, SsoEnforcementMode, SsoProviderType},
            secrets::MemorySecretManager,
        };

        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;

        let (app, state) = test_app_with_db_and_oidc(&mock_server).await;

        let services = state
            .services
            .as_ref()
            .expect("Services should be configured");
        let secret_manager: &dyn crate::secrets::SecretManager = &MemorySecretManager::new();

        // Create an organization
        let org = services
            .organizations
            .create(CreateOrganization {
                name: "No Verification Org".to_string(),
                slug: "no-verification-org".to_string(),
            })
            .await
            .expect("Failed to create org");

        // Create SSO config with allowed_email_domains but NO verification record
        services
            .org_sso_configs
            .create(
                org.id,
                CreateOrgSsoConfig {
                    provider_type: SsoProviderType::Oidc,
                    issuer: Some(mock_server.uri()),
                    client_id: Some("test-client".to_string()),
                    client_secret: Some("test-secret".to_string()),
                    allowed_email_domains: vec!["unverified-domain.com".to_string()],
                    enabled: true,
                    enforcement_mode: SsoEnforcementMode::Optional,
                    ..Default::default()
                },
                secret_manager,
            )
            .await
            .expect("Failed to create SSO config");

        // Discover SSO - no verification record exists
        let request = Request::builder()
            .method("GET")
            .uri("/auth/discover?email=user@unverified-domain.com")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["org_slug"], "no-verification-org");
        assert_eq!(json["has_sso"], false); // SSO not available - no verification record
        assert_eq!(json["sso_required"], false);
        assert_eq!(json["enforcement_mode"], "optional"); // Default mode
        assert_eq!(json["provider_type"], "oidc"); // OIDC provider type
        assert_eq!(json["domain_verified"], false);
        assert!(json["domain_verification_status"].is_null()); // No verification record
    }

    // =========================================================================
    // Auth Audit Log Tests
    // =========================================================================

    #[tokio::test]
    async fn test_oidc_callback_success_creates_audit_log() {
        use crate::models::{
            AuditLogQuery, CreateOrgSsoConfig, CreateOrganization, SsoEnforcementMode,
            SsoProviderType,
        };

        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;
        mount_jwks(&mock_server).await;

        let (app, state) = test_app_with_db_and_oidc(&mock_server).await;

        // Create org and SSO config for per-org SSO
        let services = state
            .services
            .as_ref()
            .expect("Services should be configured");
        let secret_manager = state
            .secrets
            .as_ref()
            .expect("Secrets should be configured");

        let org = services
            .organizations
            .create(CreateOrganization {
                name: "Audit Test Org".to_string(),
                slug: "audit-test-org".to_string(),
            })
            .await
            .expect("Failed to create org");

        services
            .org_sso_configs
            .create(
                org.id,
                CreateOrgSsoConfig {
                    provider_type: SsoProviderType::Oidc,
                    issuer: Some(mock_server.uri()),
                    client_id: Some("test-client".to_string()),
                    client_secret: Some("test-secret".to_string()),
                    allowed_email_domains: vec!["example.com".to_string()],
                    enabled: true,
                    enforcement_mode: SsoEnforcementMode::Optional,
                    ..Default::default()
                },
                secret_manager.as_ref(),
            )
            .await
            .expect("Failed to create SSO config");

        // Initiate login to get a valid state and nonce
        let login_request = Request::builder()
            .method("GET")
            .uri("/auth/login?org=audit-test-org")
            .body(Body::empty())
            .unwrap();

        let login_response = app.clone().oneshot(login_request).await.unwrap();
        assert!(
            login_response.status().is_redirection(),
            "Expected redirect, got {}",
            login_response.status()
        );
        let location = login_response
            .headers()
            .get("location")
            .expect("Missing location header")
            .to_str()
            .unwrap();
        let (auth_state, nonce) = extract_auth_params(location);

        // Create JWT with the nonce from the login flow
        let id_token = create_test_jwt_with_nonce(
            &mock_server.uri(),
            "user-123",
            "test-client",
            Some("test@example.com"),
            Some("Test User"),
            Some(&nonce),
        );
        mount_token_endpoint(&mock_server, &id_token).await;

        // Complete callback
        let callback_uri = format!("/auth/callback?code=test_auth_code&state={}", auth_state);
        let callback_request = Request::builder()
            .method("GET")
            .uri(&callback_uri)
            .body(Body::empty())
            .unwrap();

        let callback_response = app.oneshot(callback_request).await.unwrap();
        assert!(callback_response.status().is_redirection());

        // Verify audit log was created
        let services = state
            .services
            .as_ref()
            .expect("Services should be configured");
        let audit_logs = services
            .audit_logs
            .list(AuditLogQuery {
                action: Some("auth.oidc.login".to_string()),
                ..Default::default()
            })
            .await
            .expect("Failed to query audit logs");

        assert!(
            !audit_logs.items.is_empty(),
            "Expected at least one auth.oidc.login audit log entry"
        );

        let log_entry = &audit_logs.items[0];
        assert_eq!(log_entry.action, "auth.oidc.login");
        assert_eq!(log_entry.resource_type, "session");
        assert!(log_entry.details.get("provider").is_some());
        assert_eq!(log_entry.details["provider"], "oidc");
    }

    #[tokio::test]
    async fn test_oidc_callback_failure_creates_audit_log() {
        use crate::models::AuditLogQuery;

        let mock_server = MockServer::start().await;
        mount_oidc_discovery(&mock_server).await;

        let (app, state) = test_app_with_db_and_oidc(&mock_server).await;

        // Simulate IdP returning an error
        let request = Request::builder()
            .method("GET")
            .uri("/auth/callback?code=test&state=test&error=access_denied&error_description=User%20denied%20access")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // Verify audit log was created for the failure
        let services = state
            .services
            .as_ref()
            .expect("Services should be configured");
        let audit_logs = services
            .audit_logs
            .list(AuditLogQuery {
                action: Some("auth.oidc.login_failed".to_string()),
                ..Default::default()
            })
            .await
            .expect("Failed to query audit logs");

        assert!(
            !audit_logs.items.is_empty(),
            "Expected at least one auth.oidc.login_failed audit log entry"
        );

        let log_entry = &audit_logs.items[0];
        assert_eq!(log_entry.action, "auth.oidc.login_failed");
        assert_eq!(log_entry.resource_type, "session");
        assert!(log_entry.details.get("error").is_some());
        assert_eq!(log_entry.details["error"], "access_denied");
    }
}
