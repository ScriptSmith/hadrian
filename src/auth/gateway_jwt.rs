//! Per-org gateway JWT validation registry.
//!
//! Routes incoming JWTs to the correct per-org validator based on the `iss` claim.
//! Validators are cached across requests so the JWKS cache is reused, fixing the
//! per-request `JwtValidator` creation that previously discarded the JWKS cache.

use std::{collections::HashMap, sync::Arc, time::Instant};

use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use super::jwt::JwtValidator;
#[cfg(any(feature = "sso", test))]
use crate::config::JwtAuthConfig;

/// How long to cache "no SSO config exists for this issuer" results.
/// Prevents repeated DB queries from JWTs with unknown/attacker-controlled issuers.
const NEGATIVE_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(60);

/// Maximum number of negative cache entries before eviction kicks in.
/// Prevents unbounded memory growth from attacker-controlled JWT issuers.
const MAX_NEGATIVE_CACHE_ENTRIES: usize = 10_000;

/// Internal state behind the single `RwLock`.
struct RegistryInner {
    /// org_id → Arc<JwtValidator> (validators persist JWKS cache across requests)
    validators: HashMap<Uuid, Arc<JwtValidator>>,
    /// issuer → Vec<org_id> index for fast token routing
    issuer_index: HashMap<String, Vec<Uuid>>,
    /// Issuers that had no matching SSO config in the DB, cached to avoid repeated queries.
    negative_cache: HashMap<String, Instant>,
}

/// Registry of per-org `JwtValidator`s, indexed by issuer for fast token routing.
pub struct GatewayJwtRegistry {
    inner: RwLock<RegistryInner>,
    /// Serializes lazy-load operations to prevent thundering herd on cache miss.
    /// Only held during DB query + OIDC discovery for unknown issuers.
    load_mutex: Mutex<()>,
}

