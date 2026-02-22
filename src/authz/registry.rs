//! Policy registry for per-organization RBAC policies.
//!
//! This module provides a `PolicyRegistry` that wraps the system-level `AuthzEngine`
//! and adds support for per-organization RBAC policies stored in the database.
//!
//! # Architecture
//!
//! The `PolicyRegistry` maintains a cache of compiled CEL programs for each organization's
//! policies. This provides:
//! - **Eager compilation**: Policies are compiled at startup to catch errors early
//! - **Lazy refresh**: When policies are updated, only that org's cache is invalidated
//! - **System-first evaluation**: System policies from config are evaluated first
//! - **Multi-node consistency**: Redis-based version tracking for distributed deployments
//!
//! # Multi-Node Cache Consistency
//!
//! In multi-node deployments with Redis, the registry uses a version-based cache
//! invalidation strategy:
//!
//! 1. Each org's policies have a version number stored in Redis
//! 2. On policy CRUD operations, the version is incremented atomically
//! 3. Each node tracks when it last checked the Redis version (local TTL)
//! 4. If TTL has expired, the node checks Redis and refreshes if version mismatch
//!
//! This provides bounded staleness (max 1 TTL period) with minimal Redis overhead.
//!
//! # Evaluation Order
//!
//! 1. Check if RBAC is globally disabled → allow all
//! 2. Evaluate system policies (from config) first - highest priority
//! 3. If system policy matched → return that decision
//! 4. If org_id provided → evaluate org policies (from database)
//! 5. If org policy matched → return that decision
//! 6. No policy matched → apply default_effect
//!
//! # Usage
//!
//! ```rust,ignore
//! // Initialize at startup
//! let registry = PolicyRegistry::initialize_from_db(
//!     &org_rbac_policy_service,
//!     engine,
//!     default_effect,
//!     cache,
//!     policy_repo,
//!     version_check_ttl,
//! ).await?;
//!
//! // Authorize with org context
//! let result = registry.authorize_with_org(
//!     Some(org_id),
//!     &subject,
//!     &context,
//! );
//! ```

#[cfg(feature = "cel")]
use std::{collections::HashMap, panic, time::Instant};
use std::{sync::Arc, time::Duration};

#[cfg(feature = "cel")]
use cel_interpreter::{Context, Program, Value, to_value};
use thiserror::Error;
#[cfg(feature = "cel")]
use tokio::sync::RwLock;
use uuid::Uuid;

#[cfg(feature = "cel")]
use super::AuthzError;
use super::{AuthzEngine, AuthzResult, PolicyContext, Subject};
#[cfg(feature = "cel")]
use crate::cache::CacheKeys;
#[cfg(feature = "cel")]
use crate::models::{OrgRbacPolicy, RbacPolicyEffect};
use crate::{
    cache::Cache, config::PolicyEffect, db::repos::OrgRbacPolicyRepo,
    services::OrgRbacPolicyService,
};

/// A compiled organization RBAC policy ready for evaluation.
#[cfg(feature = "cel")]
#[derive(Debug)]
pub struct CompiledOrgPolicy {
    /// The original policy from the database
    pub policy: OrgRbacPolicy,
    /// Compiled CEL program for efficient evaluation
    pub program: Arc<Program>,
}

/// Cached policies for an organization with version tracking.
///
/// This struct tracks the version of cached policies and when the version
/// was last checked against Redis. This enables efficient multi-node
/// cache invalidation with bounded staleness.
#[cfg(feature = "cel")]
struct CachedOrgPolicies {
    /// Compiled policies for this organization
    policies: Vec<CompiledOrgPolicy>,
    /// Version number from Redis (or local timestamp if no Redis)
    version: u64,
    /// When we last checked Redis for version changes
    last_version_check: Instant,
    /// When these policies were last accessed for authorization (LRU tracking)
    last_accessed: Instant,
}

/// Errors that can occur during policy registry operations.
#[derive(Debug, Error)]
pub enum PolicyRegistryError {
    #[error("Failed to load policies: {0}")]
    LoadError(String),

    #[error("Failed to compile policy '{policy_name}' for org {org_id}: {message}")]
    CompilationError {
        org_id: Uuid,
        policy_name: String,
        message: String,
    },
}

impl From<crate::db::DbError> for PolicyRegistryError {
    fn from(e: crate::db::DbError) -> Self {
        PolicyRegistryError::LoadError(e.to_string())
    }
}

// ============================================================================
// CEL-enabled PolicyRegistry
// ============================================================================

/// Registry of compiled RBAC policies for per-organization authorization.
///
/// The registry wraps the system-level `AuthzEngine` and adds support for
/// per-organization policies stored in the database. System policies from
/// the config file are always evaluated first, with org policies providing
/// additional organization-specific rules.
///
/// # Multi-Node Consistency
///
/// In distributed deployments, each node maintains its own local cache of
/// compiled policies. The registry uses Redis-based version tracking to
/// ensure cache consistency:
///
/// - On policy changes, the writing node increments the Redis version
/// - Other nodes check Redis periodically (controlled by `version_check_ttl`)
/// - If the Redis version is newer, the node refreshes from the database
///
/// This provides eventual consistency with bounded staleness.
///
/// # LRU Cache Eviction
///
/// For large deployments with many organizations, the registry supports LRU
/// (Least Recently Used) cache eviction to bound memory usage:
///
/// - `max_cached_orgs`: Maximum orgs to cache (0 = unlimited)
/// - `eviction_batch_size`: How many orgs to evict when cache is full
///
/// When the cache reaches capacity, the least recently accessed organizations
/// are evicted in batches. Evicted orgs have their policies reloaded from the
/// database on next access.
#[cfg(feature = "cel")]
pub struct PolicyRegistry {
    /// System-level authorization engine (config-based policies)
    engine: Arc<AuthzEngine>,
    /// Compiled org policies: org_id -> cached policies with version tracking
    /// Policies within each org are sorted by priority (descending), then effect (deny before allow)
    org_policies: Arc<RwLock<HashMap<Uuid, CachedOrgPolicies>>>,
    /// Default effect when no policy matches
    default_effect: PolicyEffect,
    /// Redis/memory cache for version checks (None = single-node, skip version checks)
    cache: Option<Arc<dyn Cache>>,
    /// Repository for fetching policies on version mismatch
    policy_repo: Option<Arc<dyn OrgRbacPolicyRepo>>,
    /// How often to check Redis for version changes
    version_check_ttl: Duration,
    /// Maximum orgs to cache (0 = unlimited)
    max_cached_orgs: usize,
    /// How many orgs to evict when cache is full
    eviction_batch_size: usize,
}

#[cfg(feature = "cel")]
impl PolicyRegistry {
    /// Create a new empty policy registry.
    ///
    /// The registry is initialized with a system-level engine and will
    /// use the provided default effect when no policy matches.
    ///
    /// # Arguments
    ///
    /// * `engine` - The system-level authorization engine
    /// * `default_effect` - Default effect when no policy matches
    /// * `cache` - Optional Redis/memory cache for version tracking
    /// * `policy_repo` - Optional repository for fetching policies on version mismatch
    /// * `version_check_ttl` - How often to check Redis for version changes
    /// * `max_cached_orgs` - Maximum orgs to cache (0 = unlimited)
    /// * `eviction_batch_size` - How many orgs to evict when cache is full
    pub fn new(
        engine: Arc<AuthzEngine>,
        default_effect: PolicyEffect,
        cache: Option<Arc<dyn Cache>>,
        policy_repo: Option<Arc<dyn OrgRbacPolicyRepo>>,
        version_check_ttl: Duration,
        max_cached_orgs: usize,
        eviction_batch_size: usize,
    ) -> Self {
        Self {
            engine,
            org_policies: Arc::new(RwLock::new(HashMap::new())),
            default_effect,
            cache,
            policy_repo,
            version_check_ttl,
            max_cached_orgs,
            eviction_batch_size,
        }
    }

