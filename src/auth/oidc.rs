//! OIDC (OpenID Connect) authentication.
//!
//! This module implements the OIDC authorization code flow for browser-based
//! authentication. It handles:
//! - Generating authorization URLs with PKCE
//! - Token exchange after callback
//! - Session management via cookies
//! - Token refresh (if refresh tokens are available)

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use reqwest::Url;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{
    AuthError,
    jwt::JwtValidator,
    session_store::{
        AuthorizationState, DeviceInfo, MemorySessionStore, OidcSession, SharedSessionStore,
        enforce_session_limit, validate_and_refresh_session,
    },
};
use crate::config::OidcAuthConfig;

/// OIDC discovery document.
#[derive(Debug, Clone, Deserialize)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: String,
    #[serde(default)]
    pub end_session_endpoint: Option<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
    #[serde(default)]
    pub response_types_supported: Vec<String>,
    #[serde(default)]
    pub grant_types_supported: Vec<String>,
    #[serde(default)]
    pub token_endpoint_auth_methods_supported: Vec<String>,
}

/// Token response from the OIDC provider.
/// Note: Some fields like `token_type` and `scope` are deserialized for completeness
/// but not currently used. They may be useful for future features or debugging.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Deserialization type
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub token_type: String,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// PKCE (Proof Key for Code Exchange) data.
#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
}

impl PkceChallenge {
    /// Generate a new PKCE challenge.
    pub fn new() -> Self {
        // Generate 32 bytes of random data for the verifier
        let mut verifier_bytes = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut verifier_bytes);
        let code_verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

        // SHA256 hash the verifier to create the challenge
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let challenge_bytes = hasher.finalize();
        let code_challenge = URL_SAFE_NO_PAD.encode(challenge_bytes);

        Self {
            code_verifier,
            code_challenge,
        }
    }
}

impl Default for PkceChallenge {
    fn default() -> Self {
        Self::new()
    }
}

/// Cached OIDC discovery document.
struct CachedDiscovery {
    discovery: OidcDiscovery,
    fetched_at: Instant,
}

/// OIDC authenticator that handles the full authorization code flow.
pub struct OidcAuthenticator {
    config: OidcAuthConfig,
    http_client: reqwest::Client,
    discovery_cache: RwLock<Option<CachedDiscovery>>,
    jwt_validator: RwLock<Option<Arc<JwtValidator>>>,
    session_store: SharedSessionStore,
}

