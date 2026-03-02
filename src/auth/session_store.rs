//! Session storage backends for OIDC sessions.
//!
//! This module provides a `SessionStore` trait with three implementations:
//! - `MemorySessionStore`: In-memory storage (single-node only)
//! - `CacheSessionStore`: Uses the existing Cache infrastructure (Redis/Memory)
//! - `DatabaseSessionStore`: Persists sessions to the database
//!
//! For multi-node deployments, use Redis (via Cache) or Database storage.

use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    cache::{Cache, CacheExt},
    observability::metrics,
};

/// Result type for session store operations.
pub type SessionResult<T> = Result<T, SessionError>;

/// Errors that can occur during session operations.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session not found")]
    NotFound,

    #[error("Session expired")]
    Expired,

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Maximum length for device info string fields (user agent, description, etc.).
const DEVICE_INFO_MAX_LENGTH: usize = 512;

/// Truncate a string to a maximum byte length, ensuring valid UTF-8 boundaries.
fn truncate_device_field(value: String, max_len: usize) -> String {
    if value.len() <= max_len {
        return value;
    }
    // Find the last valid char boundary at or before max_len
    let mut end = max_len;
    while !value.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    value[..end].to_string()
}

/// Device information for enhanced session tracking.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DeviceInfo {
    /// Raw User-Agent header value (truncated to 512 chars)
    #[serde(default)]
    pub user_agent: Option<String>,

    /// Client IP address
    #[serde(default)]
    pub ip_address: Option<String>,

    /// Device fingerprint ID (SHA256 hash of user agent, first 16 chars)
    #[serde(default)]
    pub device_id: Option<String>,

    /// Human-readable device description (parsed from user agent, truncated to 512 chars)
    /// e.g., "Chrome 120 on Windows"
    #[serde(default)]
    pub device_description: Option<String>,
}

impl DeviceInfo {
    /// Create a new DeviceInfo with all string fields truncated to safe lengths.
    pub fn new(
        user_agent: Option<String>,
        ip_address: Option<String>,
        device_id: Option<String>,
        device_description: Option<String>,
    ) -> Self {
        Self {
            user_agent: user_agent.map(|s| truncate_device_field(s, DEVICE_INFO_MAX_LENGTH)),
            ip_address,
            device_id,
            device_description: device_description
                .map(|s| truncate_device_field(s, DEVICE_INFO_MAX_LENGTH)),
        }
    }
}

/// OIDC session data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcSession {
    /// Session ID
    pub id: Uuid,

    /// External identity ID (from IdP)
    pub external_id: String,

    /// Email (if available)
    #[serde(default)]
    pub email: Option<String>,

    /// Display name (if available)
    #[serde(default)]
    pub name: Option<String>,

    /// Organization (if available from claims)
    #[serde(default)]
    pub org: Option<String>,

    /// Groups (e.g., Keycloak group paths like "/cs/faculty")
    #[serde(default)]
    pub groups: Vec<String>,

    /// Roles (e.g., Keycloak realm roles like "super_admin", "user")
    #[serde(default)]
    pub roles: Vec<String>,

    /// Access token (for API calls to the IdP)
    #[serde(default)]
    pub access_token: Option<String>,

    /// Refresh token (for token refresh)
    #[serde(default)]
    pub refresh_token: Option<String>,

    /// When the session was created
    pub created_at: DateTime<Utc>,

    /// When the session expires
    pub expires_at: DateTime<Utc>,

    /// When the access token expires (if known)
    #[serde(default)]
    pub token_expires_at: Option<DateTime<Utc>>,

    /// Organization ID that this session was authenticated through (for per-org SSO).
    /// This is used for SSO enforcement - if set, it indicates the user authenticated
    /// via the org's configured SSO provider.
    #[serde(default)]
    pub sso_org_id: Option<Uuid>,

    /// SAML SessionIndex for Single Logout (SLO)
    /// Included in LogoutRequest to identify the session at the IdP
    #[serde(default)]
    pub session_index: Option<String>,

    /// Device information (for enhanced session management)
    /// Populated when `auth.session.enhanced.track_devices = true`
    #[serde(default)]
    pub device: Option<DeviceInfo>,

    /// Last activity timestamp (for inactivity timeout)
    /// Updated on session access when enhanced sessions are enabled
    #[serde(default)]
    pub last_activity: Option<DateTime<Utc>>,
}