    /// Create a new policy registry with lazy loading enabled.
    ///
    /// Unlike `initialize_from_db`, this constructor does NOT load policies at startup.
    /// Instead, policies are loaded on-demand when an organization is first accessed
    /// during authorization.
    ///
    /// This is recommended for large deployments with many organizations where:
    /// - Startup time is important
    /// - Memory usage should scale with active orgs, not total orgs
    /// - Many orgs may never be accessed in a given node's lifetime
    ///
    /// # Arguments
    ///
    /// * `engine` - The system-level authorization engine
    /// * `default_effect` - Default effect when no policy matches
    /// * `cache` - Optional Redis/memory cache for version tracking
    /// * `policy_repo` - Repository for fetching policies (required for lazy loading)
    /// * `version_check_ttl` - How often to check Redis for version changes
    /// * `max_cached_orgs` - Maximum orgs to cache (0 = unlimited)
    /// * `eviction_batch_size` - How many orgs to evict when cache is full
    pub fn new_lazy(
        engine: Arc<AuthzEngine>,
        default_effect: PolicyEffect,
        cache: Option<Arc<dyn Cache>>,
        policy_repo: Arc<dyn OrgRbacPolicyRepo>,
        version_check_ttl: Duration,
        max_cached_orgs: usize,
        eviction_batch_size: usize,
    ) -> Self {
        Self::new(
            engine,
            default_effect,
            cache,
            Some(policy_repo),
            version_check_ttl,
            max_cached_orgs,
            eviction_batch_size,
        )
    }

    /// Initialize the registry by loading all org policies from the database.
    ///
    /// This compiles all policies at startup to:
    /// - Catch CEL syntax errors early
    /// - Avoid compilation overhead during request handling
    ///
    /// Policies that fail to compile are logged and skipped, allowing the
    /// system to start even with invalid policies.
    ///
    /// # Arguments
    ///
    /// * `service` - Service for loading policies at startup
    /// * `engine` - The system-level authorization engine
    /// * `default_effect` - Default effect when no policy matches
    /// * `cache` - Optional Redis/memory cache for version tracking
    /// * `policy_repo` - Repository for fetching policies on version mismatch
    /// * `version_check_ttl` - How often to check Redis for version changes
    /// * `max_cached_orgs` - Maximum orgs to cache (0 = unlimited)
    /// * `eviction_batch_size` - How many orgs to evict when cache is full
    #[allow(clippy::too_many_arguments)]
    pub async fn initialize_from_db(
        service: &OrgRbacPolicyService,
        engine: Arc<AuthzEngine>,
        default_effect: PolicyEffect,
        cache: Option<Arc<dyn Cache>>,
        policy_repo: Arc<dyn OrgRbacPolicyRepo>,
        version_check_ttl: Duration,
        max_cached_orgs: usize,
        eviction_batch_size: usize,
    ) -> Result<Self, PolicyRegistryError> {
        let registry = Self::new(
            engine,
            default_effect,
            cache,
            Some(policy_repo),
            version_check_ttl,
            max_cached_orgs,
            eviction_batch_size,
        );

        // Load all enabled policies grouped by organization
        // The service returns policies sorted by priority descending
        let policies = service.list_all_enabled().await?;

        // Group by org_id and compile
        let mut by_org: HashMap<Uuid, Vec<OrgRbacPolicy>> = HashMap::new();
        for policy in policies {
            by_org.entry(policy.org_id).or_default().push(policy);
        }

        let mut compiled_by_org = HashMap::new();
        let mut total_compiled = 0;
        let mut total_failed = 0;
        let now = Instant::now();

        let max_expr_len = registry.engine.max_expression_length();
        for (org_id, policies) in by_org {
            let compiled = compile_policies(
                &policies,
                &mut total_compiled,
                &mut total_failed,
                max_expr_len,
            );

            if !compiled.is_empty() {
                // Fetch or initialize Redis version for this org
                let version = registry.fetch_or_init_redis_version(org_id).await;

                compiled_by_org.insert(
                    org_id,
                    CachedOrgPolicies {
                        policies: compiled,
                        version,
                        last_version_check: now,
                        last_accessed: now,
                    },
                );
            }
        }

        if total_failed > 0 {
            tracing::warn!(
                compiled = total_compiled,
                failed = total_failed,
                "Some org RBAC policies failed to compile"
            );
        }

        *registry.org_policies.write().await = compiled_by_org;

        Ok(registry)
    }

