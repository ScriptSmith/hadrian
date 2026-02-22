//! JWT validation with JWKS support.
//!
//! This module provides JWT validation against a JWKS (JSON Web Key Set) endpoint.
//! It supports automatic key rotation and caching.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use jsonwebtoken::{
    Algorithm, DecodingKey, TokenData, Validation, decode, decode_header,
    jwk::{AlgorithmParameters, Jwk, JwkSet},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::AuthError;
use crate::config::JwtAuthConfig;

/// Claims extracted from a validated JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (identity ID)
    pub sub: String,

    /// Issuer
    pub iss: String,

    /// Audience (can be string or array)
    #[serde(default)]
    pub aud: Audience,

    /// Expiration time (Unix timestamp)
    pub exp: u64,

    /// Issued at (Unix timestamp)
    #[serde(default)]
    pub iat: u64,

    /// Not before (Unix timestamp)
    #[serde(default)]
    pub nbf: u64,

    /// Email claim (common in OIDC)
    #[serde(default)]
    pub email: Option<String>,

    /// Name claim (common in OIDC)
    #[serde(default)]
    pub name: Option<String>,

    /// Organization claim (custom)
    #[serde(default)]
    pub org: Option<String>,

    /// Groups claim (e.g., Keycloak group paths like "/cs/faculty")
    #[serde(default)]
    pub groups: Option<Vec<String>>,

    /// Roles claim (e.g., Keycloak realm roles like "super_admin", "user")
    #[serde(default)]
    pub roles: Option<Vec<String>>,

    /// All other claims (for custom extraction)
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Audience can be a single string or an array of strings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Audience {
    #[default]
    None,
    Single(String),
    Multiple(Vec<String>),
}

impl Audience {
    #[cfg(test)]
    pub fn contains(&self, aud: &str) -> bool {
        match self {
            Audience::None => false,
            Audience::Single(s) => s == aud,
            Audience::Multiple(v) => v.iter().any(|s| s == aud),
        }
    }

    #[cfg(test)]
    pub fn as_vec(&self) -> Vec<String> {
        match self {
            Audience::None => vec![],
            Audience::Single(s) => vec![s.clone()],
            Audience::Multiple(v) => v.clone(),
        }
    }
}

/// Cached JWKS with expiration.
struct CachedJwks {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Instant,
}

/// JWT validator that fetches and caches JWKS.
pub struct JwtValidator {
    config: JwtAuthConfig,
    http_client: reqwest::Client,
    jwks_cache: RwLock<Option<CachedJwks>>,
}

impl JwtValidator {
    /// Create a new JWT validator.
    #[allow(dead_code)] // Auth infrastructure
    pub fn new(config: JwtAuthConfig) -> Self {
        Self {
            config,
            http_client: reqwest::Client::new(),
            jwks_cache: RwLock::new(None),
        }
    }

    /// Create a new JWT validator with a custom HTTP client.
    pub fn with_client(config: JwtAuthConfig, http_client: reqwest::Client) -> Self {
        Self {
            config,
            http_client,
            jwks_cache: RwLock::new(None),
        }
    }

    /// Validate a JWT and return the claims.
    pub async fn validate(&self, token: &str) -> Result<JwtClaims, AuthError> {
        // Decode header to get the key ID and algorithm
        let header = decode_header(token).map_err(|e| {
            tracing::debug!(error = %e, "Failed to decode JWT header");
            AuthError::InvalidToken
        })?;

        // SECURITY: Validate algorithm against allowlist to prevent algorithm confusion attacks.
        // An attacker could try to:
        // 1. Use "none" algorithm to bypass signature verification
        // 2. Use HS256 with an RSA public key as the HMAC secret
        // 3. Downgrade to weaker algorithms
        if !self.is_algorithm_allowed(header.alg) {
            tracing::warn!(
                algorithm = ?header.alg,
                allowed = ?self.allowed_algorithms(),
                "JWT algorithm not in allowlist"
            );
            return Err(AuthError::InvalidToken);
        }

        let kid = header.kid.as_ref().ok_or_else(|| {
            tracing::debug!("JWT missing key ID (kid)");
            AuthError::InvalidToken
        })?;

        // Get the decoding key from JWKS
        let decoding_key = self.get_decoding_key(kid).await?;

        // Build validation rules with explicit algorithm from allowlist
        // (we already validated header.alg is in the allowlist above)
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&self.config.audience.to_vec());

