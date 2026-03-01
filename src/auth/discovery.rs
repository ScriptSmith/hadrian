//! Shared OIDC discovery helpers.
//!
//! Extracts the `jwks_uri` from an OIDC discovery document with SSRF validation.
//! Used by both `gateway_jwt` (unconditional) and `oidc` (behind `sso` feature).

use super::AuthError;

/// Minimal OIDC discovery document — only the field we need.
#[derive(serde::Deserialize)]
struct DiscoveryDocument {
    jwks_uri: String,
}

/// Fetch the `jwks_uri` from an OIDC discovery endpoint.
///
/// Validates both `discovery_url` and the returned `jwks_uri` against SSRF
/// using [`crate::validation::validate_base_url`].
pub async fn fetch_jwks_uri(
    discovery_url: &str,
    http_client: &reqwest::Client,
    allow_loopback: bool,
) -> Result<String, AuthError> {
    // SSRF-validate the discovery URL before fetching
    crate::validation::validate_base_url(discovery_url, allow_loopback)
        .map_err(|e| AuthError::Internal(format!("Discovery URL failed SSRF validation: {e}")))?;

    let url = if discovery_url.ends_with("/.well-known/openid-configuration") {
        discovery_url.to_string()
    } else {
        format!(
            "{}/.well-known/openid-configuration",
            discovery_url.trim_end_matches('/')
        )
    };

    tracing::debug!(url = %url, "Fetching OIDC discovery for JWKS URI");

    let response = http_client
        .get(&url)
        .send()
        .await
        .map_err(|e| AuthError::Internal(format!("Failed to fetch OIDC discovery: {e}")))?;

    if !response.status().is_success() {
        return Err(AuthError::Internal(format!(
            "OIDC discovery returned {}",
            response.status()
        )));
    }

    let doc: DiscoveryDocument = response
        .json()
        .await
        .map_err(|e| AuthError::Internal(format!("Failed to parse OIDC discovery: {e}")))?;

    // SSRF-validate the returned JWKS URI before returning it
    crate::validation::validate_base_url(&doc.jwks_uri, allow_loopback)
        .map_err(|e| AuthError::Internal(format!("JWKS URI failed SSRF validation: {e}")))?;

    Ok(doc.jwks_uri)
}