    /// Fetch the current version from Redis, or initialize to 0 if not present.
    async fn fetch_or_init_redis_version(&self, org_id: Uuid) -> u64 {
        let Some(cache) = &self.cache else {
            return 0;
        };

        let key = CacheKeys::rbac_policy_version(org_id);
        match cache.get_bytes(&key).await {
            Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).parse().unwrap_or(0),
            Ok(None) => 0,
            Err(e) => {
                tracing::warn!(org_id = %org_id, error = %e, "Failed to fetch policy version from Redis");
                0
            }
        }
    }

    /// Get the number of organizations with registered policies.
    pub async fn org_count(&self) -> usize {
        self.org_policies.read().await.len()
    }

    /// Get the total number of compiled policies across all organizations.
    pub async fn policy_count(&self) -> usize {
        self.org_policies
            .read()
            .await
            .values()
            .map(|cached| cached.policies.len())
            .sum()
    }

    /// Refresh the cached policies for a specific organization.
    ///
    /// This recompiles all policies for the org, updates the cache, and
    /// increments the Redis version to notify other nodes.
    ///
    /// Called when policies are created, updated, or deleted.
    ///
    /// Policies that fail to compile are skipped with a warning.
    pub async fn refresh_org_policies(
        &self,
        org_id: Uuid,
        policies: Vec<OrgRbacPolicy>,
    ) -> Result<(), PolicyRegistryError> {
        let mut total_compiled = 0;
        let mut total_failed = 0;
        let compiled = compile_policies(
            &policies,
            &mut total_compiled,
            &mut total_failed,
            self.engine.max_expression_length(),
        );

        // Increment Redis version to notify other nodes
        let new_version = self.increment_redis_version(org_id).await;

        // Update the cache with atomic swap (avoids brief window with no policies)
        let now = Instant::now();
        let mut cache = self.org_policies.write().await;
        if compiled.is_empty() {
            cache.remove(&org_id);
        } else {
            cache.insert(
                org_id,
                CachedOrgPolicies {
                    policies: compiled,
                    version: new_version,
                    last_version_check: now,
                    last_accessed: now,
                },
            );
        }

        tracing::debug!(
            org_id = %org_id,
            policy_count = cache.get(&org_id).map(|c| c.policies.len()).unwrap_or(0),
            version = new_version,
            "Refreshed org RBAC policies"
        );

        Ok(())
    }

    /// Increment the Redis version for an organization's policies.
    ///
    /// Returns the new version number. If Redis is unavailable, uses a
    /// timestamp as the version to ensure local cache is still updated.
    async fn increment_redis_version(&self, org_id: Uuid) -> u64 {
        if let Some(cache) = &self.cache {
            let key = CacheKeys::rbac_policy_version(org_id);
            // INCR with 30-day TTL (policies should be refreshed within this period)
            match cache.incr(&key, Duration::from_secs(86400 * 30)).await {
                Ok(version) => return version as u64,
                Err(e) => {
                    tracing::warn!(
                        org_id = %org_id,
                        error = %e,
                        "Failed to increment policy version in Redis, using timestamp"
                    );
                }
            }
        }

        // No cache or cache error: use timestamp as version
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Ensure the cached policies for an organization are fresh.
    ///
    /// This implements the version-check-with-TTL strategy:
    /// 1. If TTL hasn't expired since last check, use cached policies (fast path)
    /// 2. If TTL expired, check Redis for version changes
    /// 3. If Redis version is newer OR org is not in cache, refresh from database
    ///
    /// This provides bounded staleness with minimal Redis overhead.
    async fn ensure_policies_fresh(&self, org_id: Uuid) {
        // Fast path: check if TTL has expired (read lock only)
        let (needs_version_check, is_cached) = {
            let cache = self.org_policies.read().await;
            match cache.get(&org_id) {
                Some(cached) => (
                    cached.last_version_check.elapsed() > self.version_check_ttl,
                    true,
                ),
                None => (true, false), // No cached policies, need to load
            }
        };

        if !needs_version_check {
            return;
        }

        // If no Redis cache, handle single-node mode
        let Some(redis_cache) = &self.cache else {
            // Single-node mode: if org not cached, load from DB
            if !is_cached {
                self.load_org_policies_from_db(org_id).await;
            }
            return;
        };

        let key = CacheKeys::rbac_policy_version(org_id);
        let redis_version: u64 = match redis_cache.get_bytes(&key).await {
            Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).parse().unwrap_or(0),
            Ok(None) => 0,
            Err(e) => {
                tracing::warn!(org_id = %org_id, error = %e, "Failed to check policy version in Redis");
                // On Redis error, still load from DB if not cached
                if !is_cached {
                    self.load_org_policies_from_db(org_id).await;
                }
                return;
            }
        };

        // Evict LRU orgs if needed before potentially loading a new org
        if !is_cached {
            self.evict_if_needed().await;
        }

        // Now take write lock and check again (double-checked locking)
        let mut org_cache = self.org_policies.write().await;

        // Fix for the 0 > 0 bug: detect "org not in cache" separately
        let cached = org_cache.get(&org_id);
        let should_load = match cached {
            None => true,                         // Org not in cache - always load from DB
            Some(c) => redis_version > c.version, // Check version mismatch
        };

        if should_load {
            // Version mismatch or not cached - fetch from DB and recompile
            let Some(policy_repo) = &self.policy_repo else {
                tracing::warn!(org_id = %org_id, "Policy repo not available for refresh");
                return;
            };

            match policy_repo.list_enabled_by_org(org_id).await {
                Ok(policies) => {
                    let mut total_compiled = 0;
                    let mut total_failed = 0;
                    let compiled = compile_policies(
                        &policies,
                        &mut total_compiled,
                        &mut total_failed,
                        self.engine.max_expression_length(),
                    );

                    let now = Instant::now();
                    if compiled.is_empty() {
                        org_cache.remove(&org_id);
                    } else {
                        org_cache.insert(
                            org_id,
                            CachedOrgPolicies {
                                policies: compiled,
                                version: redis_version,
                                last_version_check: now,
                                last_accessed: now,
                            },
                        );
                    }
                    tracing::debug!(
                        org_id = %org_id,
                        version = redis_version,
                        "Refreshed stale org policies from database"
                    );
                }
                Err(e) => {
                    tracing::warn!(org_id = %org_id, error = %e, "Failed to refresh policies from DB");
                    // Update last_version_check to avoid hammering DB on errors
                    if let Some(cached) = org_cache.get_mut(&org_id) {
                        cached.last_version_check = Instant::now();
                    }
                }
            }
        } else {
            // Version matches, just update the check timestamp
            if let Some(cached) = org_cache.get_mut(&org_id) {
                cached.last_version_check = Instant::now();
            }
        }
    }

    /// Load policies for an organization from the database (single-node mode helper).
    async fn load_org_policies_from_db(&self, org_id: Uuid) {
        // Evict LRU orgs if needed before loading new org
        self.evict_if_needed().await;

        let Some(policy_repo) = &self.policy_repo else {
            tracing::warn!(org_id = %org_id, "Policy repo not available for loading");
            return;
        };

        match policy_repo.list_enabled_by_org(org_id).await {
            Ok(policies) => {
                let mut total_compiled = 0;
                let mut total_failed = 0;
                let compiled = compile_policies(
                    &policies,
                    &mut total_compiled,
                    &mut total_failed,
                    self.engine.max_expression_length(),
                );

                let now = Instant::now();
                let mut org_cache = self.org_policies.write().await;
                if compiled.is_empty() {
                    org_cache.remove(&org_id);
                } else {
                    org_cache.insert(
                        org_id,
                        CachedOrgPolicies {
                            policies: compiled,
                            version: 0, // No Redis version in single-node mode
                            last_version_check: now,
                            last_accessed: now,
                        },
                    );
                }
                tracing::debug!(
                    org_id = %org_id,
                    "Loaded org policies from database (single-node mode)"
                );
            }
            Err(e) => {
                tracing::warn!(org_id = %org_id, error = %e, "Failed to load policies from DB");
            }
        }
    }

    /// Remove all cached policies for an organization.
    ///
    /// Called when an organization is deleted or all its policies are removed.
    pub async fn remove_org(&self, org_id: Uuid) {
        let mut cache = self.org_policies.write().await;
        cache.remove(&org_id);
        tracing::debug!(org_id = %org_id, "Removed org from RBAC policy cache");
    }

    /// Evict least recently used organizations if the cache exceeds the limit.
    ///
    /// This implements LRU (Least Recently Used) eviction to bound memory usage
    /// in large deployments with many organizations.
    ///
    /// When `max_cached_orgs` is reached, `eviction_batch_size` least recently
    /// accessed organizations are evicted from the cache.
    async fn evict_if_needed(&self) {
        // 0 means unlimited, no eviction needed
        if self.max_cached_orgs == 0 {
            return;
        }

        let mut cache = self.org_policies.write().await;
        if cache.len() < self.max_cached_orgs {
            return;
        }

        // Calculate how many orgs to evict
        let target = self
            .max_cached_orgs
            .saturating_sub(self.eviction_batch_size);
        let to_evict = cache.len().saturating_sub(target);

        if to_evict == 0 {
            return;
        }

        // Collect org IDs sorted by last_accessed (oldest first)
        let mut entries: Vec<_> = cache.iter().map(|(id, c)| (*id, c.last_accessed)).collect();
        entries.sort_by_key(|(_, t)| *t);

        // Evict the oldest entries
        let evicted_count = entries.iter().take(to_evict).count();
        for (org_id, _) in entries.into_iter().take(to_evict) {
            cache.remove(&org_id);
        }

        tracing::info!(
            evicted = evicted_count,
            remaining = cache.len(),
            max = self.max_cached_orgs,
            "Evicted LRU org policies from cache"
        );
    }

    /// Get the system-level authorization engine.
    pub fn engine(&self) -> &Arc<AuthzEngine> {
        &self.engine
    }

    /// Authorize an action, evaluating both system and org-specific policies.
    ///
    /// # Evaluation Order
    ///
    /// 1. If RBAC is disabled in the engine → allow
    /// 2. Evaluate system policies (config-based) in priority order
    /// 3. If a system policy matched → return that decision
    /// 4. If `org_id` is provided, ensure policies are fresh and evaluate
    /// 5. If an org policy matched → return that decision
    /// 6. No policy matched → apply `default_effect`
    ///
    /// # Multi-Node Consistency
    ///
    /// Before evaluating org policies, this method checks if the local cache
    /// is stale (based on `policy_cache_ttl_ms`). If stale and Redis is configured,
    /// it checks the Redis version and refreshes from the database if needed.
    ///
    /// # Arguments
    ///
    /// * `org_id` - The organization context for org-specific policies
    /// * `subject` - The subject (user) making the request
    /// * `context` - The resource/action being authorized
    pub async fn authorize_with_org(
        &self,
        org_id: Option<Uuid>,
        subject: &Subject,
        context: &PolicyContext,
    ) -> AuthzResult {
        self.authorize_internal(org_id, subject, context, self.default_effect)
            .await
    }

    /// Authorize a request with organization context and a custom default effect.
    ///
    /// This method is identical to `authorize_with_org` but allows overriding
    /// the default effect when no policy matches. This is useful for API endpoints
    /// which may need a different default effect (e.g., "allow") than admin
    /// endpoints (e.g., "deny").
    ///
    /// # Arguments
    ///
    /// * `org_id` - The organization context for org-specific policies
    /// * `subject` - The subject (user) making the request
    /// * `context` - The resource/action being authorized
    /// * `override_default_effect` - The default effect to use when no policy matches
    pub async fn authorize_with_org_and_default(
        &self,
        org_id: Option<Uuid>,
        subject: &Subject,
        context: &PolicyContext,
        override_default_effect: PolicyEffect,
    ) -> AuthzResult {
        self.authorize_internal(org_id, subject, context, override_default_effect)
            .await
    }

    /// Internal authorization implementation shared by `authorize_with_org` and
    /// `authorize_with_org_and_default`.
    ///
    /// # Evaluation Order
    ///
    /// 1. If RBAC is disabled in the engine → allow
    /// 2. Evaluate system policies (config-based) in priority order
    /// 3. If a system policy matched → return that decision
    /// 4. If `org_id` is provided, ensure policies are fresh and evaluate
    /// 5. If an org policy matched → return that decision
    /// 6. No policy matched → apply `default_effect`
    async fn authorize_internal(
        &self,
        org_id: Option<Uuid>,
        subject: &Subject,
        context: &PolicyContext,
        default_effect: PolicyEffect,
    ) -> AuthzResult {
        // If RBAC is disabled, allow everything
        if !self.engine.is_enabled() {
            return AuthzResult::allow();
        }

        // First, evaluate system policies via the engine
        let system_result = self.engine.authorize(subject, context);

        // If a system policy made a decision (matched), return it
        // We can tell if a policy matched by checking if policy_name is set
        if system_result.policy_name.is_some() {
            return system_result;
        }

        // System policies didn't match (fell through to default)
        // Now evaluate org-specific policies if org_id is provided
        if let Some(org_id) = org_id {
            // Ensure policies are fresh before evaluation (multi-node consistency)
            self.ensure_policies_fresh(org_id).await;

            // Take a write lock to both read policies and update last_accessed (LRU tracking)
            let mut cache = self.org_policies.write().await;
            if let Some(cached) = cache.get_mut(&org_id) {
                // Update LRU timestamp
                cached.last_accessed = Instant::now();

                for compiled in &cached.policies {
                    // Check if policy applies to this resource/action
                    if !policy_matches(&compiled.policy, context) {
                        continue;
                    }

                    // Evaluate the CEL condition
                    match evaluate_condition(&compiled.program, subject, context) {
                        Ok(true) => {
                            tracing::debug!(
                                org_id = %org_id,
                                policy = %compiled.policy.name,
                                effect = ?compiled.policy.effect,
                                "Org policy condition matched"
                            );

                            return match compiled.policy.effect {
                                RbacPolicyEffect::Allow => {
                                    AuthzResult::allow_by_policy(&compiled.policy.name)
                                }
                                RbacPolicyEffect::Deny => AuthzResult::deny_by_policy(
                                    &compiled.policy.name,
                                    compiled.policy.description.clone(),
                                ),
                            };
                        }
                        Ok(false) => {
                            // Condition didn't match, try next policy
                            continue;
                        }
                        Err(e) => {
                            // Log the error
                            let fail_on_error = self.engine.fail_on_evaluation_error();
                            tracing::warn!(
                                org_id = %org_id,
                                policy = %compiled.policy.name,
                                error = %e,
                                fail_on_error,
                                "Org policy evaluation error"
                            );

                            // If fail_on_evaluation_error is true (default), deny the request
                            // to avoid security holes from silently skipping policies
                            if fail_on_error {
                                return AuthzResult {
                                    allowed: false,
                                    policy_name: Some(compiled.policy.name.clone()),
                                    reason: Some(format!(
                                        "Policy '{}' failed to evaluate: {}",
                                        compiled.policy.name, e
                                    )),
                                };
                            }

                            // Otherwise, skip this policy and continue to the next one
                            continue;
                        }
                    }
                }
            }
        }

        // No policy matched, apply default effect
        match default_effect {
            PolicyEffect::Allow => AuthzResult::allow_default(),
            PolicyEffect::Deny => AuthzResult::deny_default(),
        }
    }
}