        if self.config.allow_expired {
            validation.validate_exp = false;
        }

        // Decode and validate the token
        let token_data: TokenData<JwtClaims> =
            decode(token, &decoding_key, &validation).map_err(|e| {
                tracing::debug!(error = %e, "JWT validation failed");
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::ExpiredToken,
                    jsonwebtoken::errors::ErrorKind::InvalidAudience => AuthError::InvalidToken,
                    jsonwebtoken::errors::ErrorKind::InvalidIssuer => AuthError::InvalidToken,
                    _ => AuthError::InvalidToken,
                }
            })?;

        Ok(token_data.claims)
    }

    /// Check if an algorithm is in the allowlist.
    fn is_algorithm_allowed(&self, alg: Algorithm) -> bool {
        self.config
            .allowed_algorithms
            .iter()
            .any(|allowed| allowed.matches(alg))
    }

    /// Get the list of allowed algorithms (for logging).
    fn allowed_algorithms(&self) -> Vec<Algorithm> {
        self.config
            .allowed_algorithms
            .iter()
            .map(|a| a.to_jwt_algorithm())
            .collect()
    }

    /// Get a decoding key for the given key ID, fetching JWKS if necessary.
    async fn get_decoding_key(&self, kid: &str) -> Result<DecodingKey, AuthError> {
        // Check cache first
        {
            let cache = self.jwks_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                let refresh_duration = Duration::from_secs(self.config.jwks_refresh_secs);
                if cached.fetched_at.elapsed() < refresh_duration
                    && let Some(key) = cached.keys.get(kid)
                {
                    return Ok(key.clone());
                }
            }
        }

        // Cache miss or expired - fetch new JWKS
        self.refresh_jwks().await?;

        // Try again with fresh cache
        let cache = self.jwks_cache.read().await;
        cache
            .as_ref()
            .and_then(|c| c.keys.get(kid).cloned())
            .ok_or_else(|| {
                tracing::warn!(kid = kid, "Key ID not found in JWKS");
                AuthError::InvalidToken
            })
    }

    /// Fetch and cache the JWKS from the configured URL.
    async fn refresh_jwks(&self) -> Result<(), AuthError> {
        tracing::debug!(url = %self.config.jwks_url, "Fetching JWKS");

        let response = self
            .http_client
            .get(&self.config.jwks_url)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, url = %self.config.jwks_url, "Failed to fetch JWKS");
                AuthError::Internal(format!("Failed to fetch JWKS: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            tracing::error!(status = %status, url = %self.config.jwks_url, "JWKS endpoint returned error");
            return Err(AuthError::Internal(format!(
                "JWKS endpoint returned {}",
                status
            )));
        }

        let jwks: JwkSet = response.json().await.map_err(|e| {
            tracing::error!(error = %e, "Failed to parse JWKS response");
            AuthError::Internal(format!("Failed to parse JWKS: {}", e))
        })?;

        // Convert JWKs to DecodingKeys
        let mut keys = HashMap::new();
        for jwk in jwks.keys {
            if let Some(kid) = &jwk.common.key_id {
                match jwk_to_decoding_key(&jwk) {
                    Ok(key) => {
                        keys.insert(kid.clone(), key);
                    }
                    Err(e) => {
                        tracing::warn!(kid = kid, error = %e, "Failed to convert JWK to decoding key");
                    }
                }
            }
        }

        tracing::info!(keys_count = keys.len(), "JWKS refreshed");

        // Update cache
        let mut cache = self.jwks_cache.write().await;
        *cache = Some(CachedJwks {
            keys,
            fetched_at: Instant::now(),
        });

        Ok(())
    }

    /// Get the identity claim name from config.
    #[allow(dead_code)] // Auth infrastructure
    pub fn identity_claim(&self) -> &str {
        &self.config.identity_claim
    }

    /// Get the org claim name from config (if any).
    #[allow(dead_code)] // Auth infrastructure
    pub fn org_claim(&self) -> Option<&str> {
        self.config.org_claim.as_deref()
    }

    /// Get the list of additional claims to extract.
    #[allow(dead_code)] // Auth infrastructure
    pub fn additional_claims(&self) -> &[String] {
        &self.config.additional_claims
    }

    /// Extract the identity from claims based on config.
    pub fn extract_identity(&self, claims: &JwtClaims) -> String {
        // If identity_claim is "sub", use claims.sub directly
        if self.config.identity_claim == "sub" {
            return claims.sub.clone();
        }

        // Otherwise, look in extra claims
        claims
            .extra
            .get(&self.config.identity_claim)
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| claims.sub.clone())
    }

    /// Extract the organization from claims based on config.
    #[cfg(feature = "sso")]
    pub fn extract_org(&self, claims: &JwtClaims) -> Option<String> {
        let org_claim = self.config.org_claim.as_ref()?;

        // Check the org field first
        if org_claim == "org"
            && let Some(ref org) = claims.org
        {
            return Some(org.clone());
        }

        // Otherwise, look in extra claims
        claims
            .extra
            .get(org_claim)
            .and_then(|v| v.as_str())
            .map(String::from)
    }
}

