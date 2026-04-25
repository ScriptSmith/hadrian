//! Public counterpart to [`crate::routes::admin::oauth`] — exchanges an
//! authorization code (issued from the in-browser consent page) for a
//! user-scoped API key. This endpoint is unauthenticated; the PKCE proof
//! is the authentication.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::{
    AppState,
    models::{ApiKeyOwner, ApiKeyScope, CreateApiKey, ExchangeCodeForKey},
    openapi::ErrorResponse,
    services::{OAuthPkceError, Services},
};

/// Errors specific to the public token exchange endpoint.
///
/// Kept minimal so we never leak internal details to unauthenticated callers.
#[derive(Debug)]
pub enum OAuthTokenError {
    /// Endpoint disabled via `auth.oauth_pkce.enabled = false`.
    NotFound,
    /// Code unknown, expired, already redeemed, or PKCE verifier mismatch.
    /// Returned as 400 — RFC 6749 §5.2 calls this `invalid_grant`.
    InvalidGrant,
    /// Internal error from the database, services layer, or downstream call.
    Internal,
}

impl IntoResponse for OAuthTokenError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "OAuth PKCE flow is disabled",
            ),
            Self::InvalidGrant => (
                StatusCode::BAD_REQUEST,
                "invalid_grant",
                "Authorization code is invalid, expired, or already used",
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "An internal error occurred",
            ),
        };
        (status, Json(ErrorResponse::new(code, message.to_string()))).into_response()
    }
}

/// Response from `POST /oauth/token`. Mirrors OpenRouter's shape so that
/// existing client libraries written against their PKCE flow drop in.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OAuthTokenResponse {
    /// The newly-issued API key. Only returned once — store it securely.
    pub key: String,
    /// User-facing prefix of the key (e.g. `gw_abc...`). Useful for display.
    pub key_prefix: String,
    /// ID of the API key record, for revocation via the admin API.
    pub key_id: uuid::Uuid,
}

/// Exchange a PKCE authorization code for a user-scoped API key.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/oauth/token",
    tag = "oauth",
    operation_id = "oauth_token",
    request_body = ExchangeCodeForKey,
    responses(
        (status = 200, description = "API key issued", body = OAuthTokenResponse),
        (status = 400, description = "Authorization code is invalid, expired, or already used", body = ErrorResponse),
        (status = 404, description = "OAuth PKCE flow is disabled", body = ErrorResponse),
    )
))]
pub async fn token(
    State(state): State<AppState>,
    Json(input): Json<ExchangeCodeForKey>,
) -> Result<Json<OAuthTokenResponse>, OAuthTokenError> {
    let pkce_config = &state.config.auth.oauth_pkce;
    if !pkce_config.enabled {
        return Err(OAuthTokenError::NotFound);
    }
    let services: &Services = state.services.as_ref().ok_or(OAuthTokenError::Internal)?;

    let stored = match services
        .oauth_pkce
        .redeem_code(
            &input.code,
            &input.code_verifier,
            input.code_challenge_method,
        )
        .await
    {
        Ok(s) => s,
        Err(OAuthPkceError::InvalidCode) | Err(OAuthPkceError::PkceMismatch) => {
            return Err(OAuthTokenError::InvalidGrant);
        }
        Err(OAuthPkceError::Db(err)) => {
            tracing::error!(error = %err, "Database error redeeming OAuth PKCE code");
            return Err(OAuthTokenError::Internal);
        }
    };

    let prefix = state.config.auth.api_key_config().generation_prefix();

    let opts = stored.key_options.clone();
    let key_name = opts
        .name
        .filter(|n| !n.trim().is_empty())
        .or_else(|| stored.app_name.clone())
        .unwrap_or_else(|| "OAuth app".to_string());

    // Treat `0` and `None` as "no budget" — matches the convention in the
    // self-service modal where leaving the field blank means unlimited.
    let budget_limit_cents = opts.budget_limit_cents.filter(|cents| *cents > 0);
    let budget_period = if budget_limit_cents.is_some() {
        opts.budget_period
    } else {
        None
    };

    // The authorize endpoint stores the resolved owner on the code; fall
    // back to a user-owned key if it's missing (e.g. an old code from before
    // owner selection shipped).
    let owner = opts.owner.unwrap_or(ApiKeyOwner::User {
        user_id: stored.user_id,
    });

    let create_input = CreateApiKey {
        name: key_name,
        owner,
        budget_limit_cents,
        budget_period,
        expires_at: opts.expires_at,
        scopes: opts.scopes,
        allowed_models: opts.allowed_models,
        ip_allowlist: opts.ip_allowlist,
        rate_limit_rpm: opts.rate_limit_rpm,
        rate_limit_tpm: opts.rate_limit_tpm,
        sovereignty_requirements: opts.sovereignty_requirements,
    };

    let created = services
        .api_keys
        .create(create_input, &prefix)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "Failed to create OAuth-issued API key");
            OAuthTokenError::Internal
        })?;

    Ok(Json(OAuthTokenResponse {
        key: created.key,
        key_prefix: created.api_key.key_prefix,
        key_id: created.api_key.id,
    }))
}

