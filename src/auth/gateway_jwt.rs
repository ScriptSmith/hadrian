//! Per-org gateway JWT validation registry.
//!
//! Routes incoming JWTs to the correct per-org validator based on the `iss` claim.
//! Validators are cached across requests so the JWKS cache is reused, fixing the
//! per-request `JwtValidator` creation that previously discarded the JWKS cache.

use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    sync::Arc,
    time::Instant,
};

#[cfg(feature = "sso")]
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::jwt::JwtValidator;
#[cfg(any(feature = "sso", test))]
use crate::config::JwtAuthConfig;

/// How long to cache "no SSO config exists for this issuer" results.
/// Prevents repeated DB queries from JWTs with unknown/attacker-controlled issuers.
#[cfg(feature = "sso")]
const NEGATIVE_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(60);

/// Maximum number of negative cache entries before eviction kicks in.
/// Prevents unbounded memory growth from attacker-controlled JWT issuers.
#[cfg(feature = "sso")]
const MAX_NEGATIVE_CACHE_ENTRIES: usize = 1_000;

/// Per-IP rate limit on JWT lazy-loads (cache misses that hit the DB / OIDC discovery).
/// An attacker rotating issuer strings per request would otherwise bypass the negative
/// cache and amplify each request into a DB query + JWKS fetch.
#[cfg(feature = "sso")]
const LAZY_LOAD_RATE_LIMIT: u32 = 30;
#[cfg(feature = "sso")]
const LAZY_LOAD_RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Per-IP rate limit context for `find_or_load_by_issuer`. Optional — when omitted,
/// no rate limit is applied (used by tests and call sites where the IP is unknown).
#[cfg(feature = "sso")]
pub struct LazyLoadRateLimit<'a> {
    pub cache: &'a Arc<dyn crate::cache::Cache>,
    pub ip: IpAddr,
}

/// Internal state behind the single `RwLock`.
struct RegistryInner {
    /// org_id → Arc<JwtValidator> (validators persist JWKS cache across requests)
    validators: HashMap<Uuid, Arc<JwtValidator>>,
    /// issuer → Vec<org_id> index for fast token routing
    issuer_index: HashMap<String, Vec<Uuid>>,
    /// Issuers that had no matching SSO config in the DB, cached to avoid repeated queries.
    /// `negative_cache` is the lookup map; `negative_cache_order` maintains insertion order
    /// for O(1) LRU eviction (we never refresh on read so FIFO == LRU here).
    negative_cache: HashMap<String, Instant>,
    negative_cache_order: VecDeque<String>,
}

/// Registry of per-org `JwtValidator`s, indexed by issuer for fast token routing.
pub struct GatewayJwtRegistry {
    inner: RwLock<RegistryInner>,
    /// Serializes lazy-load operations to prevent thundering herd on cache miss.
    /// Only held during DB query + OIDC discovery for unknown issuers.
    #[cfg(feature = "sso")]
    load_mutex: Mutex<()>,
}