impl OidcSession {
    /// Check if the session has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Check if the access token has expired.
    pub fn is_token_expired(&self) -> bool {
        self.token_expires_at
            .map(|exp| Utc::now() >= exp)
            .unwrap_or(false)
    }

    /// Check if the session is inactive (idle too long).
    ///
    /// Returns `false` if:
    /// - `timeout_secs` is 0 (inactivity timeout disabled)
    /// - `last_activity` is None (no activity tracking)
    ///
    /// Returns `true` if the session has been idle longer than `timeout_secs`.
    pub fn is_inactive(&self, timeout_secs: u64) -> bool {
        if timeout_secs == 0 {
            return false;
        }
        self.last_activity
            .map(|last| Utc::now() >= last + chrono::Duration::seconds(timeout_secs as i64))
            .unwrap_or(false)
    }

    /// Get TTL as a Duration.
    pub fn ttl(&self) -> Duration {
        let now = Utc::now();
        if self.expires_at <= now {
            Duration::ZERO
        } else {
            (self.expires_at - now).to_std().unwrap_or(Duration::ZERO)
        }
    }
}

/// Pending authorization state (stored temporarily during OIDC auth flow).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationState {
    /// State parameter for CSRF protection
    pub state: String,

    /// Nonce for ID token replay protection
    pub nonce: String,

    /// PKCE code verifier
    pub code_verifier: String,

    /// Where to redirect after auth completes
    #[serde(default)]
    pub return_to: Option<String>,

    /// Organization ID for per-org SSO (if using org-specific IdP)
    #[serde(default)]
    pub org_id: Option<Uuid>,

    /// When this state was created
    pub created_at: DateTime<Utc>,
}

impl AuthorizationState {
    /// Check if the state has expired (10 minute limit).
    pub fn is_expired(&self) -> bool {
        let age = Utc::now() - self.created_at;
        age > chrono::Duration::minutes(10)
    }
}