impl GatewayJwtRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(RegistryInner {
                validators: HashMap::new(),
                issuer_index: HashMap::new(),
                negative_cache: HashMap::new(),
            }),
            load_mutex: Mutex::new(()),
        }
    }

    /// Register (or re-register) a validator built from an org SSO config.
    ///
    /// Fetches the JWKS URI from the OIDC discovery endpoint, builds a
    /// `JwtAuthConfig`, creates a `JwtValidator`, and stores it keyed by org_id.
    #[cfg(feature = "sso")]
    pub async fn register_from_sso_config(
        &self,
        config: &crate::models::OrgSsoConfig,
        http_client: &reqwest::Client,
        allow_loopback: bool,
    ) -> Result<(), super::AuthError> {
        use super::AuthError;

        let issuer = config
            .issuer
            .as_deref()
            .ok_or_else(|| AuthError::Internal("SSO config missing issuer".to_string()))?;
        let client_id = config
            .client_id
            .as_deref()
            .ok_or_else(|| AuthError::Internal("SSO config missing client_id".to_string()))?;

        // Determine the discovery URL
        let discovery_url = config.discovery_url.as_deref().unwrap_or(issuer);

        let jwks_url = super::fetch_jwks_uri(discovery_url, http_client, allow_loopback).await?;
        let jwt_config = build_jwt_config_from_sso(issuer, client_id, &jwks_url, config);
        let validator = Arc::new(JwtValidator::with_client(jwt_config, http_client.clone()));

        // Single write lock: remove old issuer index, insert validator, update index
        let mut inner = self.inner.write().await;
        remove_from_issuer_index(&mut inner, config.org_id);
        inner.validators.insert(config.org_id, validator);
        inner
            .issuer_index
            .entry(issuer.to_string())
            .or_default()
            .push(config.org_id);

        tracing::info!(
            org_id = %config.org_id,
            issuer = %issuer,
            "Registered gateway JWT validator for org"
        );

        Ok(())
    }

    /// Find all validators whose issuer matches the given string.
    pub async fn find_validators_by_issuer(&self, issuer: &str) -> Vec<(Uuid, Arc<JwtValidator>)> {
        let inner = self.inner.read().await;
        let Some(org_ids) = inner.issuer_index.get(issuer) else {
            return Vec::new();
        };
        org_ids
            .iter()
            .filter_map(|id| inner.validators.get(id).map(|v| (*id, v.clone())))
            .collect()
    }

    /// Remove a validator by org_id and clean up the issuer index.
    pub async fn remove(&self, org_id: Uuid) {
        let mut inner = self.inner.write().await;
        inner.validators.remove(&org_id);
        remove_from_issuer_index(&mut inner, org_id);
    }

    /// Find validators for an issuer, lazy-loading from the DB if needed.
    ///
    /// Deduplicates concurrent loads via `load_mutex` and caches negative results
    /// (unknown issuers) for [`NEGATIVE_CACHE_TTL`] to prevent DB query amplification.
    #[cfg(feature = "sso")]
    pub async fn find_or_load_by_issuer(
        &self,
        issuer: &str,
        db: &crate::db::DbPool,
        http_client: &reqwest::Client,
        allow_loopback: bool,
    ) -> Result<Vec<(Uuid, Arc<JwtValidator>)>, super::AuthError> {
        // Fast path: already cached
        let validators = self.find_validators_by_issuer(issuer).await;
        if !validators.is_empty() {
            return Ok(validators);
        }

        // Check negative cache (read lock only)
        {
            let inner = self.inner.read().await;
            if let Some(cached_at) = inner.negative_cache.get(issuer)
                && cached_at.elapsed() < NEGATIVE_CACHE_TTL
            {
                return Ok(Vec::new());
            }
        }

        // Serialize lazy-loads to prevent thundering herd.
        // The lock is held across DB query + OIDC discovery, but this only
        // triggers on cache miss (first request for an unknown issuer).
        let _guard = self.load_mutex.lock().await;

        // Re-check after acquiring lock — another request may have loaded it
        let validators = self.find_validators_by_issuer(issuer).await;
        if !validators.is_empty() {
            return Ok(validators);
        }

        // Also re-check negative cache (may have been populated while waiting)
        {
            let inner = self.inner.read().await;
            if let Some(cached_at) = inner.negative_cache.get(issuer)
                && cached_at.elapsed() < NEGATIVE_CACHE_TTL
            {
                return Ok(Vec::new());
            }
        }

        // Load from DB
        let configs = db
            .org_sso_configs()
            .find_enabled_oidc_by_issuer(issuer)
            .await
            .map_err(|e| super::AuthError::Internal(e.to_string()))?;

        if configs.is_empty() {
            // Cache negative result to avoid repeated DB queries
            let mut inner = self.inner.write().await;
            // Evict expired entries if at capacity
            if inner.negative_cache.len() >= MAX_NEGATIVE_CACHE_ENTRIES {
                inner
                    .negative_cache
                    .retain(|_, cached_at| cached_at.elapsed() < NEGATIVE_CACHE_TTL);
            }
            // If still at capacity after expiry cleanup, drop oldest half
            if inner.negative_cache.len() >= MAX_NEGATIVE_CACHE_ENTRIES {
                let mut entries: Vec<_> = inner.negative_cache.drain().collect();
                entries.sort_by_key(|(_, instant)| *instant);
                let half = entries.len() / 2;
                inner.negative_cache = entries.into_iter().skip(half).collect();
            }
            inner
                .negative_cache
                .insert(issuer.to_string(), Instant::now());
            return Ok(Vec::new());
        }

        for config in &configs {
            if let Err(e) = self
                .register_from_sso_config(config, http_client, allow_loopback)
                .await
            {
                tracing::warn!(
                    org_id = %config.org_id,
                    issuer = ?config.issuer,
                    error = %e,
                    "Failed to lazy-load gateway JWT validator for org"
                );
            }
        }

        Ok(self.find_validators_by_issuer(issuer).await)
    }

    /// Invalidate the negative cache for an issuer.
    ///
    /// Called when a new SSO config is created so that subsequent JWT requests
    /// for that issuer aren't blocked by a stale negative cache entry.
    pub async fn invalidate_negative_cache(&self, issuer: &str) {
        let mut inner = self.inner.write().await;
        inner.negative_cache.remove(issuer);
    }

    /// Number of registered validators.
    pub async fn len(&self) -> usize {
        self.inner.read().await.validators.len()
    }
}