/// Convert a JWK to a DecodingKey.
fn jwk_to_decoding_key(jwk: &Jwk) -> Result<DecodingKey, AuthError> {
    match &jwk.algorithm {
        AlgorithmParameters::RSA(rsa) => DecodingKey::from_rsa_components(&rsa.n, &rsa.e)
            .map_err(|e| AuthError::Internal(format!("Failed to create RSA decoding key: {}", e))),
        AlgorithmParameters::EllipticCurve(ec) => {
            use jsonwebtoken::jwk::KeyAlgorithm;

            match jwk.common.key_algorithm {
                // ES256 and ES384 are supported EC algorithms; None defaults to ES256
                Some(KeyAlgorithm::ES256) | Some(KeyAlgorithm::ES384) | None => {
                    DecodingKey::from_ec_components(&ec.x, &ec.y).map_err(|e| {
                        AuthError::Internal(format!("Failed to create EC decoding key: {}", e))
                    })
                }
                Some(alg) => Err(AuthError::Internal(format!(
                    "Unsupported EC algorithm: {alg:?}"
                ))),
            }
        }
        AlgorithmParameters::OctetKey(oct) => DecodingKey::from_base64_secret(&oct.value)
            .map_err(|e| AuthError::Internal(format!("Failed to create HMAC decoding key: {}", e))),
        _ => Err(AuthError::Internal(
            "Unsupported JWK algorithm type".to_string(),
        )),
    }
}