/// Authorization Server Metadata document per
/// [RFC 8414](https://www.rfc-editor.org/rfc/rfc8414). Lets PKCE clients
/// discover Hadrian's authorize/token endpoints, supported challenge
/// methods, and supported scopes without hard-coding URLs.
///
/// Hadrian implements only the subset of OAuth 2.0 needed for the PKCE
/// authorization-code flow: there is no dynamic client registration, no
/// refresh tokens, and no client credentials grant. Fields the spec marks
/// as required but that are not meaningful for our flow are emitted with
/// the closest accurate value (e.g. `token_endpoint_auth_methods_supported`
/// is `["none"]` because clients authenticate via PKCE proof, not a secret).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AuthorizationServerMetadata {
    /// Issuer identifier — the deployment's base URL.
    pub issuer: String,
    /// URL of the authorization (consent) endpoint.
    pub authorization_endpoint: String,
    /// URL of the token (code-exchange) endpoint.
    pub token_endpoint: String,
    /// PKCE challenge methods supported by this server.
    pub code_challenge_methods_supported: Vec<&'static str>,
    /// OAuth 2.0 response types supported. Always `["code"]`.
    pub response_types_supported: Vec<&'static str>,
    /// OAuth 2.0 grant types supported. Always `["authorization_code"]`.
    pub grant_types_supported: Vec<&'static str>,
    /// Token endpoint client-authentication methods. Always `["none"]` —
    /// PKCE is the authentication mechanism.
    pub token_endpoint_auth_methods_supported: Vec<&'static str>,
    /// API-key scopes that may appear in `scopes` on the authorize request.
    pub scopes_supported: Vec<&'static str>,
    /// Whether the server provides a metadata document (always true here —
    /// included for clients that probe the field).
    pub service_documentation: Option<String>,
}

/// Derive the issuer base URL (scheme + host, no trailing slash) from
/// request headers. Honours `X-Forwarded-Proto` and `X-Forwarded-Host` when
/// the deployment is behind a reverse proxy; otherwise falls back to the
/// `Host` header and infers the scheme from the TLS config.
fn derive_issuer(headers: &HeaderMap, tls_configured: bool) -> Option<String> {
    let header_str = |name: &header::HeaderName| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };

    let scheme = header_str(&header::HeaderName::from_static("x-forwarded-proto"))
        .map(|s| {
            // Some proxies send a comma-separated chain; the first entry is
            // the original scheme.
            s.split(',').next().unwrap_or("").trim().to_string()
        })
        .filter(|s| s == "http" || s == "https")
        .unwrap_or_else(|| {
            if tls_configured {
                "https".into()
            } else {
                "http".into()
            }
        });

    let host = header_str(&header::HeaderName::from_static("x-forwarded-host"))
        .or_else(|| header_str(&header::HOST))?;
    Some(format!("{scheme}://{host}"))
}

/// Serve the OAuth 2.0 Authorization Server Metadata document.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/.well-known/oauth-authorization-server",
    tag = "oauth",
    operation_id = "oauth_authorization_server_metadata",
    responses(
        (status = 200, description = "Authorization server metadata", body = AuthorizationServerMetadata),
        (status = 404, description = "OAuth PKCE flow disabled", body = ErrorResponse),
    )
))]
pub async fn authorization_server_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthorizationServerMetadata>, OAuthTokenError> {
    let pkce = &state.config.auth.oauth_pkce;
    if !pkce.enabled {
        return Err(OAuthTokenError::NotFound);
    }

    let issuer = derive_issuer(&headers, state.config.server.tls.is_some()).ok_or_else(|| {
        tracing::warn!("Cannot derive issuer URL for /.well-known/oauth-authorization-server");
        OAuthTokenError::Internal
    })?;

    let mut methods: Vec<&'static str> = vec!["S256"];
    if pkce.allow_plain_method {
        methods.push("plain");
    }

    Ok(Json(AuthorizationServerMetadata {
        issuer: issuer.clone(),
        authorization_endpoint: format!("{issuer}/oauth/authorize"),
        token_endpoint: format!("{issuer}/oauth/token"),
        code_challenge_methods_supported: methods,
        response_types_supported: vec!["code"],
        grant_types_supported: vec!["authorization_code"],
        token_endpoint_auth_methods_supported: vec!["none"],
        scopes_supported: ApiKeyScope::all_names(),
        service_documentation: Some(format!("{issuer}/docs/features/oauth-pkce")),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hm(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        h
    }

    #[test]
    fn issuer_uses_x_forwarded_chain_when_present() {
        let headers = hm(&[
            ("host", "internal:8080"),
            ("x-forwarded-host", "hadrian.example.com"),
            ("x-forwarded-proto", "https"),
        ]);
        assert_eq!(
            derive_issuer(&headers, false).unwrap(),
            "https://hadrian.example.com"
        );
    }

    #[test]
    fn issuer_falls_back_to_host_and_tls_config() {
        let headers = hm(&[("host", "hadrian.example.com")]);
        assert_eq!(
            derive_issuer(&headers, true).unwrap(),
            "https://hadrian.example.com"
        );
        assert_eq!(
            derive_issuer(&headers, false).unwrap(),
            "http://hadrian.example.com"
        );
    }

    #[test]
    fn issuer_returns_none_without_host() {
        assert!(derive_issuer(&HeaderMap::new(), false).is_none());
    }

    #[test]
    fn issuer_picks_first_proto_in_chain() {
        let headers = hm(&[("host", "h:8080"), ("x-forwarded-proto", "https, http")]);
        assert_eq!(derive_issuer(&headers, false).unwrap(), "https://h:8080");
    }
}