impl OidcAuthenticator {
    /// Create a new OIDC authenticator with a session store.
    ///
    /// For multi-node deployments, pass a `CacheSessionStore` backed by Redis.
    /// For single-node deployments, a `MemorySessionStore` can be used.
    pub fn new(config: OidcAuthConfig, session_store: SharedSessionStore) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
            discovery_cache: RwLock::new(None),
            jwt_validator: RwLock::new(None),
            session_store,
        }
    }

    /// Create a new OIDC authenticator with a fallback in-memory session store.
    ///
    /// **Warning**: Sessions will not be shared across nodes and will be lost on restart.
    /// Use `new()` with a proper session store for production.
    pub fn new_with_memory_store(config: OidcAuthConfig) -> Self {
        tracing::warn!(
            "Creating OidcAuthenticator with in-memory session store. \
             Sessions will not be shared across nodes."
        );
        Self::new(config, Arc::new(MemorySessionStore::new()))
    }

    /// Create a new OIDC authenticator with a custom HTTP client.
    pub fn with_client(
        config: OidcAuthConfig,
        http_client: reqwest::Client,
        session_store: SharedSessionStore,
    ) -> Self {
        Self {
            config,
            http_client,
            discovery_cache: RwLock::new(None),
            jwt_validator: RwLock::new(None),
            session_store,
        }
    }

    /// Get the session store.
    pub fn session_store(&self) -> &SharedSessionStore {
        &self.session_store
    }

    /// Get the OIDC discovery document, fetching it if necessary.
    pub async fn get_discovery(&self) -> Result<OidcDiscovery, AuthError> {
        // Check cache first
        {
            let cache = self.discovery_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                // Cache for 1 hour
                if cached.fetched_at.elapsed() < Duration::from_secs(3600) {
                    return Ok(cached.discovery.clone());
                }
            }
        }

        // Fetch discovery document
        // Use discovery_url if set (for Docker networking), otherwise fall back to issuer
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            self.config.discovery_base_url().trim_end_matches('/')
        );

        tracing::debug!(url = %discovery_url, "Fetching OIDC discovery document");

        let response = self
            .http_client
            .get(&discovery_url)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, url = %discovery_url, "Failed to fetch OIDC discovery");
                AuthError::Internal(format!("Failed to fetch OIDC discovery: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            tracing::error!(status = %status, "OIDC discovery endpoint returned error");
            return Err(AuthError::Internal(format!(
                "OIDC discovery returned {}",
                status
            )));
        }

        let discovery: OidcDiscovery = response.json().await.map_err(|e| {
            tracing::error!(error = %e, "Failed to parse OIDC discovery");
            AuthError::Internal(format!("Failed to parse OIDC discovery: {}", e))
        })?;

        // Update cache
        {
            let mut cache = self.discovery_cache.write().await;
            *cache = Some(CachedDiscovery {
                discovery: discovery.clone(),
                fetched_at: Instant::now(),
            });
        }

        // Initialize JWT validator with JWKS URL from discovery
        {
            let mut validator = self.jwt_validator.write().await;
            if validator.is_none() {
                let jwt_config = crate::config::JwtAuthConfig {
                    issuer: discovery.issuer.clone(),
                    audience: crate::config::OneOrMany::One(self.config.client_id.clone()),
                    jwks_url: discovery.jwks_uri.clone(),
                    jwks_refresh_secs: 3600,
                    identity_claim: self.config.identity_claim.clone(),
                    org_claim: self.config.org_claim.clone(),
                    additional_claims: vec![],
                    allow_expired: false,
                    // OIDC providers typically use RS256 or ES256
                    allowed_algorithms: vec![
                        crate::config::JwtAlgorithm::RS256,
                        crate::config::JwtAlgorithm::RS384,
                        crate::config::JwtAlgorithm::RS512,
                        crate::config::JwtAlgorithm::ES256,
                        crate::config::JwtAlgorithm::ES384,
                    ],
                };
                *validator = Some(Arc::new(JwtValidator::with_client(
                    jwt_config,
                    self.http_client.clone(),
                )));
            }
        }

        Ok(discovery)
    }

    /// Generate an authorization URL for the OIDC flow.
    ///
    /// The `org_id` parameter is used for per-organization SSO. When set, the callback
    /// will use the org-specific authenticator from the registry instead of the global one.
    pub async fn authorization_url(
        &self,
        return_to: Option<String>,
    ) -> Result<(String, AuthorizationState), AuthError> {
        self.authorization_url_with_org(return_to, None).await
    }

    /// Generate an authorization URL for the OIDC flow with org context.
    ///
    /// The `org_id` parameter is used for per-organization SSO. When set, the callback
    /// will use the org-specific authenticator from the registry instead of the global one.
    pub async fn authorization_url_with_org(
        &self,
        return_to: Option<String>,
        org_id: Option<Uuid>,
    ) -> Result<(String, AuthorizationState), AuthError> {
        let discovery = self.get_discovery().await?;

        // Generate state, nonce, and PKCE challenge
        let state = Uuid::new_v4().to_string();
        let nonce = Uuid::new_v4().to_string();
        let pkce = PkceChallenge::new();

        // Build authorization URL
        let mut url = Url::parse(&discovery.authorization_endpoint).map_err(|e| {
            AuthError::Internal(format!("Invalid authorization endpoint URL: {}", e))
        })?;

        {
            let mut query = url.query_pairs_mut();
            query.append_pair("response_type", "code");
            query.append_pair("client_id", &self.config.client_id);
            query.append_pair("redirect_uri", &self.config.redirect_uri);
            query.append_pair("scope", &self.config.scopes.join(" "));
            query.append_pair("state", &state);
            query.append_pair("nonce", &nonce);
            query.append_pair("code_challenge", &pkce.code_challenge);
            query.append_pair("code_challenge_method", "S256");
        }

        let auth_state = AuthorizationState {
            state: state.clone(),
            nonce,
            code_verifier: pkce.code_verifier,
            return_to,
            org_id,
            created_at: Utc::now(),
        };

        // Store the state for later verification
        self.session_store
            .store_auth_state(auth_state.clone())
            .await
            .map_err(|e| AuthError::Internal(format!("Failed to store auth state: {}", e)))?;

        Ok((url.to_string(), auth_state))
    }

    /// Exchange an authorization code for tokens.
    ///
    /// Returns the session and the optional `return_to` URL from the original login request.
    ///
    /// If `device_info` is provided (for enhanced session management), it will be stored
    /// with the session for device tracking and session listing.
    pub async fn exchange_code(
        &self,
        code: &str,
        state: &str,
    ) -> Result<(OidcSession, Option<String>), AuthError> {
        self.exchange_code_with_device(code, state, None).await
    }

    /// Exchange an authorization code for tokens with optional device info.
    ///
    /// Returns the session and the optional `return_to` URL from the original login request.
    pub async fn exchange_code_with_device(
        &self,
        code: &str,
        state: &str,
        device_info: Option<DeviceInfo>,
    ) -> Result<(OidcSession, Option<String>), AuthError> {
        // Verify and retrieve the auth state
        let auth_state = self
            .session_store
            .take_auth_state(state)
            .await
            .map_err(|e| AuthError::Internal(format!("Failed to retrieve auth state: {}", e)))?
            .ok_or(AuthError::InvalidToken)?;

        // Check if state is too old (10 minute limit)
        let age = Utc::now() - auth_state.created_at;
        if age > chrono::Duration::minutes(10) {
            return Err(AuthError::ExpiredToken);
        }

        let discovery = self.get_discovery().await?;

        // Exchange code for tokens
        let token_response = self
            .http_client
            .post(&discovery.token_endpoint)
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &self.config.redirect_uri),
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
                ("code_verifier", &auth_state.code_verifier),
            ])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to exchange code for tokens");
                AuthError::Internal(format!("Token exchange failed: {}", e))
            })?;

        if !token_response.status().is_success() {
            let status = token_response.status();
            let body = token_response.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %body, "Token endpoint returned error");
            return Err(AuthError::Internal(format!(
                "Token exchange failed: {}",
                status
            )));
        }

        let tokens: TokenResponse = token_response.json().await.map_err(|e| {
            tracing::error!(error = %e, "Failed to parse token response");
            AuthError::Internal(format!("Failed to parse token response: {}", e))
        })?;

        // Validate ID token and extract claims
        let id_token = tokens.id_token.as_ref().ok_or_else(|| {
            tracing::error!("No ID token in response");
            AuthError::Internal("No ID token in response".to_string())
        })?;

        let validator = {
            let v = self.jwt_validator.read().await;
            v.clone()
                .ok_or_else(|| AuthError::Internal("JWT validator not initialized".to_string()))?
        };

        let claims = validator.validate(id_token).await?;

        // Validate nonce to prevent token substitution/replay attacks
        let token_nonce = claims
            .extra
            .get("nonce")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if token_nonce != auth_state.nonce {
            tracing::warn!("OIDC nonce mismatch: possible token substitution or replay attack");
            return Err(AuthError::InvalidToken);
        }

        // Create session
        let now = Utc::now();
        let session_duration = chrono::Duration::seconds(self.config.session.duration_secs as i64);
        let token_expires_at = tokens
            .expires_in
            .map(|secs| now + chrono::Duration::seconds(secs as i64));

        let external_id = validator.extract_identity(&claims);
        let org = validator.extract_org(&claims);

        let session = OidcSession {
            id: Uuid::new_v4(),
            external_id,
            email: claims.email.clone(),
            name: claims.name.clone(),
            org,
            groups: claims.groups.clone().unwrap_or_default(),
            roles: claims.roles.clone().unwrap_or_default(),
            access_token: Some(tokens.access_token),
            refresh_token: tokens.refresh_token,
            created_at: now,
            expires_at: now + session_duration,
            token_expires_at,
            sso_org_id: auth_state.org_id,
            session_index: None, // OIDC doesn't use session_index (SAML only)
            device: device_info,
            last_activity: Some(now),
        };

        // Store session
        self.session_store
            .create_session(session.clone())
            .await
            .map_err(|e| AuthError::Internal(format!("Failed to store session: {}", e)))?;

        // Enforce concurrent session limit (Phase 2)
        let enhanced = &self.config.session.enhanced;
        if enhanced.enabled
            && enhanced.max_concurrent_sessions > 0
            && let Err(e) = enforce_session_limit(
                self.session_store.as_ref(),
                &session.external_id,
                enhanced.max_concurrent_sessions,
            )
            .await
        {
            // Non-fatal: log but don't fail the login
            tracing::warn!(
                external_id = %session.external_id,
                error = %e,
                "Failed to enforce session limit"
            );
        }

        Ok((session, auth_state.return_to))
    }

    /// Get a session by ID.
    ///
    /// This method performs the following checks:
    /// 1. Verifies the session exists
    /// 2. Checks absolute expiration (`expires_at`)
    /// 3. Checks inactivity timeout (if enhanced sessions are enabled)
    /// 4. Updates `last_activity` timestamp (if enhanced sessions are enabled)
    pub async fn get_session(&self, session_id: Uuid) -> Result<OidcSession, AuthError> {
        validate_and_refresh_session(
            self.session_store.as_ref(),
            session_id,
            &self.config.session.enhanced,
        )
        .await
        .map_err(|e| match e {
            super::session_store::SessionError::NotFound => AuthError::SessionNotFound,
            super::session_store::SessionError::Expired => AuthError::SessionExpired,
            _ => AuthError::Internal(format!("Session error: {}", e)),
        })
    }

    /// Delete a session (logout).
    pub async fn logout(&self, session_id: Uuid) -> Result<Option<String>, AuthError> {
        let _ = self.session_store.delete_session(session_id).await;

        // Get logout URL if available
        let discovery = self.get_discovery().await?;
        Ok(discovery.end_session_endpoint)
    }

    /// Refresh tokens for a session.
    #[allow(dead_code)] // Auth infrastructure
    pub async fn refresh_tokens(&self, session_id: Uuid) -> Result<OidcSession, AuthError> {
        let mut session = self.get_session(session_id).await?;

        let refresh_token = session
            .refresh_token
            .as_ref()
            .ok_or_else(|| AuthError::Internal("No refresh token available".to_string()))?;

        let discovery = self.get_discovery().await?;

        let token_response = self
            .http_client
            .post(&discovery.token_endpoint)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
            ])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to refresh tokens");
                AuthError::Internal(format!("Token refresh failed: {}", e))
            })?;

        if !token_response.status().is_success() {
            let status = token_response.status();
            tracing::error!(status = %status, "Token refresh endpoint returned error");
            // Refresh failed - session is no longer valid
            let _ = self.session_store.delete_session(session_id).await;
            return Err(AuthError::SessionExpired);
        }

        let tokens: TokenResponse = token_response.json().await.map_err(|e| {
            tracing::error!(error = %e, "Failed to parse refresh token response");
            AuthError::Internal(format!("Failed to parse refresh response: {}", e))
        })?;

        // Update session
        let now = Utc::now();
        session.access_token = Some(tokens.access_token);
        if let Some(refresh) = tokens.refresh_token {
            session.refresh_token = Some(refresh);
        }
        session.token_expires_at = tokens
            .expires_in
            .map(|secs| now + chrono::Duration::seconds(secs as i64));

        self.session_store
            .update_session(session.clone())
            .await
            .map_err(|e| AuthError::Internal(format!("Failed to update session: {}", e)))?;

        Ok(session)
    }

    /// Get the session cookie name from config.
    #[allow(dead_code)] // Auth infrastructure
    pub fn cookie_name(&self) -> &str {
        &self.config.session.cookie_name
    }

    /// Get session configuration.
    #[allow(dead_code)] // Auth infrastructure
    pub fn session_config(&self) -> &crate::config::SessionConfig {
        &self.config.session
    }

    /// Get the identity claim name.
    #[allow(dead_code)] // Auth infrastructure
    pub fn identity_claim(&self) -> &str {
        &self.config.identity_claim
    }

    /// Get the org claim name (if any).
    #[allow(dead_code)] // Auth infrastructure
    pub fn org_claim(&self) -> Option<&str> {
        self.config.org_claim.as_deref()
    }

    /// Get the groups claim name (if any).
    #[allow(dead_code)] // Auth infrastructure
    pub fn groups_claim(&self) -> Option<&str> {
        self.config.groups_claim.as_deref()
    }
}