/// Trait for OIDC session storage.
///
/// Implementations must be thread-safe and handle concurrent access.
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Store a new session.
    async fn create_session(&self, session: OidcSession) -> SessionResult<Uuid>;

    /// Get a session by ID.
    async fn get_session(&self, id: Uuid) -> SessionResult<Option<OidcSession>>;

    /// Delete a session.
    async fn delete_session(&self, id: Uuid) -> SessionResult<()>;

    /// Update a session (e.g., after token refresh).
    async fn update_session(&self, session: OidcSession) -> SessionResult<()>;

    /// Store pending authorization state.
    async fn store_auth_state(&self, state: AuthorizationState) -> SessionResult<()>;

    /// Get pending authorization state without removing it.
    /// Used to peek at the org_id before deciding which authenticator to use.
    async fn peek_auth_state(&self, state: &str) -> SessionResult<Option<AuthorizationState>>;

    /// Get and remove pending authorization state.
    async fn take_auth_state(&self, state: &str) -> SessionResult<Option<AuthorizationState>>;

    /// Clean up expired sessions and stale auth states.
    async fn cleanup(&self) -> SessionResult<()>;

    // ─────────────────────────────────────────────────────────────────────────────
    // Enhanced Session Management (User-Sessions Index)
    // ─────────────────────────────────────────────────────────────────────────────

    /// List all active sessions for a user.
    /// Returns an empty vec if enhanced sessions are not enabled.
    async fn list_user_sessions(&self, external_id: &str) -> SessionResult<Vec<OidcSession>>;

    /// Count the number of active sessions for a user.
    /// Returns 0 if enhanced sessions are not enabled.
    async fn count_user_sessions(&self, external_id: &str) -> SessionResult<usize>;

    /// Delete all sessions for a user and return the count of deleted sessions.
    /// Returns 0 if enhanced sessions are not enabled.
    async fn delete_user_sessions(&self, external_id: &str) -> SessionResult<usize>;

    /// Check if enhanced session management is enabled.
    fn is_enhanced_enabled(&self) -> bool {
        false
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Memory Session Store
// ─────────────────────────────────────────────────────────────────────────────

/// In-memory session store.
///
/// Suitable for development and single-node deployments.
/// Sessions are lost on restart and not shared across nodes.
pub struct MemorySessionStore {
    sessions: RwLock<HashMap<Uuid, OidcSession>>,
    pending_auth: RwLock<HashMap<String, AuthorizationState>>,
}

impl MemorySessionStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            pending_auth: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn create_session(&self, session: OidcSession) -> SessionResult<Uuid> {
        let id = session.id;
        let mut sessions = self.sessions.write().await;
        sessions.insert(id, session);
        Ok(id)
    }

    async fn get_session(&self, id: Uuid) -> SessionResult<Option<OidcSession>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(&id).cloned())
    }

    async fn delete_session(&self, id: Uuid) -> SessionResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&id);
        Ok(())
    }

    async fn update_session(&self, session: OidcSession) -> SessionResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id, session);
        Ok(())
    }

    async fn store_auth_state(&self, state: AuthorizationState) -> SessionResult<()> {
        let mut pending = self.pending_auth.write().await;
        pending.insert(state.state.clone(), state);
        Ok(())
    }

    async fn peek_auth_state(&self, state: &str) -> SessionResult<Option<AuthorizationState>> {
        let pending = self.pending_auth.read().await;
        Ok(pending.get(state).cloned())
    }

    async fn take_auth_state(&self, state: &str) -> SessionResult<Option<AuthorizationState>> {
        let mut pending = self.pending_auth.write().await;
        Ok(pending.remove(state))
    }

    async fn cleanup(&self) -> SessionResult<()> {
        let now = Utc::now();

        // Clean up expired sessions
        {
            let mut sessions = self.sessions.write().await;
            sessions.retain(|_, s| s.expires_at > now);
        }

        // Clean up stale auth states (older than 10 minutes)
        {
            let cutoff = now - chrono::Duration::minutes(10);
            let mut pending = self.pending_auth.write().await;
            pending.retain(|_, s| s.created_at > cutoff);
        }

        Ok(())
    }

    // MemorySessionStore does not support enhanced session management.
    // Use CacheSessionStore with enhanced = true for this feature.

    async fn list_user_sessions(&self, _external_id: &str) -> SessionResult<Vec<OidcSession>> {
        Ok(Vec::new())
    }

    async fn count_user_sessions(&self, _external_id: &str) -> SessionResult<usize> {
        Ok(0)
    }

    async fn delete_user_sessions(&self, _external_id: &str) -> SessionResult<usize> {
        Ok(0)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cache Session Store (Redis/Memory via Cache trait)
// ─────────────────────────────────────────────────────────────────────────────

/// Session store backed by the Cache infrastructure (Redis or Memory cache).
///
/// Sessions are stored as JSON with TTL. Suitable for multi-node deployments
/// when using Redis as the cache backend.
///
/// When enhanced session management is enabled (`with_enhanced()`), an additional
/// user-sessions index is maintained for listing and managing sessions by user.
pub struct CacheSessionStore {
    cache: Arc<dyn Cache>,
    key_prefix: String,
    /// Whether enhanced session management features are enabled
    enhanced_enabled: bool,
}

impl CacheSessionStore {
    /// Create a new cache-backed session store (enhanced sessions disabled).
    /// Use `with_enhanced` for session listing/management features.
    #[allow(dead_code)] // Public API convenience constructor
    pub fn new(cache: Arc<dyn Cache>) -> Self {
        Self {
            cache,
            key_prefix: "oidc:session:".to_string(),
            enhanced_enabled: false,
        }
    }

    /// Create with enhanced session management enabled.
    ///
    /// When enabled, sessions are indexed by user for listing and management.
    pub fn with_enhanced(cache: Arc<dyn Cache>, enabled: bool) -> Self {
        Self {
            cache,
            key_prefix: "oidc:session:".to_string(),
            enhanced_enabled: enabled,
        }
    }

    /// Create with a custom key prefix.
    #[allow(dead_code)] // Public API convenience constructor
    pub fn with_prefix(cache: Arc<dyn Cache>, prefix: impl Into<String>) -> Self {
        Self {
            cache,
            key_prefix: prefix.into(),
            enhanced_enabled: false,
        }
    }

    fn session_key(&self, id: Uuid) -> String {
        format!("{}session:{}", self.key_prefix, id)
    }

    fn auth_state_key(&self, state: &str) -> String {
        format!("{}auth_state:{}", self.key_prefix, state)
    }

    /// Key for the user-sessions index set.
    /// Stores all session IDs for a given external_id.
    fn user_sessions_key(&self, external_id: &str) -> String {
        format!("{}user_sessions:{}", self.key_prefix, external_id)
    }
}

#[async_trait]
impl SessionStore for CacheSessionStore {
    async fn create_session(&self, session: OidcSession) -> SessionResult<Uuid> {
        let id = session.id;
        let key = self.session_key(id);
        let ttl = session.ttl();

        match self.cache.set_json(&key, &session, ttl).await {
            Ok(_) => {
                metrics::record_cache_operation("session", "set", "success");

                // Add to user-sessions index if enhanced sessions are enabled
                if self.enhanced_enabled {
                    let user_sessions_key = self.user_sessions_key(&session.external_id);
                    if let Err(e) = self
                        .cache
                        .set_add(&user_sessions_key, &id.to_string(), Some(ttl))
                        .await
                    {
                        tracing::warn!(
                            session_id = %id,
                            external_id = %session.external_id,
                            error = %e,
                            "Failed to add session to user-sessions index"
                        );
                        // Don't fail the session creation - the index is best-effort
                    }
                }

                Ok(id)
            }
            Err(e) => {
                metrics::record_cache_operation("session", "set", "error");
                Err(SessionError::Cache(e.to_string()))
            }
        }
    }

    async fn get_session(&self, id: Uuid) -> SessionResult<Option<OidcSession>> {
        let key = self.session_key(id);

        let session: Option<OidcSession> = match self.cache.get_json(&key).await {
            Ok(Some(s)) => {
                metrics::record_cache_operation("session", "get", "hit");
                Some(s)
            }
            Ok(None) => {
                metrics::record_cache_operation("session", "get", "miss");
                None
            }
            Err(e) => {
                metrics::record_cache_operation("session", "get", "error");
                return Err(SessionError::Cache(e.to_string()));
            }
        };

        // Check expiration (belt and suspenders - TTL should handle this)
        if let Some(ref s) = session
            && s.is_expired()
        {
            self.delete_session(id).await?;
            return Ok(None);
        }

        Ok(session)
    }

    async fn delete_session(&self, id: Uuid) -> SessionResult<()> {
        // If enhanced sessions are enabled, we need to remove from the user-sessions index
        // Get the session first to find the external_id
        let external_id = if self.enhanced_enabled {
            let key = self.session_key(id);
            match self.cache.get_json::<OidcSession>(&key).await {
                Ok(Some(session)) => Some(session.external_id),
                _ => None,
            }
        } else {
            None
        };

        let key = self.session_key(id);
        match self.cache.delete(&key).await {
            Ok(_) => {
                metrics::record_cache_operation("session", "delete", "success");

                // Remove from user-sessions index if enhanced sessions are enabled
                if let Some(ext_id) = external_id {
                    let user_sessions_key = self.user_sessions_key(&ext_id);
                    if let Err(e) = self
                        .cache
                        .set_remove(&user_sessions_key, &id.to_string())
                        .await
                    {
                        tracing::warn!(
                            session_id = %id,
                            external_id = %ext_id,
                            error = %e,
                            "Failed to remove session from user-sessions index"
                        );
                        // Don't fail the deletion - the index is best-effort
                    }
                }

                Ok(())
            }
            Err(e) => {
                metrics::record_cache_operation("session", "delete", "error");
                Err(SessionError::Cache(e.to_string()))
            }
        }
    }

    async fn update_session(&self, session: OidcSession) -> SessionResult<()> {
        let key = self.session_key(session.id);
        let ttl = session.ttl();

        match self.cache.set_json(&key, &session, ttl).await {
            Ok(_) => {
                metrics::record_cache_operation("session", "set", "success");

                // Refresh TTL on user-sessions index if enhanced sessions are enabled
                if self.enhanced_enabled {
                    let user_sessions_key = self.user_sessions_key(&session.external_id);
                    if let Err(e) = self.cache.set_expire(&user_sessions_key, ttl).await {
                        tracing::warn!(
                            session_id = %session.id,
                            external_id = %session.external_id,
                            error = %e,
                            "Failed to refresh TTL on user-sessions index"
                        );
                        // Non-fatal: the index will still work, just might expire
                    }
                }

                Ok(())
            }
            Err(e) => {
                metrics::record_cache_operation("session", "set", "error");
                Err(SessionError::Cache(e.to_string()))
            }
        }
    }

    async fn store_auth_state(&self, state: AuthorizationState) -> SessionResult<()> {
        let key = self.auth_state_key(&state.state);
        // Auth states expire after 10 minutes
        let ttl = Duration::from_secs(600);

        match self.cache.set_json(&key, &state, ttl).await {
            Ok(_) => {
                metrics::record_cache_operation("auth_state", "set", "success");
                Ok(())
            }
            Err(e) => {
                metrics::record_cache_operation("auth_state", "set", "error");
                Err(SessionError::Cache(e.to_string()))
            }
        }
    }

    async fn peek_auth_state(&self, state: &str) -> SessionResult<Option<AuthorizationState>> {
        let key = self.auth_state_key(state);

        match self.cache.get_json(&key).await {
            Ok(Some(s)) => {
                metrics::record_cache_operation("auth_state", "peek", "hit");
                Ok(Some(s))
            }
            Ok(None) => {
                metrics::record_cache_operation("auth_state", "peek", "miss");
                Ok(None)
            }
            Err(e) => {
                metrics::record_cache_operation("auth_state", "peek", "error");
                Err(SessionError::Cache(e.to_string()))
            }
        }
    }

    async fn take_auth_state(&self, state: &str) -> SessionResult<Option<AuthorizationState>> {
        let key = self.auth_state_key(state);

        let auth_state: Option<AuthorizationState> = match self.cache.get_json(&key).await {
            Ok(Some(s)) => {
                metrics::record_cache_operation("auth_state", "get", "hit");
                Some(s)
            }
            Ok(None) => {
                metrics::record_cache_operation("auth_state", "get", "miss");
                None
            }
            Err(e) => {
                metrics::record_cache_operation("auth_state", "get", "error");
                return Err(SessionError::Cache(e.to_string()));
            }
        };

        // Delete after retrieval (one-time use)
        if auth_state.is_some() {
            match self.cache.delete(&key).await {
                Ok(_) => metrics::record_cache_operation("auth_state", "delete", "success"),
                Err(e) => {
                    metrics::record_cache_operation("auth_state", "delete", "error");
                    return Err(SessionError::Cache(e.to_string()));
                }
            }
        }

        Ok(auth_state)
    }

    async fn cleanup(&self) -> SessionResult<()> {
        // Cache TTLs handle cleanup automatically
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Enhanced Session Management
    // ─────────────────────────────────────────────────────────────────────────────

    async fn list_user_sessions(&self, external_id: &str) -> SessionResult<Vec<OidcSession>> {
        if !self.enhanced_enabled {
            return Ok(Vec::new());
        }

        let user_sessions_key = self.user_sessions_key(external_id);

        // Get all session IDs from the index
        let session_ids = match self.cache.set_members(&user_sessions_key).await {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!(
                    external_id = %external_id,
                    error = %e,
                    "Failed to get user sessions from index"
                );
                return Ok(Vec::new());
            }
        };

        // Fetch each session and clean up stale index entries
        let mut sessions = Vec::with_capacity(session_ids.len());
        let mut stale_ids = Vec::new();

        for id_str in session_ids {
            let Ok(id) = Uuid::parse_str(&id_str) else {
                // Invalid UUID in index, mark for cleanup
                stale_ids.push(id_str);
                continue;
            };

            match self.get_session(id).await {
                Ok(Some(session)) => {
                    if !session.is_expired() {
                        sessions.push(session);
                    } else {
                        // Session expired, mark for cleanup
                        stale_ids.push(id_str);
                    }
                }
                Ok(None) => {
                    // Session no longer exists, mark for cleanup
                    stale_ids.push(id_str);
                }
                Err(_) => {
                    // Error fetching session, skip but don't remove from index
                }
            }
        }

        // Clean up stale entries from the index
        for stale_id in stale_ids {
            let _ = self.cache.set_remove(&user_sessions_key, &stale_id).await;
        }

        Ok(sessions)
    }

    async fn count_user_sessions(&self, external_id: &str) -> SessionResult<usize> {
        if !self.enhanced_enabled {
            return Ok(0);
        }

        let user_sessions_key = self.user_sessions_key(external_id);

        match self.cache.set_cardinality(&user_sessions_key).await {
            Ok(count) => Ok(count),
            Err(e) => {
                tracing::warn!(
                    external_id = %external_id,
                    error = %e,
                    "Failed to count user sessions"
                );
                Ok(0)
            }
        }
    }

    async fn delete_user_sessions(&self, external_id: &str) -> SessionResult<usize> {
        if !self.enhanced_enabled {
            return Ok(0);
        }

        // First list all sessions to delete them
        let sessions = self.list_user_sessions(external_id).await?;
        let count = sessions.len();

        // Delete each session
        for session in sessions {
            let _ = self.delete_session(session.id).await;
        }

        // Delete the index key itself
        let user_sessions_key = self.user_sessions_key(external_id);
        let _ = self.cache.delete(&user_sessions_key).await;

        Ok(count)
    }

    fn is_enhanced_enabled(&self) -> bool {
        self.enhanced_enabled
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared Session Store Type
// ─────────────────────────────────────────────────────────────────────────────

/// Type alias for a shared session store.
pub type SharedSessionStore = Arc<dyn SessionStore>;

/// Create a session store with optional enhanced session management.
///
/// When `enhanced` is true, the session store will maintain a user-sessions index
/// for listing and managing sessions by user.
///
/// Priority:
/// 1. If Redis cache is configured, use CacheSessionStore
/// 2. If memory cache is configured, use CacheSessionStore
/// 3. Otherwise, use MemorySessionStore
pub fn create_session_store_with_enhanced(
    cache: Option<Arc<dyn Cache>>,
    enhanced: bool,
) -> SharedSessionStore {
    match cache {
        Some(cache) => {
            if enhanced {
                tracing::info!("Using cache-backed session store with enhanced session management");
            } else {
                tracing::info!("Using cache-backed session store for OIDC sessions");
            }
            Arc::new(CacheSessionStore::with_enhanced(cache, enhanced))
        }
        None => {
            if enhanced {
                tracing::warn!(
                    "Enhanced session management requires Redis/cache backend. \
                     Falling back to basic in-memory session store."
                );
            }
            tracing::warn!(
                "Using in-memory session store for OIDC sessions. \
                 Sessions will be lost on restart and not shared across nodes."
            );
            Arc::new(MemorySessionStore::new())
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Session Limit Enforcement
// ─────────────────────────────────────────────────────────────────────────────

/// Enforce the maximum concurrent sessions limit for a user.
///
/// When enhanced session management is enabled and `max_sessions > 0`, this function
/// checks if the user has exceeded the session limit. If so, it evicts the oldest
/// sessions until the count is within the limit.
///
/// # Arguments
/// * `session_store` - The session store to operate on
/// * `external_id` - The user's external identity ID
/// * `max_sessions` - Maximum allowed concurrent sessions (0 = unlimited)
///
/// # Returns
/// * `Ok(count)` - Number of sessions evicted
/// * `Err(SessionError)` - If an error occurred during eviction
///
/// # Notes
/// - This is a no-op if enhanced sessions are not enabled
/// - This is a no-op if `max_sessions` is 0 (unlimited)
/// - Sessions are evicted oldest-first based on `created_at`
pub async fn enforce_session_limit(
    session_store: &dyn SessionStore,
    external_id: &str,
    max_sessions: u32,
) -> SessionResult<usize> {
    // Skip if enhanced sessions not enabled or no limit set
    if !session_store.is_enhanced_enabled() || max_sessions == 0 {
        return Ok(0);
    }

    // Check current session count
    let count = session_store.count_user_sessions(external_id).await?;
    if count <= max_sessions as usize {
        return Ok(0);
    }

    // Need to evict some sessions
    let mut sessions = session_store.list_user_sessions(external_id).await?;

    // Sort by created_at (oldest first)
    sessions.sort_by_key(|s| s.created_at);

    let to_evict = count - max_sessions as usize;
    let mut evicted = 0;

    for session in sessions.into_iter().take(to_evict) {
        if session_store.delete_session(session.id).await.is_ok() {
            evicted += 1;
            tracing::info!(
                session_id = %session.id,
                external_id = %external_id,
                created_at = %session.created_at,
                "Evicted session due to concurrent session limit"
            );
        }
    }

    if evicted > 0 {
        tracing::info!(
            external_id = %external_id,
            evicted = evicted,
            max_sessions = max_sessions,
            "Enforced concurrent session limit"
        );
    }

    Ok(evicted)
}

// ─────────────────────────────────────────────────────────────────────────────
// Session Validation
// ─────────────────────────────────────────────────────────────────────────────

use crate::config::EnhancedSessionConfig;

/// Validate a session and refresh its last_activity timestamp.
///
/// This function centralizes session validation logic shared by OIDC and SAML authenticators.
/// It performs the following checks:
/// 1. Verifies the session exists
/// 2. Checks absolute expiration (`expires_at`)
/// 3. Checks inactivity timeout (if enhanced sessions are enabled)
/// 4. Updates `last_activity` timestamp (if enhanced sessions are enabled)
///
/// # Arguments
/// * `session_store` - The session store to operate on
/// * `session_id` - The session ID to validate
/// * `enhanced_config` - Enhanced session configuration for timeout settings
///
/// # Returns
/// * `Ok(OidcSession)` - The validated (and potentially updated) session
/// * `Err(SessionError::NotFound)` - If the session doesn't exist
/// * `Err(SessionError::Expired)` - If the session is expired or inactive
pub async fn validate_and_refresh_session(
    session_store: &dyn SessionStore,
    session_id: Uuid,
    enhanced_config: &EnhancedSessionConfig,
) -> SessionResult<OidcSession> {
    let mut session = session_store
        .get_session(session_id)
        .await?
        .ok_or(SessionError::NotFound)?;

    // Check absolute expiration
    if session.is_expired() {
        let _ = session_store.delete_session(session_id).await;
        return Err(SessionError::Expired);
    }

    // Check inactivity timeout
    if enhanced_config.enabled && session.is_inactive(enhanced_config.inactivity_timeout_secs) {
        let _ = session_store.delete_session(session_id).await;
        tracing::info!(
            session_id = %session_id,
            external_id = %session.external_id,
            last_activity = ?session.last_activity,
            timeout_secs = enhanced_config.inactivity_timeout_secs,
            "Session invalidated due to inactivity"
        );
        return Err(SessionError::Expired);
    }

    // Update last_activity if enhanced sessions enabled, with rate limiting
    if enhanced_config.enabled {
        let now = Utc::now();
        let should_update = match session.last_activity {
            Some(last) => {
                // Only update if activity_update_interval_secs has passed
                let elapsed_secs = (now - last).num_seconds();
                elapsed_secs >= enhanced_config.activity_update_interval_secs as i64
            }
            // Always update if last_activity is not set
            None => true,
        };

        if should_update {
            session.last_activity = Some(now);
            if let Err(e) = session_store.update_session(session.clone()).await {
                // Non-fatal: log but don't fail the request
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to update session last_activity"
                );
            }
        }
    }

    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_session_store() {
        let store = MemorySessionStore::new();

        let session = OidcSession {
            id: Uuid::new_v4(),
            external_id: "user@example.com".to_string(),
            email: Some("user@example.com".to_string()),
            name: Some("Test User".to_string()),
            org: None,
            groups: vec![],
            roles: vec![],
            access_token: Some("token".to_string()),
            refresh_token: None,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            token_expires_at: None,
            sso_org_id: None,
            session_index: None,
            device: None,
            last_activity: None,
        };

        let id = session.id;

        // Create session
        store.create_session(session.clone()).await.unwrap();

        // Get session
        let retrieved = store.get_session(id).await.unwrap().unwrap();
        assert_eq!(retrieved.external_id, "user@example.com");

        // Delete session
        store.delete_session(id).await.unwrap();
        assert!(store.get_session(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_memory_auth_state() {
        let store = MemorySessionStore::new();

        let state = AuthorizationState {
            state: "test-state".to_string(),
            nonce: "test-nonce".to_string(),
            code_verifier: "verifier".to_string(),
            return_to: Some("/dashboard".to_string()),
            org_id: None,
            created_at: Utc::now(),
        };

        // Store auth state
        store.store_auth_state(state.clone()).await.unwrap();

        // Take auth state (should remove it)
        let retrieved = store.take_auth_state("test-state").await.unwrap().unwrap();
        assert_eq!(retrieved.code_verifier, "verifier");

        // Should be gone now
        assert!(store.take_auth_state("test-state").await.unwrap().is_none());
    }

    #[test]
    fn test_session_expired() {
        let session = OidcSession {
            id: Uuid::new_v4(),
            external_id: "user@example.com".to_string(),
            email: None,
            name: None,
            org: None,
            groups: vec![],
            roles: vec![],
            access_token: None,
            refresh_token: None,
            created_at: Utc::now() - chrono::Duration::hours(2),
            expires_at: Utc::now() - chrono::Duration::hours(1),
            token_expires_at: None,
            sso_org_id: None,
            session_index: None,
            device: None,
            last_activity: None,
        };

        assert!(session.is_expired());
    }

    #[test]
    fn test_auth_state_expired() {
        let state = AuthorizationState {
            state: "test".to_string(),
            nonce: "test-nonce".to_string(),
            code_verifier: "verifier".to_string(),
            return_to: None,
            org_id: None,
            created_at: Utc::now() - chrono::Duration::minutes(15),
        };

        assert!(state.is_expired());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 2: Inactivity Timeout Tests
    // ─────────────────────────────────────────────────────────────────────────

    fn create_test_session(external_id: &str, last_activity: Option<DateTime<Utc>>) -> OidcSession {
        OidcSession {
            id: Uuid::new_v4(),
            external_id: external_id.to_string(),
            email: Some(format!("{}@example.com", external_id)),
            name: Some("Test User".to_string()),
            org: None,
            groups: vec![],
            roles: vec![],
            access_token: None,
            refresh_token: None,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            token_expires_at: None,
            sso_org_id: None,
            session_index: None,
            device: None,
            last_activity,
        }
    }

    #[test]
    fn test_is_inactive_with_timeout() {
        // Session was last active 10 minutes ago
        let last_activity = Utc::now() - chrono::Duration::minutes(10);
        let session = create_test_session("user1", Some(last_activity));

        // 5 minute timeout -> should be inactive
        assert!(session.is_inactive(300));

        // 15 minute timeout -> should NOT be inactive
        assert!(!session.is_inactive(900));
    }

    #[test]
    fn test_is_inactive_zero_timeout() {
        // Even with old last_activity, zero timeout means disabled
        let last_activity = Utc::now() - chrono::Duration::hours(1);
        let session = create_test_session("user1", Some(last_activity));

        // Zero timeout = disabled, should return false
        assert!(!session.is_inactive(0));
    }

    #[test]
    fn test_is_inactive_no_last_activity() {
        // No last_activity recorded -> should return false
        let session = create_test_session("user1", None);

        // Even with a short timeout, no last_activity = not inactive
        assert!(!session.is_inactive(60));
    }

    #[test]
    fn test_is_inactive_recent_activity() {
        // Recent activity (just now)
        let session = create_test_session("user1", Some(Utc::now()));

        // Should not be inactive with any reasonable timeout
        assert!(!session.is_inactive(300));
        assert!(!session.is_inactive(60));
    }

    #[test]
    fn test_is_inactive_boundary() {
        // Activity exactly at the boundary
        let timeout_secs = 300u64; // 5 minutes
        let last_activity = Utc::now() - chrono::Duration::seconds(timeout_secs as i64);
        let session = create_test_session("user1", Some(last_activity));

        // At exactly the timeout boundary, should be considered inactive
        // (using >= comparison)
        assert!(session.is_inactive(timeout_secs));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 2: Session Limit Enforcement Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_enforce_session_limit_no_op_when_disabled() {
        let store = MemorySessionStore::new();

        // MemorySessionStore has enhanced disabled by default
        assert!(!store.is_enhanced_enabled());

        // Should return 0 evicted (no-op)
        let evicted = enforce_session_limit(&store, "user1", 3).await.unwrap();
        assert_eq!(evicted, 0);
    }

    #[tokio::test]
    async fn test_enforce_session_limit_no_op_when_unlimited() {
        let store = MemorySessionStore::new();

        // max_sessions = 0 means unlimited
        let evicted = enforce_session_limit(&store, "user1", 0).await.unwrap();
        assert_eq!(evicted, 0);
    }

    // Note: Full eviction tests require CacheSessionStore with enhanced = true,
    // which needs a Cache implementation. These would be integration tests.
}