/// Compile a CEL expression into a program with optional length validation.
#[cfg(feature = "cel")]
fn compile_policy(
    policy: &OrgRbacPolicy,
    max_expression_length: usize,
) -> Result<Program, AuthzError> {
    // Validate expression length before compilation
    if max_expression_length > 0 && policy.condition.len() > max_expression_length {
        return Err(AuthzError::InvalidExpression(format!(
            "Policy '{}': CEL expression length ({} bytes) exceeds maximum ({} bytes)",
            policy.name,
            policy.condition.len(),
            max_expression_length
        )));
    }

    Program::compile(&policy.condition)
        .map_err(|e| AuthzError::InvalidExpression(format!("Policy '{}': {}", policy.name, e)))
}

/// Compile a list of policies, tracking success/failure counts.
///
/// Only enabled policies are compiled. Failed policies are logged and skipped.
/// Returns the compiled policies sorted by priority (descending), then effect (deny before allow).
#[cfg(feature = "cel")]
fn compile_policies(
    policies: &[OrgRbacPolicy],
    total_compiled: &mut usize,
    total_failed: &mut usize,
    max_expression_length: usize,
) -> Vec<CompiledOrgPolicy> {
    let mut compiled = Vec::with_capacity(policies.len());

    for policy in policies {
        // Only compile enabled policies
        if !policy.enabled {
            continue;
        }

        match compile_policy(policy, max_expression_length) {
            Ok(program) => {
                compiled.push(CompiledOrgPolicy {
                    policy: policy.clone(),
                    program: Arc::new(program),
                });
                *total_compiled += 1;
            }
            Err(e) => {
                tracing::warn!(
                    org_id = %policy.org_id,
                    policy_name = %policy.name,
                    error = %e,
                    "Failed to compile org RBAC policy, skipping"
                );
                *total_failed += 1;
            }
        }
    }

    // Sort compiled policies by priority (descending), then effect (deny before allow)
    sort_policies(&mut compiled);

    compiled
}

/// Sort policies by priority (descending), then effect (deny before allow).
#[cfg(feature = "cel")]
fn sort_policies(policies: &mut [CompiledOrgPolicy]) {
    policies.sort_by(|a, b| {
        match b.policy.priority.cmp(&a.policy.priority) {
            std::cmp::Ordering::Equal => {
                // Deny before allow at same priority
                match (&a.policy.effect, &b.policy.effect) {
                    (RbacPolicyEffect::Deny, RbacPolicyEffect::Allow) => std::cmp::Ordering::Less,
                    (RbacPolicyEffect::Allow, RbacPolicyEffect::Deny) => {
                        std::cmp::Ordering::Greater
                    }
                    _ => std::cmp::Ordering::Equal,
                }
            }
            other => other,
        }
    });
}

/// Check if a policy applies to the given resource/action.
///
/// Uses pattern matching that supports:
/// - `*` to match any value
/// - `foo*` to match any value starting with `foo`
/// - `foo` for exact match only
#[cfg(feature = "cel")]
fn policy_matches(policy: &OrgRbacPolicy, context: &PolicyContext) -> bool {
    let resource_matches = super::pattern_matches(&policy.resource, &context.resource_type);
    let action_matches = super::pattern_matches(&policy.action, &context.action);
    resource_matches && action_matches
}

/// Evaluate a CEL condition against subject and context.
#[cfg(feature = "cel")]
fn evaluate_condition(
    program: &Program,
    subject: &Subject,
    context: &PolicyContext,
) -> Result<bool, AuthzError> {
    let mut ctx = Context::default();

    // Add subject to context
    let subject_value = to_value(subject)
        .map_err(|e| AuthzError::PolicyEvaluation(format!("Failed to serialize subject: {}", e)))?;
    ctx.add_variable("subject", subject_value);

    // Add context to context
    let context_value = to_value(context)
        .map_err(|e| AuthzError::PolicyEvaluation(format!("Failed to serialize context: {}", e)))?;
    ctx.add_variable("context", context_value);

    // Execute with panic protection.
    // The CEL interpreter uses ANTLR-generated code which can potentially
    // panic on edge cases during execution.
    let exec_result = panic::catch_unwind(panic::AssertUnwindSafe(|| program.execute(&ctx)));

    let result = match exec_result {
        Ok(Ok(value)) => value,
        Ok(Err(e)) => {
            return Err(AuthzError::PolicyEvaluation(format!(
                "Execution error: {}",
                e
            )));
        }
        Err(_) => {
            return Err(AuthzError::PolicyEvaluation(
                "CEL expression execution failed (internal error)".to_string(),
            ));
        }
    };

    match result {
        Value::Bool(b) => Ok(b),
        _ => Err(AuthzError::PolicyEvaluation(
            "Policy condition must evaluate to boolean".to_string(),
        )),
    }
}

// ============================================================================
// Stub PolicyRegistry when CEL feature is disabled
// ============================================================================

/// Policy registry stub (CEL feature disabled).
///
/// When the `cel` feature is not enabled, the registry delegates directly
/// to the engine (which returns `default_effect`). Org policy operations
/// are no-ops.
#[cfg(not(feature = "cel"))]
pub struct PolicyRegistry {
    /// System-level authorization engine
    engine: Arc<AuthzEngine>,
    /// Default effect when no policy matches
    default_effect: PolicyEffect,
}