/// Fetch the JWKS URI from an OIDC discovery document.
///
/// This is a standalone function that can be used to get the JWKS URL without
/// needing a full OidcAuthenticator instance. It fetches the OIDC discovery
/// document and extracts the `jwks_uri` field.
///
/// # Arguments
/// * `discovery_url` - The base URL for OIDC discovery (typically the issuer URL).
///   The `/.well-known/openid-configuration` path will be appended if not present.
/// * `http_client` - An HTTP client to use for the request.
///
/// # Returns
/// The `jwks_uri` from the discovery document, or an error if the discovery
/// document could not be fetched or parsed.
pub async fn fetch_jwks_uri(
    discovery_url: &str,
    http_client: &reqwest::Client,
) -> Result<String, AuthError> {
    // Build the full discovery URL
    let url = if discovery_url.ends_with("/.well-known/openid-configuration") {
        discovery_url.to_string()
    } else {
        format!(
            "{}/.well-known/openid-configuration",
            discovery_url.trim_end_matches('/')
        )
    };

    tracing::debug!(url = %url, "Fetching OIDC discovery document for JWKS URI");

    let response = http_client.get(&url).send().await.map_err(|e| {
        tracing::error!(error = %e, url = %url, "Failed to fetch OIDC discovery");
        AuthError::Internal(format!("Failed to fetch OIDC discovery: {}", e))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        tracing::error!(status = %status, url = %url, "OIDC discovery endpoint returned error");
        return Err(AuthError::Internal(format!(
            "OIDC discovery returned {}",
            status
        )));
    }

    let discovery: OidcDiscovery = response.json().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to parse OIDC discovery");
        AuthError::Internal(format!("Failed to parse OIDC discovery: {}", e))
    })?;

    Ok(discovery.jwks_uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_challenge() {
        let pkce = PkceChallenge::new();

        // Verifier should be non-empty
        assert!(!pkce.code_verifier.is_empty());
        assert!(!pkce.code_challenge.is_empty());

        // Challenge should be different from verifier
        assert_ne!(pkce.code_verifier, pkce.code_challenge);

        // Verify the challenge is the SHA256 of the verifier
        let mut hasher = Sha256::new();
        hasher.update(pkce.code_verifier.as_bytes());
        let expected = URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert_eq!(pkce.code_challenge, expected);
    }

    // Session store tests are in session_store.rs
}