/// Shared JWT validator that can be used across requests.
#[allow(dead_code)] // Auth infrastructure
pub type SharedJwtValidator = Arc<JwtValidator>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{JwtAlgorithm, OneOrMany};

    #[test]
    fn test_audience_contains() {
        let single = Audience::Single("api".to_string());
        assert!(single.contains("api"));
        assert!(!single.contains("other"));

        let multiple = Audience::Multiple(vec!["api".to_string(), "web".to_string()]);
        assert!(multiple.contains("api"));
        assert!(multiple.contains("web"));
        assert!(!multiple.contains("other"));

        let none = Audience::None;
        assert!(!none.contains("api"));
    }

    #[test]
    fn test_audience_as_vec() {
        let single = Audience::Single("api".to_string());
        assert_eq!(single.as_vec(), vec!["api"]);

        let multiple = Audience::Multiple(vec!["api".to_string(), "web".to_string()]);
        assert_eq!(multiple.as_vec(), vec!["api", "web"]);

        let none = Audience::None;
        assert!(none.as_vec().is_empty());
    }

    fn test_config() -> JwtAuthConfig {
        JwtAuthConfig {
            issuer: "https://example.com".to_string(),
            audience: OneOrMany::One("test".to_string()),
            jwks_url: "https://example.com/.well-known/jwks.json".to_string(),
            jwks_refresh_secs: 3600,
            identity_claim: "sub".to_string(),
            org_claim: None,
            additional_claims: vec![],
            allow_expired: false,
            allowed_algorithms: vec![JwtAlgorithm::RS256, JwtAlgorithm::ES256],
        }
    }

    #[test]
    fn test_algorithm_allowlist_rs256_allowed() {
        let config = test_config();
        let validator = JwtValidator::new(config);

        assert!(validator.is_algorithm_allowed(Algorithm::RS256));
    }

    #[test]
    fn test_algorithm_allowlist_es256_allowed() {
        let config = test_config();
        let validator = JwtValidator::new(config);

        assert!(validator.is_algorithm_allowed(Algorithm::ES256));
    }

    #[test]
    fn test_algorithm_allowlist_hs256_rejected() {
        let config = test_config();
        let validator = JwtValidator::new(config);

        // HS256 is not in the allowed list
        assert!(!validator.is_algorithm_allowed(Algorithm::HS256));
    }

    #[test]
    fn test_algorithm_allowlist_rs384_rejected() {
        let config = test_config();
        let validator = JwtValidator::new(config);

        // RS384 is not in the allowed list (only RS256 and ES256)
        assert!(!validator.is_algorithm_allowed(Algorithm::RS384));
    }

    #[test]
    fn test_algorithm_allowlist_with_hs256() {
        // Test that HS256 works when explicitly allowed
        let config = JwtAuthConfig {
            allowed_algorithms: vec![JwtAlgorithm::HS256],
            ..test_config()
        };
        let validator = JwtValidator::new(config);

        assert!(validator.is_algorithm_allowed(Algorithm::HS256));
        assert!(!validator.is_algorithm_allowed(Algorithm::RS256));
    }

    #[test]
    fn test_algorithm_allowlist_all_rsa() {
        let config = JwtAuthConfig {
            allowed_algorithms: vec![
                JwtAlgorithm::RS256,
                JwtAlgorithm::RS384,
                JwtAlgorithm::RS512,
            ],
            ..test_config()
        };
        let validator = JwtValidator::new(config);

        assert!(validator.is_algorithm_allowed(Algorithm::RS256));
        assert!(validator.is_algorithm_allowed(Algorithm::RS384));
        assert!(validator.is_algorithm_allowed(Algorithm::RS512));
        assert!(!validator.is_algorithm_allowed(Algorithm::ES256));
        assert!(!validator.is_algorithm_allowed(Algorithm::HS256));
    }

    #[test]
    fn test_algorithm_allowlist_empty_rejects_all() {
        let config = JwtAuthConfig {
            allowed_algorithms: vec![],
            ..test_config()
        };
        let validator = JwtValidator::new(config);

        assert!(!validator.is_algorithm_allowed(Algorithm::RS256));
        assert!(!validator.is_algorithm_allowed(Algorithm::ES256));
        assert!(!validator.is_algorithm_allowed(Algorithm::HS256));
    }

    #[test]
    fn test_jwt_algorithm_matches() {
        assert!(JwtAlgorithm::RS256.matches(Algorithm::RS256));
        assert!(!JwtAlgorithm::RS256.matches(Algorithm::RS384));
        assert!(!JwtAlgorithm::RS256.matches(Algorithm::ES256));

        assert!(JwtAlgorithm::ES256.matches(Algorithm::ES256));
        assert!(JwtAlgorithm::HS256.matches(Algorithm::HS256));
    }

    #[test]
    fn test_jwt_algorithm_to_jwt_algorithm() {
        assert_eq!(JwtAlgorithm::RS256.to_jwt_algorithm(), Algorithm::RS256);
        assert_eq!(JwtAlgorithm::ES256.to_jwt_algorithm(), Algorithm::ES256);
        assert_eq!(JwtAlgorithm::HS256.to_jwt_algorithm(), Algorithm::HS256);
        assert_eq!(JwtAlgorithm::EdDSA.to_jwt_algorithm(), Algorithm::EdDSA);
    }

    #[test]
    fn test_allowed_algorithms_returns_correct_list() {
        let config = JwtAuthConfig {
            allowed_algorithms: vec![JwtAlgorithm::RS256, JwtAlgorithm::ES256],
            ..test_config()
        };
        let validator = JwtValidator::new(config);

        let allowed = validator.allowed_algorithms();
        assert_eq!(allowed.len(), 2);
        assert!(allowed.contains(&Algorithm::RS256));
        assert!(allowed.contains(&Algorithm::ES256));
    }
}