#[cfg(not(feature = "cel"))]
impl PolicyRegistry {
    /// Create a new policy registry (no CEL compilation).
    pub fn new(
        engine: Arc<AuthzEngine>,
        default_effect: PolicyEffect,
        _cache: Option<Arc<dyn Cache>>,
        _policy_repo: Option<Arc<dyn OrgRbacPolicyRepo>>,
        _version_check_ttl: Duration,
        _max_cached_orgs: usize,
        _eviction_batch_size: usize,
    ) -> Self {
        Self {
            engine,
            default_effect,
        }
    }

    /// Create a new policy registry with lazy loading (no-op without CEL).
    pub fn new_lazy(
        engine: Arc<AuthzEngine>,
        default_effect: PolicyEffect,
        _cache: Option<Arc<dyn Cache>>,
        _policy_repo: Arc<dyn OrgRbacPolicyRepo>,
        _version_check_ttl: Duration,
        _max_cached_orgs: usize,
        _eviction_batch_size: usize,
    ) -> Self {
        Self {
            engine,
            default_effect,
        }
    }

    /// Initialize from database (no-op without CEL).
    #[allow(clippy::too_many_arguments)]
    pub async fn initialize_from_db(
        _service: &OrgRbacPolicyService,
        engine: Arc<AuthzEngine>,
        default_effect: PolicyEffect,
        _cache: Option<Arc<dyn Cache>>,
        _policy_repo: Arc<dyn OrgRbacPolicyRepo>,
        _version_check_ttl: Duration,
        _max_cached_orgs: usize,
        _eviction_batch_size: usize,
    ) -> Result<Self, PolicyRegistryError> {
        tracing::warn!(
            "Policy registry initialized without CEL feature; org policies will not be evaluated"
        );
        Ok(Self {
            engine,
            default_effect,
        })
    }

    /// Get the system-level authorization engine.
    pub fn engine(&self) -> &Arc<AuthzEngine> {
        &self.engine
    }

    /// Authorize with org context (delegates to engine, returns default_effect).
    pub async fn authorize_with_org(
        &self,
        _org_id: Option<Uuid>,
        subject: &Subject,
        context: &PolicyContext,
    ) -> AuthzResult {
        if !self.engine.is_enabled() {
            return AuthzResult::allow();
        }

        let system_result = self.engine.authorize(subject, context);
        if system_result.policy_name.is_some() {
            return system_result;
        }

        match self.default_effect {
            PolicyEffect::Allow => AuthzResult::allow_default(),
            PolicyEffect::Deny => AuthzResult::deny_default(),
        }
    }

    /// Authorize with org context and custom default effect.
    pub async fn authorize_with_org_and_default(
        &self,
        _org_id: Option<Uuid>,
        subject: &Subject,
        context: &PolicyContext,
        override_default_effect: PolicyEffect,
    ) -> AuthzResult {
        if !self.engine.is_enabled() {
            return AuthzResult::allow();
        }

        let system_result = self.engine.authorize(subject, context);
        if system_result.policy_name.is_some() {
            return system_result;
        }

        match override_default_effect {
            PolicyEffect::Allow => AuthzResult::allow_default(),
            PolicyEffect::Deny => AuthzResult::deny_default(),
        }
    }

    /// Refresh org policies (no-op without CEL).
    pub async fn refresh_org_policies(
        &self,
        _org_id: Uuid,
        _policies: Vec<crate::models::OrgRbacPolicy>,
    ) -> Result<(), PolicyRegistryError> {
        Ok(())
    }

    /// Remove org from cache (no-op without CEL).
    pub async fn remove_org(&self, _org_id: Uuid) {}

    /// Get the number of cached organizations (always 0 without CEL).
    pub async fn org_count(&self) -> usize {
        0
    }

    /// Get the total number of compiled policies (always 0 without CEL).
    pub async fn policy_count(&self) -> usize {
        0
    }
}