impl Default for GatewayJwtRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayJwtRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(RegistryInner {
                validators: HashMap::new(),
                issuer_index: HashMap::new(),
                negative_cache: HashMap::new(),
                negative_cache_order: VecDeque::new(),
            }),
            #[cfg(feature = "sso")]
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
        allow_private: bool,
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

        let jwks_url =
            super::fetch_jwks_uri(discovery_url, http_client, allow_loopback, allow_private)
                .await?;
        let jwt_config = build_jwt_config_from_sso(issuer, client_id, &jwks_url, config);
        let validator = Arc::new(JwtValidator::with_client(jwt_config, http_client.clone())?);

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
    /// When `rate_limit` is provided, lazy-loads are additionally rate-limited
    /// per-IP so attackers rotating issuer strings can't bypass the negative cache.
    #[cfg(feature = "sso")]
    pub async fn find_or_load_by_issuer(
        &self,
        issuer: &str,
        db: &crate::db::DbPool,
        http_client: &reqwest::Client,
        allow_loopback: bool,
        allow_private: bool,
        rate_limit: Option<LazyLoadRateLimit<'_>>,
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

        // Per-IP rate limit on cache miss (before DB / load_mutex contention).
        // Failure to talk to the cache must not block legitimate logins, so an
        // error from the cache is logged and treated as "allow".
        if let Some(rl) = &rate_limit {
            let key = format!("gw:jwt:lazy_load:{}", rl.ip);
            match rl
                .cache
                .check_and_incr_rate_limit(
                    &key,
                    LAZY_LOAD_RATE_LIMIT,
                    LAZY_LOAD_RATE_LIMIT_WINDOW_SECS,
                )
                .await
            {
                Ok(result) if !result.allowed => {
                    tracing::warn!(
                        ip = %rl.ip,
                        issuer = %issuer,
                        limit = LAZY_LOAD_RATE_LIMIT,
                        "JWT lazy-load rate limit exceeded; treating issuer as unknown"
                    );
                    return Ok(Vec::new());
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::debug!(
                        ip = %rl.ip,
                        error = %e,
                        "JWT lazy-load rate limit cache call failed; allowing"
                    );
                }
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
            insert_negative_entry(&mut inner, issuer);
            return Ok(Vec::new());
        }

        for config in &configs {
            if let Err(e) = self
                .register_from_sso_config(config, http_client, allow_loopback, allow_private)
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
        if inner.negative_cache.remove(issuer).is_some() {
            inner.negative_cache_order.retain(|k| k != issuer);
        }
    }

    /// Number of registered validators.
    pub async fn len(&self) -> usize {
        self.inner.read().await.validators.len()
    }

    /// Whether the registry has no validators.
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.validators.is_empty()
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

/// Insert a negative-cache entry, evicting the LRU entry first if at capacity.
/// Re-inserting an existing issuer refreshes both its timestamp and its LRU position
/// so an issuer queried in a tight loop doesn't churn through eviction.
#[cfg(feature = "sso")]
fn insert_negative_entry(inner: &mut RegistryInner, issuer: &str) {
    if inner.negative_cache.contains_key(issuer) {
        inner.negative_cache_order.retain(|k| k != issuer);
    } else if inner.negative_cache.len() >= MAX_NEGATIVE_CACHE_ENTRIES {
        // Evict expired entries from the front of the order until we drop one
        // that is still live, or until we're back under capacity.
        while let Some(oldest) = inner.negative_cache_order.front() {
            let oldest = oldest.clone();
            inner.negative_cache_order.pop_front();
            let was_present = inner.negative_cache.remove(&oldest).is_some();
            if was_present && inner.negative_cache.len() < MAX_NEGATIVE_CACHE_ENTRIES {
                break;
            }
        }
    }
    inner
        .negative_cache
        .insert(issuer.to_string(), Instant::now());
    inner.negative_cache_order.push_back(issuer.to_string());
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

        let validator = Arc::new(JwtValidator::new(config).unwrap());
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
            Arc::new(
                JwtValidator::new(JwtAuthConfig {
                    issuer: issuer.to_string(),
                    audience: OneOrMany::One("test".to_string()),
                    jwks_url: "https://shared-idp.example.com/jwks".to_string(),
                    jwks_refresh_secs: 3600,
                    identity_claim: "sub".to_string(),
                    org_claim: None,
                    additional_claims: vec![],
                    allow_expired: false,
                    allowed_algorithms: vec![JwtAlgorithm::RS256],
                })
                .unwrap(),
            )
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

        // Insert via the helper so the order index stays in sync
        {
            let mut inner = registry.inner.write().await;
            insert_negative_entry(&mut inner, issuer);
        }

        // Verify the entry is present in both maps
        {
            let inner = registry.inner.read().await;
            assert!(inner.negative_cache.contains_key(issuer));
            assert!(inner.negative_cache_order.iter().any(|k| k == issuer));
        }

        // Invalidate and verify removal from both
        registry.invalidate_negative_cache(issuer).await;
        {
            let inner = registry.inner.read().await;
            assert!(!inner.negative_cache.contains_key(issuer));
            assert!(!inner.negative_cache_order.iter().any(|k| k == issuer));
        }
    }

    #[cfg(feature = "sso")]
    #[tokio::test]
    async fn test_negative_cache_lru_eviction() {
        let registry = GatewayJwtRegistry::new();

        // Fill past capacity; the oldest issuer should get evicted.
        {
            let mut inner = registry.inner.write().await;
            for i in 0..MAX_NEGATIVE_CACHE_ENTRIES {
                insert_negative_entry(&mut inner, &format!("https://idp{i}.example.com"));
            }
            assert_eq!(inner.negative_cache.len(), MAX_NEGATIVE_CACHE_ENTRIES);
            assert!(
                inner
                    .negative_cache
                    .contains_key("https://idp0.example.com")
            );

            insert_negative_entry(&mut inner, "https://overflow.example.com");
            assert_eq!(inner.negative_cache.len(), MAX_NEGATIVE_CACHE_ENTRIES);
            // Oldest gone, newest present.
            assert!(
                !inner
                    .negative_cache
                    .contains_key("https://idp0.example.com")
            );
            assert!(
                inner
                    .negative_cache
                    .contains_key("https://overflow.example.com")
            );
            // Order list still bounded.
            assert_eq!(inner.negative_cache_order.len(), MAX_NEGATIVE_CACHE_ENTRIES);
        }
    }

    #[cfg(feature = "sso")]
    #[tokio::test]
    async fn test_negative_cache_reinsert_refreshes_position() {
        let registry = GatewayJwtRegistry::new();

        {
            let mut inner = registry.inner.write().await;
            insert_negative_entry(&mut inner, "https://a.example.com");
            insert_negative_entry(&mut inner, "https://b.example.com");
            // Re-insert "a"; it should move to the back of the eviction queue.
            insert_negative_entry(&mut inner, "https://a.example.com");
            assert_eq!(inner.negative_cache.len(), 2);
            // "b" is now the oldest in the order index.
            assert_eq!(
                inner.negative_cache_order.front().map(String::as_str),
                Some("https://b.example.com")
            );
            assert_eq!(
                inner.negative_cache_order.back().map(String::as_str),
                Some("https://a.example.com")
            );
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