/// Clean up issuer index entries for a given org_id. Operates on `&mut RegistryInner`
/// so callers can combine this with other mutations under a single write lock.
fn remove_from_issuer_index(inner: &mut RegistryInner, org_id: Uuid) {
    inner.issuer_index.retain(|_, ids| {
        ids.retain(|id| *id != org_id);
        !ids.is_empty()
    });
}

/// Build a `JwtAuthConfig` from per-org SSO config fields with secure defaults.
#[cfg(feature = "sso")]
fn build_jwt_config_from_sso(
    issuer: &str,
    client_id: &str,
    jwks_url: &str,
    config: &crate::models::OrgSsoConfig,
) -> JwtAuthConfig {
    use crate::config::{JwtAlgorithm, OneOrMany};

    JwtAuthConfig {
        issuer: issuer.to_string(),
        audience: OneOrMany::One(client_id.to_string()),
        jwks_url: jwks_url.to_string(),
        jwks_refresh_secs: 3600,
        identity_claim: config
            .identity_claim
            .clone()
            .unwrap_or_else(|| "sub".to_string()),
        org_claim: config.org_claim.clone(),
        additional_claims: Vec::new(),
        allow_expired: false,
        allowed_algorithms: vec![
            JwtAlgorithm::RS256,
            JwtAlgorithm::RS384,
            JwtAlgorithm::RS512,
            JwtAlgorithm::ES256,
            JwtAlgorithm::ES384,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{JwtAlgorithm, OneOrMany};

    #[tokio::test]
    async fn test_register_and_lookup() {
        let registry = GatewayJwtRegistry::new();
        assert_eq!(registry.len().await, 0);

        // Without a real OIDC server, we can only test the index bookkeeping.
        // Manually insert a validator to test index ops.
        let org_id = Uuid::new_v4();
        let issuer = "https://idp.acme.com";

        let config = JwtAuthConfig {
            issuer: issuer.to_string(),
            audience: OneOrMany::One("test".to_string()),
            jwks_url: "https://idp.acme.com/.well-known/jwks.json".to_string(),
            jwks_refresh_secs: 3600,
            identity_claim: "sub".to_string(),
            org_claim: None,
            additional_claims: vec![],
            allow_expired: false,
            allowed_algorithms: vec![JwtAlgorithm::RS256],
        };

        let validator = Arc::new(JwtValidator::new(config));
        {
            let mut inner = registry.inner.write().await;
            inner.validators.insert(org_id, validator);
            inner
                .issuer_index
                .entry(issuer.to_string())
                .or_default()
                .push(org_id);
        }

        assert_eq!(registry.len().await, 1);

        // Lookup by issuer should return the validator
        let found = registry.find_validators_by_issuer(issuer).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, org_id);

        // Lookup with unknown issuer returns empty
        let not_found = registry
            .find_validators_by_issuer("https://other.com")
            .await;
        assert!(not_found.is_empty());

        // Remove should clean up both maps
        registry.remove(org_id).await;
        assert_eq!(registry.len().await, 0);
        assert!(registry.find_validators_by_issuer(issuer).await.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_orgs_same_issuer() {
        let registry = GatewayJwtRegistry::new();
        let issuer = "https://shared-idp.example.com";

        let org1 = Uuid::new_v4();
        let org2 = Uuid::new_v4();

        let make_validator = || {
            Arc::new(JwtValidator::new(JwtAuthConfig {
                issuer: issuer.to_string(),
                audience: OneOrMany::One("test".to_string()),
                jwks_url: "https://shared-idp.example.com/jwks".to_string(),
                jwks_refresh_secs: 3600,
                identity_claim: "sub".to_string(),
                org_claim: None,
                additional_claims: vec![],
                allow_expired: false,
                allowed_algorithms: vec![JwtAlgorithm::RS256],
            }))
        };

        {
            let mut inner = registry.inner.write().await;
            inner.validators.insert(org1, make_validator());
            inner.validators.insert(org2, make_validator());
            inner
                .issuer_index
                .entry(issuer.to_string())
                .or_default()
                .extend([org1, org2]);
        }

        let found = registry.find_validators_by_issuer(issuer).await;
        assert_eq!(found.len(), 2);

        // Remove one org, the other should remain
        registry.remove(org1).await;
        let found = registry.find_validators_by_issuer(issuer).await;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, org2);
    }

    #[tokio::test]
    async fn test_negative_cache_invalidation() {
        let registry = GatewayJwtRegistry::new();
        let issuer = "https://unknown-idp.example.com";

        // Manually insert a negative cache entry
        {
            let mut inner = registry.inner.write().await;
            inner
                .negative_cache
                .insert(issuer.to_string(), Instant::now());
        }

        // Verify the entry is present
        {
            let inner = registry.inner.read().await;
            assert!(inner.negative_cache.contains_key(issuer));
        }

        // Invalidate and verify removal
        registry.invalidate_negative_cache(issuer).await;
        {
            let inner = registry.inner.read().await;
            assert!(!inner.negative_cache.contains_key(issuer));
        }
    }

    #[tokio::test]
    async fn test_empty_registry() {
        let registry = GatewayJwtRegistry::new();
        assert_eq!(registry.len().await, 0);
        assert!(
            registry
                .find_validators_by_issuer("https://any.example.com")
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn test_remove_nonexistent_org() {
        let registry = GatewayJwtRegistry::new();
        // Should be a no-op, not panic
        registry.remove(Uuid::new_v4()).await;
        assert_eq!(registry.len().await, 0);
    }

    #[cfg(feature = "sso")]
    #[test]
    fn test_build_jwt_config_from_sso() {
        use crate::models::OrgSsoConfig;

        let config = OrgSsoConfig {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            provider_type: crate::models::SsoProviderType::Oidc,
            issuer: Some("https://idp.acme.com".to_string()),
            discovery_url: None,
            client_id: Some("acme-client".to_string()),
            redirect_uri: None,
            scopes: vec!["openid".to_string()],
            identity_claim: Some("email".to_string()),
            org_claim: Some("org".to_string()),
            groups_claim: None,
            saml_metadata_url: None,
            saml_idp_entity_id: None,
            saml_idp_sso_url: None,
            saml_idp_slo_url: None,
            saml_idp_certificate: None,
            saml_sp_entity_id: None,
            saml_name_id_format: None,
            saml_sign_requests: false,
            saml_sp_certificate: None,
            saml_force_authn: false,
            saml_authn_context_class_ref: None,
            saml_identity_attribute: None,
            saml_email_attribute: None,
            saml_name_attribute: None,
            saml_groups_attribute: None,
            provisioning_enabled: true,
            create_users: true,
            default_team_id: None,
            default_org_role: "member".to_string(),
            default_team_role: "member".to_string(),
            allowed_email_domains: vec![],
            sync_attributes_on_login: false,
            sync_memberships_on_login: true,
            enforcement_mode: crate::models::SsoEnforcementMode::Optional,
            enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let jwt = build_jwt_config_from_sso(
            "https://idp.acme.com",
            "acme-client",
            "https://idp.acme.com/jwks",
            &config,
        );

        assert_eq!(jwt.issuer, "https://idp.acme.com");
        assert_eq!(jwt.identity_claim, "email");
        assert_eq!(jwt.org_claim, Some("org".to_string()));
        assert!(!jwt.allow_expired);
        assert_eq!(jwt.allowed_algorithms.len(), 5);
    }
}