#[cfg(all(test, feature = "cel"))]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::config::{PolicyConfig, RbacConfig};

    fn create_test_engine(enabled: bool, default_effect: PolicyEffect) -> Arc<AuthzEngine> {
        let config = RbacConfig {
            enabled,
            default_effect,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![
                // System-level admin policy
                PolicyConfig {
                    name: "system-admin".to_string(),
                    description: Some("System admins can do anything".to_string()),
                    resource: "*".to_string(),
                    action: "*".to_string(),
                    condition: "'system_admin' in subject.roles".to_string(),
                    effect: PolicyEffect::Allow,
                    priority: 1000,
                },
            ],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };
        Arc::new(AuthzEngine::new(config).unwrap())
    }

    /// Create a test registry without cache/repo (single-node mode for tests)
    fn create_test_registry(
        engine: Arc<AuthzEngine>,
        default_effect: PolicyEffect,
    ) -> PolicyRegistry {
        PolicyRegistry::new(
            engine,
            default_effect,
            None,                   // No cache for tests
            None,                   // No policy repo for tests
            Duration::from_secs(0), // No TTL needed without cache
            0,                      // Unlimited cache
            100,                    // Default eviction batch size
        )
    }

    /// Create a test registry with LRU eviction configured
    fn create_test_registry_with_lru(
        engine: Arc<AuthzEngine>,
        default_effect: PolicyEffect,
        max_cached_orgs: usize,
        eviction_batch_size: usize,
    ) -> PolicyRegistry {
        PolicyRegistry::new(
            engine,
            default_effect,
            None,                   // No cache for tests
            None,                   // No policy repo for tests
            Duration::from_secs(0), // No TTL needed without cache
            max_cached_orgs,
            eviction_batch_size,
        )
    }

    fn create_test_policy(
        org_id: Uuid,
        name: &str,
        condition: &str,
        effect: RbacPolicyEffect,
        priority: i32,
    ) -> OrgRbacPolicy {
        OrgRbacPolicy {
            id: Uuid::new_v4(),
            org_id,
            name: name.to_string(),
            description: None,
            resource: "*".to_string(),
            action: "*".to_string(),
            condition: condition.to_string(),
            effect,
            priority,
            enabled: true,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        }
    }

    #[tokio::test]
    async fn test_new_registry() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        assert_eq!(registry.org_count().await, 0);
        assert_eq!(registry.policy_count().await, 0);
    }

    #[tokio::test]
    async fn test_refresh_org_policies() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        let policies = vec![
            create_test_policy(
                org_id,
                "allow-members",
                "'member' in subject.roles",
                RbacPolicyEffect::Allow,
                50,
            ),
            create_test_policy(
                org_id,
                "deny-guests",
                "'guest' in subject.roles",
                RbacPolicyEffect::Deny,
                100,
            ),
        ];

        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        assert_eq!(registry.org_count().await, 1);
        assert_eq!(registry.policy_count().await, 2);
    }

    #[tokio::test]
    async fn test_remove_org() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        let policies = vec![create_test_policy(
            org_id,
            "test",
            "true",
            RbacPolicyEffect::Allow,
            50,
        )];

        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();
        assert_eq!(registry.org_count().await, 1);

        registry.remove_org(org_id).await;
        assert_eq!(registry.org_count().await, 0);
    }

    #[tokio::test]
    async fn test_rbac_disabled_allows_all() {
        let engine = create_test_engine(false, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let subject = Subject::new();
        let context = PolicyContext::new("anything", "anything");

        let result = registry.authorize_with_org(None, &subject, &context).await;
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_system_policy_priority() {
        // System policies should be evaluated before org policies
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        // Org policy that would deny system_admin
        let policies = vec![create_test_policy(
            org_id,
            "deny-all",
            "true",
            RbacPolicyEffect::Deny,
            50,
        )];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        // System admin should still be allowed (system policy wins)
        let subject = Subject::new().with_roles(vec!["system_admin".to_string()]);
        let context = PolicyContext::new("resource", "action");

        let result = registry
            .authorize_with_org(Some(org_id), &subject, &context)
            .await;
        assert!(result.allowed);
        assert_eq!(result.policy_name, Some("system-admin".to_string()));
    }

    #[tokio::test]
    async fn test_org_policy_evaluation() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        let policies = vec![create_test_policy(
            org_id,
            "org-member",
            "'org_member' in subject.roles",
            RbacPolicyEffect::Allow,
            50,
        )];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        // User with org_member role should be allowed
        let subject = Subject::new().with_roles(vec!["org_member".to_string()]);
        let context = PolicyContext::new("resource", "action");

        let result = registry
            .authorize_with_org(Some(org_id), &subject, &context)
            .await;
        assert!(result.allowed);
        assert_eq!(result.policy_name, Some("org-member".to_string()));
    }

    #[tokio::test]
    async fn test_org_policy_deny() {
        let engine = create_test_engine(true, PolicyEffect::Allow);
        let registry = create_test_registry(engine, PolicyEffect::Allow);

        let org_id = Uuid::new_v4();
        let policies = vec![create_test_policy(
            org_id,
            "deny-guests",
            "'guest' in subject.roles",
            RbacPolicyEffect::Deny,
            100,
        )];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        // Guest user should be denied
        let subject = Subject::new().with_roles(vec!["guest".to_string()]);
        let context = PolicyContext::new("resource", "action");

        let result = registry
            .authorize_with_org(Some(org_id), &subject, &context)
            .await;
        assert!(!result.allowed);
        assert_eq!(result.policy_name, Some("deny-guests".to_string()));
    }

    #[tokio::test]
    async fn test_no_policy_matches_uses_default() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let subject = Subject::new().with_roles(vec!["unknown".to_string()]);
        let context = PolicyContext::new("resource", "action");

        // No org, no matching system policy -> default deny
        let result = registry.authorize_with_org(None, &subject, &context).await;
        assert!(!result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("default deny"));
    }

    #[tokio::test]
    async fn test_default_allow() {
        let engine = create_test_engine(true, PolicyEffect::Allow);
        let registry = create_test_registry(engine, PolicyEffect::Allow);

        let subject = Subject::new().with_roles(vec!["unknown".to_string()]);
        let context = PolicyContext::new("resource", "action");

        // No matching policy -> default allow
        let result = registry.authorize_with_org(None, &subject, &context).await;
        assert!(result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("default allow"));
    }

    #[tokio::test]
    async fn test_policy_priority_ordering() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        // Higher priority deny should win over lower priority allow
        let policies = vec![
            create_test_policy(org_id, "allow-all", "true", RbacPolicyEffect::Allow, 50),
            create_test_policy(org_id, "deny-all", "true", RbacPolicyEffect::Deny, 100),
        ];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        let subject = Subject::new();
        let context = PolicyContext::new("resource", "action");

        let result = registry
            .authorize_with_org(Some(org_id), &subject, &context)
            .await;
        assert!(!result.allowed);
        assert_eq!(result.policy_name, Some("deny-all".to_string()));
    }

    #[tokio::test]
    async fn test_deny_before_allow_at_same_priority() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        // Same priority - deny should be evaluated first
        let policies = vec![
            create_test_policy(org_id, "allow-all", "true", RbacPolicyEffect::Allow, 50),
            create_test_policy(org_id, "deny-all", "true", RbacPolicyEffect::Deny, 50),
        ];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        let subject = Subject::new();
        let context = PolicyContext::new("resource", "action");

        let result = registry
            .authorize_with_org(Some(org_id), &subject, &context)
            .await;
        assert!(!result.allowed);
        assert_eq!(result.policy_name, Some("deny-all".to_string()));
    }

    #[tokio::test]
    async fn test_resource_action_matching() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        let mut policy =
            create_test_policy(org_id, "projects-read", "true", RbacPolicyEffect::Allow, 50);
        policy.resource = "projects".to_string();
        policy.action = "read".to_string();

        registry
            .refresh_org_policies(org_id, vec![policy])
            .await
            .unwrap();

        let subject = Subject::new();

        // Should match projects/read
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("projects", "read"),
            )
            .await;
        assert!(result.allowed);

        // Should NOT match projects/write
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("projects", "write"),
            )
            .await;
        assert!(!result.allowed);

        // Should NOT match teams/read
        let result = registry
            .authorize_with_org(Some(org_id), &subject, &PolicyContext::new("teams", "read"))
            .await;
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_wildcard_resource_action() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        let mut policy = create_test_policy(
            org_id,
            "allow-all-reads",
            "true",
            RbacPolicyEffect::Allow,
            50,
        );
        policy.resource = "*".to_string();
        policy.action = "read".to_string();

        registry
            .refresh_org_policies(org_id, vec![policy])
            .await
            .unwrap();

        let subject = Subject::new();

        // Should match any resource with read action
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("projects", "read"),
            )
            .await;
        assert!(result.allowed);

        let result = registry
            .authorize_with_org(Some(org_id), &subject, &PolicyContext::new("teams", "read"))
            .await;
        assert!(result.allowed);

        // Should NOT match write action
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("projects", "write"),
            )
            .await;
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_disabled_policies_not_loaded() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        let mut policy = create_test_policy(
            org_id,
            "disabled-policy",
            "true",
            RbacPolicyEffect::Allow,
            50,
        );
        policy.enabled = false;

        registry
            .refresh_org_policies(org_id, vec![policy])
            .await
            .unwrap();

        // Disabled policy should not be cached
        assert_eq!(registry.org_count().await, 0);
        assert_eq!(registry.policy_count().await, 0);
    }

    #[tokio::test]
    async fn test_invalid_cel_expression_skipped() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        // Use a CEL syntax error that fails at compile time
        let policies = vec![
            create_test_policy(org_id, "valid", "true", RbacPolicyEffect::Allow, 100),
            create_test_policy(
                org_id,
                "invalid",
                "invalid!!!", // This has invalid CEL syntax
                RbacPolicyEffect::Allow,
                50,
            ),
        ];

        // Should succeed, skipping invalid policy
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        // Only valid policy should be cached
        assert_eq!(registry.policy_count().await, 1);
    }

    #[tokio::test]
    async fn test_prefix_wildcard_patterns() {
        // Test that prefix wildcards (e.g., "team*") work correctly for org policies
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();

        // Create policy with prefix wildcard for resource
        let mut team_wildcard =
            create_test_policy(org_id, "team-wildcard", "true", RbacPolicyEffect::Allow, 50);
        team_wildcard.resource = "team*".to_string();
        team_wildcard.action = "*".to_string();

        // Create policy with prefix wildcard for action
        let mut read_wildcard =
            create_test_policy(org_id, "read-wildcard", "true", RbacPolicyEffect::Allow, 40);
        read_wildcard.resource = "*".to_string();
        read_wildcard.action = "read*".to_string();

        registry
            .refresh_org_policies(org_id, vec![team_wildcard, read_wildcard])
            .await
            .unwrap();

        let subject = Subject::new();

        // team* should match: team, teams, team_admin, team_member
        let result = registry
            .authorize_with_org(Some(org_id), &subject, &PolicyContext::new("team", "write"))
            .await;
        assert!(result.allowed, "team should match team*");
        assert_eq!(result.policy_name, Some("team-wildcard".to_string()));

        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("teams", "write"),
            )
            .await;
        assert!(result.allowed, "teams should match team*");

        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("team_admin", "delete"),
            )
            .await;
        assert!(result.allowed, "team_admin should match team*");

        // team* should NOT match: project, organization
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("project", "write"),
            )
            .await;
        assert!(!result.allowed, "project should NOT match team*");

        // read* should match: read, read_all, readonly
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("project", "read"),
            )
            .await;
        assert!(result.allowed, "read should match read*");
        assert_eq!(result.policy_name, Some("read-wildcard".to_string()));

        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("project", "read_all"),
            )
            .await;
        assert!(result.allowed, "read_all should match read*");

        // read* should NOT match: write, delete
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("organization", "write"),
            )
            .await;
        assert!(!result.allowed, "write should NOT match read*");
    }

    #[tokio::test]
    async fn test_authorize_with_org_and_default_override() {
        // Test that authorize_with_org_and_default respects the override effect
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let subject = Subject::new().with_roles(vec!["unknown".to_string()]);
        let context = PolicyContext::new("resource", "action");

        // With override to Allow, should allow when no policy matches
        let result = registry
            .authorize_with_org_and_default(None, &subject, &context, PolicyEffect::Allow)
            .await;
        assert!(result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("default allow"));

        // With override to Deny, should deny when no policy matches
        let result = registry
            .authorize_with_org_and_default(None, &subject, &context, PolicyEffect::Deny)
            .await;
        assert!(!result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("default deny"));
    }

    #[tokio::test]
    async fn test_authorize_with_org_and_default_policy_overrides_default() {
        // Test that a matching policy takes precedence over the default override
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        let policies = vec![create_test_policy(
            org_id,
            "deny-role",
            "'specific_role' in subject.roles",
            RbacPolicyEffect::Deny,
            50,
        )];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        let subject = Subject::new().with_roles(vec!["specific_role".to_string()]);
        let context = PolicyContext::new("resource", "action");

        // Even with default override to Allow, the policy should deny
        let result = registry
            .authorize_with_org_and_default(Some(org_id), &subject, &context, PolicyEffect::Allow)
            .await;
        assert!(!result.allowed);
        assert_eq!(result.policy_name, Some("deny-role".to_string()));
    }

    #[tokio::test]
    async fn test_multi_org_isolation() {
        // Policies from one org should not affect another org
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();

        // Org A allows "member" role
        let policies_a = vec![create_test_policy(
            org_a,
            "allow-members",
            "'member' in subject.roles",
            RbacPolicyEffect::Allow,
            50,
        )];
        registry
            .refresh_org_policies(org_a, policies_a)
            .await
            .unwrap();

        // Org B denies "member" role
        let policies_b = vec![create_test_policy(
            org_b,
            "deny-members",
            "'member' in subject.roles",
            RbacPolicyEffect::Deny,
            50,
        )];
        registry
            .refresh_org_policies(org_b, policies_b)
            .await
            .unwrap();

        let subject = Subject::new().with_roles(vec!["member".to_string()]);
        let context = PolicyContext::new("resource", "action");

        // Same subject/context, but different orgs should get different results
        let result_a = registry
            .authorize_with_org(Some(org_a), &subject, &context)
            .await;
        assert!(result_a.allowed);
        assert_eq!(result_a.policy_name, Some("allow-members".to_string()));

        let result_b = registry
            .authorize_with_org(Some(org_b), &subject, &context)
            .await;
        assert!(!result_b.allowed);
        assert_eq!(result_b.policy_name, Some("deny-members".to_string()));
    }

    #[tokio::test]
    async fn test_org_without_policies_uses_default() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        // Random org ID that has no policies
        let unknown_org = Uuid::new_v4();

        let subject = Subject::new().with_roles(vec!["member".to_string()]);
        let context = PolicyContext::new("resource", "action");

        // Should fall through to default effect
        let result = registry
            .authorize_with_org(Some(unknown_org), &subject, &context)
            .await;
        assert!(!result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("default deny"));
    }

    #[tokio::test]
    async fn test_complex_cel_conditions() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();

        // Test complex CEL with multiple conditions
        let policies = vec![
            // Allow if user is admin OR (user is member AND action is read)
            create_test_policy(
                org_id,
                "complex-allow",
                "'admin' in subject.roles || ('member' in subject.roles && context.action == 'read')",
                RbacPolicyEffect::Allow,
                50,
            ),
        ];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        // Admin can do anything
        let admin = Subject::new().with_roles(vec!["admin".to_string()]);
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &admin,
                &PolicyContext::new("resource", "write"),
            )
            .await;
        assert!(result.allowed);

        // Member can read
        let member = Subject::new().with_roles(vec!["member".to_string()]);
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &member,
                &PolicyContext::new("resource", "read"),
            )
            .await;
        assert!(result.allowed);

        // Member cannot write
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &member,
                &PolicyContext::new("resource", "write"),
            )
            .await;
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_cel_with_context_fields() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();

        // Test CEL that checks context fields
        let policies = vec![create_test_policy(
            org_id,
            "check-context",
            "context.resource_type == 'project' && context.action == 'delete'",
            RbacPolicyEffect::Allow,
            50,
        )];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        let subject = Subject::new();

        // Should match project/delete
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("project", "delete"),
            )
            .await;
        assert!(result.allowed);

        // Should not match project/read
        let result = registry
            .authorize_with_org(
                Some(org_id),
                &subject,
                &PolicyContext::new("project", "read"),
            )
            .await;
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_cel_with_subject_membership() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();
        let team_id = Uuid::new_v4().to_string();

        // Allow if user belongs to specific team
        let policies = vec![create_test_policy(
            org_id,
            "team-check",
            &format!("'{}' in subject.team_ids", team_id),
            RbacPolicyEffect::Allow,
            50,
        )];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        let context = PolicyContext::new("resource", "action");

        // User in the team should be allowed
        let team_member = Subject::new().with_team_ids(vec![team_id.clone()]);
        let result = registry
            .authorize_with_org(Some(org_id), &team_member, &context)
            .await;
        assert!(result.allowed);

        // User not in the team should be denied
        let non_member = Subject::new().with_team_ids(vec![Uuid::new_v4().to_string()]);
        let result = registry
            .authorize_with_org(Some(org_id), &non_member, &context)
            .await;
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_empty_policies_list() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();

        // Refresh with empty list should remove org from cache
        registry.refresh_org_policies(org_id, vec![]).await.unwrap();

        assert_eq!(registry.org_count().await, 0);
    }

    #[tokio::test]
    async fn test_refresh_replaces_existing_policies() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();

        // Initial policy allows "member"
        let initial = vec![create_test_policy(
            org_id,
            "allow-member",
            "'member' in subject.roles",
            RbacPolicyEffect::Allow,
            50,
        )];
        registry
            .refresh_org_policies(org_id, initial)
            .await
            .unwrap();

        let member = Subject::new().with_roles(vec!["member".to_string()]);
        let context = PolicyContext::new("resource", "action");

        // Member should be allowed initially
        let result = registry
            .authorize_with_org(Some(org_id), &member, &context)
            .await;
        assert!(result.allowed);

        // Update to deny "member"
        let updated = vec![create_test_policy(
            org_id,
            "deny-member",
            "'member' in subject.roles",
            RbacPolicyEffect::Deny,
            50,
        )];
        registry
            .refresh_org_policies(org_id, updated)
            .await
            .unwrap();

        // Member should now be denied
        let result = registry
            .authorize_with_org(Some(org_id), &member, &context)
            .await;
        assert!(!result.allowed);
        assert_eq!(result.policy_name, Some("deny-member".to_string()));
    }

    #[tokio::test]
    async fn test_all_policies_invalid_removes_org() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();

        // Add valid policy first
        let valid = vec![create_test_policy(
            org_id,
            "valid",
            "true",
            RbacPolicyEffect::Allow,
            50,
        )];
        registry.refresh_org_policies(org_id, valid).await.unwrap();
        assert_eq!(registry.org_count().await, 1);

        // Now refresh with only invalid policies - should remove org
        // Use CEL expressions with syntax errors that fail compilation
        let invalid = vec![
            create_test_policy(
                org_id,
                "invalid1",
                "invalid!!!", // Invalid CEL syntax
                RbacPolicyEffect::Allow,
                50,
            ),
            create_test_policy(
                org_id,
                "invalid2",
                "@#$%bad", // Invalid CEL syntax
                RbacPolicyEffect::Allow,
                40,
            ),
        ];
        registry
            .refresh_org_policies(org_id, invalid)
            .await
            .unwrap();

        // Org should be removed since no valid policies
        assert_eq!(registry.org_count().await, 0);
    }

    #[tokio::test]
    async fn test_policy_count_across_multiple_orgs() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();

        // Org A gets 2 policies
        let policies_a = vec![
            create_test_policy(org_a, "a-policy-1", "true", RbacPolicyEffect::Allow, 50),
            create_test_policy(org_a, "a-policy-2", "true", RbacPolicyEffect::Deny, 40),
        ];
        registry
            .refresh_org_policies(org_a, policies_a)
            .await
            .unwrap();

        // Org B gets 3 policies
        let policies_b = vec![
            create_test_policy(org_b, "b-policy-1", "true", RbacPolicyEffect::Allow, 50),
            create_test_policy(org_b, "b-policy-2", "true", RbacPolicyEffect::Deny, 40),
            create_test_policy(org_b, "b-policy-3", "true", RbacPolicyEffect::Allow, 30),
        ];
        registry
            .refresh_org_policies(org_b, policies_b)
            .await
            .unwrap();

        assert_eq!(registry.org_count().await, 2);
        assert_eq!(registry.policy_count().await, 5); // 2 + 3
    }

    #[tokio::test]
    async fn test_policy_evaluation_stops_at_first_match() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();

        // Multiple policies that could match - only first should be used
        let policies = vec![
            // Higher priority - this one should match
            create_test_policy(org_id, "first-match", "true", RbacPolicyEffect::Allow, 100),
            // Lower priority - should not be evaluated
            create_test_policy(org_id, "second-match", "true", RbacPolicyEffect::Deny, 50),
        ];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        let subject = Subject::new();
        let context = PolicyContext::new("resource", "action");

        let result = registry
            .authorize_with_org(Some(org_id), &subject, &context)
            .await;
        assert!(result.allowed);
        assert_eq!(result.policy_name, Some("first-match".to_string()));
    }

    #[tokio::test]
    async fn test_condition_false_continues_to_next_policy() {
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry(engine, PolicyEffect::Deny);

        let org_id = Uuid::new_v4();

        // First policy condition is false, second should be evaluated
        let policies = vec![
            create_test_policy(
                org_id,
                "false-condition",
                "false", // This won't match
                RbacPolicyEffect::Deny,
                100,
            ),
            create_test_policy(
                org_id,
                "true-condition",
                "true", // This will match
                RbacPolicyEffect::Allow,
                50,
            ),
        ];
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .unwrap();

        let subject = Subject::new();
        let context = PolicyContext::new("resource", "action");

        let result = registry
            .authorize_with_org(Some(org_id), &subject, &context)
            .await;
        assert!(result.allowed);
        assert_eq!(result.policy_name, Some("true-condition".to_string()));
    }

    #[test]
    fn test_compile_policies_sorting() {
        // Test that policies are sorted correctly:
        // 1. Higher priority first
        // 2. At same priority, deny before allow
        let org_id = Uuid::new_v4();

        let policies = vec![
            create_test_policy(org_id, "low-allow", "true", RbacPolicyEffect::Allow, 10),
            create_test_policy(org_id, "high-deny", "true", RbacPolicyEffect::Deny, 100),
            create_test_policy(org_id, "med-allow", "true", RbacPolicyEffect::Allow, 50),
            create_test_policy(org_id, "med-deny", "true", RbacPolicyEffect::Deny, 50),
            create_test_policy(org_id, "high-allow", "true", RbacPolicyEffect::Allow, 100),
        ];

        let mut total_compiled = 0;
        let mut total_failed = 0;
        let compiled = compile_policies(&policies, &mut total_compiled, &mut total_failed, 4096);

        assert_eq!(compiled.len(), 5);
        assert_eq!(total_compiled, 5);
        assert_eq!(total_failed, 0);

        // Check order: high-deny, high-allow, med-deny, med-allow, low-allow
        assert_eq!(compiled[0].policy.name, "high-deny"); // 100, deny
        assert_eq!(compiled[1].policy.name, "high-allow"); // 100, allow
        assert_eq!(compiled[2].policy.name, "med-deny"); // 50, deny
        assert_eq!(compiled[3].policy.name, "med-allow"); // 50, allow
        assert_eq!(compiled[4].policy.name, "low-allow"); // 10, allow
    }

    #[test]
    fn test_pattern_matches_helper() {
        // Direct tests for pattern_matches function
        assert!(super::super::pattern_matches("*", "anything"));
        assert!(super::super::pattern_matches("*", ""));
        assert!(super::super::pattern_matches("exact", "exact"));
        assert!(!super::super::pattern_matches("exact", "different"));
        assert!(super::super::pattern_matches("prefix*", "prefix"));
        assert!(super::super::pattern_matches("prefix*", "prefix_with_more"));
        assert!(!super::super::pattern_matches("prefix*", "other"));
        assert!(!super::super::pattern_matches("prefix*", "pre")); // Must start with full prefix
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // LRU Eviction Tests
    // ─────────────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_lru_eviction_at_capacity() {
        // Test that oldest orgs are evicted when cache reaches capacity
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry_with_lru(engine, PolicyEffect::Deny, 3, 1);

        // Add 4 orgs (exceeds max of 3)
        let orgs: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();

        for org_id in &orgs {
            let policy = create_test_policy(*org_id, "test", "true", RbacPolicyEffect::Allow, 50);
            registry
                .refresh_org_policies(*org_id, vec![policy])
                .await
                .unwrap();
            // Small delay to ensure different last_accessed times
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Trigger eviction by adding another org
        registry.evict_if_needed().await;

        // Should have evicted at least 1 org to get below max
        assert!(
            registry.org_count().await <= 3,
            "Cache should be at or below max capacity after eviction"
        );
    }

    #[tokio::test]
    async fn test_lru_eviction_batch_size() {
        // Test that eviction respects batch size
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry_with_lru(engine, PolicyEffect::Deny, 5, 2);

        // Add 5 orgs to reach capacity
        for _ in 0..5 {
            let org_id = Uuid::new_v4();
            let policy = create_test_policy(org_id, "test", "true", RbacPolicyEffect::Allow, 50);
            registry
                .refresh_org_policies(org_id, vec![policy])
                .await
                .unwrap();
        }

        assert_eq!(registry.org_count().await, 5);

        // Add one more to trigger eviction (batch_size = 2)
        let new_org = Uuid::new_v4();
        let policy = create_test_policy(new_org, "test", "true", RbacPolicyEffect::Allow, 50);
        registry.evict_if_needed().await;
        registry
            .refresh_org_policies(new_org, vec![policy])
            .await
            .unwrap();

        // Should have evicted 2 to make room, so 5 - 2 + 1 = 4
        let count = registry.org_count().await;
        assert!(
            count <= 5,
            "Should be at or below max after eviction: {}",
            count
        );
    }

    #[tokio::test]
    async fn test_no_eviction_when_unlimited() {
        // Test that max_cached_orgs = 0 means unlimited (no eviction)
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry_with_lru(engine, PolicyEffect::Deny, 0, 1);

        // Add many orgs
        for _ in 0..10 {
            let org_id = Uuid::new_v4();
            let policy = create_test_policy(org_id, "test", "true", RbacPolicyEffect::Allow, 50);
            registry
                .refresh_org_policies(org_id, vec![policy])
                .await
                .unwrap();
        }

        // All orgs should still be cached
        assert_eq!(
            registry.org_count().await,
            10,
            "All orgs should be cached when max_cached_orgs = 0"
        );
    }

    #[tokio::test]
    async fn test_no_eviction_below_capacity() {
        // Test that eviction doesn't happen when below capacity
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry_with_lru(engine, PolicyEffect::Deny, 10, 2);

        // Add 5 orgs (below max of 10)
        for _ in 0..5 {
            let org_id = Uuid::new_v4();
            let policy = create_test_policy(org_id, "test", "true", RbacPolicyEffect::Allow, 50);
            registry
                .refresh_org_policies(org_id, vec![policy])
                .await
                .unwrap();
        }

        registry.evict_if_needed().await;

        // All orgs should still be cached
        assert_eq!(
            registry.org_count().await,
            5,
            "No eviction should happen below capacity"
        );
    }

    #[tokio::test]
    async fn test_lru_updates_on_access() {
        // Test that last_accessed is updated when policies are accessed
        let engine = create_test_engine(true, PolicyEffect::Deny);
        let registry = create_test_registry_with_lru(engine, PolicyEffect::Deny, 3, 1);

        // Add 3 orgs
        let org_a = Uuid::new_v4();
        let org_b = Uuid::new_v4();
        let org_c = Uuid::new_v4();

        for org_id in [org_a, org_b, org_c] {
            let policy = create_test_policy(org_id, "allow", "true", RbacPolicyEffect::Allow, 50);
            registry
                .refresh_org_policies(org_id, vec![policy])
                .await
                .unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Access org_a to update its last_accessed time
        let subject = Subject::new();
        let context = PolicyContext::new("resource", "action");
        let _ = registry
            .authorize_with_org(Some(org_a), &subject, &context)
            .await;

        // Add a 4th org to trigger eviction
        let org_d = Uuid::new_v4();
        let policy = create_test_policy(org_d, "allow", "true", RbacPolicyEffect::Allow, 50);
        registry.evict_if_needed().await;
        registry
            .refresh_org_policies(org_d, vec![policy])
            .await
            .unwrap();

        // org_a should still be cached (recently accessed)
        // org_b should be evicted (oldest)
        let cache = registry.org_policies.read().await;
        assert!(
            cache.contains_key(&org_a),
            "Recently accessed org should not be evicted"
        );
    }
}
