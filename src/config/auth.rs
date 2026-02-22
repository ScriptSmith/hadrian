use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::ConfigError;

/// Authentication and authorization configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AuthConfig {
    /// Gateway (data-plane) authentication configuration for `/v1/*` endpoints.
    #[serde(default)]
    pub gateway: GatewayAuthConfig,

    /// Admin (control-plane) authentication configuration for `/admin/*` endpoints and the web UI.
    #[serde(default)]
    pub admin: Option<AdminAuthConfig>,

    /// Authorization (RBAC) configuration.
    #[serde(default)]
    pub rbac: RbacConfig,

    /// Bootstrap admin configuration.
    /// Used to create the initial admin user/org on first run.
    #[serde(default)]
    pub bootstrap: Option<BootstrapConfig>,

    /// Emergency access configuration.
    /// Provides break-glass admin access when SSO is unavailable.
    #[serde(default)]
    pub emergency: Option<EmergencyAccessConfig>,
}

impl AuthConfig {
    pub fn validate(&mut self) -> Result<(), ConfigError> {
        self.gateway.validate()?;
        // Normalize Some(AdminAuthConfig::None) → None so that is_some()/is_none()
        // checks throughout the codebase correctly treat "type = none" as disabled.
        if matches!(&self.admin, Some(AdminAuthConfig::None)) {
            self.admin = None;
        }
        if let Some(admin) = &mut self.admin {
            admin.validate()?;
        }
        self.rbac.validate()?;
        if let Some(emergency) = &self.emergency {
            emergency.validate()?;
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RBAC Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Role-based access control configuration.
///
/// Roles come from the IdP (JWT claims), and policies are defined here
/// with CEL conditions for fine-grained access control.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RbacConfig {
    /// Whether RBAC is enabled for admin endpoints. If false, all admin requests are allowed.
    #[serde(default)]
    pub enabled: bool,

    /// Default effect when no policy matches. Defaults to "deny".
    #[serde(default = "default_deny")]
    pub default_effect: PolicyEffect,

    /// JWT claim containing user roles (e.g., "roles", "groups", "permissions").
    #[serde(default = "default_role_claim")]
    pub role_claim: String,

    /// JWT claim containing organization IDs the user belongs to.
    /// If not set, org membership must be determined from the database.
    #[serde(default)]
    pub org_claim: Option<String>,

    /// JWT claim containing team IDs the user belongs to.
    #[serde(default)]
    pub team_claim: Option<String>,

    /// JWT claim containing project IDs the user belongs to.
    #[serde(default)]
    pub project_claim: Option<String>,

    /// Map IdP role names to internal role names.
    /// Useful when IdP uses different naming conventions.
    #[serde(default)]
    pub role_mapping: HashMap<String, String>,

    /// Authorization policies evaluated using CEL.
    #[serde(default)]
    pub policies: Vec<PolicyConfig>,

    /// Audit logging configuration for authorization decisions.
    #[serde(default)]
    pub audit: AuthzAuditConfig,

    /// Gateway endpoint authorization configuration.
    /// Controls authorization for `/v1/*` endpoints (chat completions, embeddings, etc.).
    #[serde(default)]
    pub gateway: GatewayRbacConfig,

    /// Maximum allowed length of a CEL expression in bytes.
    ///
    /// Limits the size of CEL expressions in both system and organization policies
    /// to prevent excessively complex expressions that could cause performance issues
    /// during evaluation.
    ///
    /// Default: 4096 bytes. Set to 0 to disable the limit.
    #[serde(default = "default_max_expression_length")]
    pub max_expression_length: usize,

    /// Behavior when a CEL policy condition fails to evaluate at runtime.
    ///
    /// Even though policies are validated at creation time, runtime errors can occur
    /// due to unexpected data shapes (e.g., null values, type mismatches).
    ///
    /// - `true` (default): Deny the request on evaluation error (fail-closed).
    ///   This is the secure option - errors don't create security holes.
    /// - `false`: Skip the erroring policy and continue to the next one (fail-open).
    ///   Use only if availability is more important than security.
    ///
    /// Errors are always logged regardless of this setting.
    #[serde(default = "default_true")]
    pub fail_on_evaluation_error: bool,

    /// How often to check Redis for policy version changes (milliseconds).
    ///
    /// In multi-node deployments, each node maintains a local cache of compiled
    /// RBAC policies. This TTL controls how often nodes check Redis for version
    /// changes triggered by other nodes.
    ///
    /// - Lower values: Faster policy propagation, more Redis round-trips
    /// - Higher values: Slower policy propagation, fewer Redis operations
    /// - Set to 0: Check Redis on every authorization request (not recommended)
    ///
    /// Default: 1000 (1 second). This provides a good balance between
    /// propagation speed and Redis load.
    ///
    /// Only applies when Redis cache is configured. With in-memory cache only,
    /// policies are refreshed immediately on the node that made the change.
    #[serde(default = "default_policy_cache_ttl_ms")]
    pub policy_cache_ttl_ms: u64,

    /// Load org policies lazily on first request instead of at startup.
    ///
    /// - `false` (default): Load all org policies at startup (eager loading).
    ///   Good for smaller deployments where startup time isn't critical.
    /// - `true`: Load policies on-demand when an org is first accessed.
    ///   Recommended for large deployments with many organizations.
    ///
    /// Lazy loading eliminates startup memory spikes and reduces initial load time,
    /// but the first request for each org may be slightly slower as policies
    /// are loaded from the database.
    #[serde(default)]
    pub lazy_load_policies: bool,

    /// Maximum number of organizations to keep in the policy cache.
    ///
    /// When the cache exceeds this limit, the least recently used (LRU)
    /// organizations are evicted to make room for new ones.
    ///
    /// - 0 (default): No limit, cache grows unbounded
    /// - >0: Enforce LRU eviction when cache size exceeds this value
    ///
    /// Setting a limit is recommended for large deployments to bound memory usage.
    /// Evicted orgs will have their policies reloaded from the database on next access.
    #[serde(default)]
    pub max_cached_orgs: usize,

    /// Number of organizations to evict when the cache is full.
    ///
    /// When `max_cached_orgs` is reached, this many least-recently-used
    /// organizations are evicted in a single batch to avoid frequent evictions.
    ///
    /// Default: 100. Higher values reduce eviction frequency but may cause
    /// more cache misses after eviction.
    #[serde(default = "default_policy_eviction_batch_size")]
    pub policy_eviction_batch_size: usize,
}

fn default_max_expression_length() -> usize {
    4096
}

fn default_policy_cache_ttl_ms() -> u64 {
    1000 // 1 second
}

fn default_policy_eviction_batch_size() -> usize {
    100
}

/// Gateway endpoint authorization configuration.
///
/// Controls policy-based authorization for `/v1/*` gateway endpoints.
/// This is separate from admin RBAC to allow independent rollout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GatewayRbacConfig {
    /// Whether gateway authorization is enabled.
    /// When false, gateway endpoints only check authentication (API key validity).
    /// When true, policies are evaluated for model access, token limits, etc.
    #[serde(default)]
    pub enabled: bool,

    /// Default effect for gateway endpoints when no policy matches.
    /// Defaults to "allow" (fail-open).
    /// Set to "deny" for stricter security (fail-closed).
    #[serde(default = "default_allow")]
    pub default_effect: PolicyEffect,
}

impl Default for GatewayRbacConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_effect: PolicyEffect::Allow,
        }
    }
}

/// Configuration for authorization decision audit logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AuthzAuditConfig {
    /// Log allowed authorization decisions.
    /// Defaults to false (only denied decisions are logged).
    #[serde(default)]
    pub log_allowed: bool,

    /// Log denied authorization decisions.
    /// Defaults to true for security monitoring.
    #[serde(default = "default_true")]
    pub log_denied: bool,
}

impl Default for AuthzAuditConfig {
    fn default() -> Self {
        Self {
            log_allowed: false,
            log_denied: true,
        }
    }
}

impl RbacConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        for (i, policy) in self.policies.iter().enumerate() {
            policy.validate().map_err(|e| {
                ConfigError::Validation(format!("Policy {} ({}): {}", i, policy.name, e))
            })?;

            // Validate expression length
            if self.max_expression_length > 0 && policy.condition.len() > self.max_expression_length
            {
                return Err(ConfigError::Validation(format!(
                    "Policy {} ({}): CEL expression length ({} bytes) exceeds maximum ({} bytes)",
                    i,
                    policy.name,
                    policy.condition.len(),
                    self.max_expression_length
                )));
            }
        }
        Ok(())
    }

    /// Map a role from IdP naming to internal naming.
    pub fn map_role(&self, role: &str) -> String {
        self.role_mapping
            .get(role)
            .cloned()
            .unwrap_or_else(|| role.to_string())
    }

    /// Map multiple roles.
    pub fn map_roles(&self, roles: &[String]) -> Vec<String> {
        roles.iter().map(|r| self.map_role(r)).collect()
    }
}

fn default_deny() -> PolicyEffect {
    PolicyEffect::Deny
}

fn default_allow() -> PolicyEffect {
    PolicyEffect::Allow
}

fn default_role_claim() -> String {
    "roles".to_string()
}

/// Policy effect (allow or deny).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    Allow,
    #[default]
    Deny,
}

impl PolicyEffect {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
}

/// A policy for fine-grained access control.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PolicyConfig {
    /// Unique name for this policy.
    pub name: String,

    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,

    /// Resource type this policy applies to (e.g., "organization", "project", "*").
    #[serde(default = "default_wildcard")]
    pub resource: String,

    /// Action this policy applies to (e.g., "read", "create", "*").
    #[serde(default = "default_wildcard")]
    pub action: String,

    /// CEL expression that must evaluate to true for the policy to apply.
    ///
    /// Available variables for all endpoints:
    /// - `subject.user_id`: User's internal ID
    /// - `subject.external_id`: User's IdP ID
    /// - `subject.email`: User's email
    /// - `subject.roles`: List of role names
    /// - `subject.org_ids`: List of organization IDs the user belongs to
    /// - `subject.team_ids`: List of team IDs the user belongs to
    /// - `subject.project_ids`: List of project IDs the user belongs to
    /// - `context.resource_type`: Resource being accessed (e.g., "model", "chat", "team")
    /// - `context.action`: Action being performed (e.g., "use", "read", "create")
    /// - `context.org_id`: Organization ID scope (if applicable)
    /// - `context.team_id`: Team ID scope (if applicable)
    /// - `context.project_id`: Project ID scope (if applicable)
    /// - `context.resource_id`: Specific resource ID being accessed
    ///
    /// Additional variables for API endpoints (`/v1/*`):
    /// - `context.model`: Model being requested (e.g., "gpt-4o", "claude-3-opus")
    /// - `context.request.max_tokens`: Maximum tokens requested
    /// - `context.request.messages_count`: Number of messages in conversation
    /// - `context.request.has_tools`: Whether tools/functions are being used
    /// - `context.request.has_file_search`: Whether file_search tool is present
    /// - `context.request.stream`: Whether streaming is requested
    /// - `context.now.hour`: Current hour (0-23)
    /// - `context.now.day_of_week`: Day of week (1=Monday, 7=Sunday)
    /// - `context.now.timestamp`: Unix timestamp
    pub condition: String,

    /// Whether this policy allows or denies the action.
    pub effect: PolicyEffect,

    /// Priority for evaluation order (higher = evaluated first).
    /// At the same priority, deny policies are evaluated before allow.
    #[serde(default)]
    pub priority: i32,
}

impl PolicyConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Policy name cannot be empty".to_string());
        }
        if self.condition.is_empty() {
            return Err("Policy condition cannot be empty".to_string());
        }
        // CEL expression validation happens at runtime when policies are compiled
        Ok(())
    }
}

fn default_wildcard() -> String {
    "*".to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Gateway Authentication
// ─────────────────────────────────────────────────────────────────────────────

/// Gateway (data-plane) authentication configuration for `/v1/*` endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum GatewayAuthConfig {
    /// No authentication. Any request is allowed.
    /// Only suitable for local development.
    #[default]
    None,

    /// API key authentication.
    /// Keys are stored in the database and validated on each request.
    ApiKey(ApiKeyAuthConfig),

    /// JWT authentication.
    /// Tokens are validated against a JWKS endpoint.
    Jwt(JwtAuthConfig),

    /// Support both API key and JWT authentication.
    /// The gateway tries API key first, then JWT.
    Multi(MultiAuthConfig),
}

impl GatewayAuthConfig {
    pub fn is_enabled(&self) -> bool {
        !matches!(self, GatewayAuthConfig::None)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        match self {
            GatewayAuthConfig::None => Ok(()),
            GatewayAuthConfig::ApiKey(c) => c.validate(),
            GatewayAuthConfig::Jwt(c) => c.validate(),
            GatewayAuthConfig::Multi(c) => c.validate(),
        }
    }
}

/// API key authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApiKeyAuthConfig {
    /// Header name for the API key.
    #[serde(default = "default_api_key_header")]
    pub header_name: String,

    /// Prefix for validating API keys (e.g., "gw_" to accept any gw_* key).
    #[serde(default = "default_api_key_prefix")]
    pub key_prefix: String,

    /// Prefix for generating new API keys (e.g., "gw_live_" for production).
    /// If not specified, uses key_prefix with "_live" appended if it doesn't end with "_".
    #[serde(default)]
    pub generation_prefix: Option<String>,

    /// Hash algorithm for storing keys.
    #[serde(default)]
    pub hash_algorithm: HashAlgorithm,

    /// Cache API key lookups for this many seconds.
    /// Set to 0 to disable caching (every request hits the database).
    #[serde(default = "default_key_cache_ttl")]
    pub cache_ttl_secs: u64,
}

impl ApiKeyAuthConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.header_name.is_empty() {
            return Err(ConfigError::Validation(
                "API key header name cannot be empty".into(),
            ));
        }
        Ok(())
    }

    /// Get the prefix to use when generating new API keys.
    pub fn generation_prefix(&self) -> String {
        if let Some(ref prefix) = self.generation_prefix {
            prefix.clone()
        } else if self.key_prefix.ends_with('_') {
            format!("{}live_", self.key_prefix)
        } else {
            format!("{}_live_", self.key_prefix)
        }
    }
}

fn default_api_key_header() -> String {
    "X-API-Key".to_string()
}

fn default_api_key_prefix() -> String {
    "gw_".to_string()
}

fn default_key_cache_ttl() -> u64 {
    60 // 1 minute
}

/// Hash algorithm for API keys.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum HashAlgorithm {
    /// SHA-256 (fast, suitable for high-entropy keys).
    #[default]
    Sha256,
    /// Argon2id (slow, more secure if keys might be low-entropy).
    Argon2,
}

/// JWT authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct JwtAuthConfig {
    /// Expected issuer (iss claim).
    pub issuer: String,

    /// Expected audience (aud claim). Can be a single value or a list.
    pub audience: OneOrMany<String>,

    /// JWKS URL for fetching public keys.
    pub jwks_url: String,

    /// How often to refresh the JWKS in seconds.
    #[serde(default = "default_jwks_refresh")]
    pub jwks_refresh_secs: u64,

    /// Claim to use as the identity ID.
    #[serde(default = "default_identity_claim")]
    pub identity_claim: String,

    /// Claim to use as the organization ID (optional).
    #[serde(default)]
    pub org_claim: Option<String>,

    /// Additional claims to extract and include in the identity.
    #[serde(default)]
    pub additional_claims: Vec<String>,

    /// Allow expired tokens (for testing only!).
    #[serde(default)]
    pub allow_expired: bool,

    /// Allowed JWT signing algorithms.
    /// If not specified, defaults to secure asymmetric algorithms (RS256, RS384, RS512, ES256, ES384).
    /// SECURITY: Always specify this explicitly to prevent algorithm confusion attacks.
    #[serde(default = "default_allowed_algorithms")]
    pub allowed_algorithms: Vec<JwtAlgorithm>,
}

impl JwtAuthConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.issuer.is_empty() {
            return Err(ConfigError::Validation("JWT issuer cannot be empty".into()));
        }
        if self.jwks_url.is_empty() {
            return Err(ConfigError::Validation("JWKS URL cannot be empty".into()));
        }
        if self.allowed_algorithms.is_empty() {
            return Err(ConfigError::Validation(
                "At least one JWT algorithm must be allowed".into(),
            ));
        }
        // Check for insecure algorithms
        for alg in &self.allowed_algorithms {
            if matches!(
                alg,
                JwtAlgorithm::HS256 | JwtAlgorithm::HS384 | JwtAlgorithm::HS512
            ) {
                tracing::warn!(
                    algorithm = ?alg,
                    "HMAC algorithms (HS256/HS384/HS512) are less secure for public key scenarios. \
                     Consider using asymmetric algorithms (RS256, ES256) instead."
                );
            }
        }
        Ok(())
    }
}

/// JWT signing algorithm.
/// SECURITY: Asymmetric algorithms (RS*, ES*) are strongly recommended.
/// HMAC algorithms (HS*) should only be used when you control both signing and verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum JwtAlgorithm {
    /// HMAC with SHA-256 (symmetric, use with caution)
    HS256,
    /// HMAC with SHA-384 (symmetric, use with caution)
    HS384,
    /// HMAC with SHA-512 (symmetric, use with caution)
    HS512,
    /// RSA with SHA-256 (asymmetric, recommended)
    RS256,
    /// RSA with SHA-384 (asymmetric, recommended)
    RS384,
    /// RSA with SHA-512 (asymmetric, recommended)
    RS512,
    /// ECDSA with P-256 and SHA-256 (asymmetric, recommended)
    ES256,
    /// ECDSA with P-384 and SHA-384 (asymmetric, recommended)
    ES384,
    /// RSA-PSS with SHA-256
    PS256,
    /// RSA-PSS with SHA-384
    PS384,
    /// RSA-PSS with SHA-512
    PS512,
    /// EdDSA (Ed25519)
    EdDSA,
}

impl JwtAlgorithm {
    /// Convert to jsonwebtoken Algorithm.
    pub fn to_jwt_algorithm(self) -> jsonwebtoken::Algorithm {
        match self {
            JwtAlgorithm::HS256 => jsonwebtoken::Algorithm::HS256,
            JwtAlgorithm::HS384 => jsonwebtoken::Algorithm::HS384,
            JwtAlgorithm::HS512 => jsonwebtoken::Algorithm::HS512,
            JwtAlgorithm::RS256 => jsonwebtoken::Algorithm::RS256,
            JwtAlgorithm::RS384 => jsonwebtoken::Algorithm::RS384,
            JwtAlgorithm::RS512 => jsonwebtoken::Algorithm::RS512,
            JwtAlgorithm::ES256 => jsonwebtoken::Algorithm::ES256,
            JwtAlgorithm::ES384 => jsonwebtoken::Algorithm::ES384,
            JwtAlgorithm::PS256 => jsonwebtoken::Algorithm::PS256,
            JwtAlgorithm::PS384 => jsonwebtoken::Algorithm::PS384,
            JwtAlgorithm::PS512 => jsonwebtoken::Algorithm::PS512,
            JwtAlgorithm::EdDSA => jsonwebtoken::Algorithm::EdDSA,
        }
    }

    /// Check if this algorithm matches a jsonwebtoken Algorithm.
    pub fn matches(self, alg: jsonwebtoken::Algorithm) -> bool {
        self.to_jwt_algorithm() == alg
    }
}

fn default_allowed_algorithms() -> Vec<JwtAlgorithm> {
    // Default to secure asymmetric algorithms only
    vec![
        JwtAlgorithm::RS256,
        JwtAlgorithm::RS384,
        JwtAlgorithm::RS512,
        JwtAlgorithm::ES256,
        JwtAlgorithm::ES384,
    ]
}

fn default_jwks_refresh() -> u64 {
    3600 // 1 hour
}

fn default_identity_claim() -> String {
    "sub".to_string()
}

/// Multiple authentication methods configuration.
///
/// When using multi-auth, the gateway uses **format-based detection** to determine
/// which authentication method to use:
///
/// - Tokens in the `Authorization: Bearer` header starting with the configured
///   API key prefix (default: `gw_`) are validated as API keys
/// - All other Bearer tokens are validated as JWTs
/// - The `X-API-Key` header is always validated as an API key
///
/// **Important:** Providing both `X-API-Key` and `Authorization` headers simultaneously
/// results in a 400 error (ambiguous credentials). Choose one authentication method per request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MultiAuthConfig {
    /// API key configuration.
    pub api_key: ApiKeyAuthConfig,

    /// JWT configuration.
    pub jwt: JwtAuthConfig,
}

impl MultiAuthConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        self.api_key.validate()?;
        self.jwt.validate()?;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Admin Authentication
// ─────────────────────────────────────────────────────────────────────────────

/// Admin (control-plane) authentication configuration for `/admin/*` endpoints and the web UI.
///
/// **Note:** Global OIDC configuration has been removed. For SSO authentication,
/// configure per-organization SSO connections via the admin API or database.
/// Users authenticate by visiting `/auth/login?org=<org_slug>` which redirects
/// to the organization's configured IdP.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum AdminAuthConfig {
    /// No authentication required for UI access.
    /// Useful for local development or internal deployments.
    #[default]
    None,

    /// Reverse proxy authentication.
    /// Identity is extracted from headers set by an authenticating reverse proxy
    /// (Cloudflare Access, oauth2-proxy, Tailscale, Authelia, etc.)
    ///
    /// **Security:** Headers are only trusted when the request originates from
    /// a trusted proxy IP (configured via `server.trusted_proxies`).
    ProxyAuth(Box<ProxyAuthConfig>),

    /// Session-only authentication (for per-org SSO).
    /// Configures session management without global OIDC. Users authenticate
    /// through their organization's SSO connection (configured in the database).
    ///
    /// Use this when:
    /// - Different organizations use different IdPs
    /// - You want SSO configuration to be dynamic (via admin API)
    /// - You're migrating from global OIDC to per-org SSO
    #[cfg(feature = "sso")]
    Session(SessionConfig),
}

impl AdminAuthConfig {
    fn validate(&mut self) -> Result<(), ConfigError> {
        match self {
            AdminAuthConfig::None => Ok(()),
            AdminAuthConfig::ProxyAuth(c) => c.validate(),
            #[cfg(feature = "sso")]
            AdminAuthConfig::Session(c) => c.validate(),
        }
    }
}

/// Reverse proxy authentication configuration.
///
/// This auth method trusts identity headers set by an authenticating reverse proxy.
/// Common proxies that work with this include:
/// - Cloudflare Access (Cf-Access-Authenticated-User-Email)
/// - oauth2-proxy (X-Forwarded-User, X-Forwarded-Email)
/// - Tailscale (Tailscale-User-Login)
/// - Authelia, Authentik, Keycloak Gatekeeper, etc.
///
/// **Security:** Configure `server.trusted_proxies` to ensure headers are only
/// trusted from known proxy IPs. Without this, attackers can spoof identity headers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProxyAuthConfig {
    /// Header containing the authenticated user's identity.
    pub identity_header: String,

    /// Header containing the user's email (if different from identity).
    #[serde(default)]
    pub email_header: Option<String>,

    /// Header containing the user's name.
    #[serde(default)]
    pub name_header: Option<String>,

    /// Header containing groups/roles (comma-separated or JSON array).
    #[serde(default)]
    pub groups_header: Option<String>,

    /// Optional: JWT assertion header for additional validation.
    /// If set, the JWT is validated and claims are extracted.
    #[serde(default)]
    pub jwt_assertion: Option<ProxyAuthJwtConfig>,

    /// Require all requests to have identity headers.
    /// If false, unauthenticated requests are allowed to public endpoints.
    #[serde(default = "default_true")]
    pub require_identity: bool,
}

impl ProxyAuthConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.identity_header.is_empty() {
            return Err(ConfigError::Validation(
                "Proxy auth identity header cannot be empty".into(),
            ));
        }
        Ok(())
    }
}

fn default_true() -> bool {
    true
}

/// JWT assertion configuration for proxy auth.
/// Used when the proxy also provides a signed JWT for additional verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProxyAuthJwtConfig {
    /// Header containing the JWT.
    pub header: String,

    /// JWKS URL for validating the JWT.
    pub jwks_url: String,

    /// Expected issuer.
    pub issuer: String,

    /// Expected audience.
    pub audience: OneOrMany<String>,
}

/// OIDC authentication configuration.
#[cfg(feature = "sso")]
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OidcAuthConfig {
    /// OIDC issuer URL. Used for token validation and browser redirects.
    pub issuer: String,

    /// URL to use for OIDC discovery (fetching .well-known/openid-configuration).
    /// If not set, defaults to `issuer`. This is useful in Docker environments
    /// where the backend needs to reach the IdP via an internal URL (e.g., `http://keycloak:8080`)
    /// while the browser uses an external URL (e.g., `http://localhost:8080`).
    #[serde(default)]
    pub discovery_url: Option<String>,

    /// Client ID.
    pub client_id: String,

    /// Client secret.
    pub client_secret: String,

    /// Redirect URI (must be registered with the IdP).
    pub redirect_uri: String,

    /// Scopes to request.
    #[serde(default = "default_oidc_scopes")]
    pub scopes: Vec<String>,

    /// Claim to use as the identity ID.
    #[serde(default = "default_identity_claim")]
    pub identity_claim: String,

    /// Claim to use as the organization ID.
    #[serde(default)]
    pub org_claim: Option<String>,

    /// Claim containing groups/roles.
    #[serde(default)]
    pub groups_claim: Option<String>,

    /// Session cookie configuration.
    #[serde(default)]
    pub session: SessionConfig,

    /// JIT (Just-in-Time) provisioning configuration.
    /// When enabled, users, organizations, and teams are automatically created
    /// in the database on first login based on OIDC claims.
    #[serde(default)]
    pub provisioning: ProvisioningConfig,
}

/// JIT (Just-in-Time) provisioning configuration.
///
/// Controls automatic creation of users, organizations, and teams
/// in the database based on OIDC claims when a user logs in.
///
/// **Important:** JIT provisioning only creates resources on login. By default, it does NOT
/// handle deprovisioning when users are removed from the IdP or groups. Enable
/// `sync_memberships_on_login` to remove memberships that are no longer present in OIDC groups.
#[cfg(feature = "sso")]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProvisioningConfig {
    /// Whether JIT provisioning is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Automatically create users in the database on first login.
    /// The user's external_id, email, and name are populated from OIDC claims.
    #[serde(default)]
    pub create_users: bool,

    /// Organization ID (UUID) or slug to provision users into.
    ///
    /// All users authenticating via this SSO connection are provisioned
    /// into this organization. This works with any IdP (Okta, Azure AD,
    /// Auth0, Google, Keycloak, etc.) regardless of their group claim format.
    ///
    /// The organization must exist in the database before users can be
    /// provisioned into it.
    #[serde(default)]
    pub organization_id: Option<String>,

    /// Default team ID (UUID) or slug within the organization.
    ///
    /// When set along with `organization_id`, users are also added to this team.
    /// This provides a simple way to assign all SSO users to a default team.
    #[serde(default)]
    pub default_team_id: Option<String>,

    /// Default role for users when added to organizations.
    /// Defaults to "member".
    #[serde(default = "default_member_role")]
    pub default_org_role: String,

    /// Default role for users when added to teams.
    /// Defaults to "member".
    #[serde(default = "default_member_role")]
    pub default_team_role: String,

    /// Restrict JIT provisioning to users with emails from these domains.
    /// If empty, all email domains are allowed.
    /// Example: ["acme.com", "example.org"]
    #[serde(default)]
    pub allowed_email_domains: Vec<String>,

    /// Update user attributes (name, email) on subsequent logins if they've changed in the IdP.
    ///
    /// - `false` (default): User attributes are only set on first login. Manual changes
    ///   in the database are preserved.
    /// - `true`: User name and email are updated from IdP claims on every login.
    ///   Manual changes will be overwritten.
    ///
    /// This setting is independent of `sync_memberships_on_login` - you can sync
    /// attributes without syncing memberships, or vice versa.
    #[serde(default)]
    pub sync_attributes_on_login: bool,

    /// Sync org/team memberships on each login based on current provisioning config.
    ///
    /// - `false` (default): Memberships are additive. Users keep any manually-assigned
    ///   memberships even if not in the provisioning config.
    /// - `true`: Memberships are synchronized. On each login, memberships are reconciled
    ///   to match exactly what's configured in `organization_id` and `default_team_id`.
    ///   Memberships not in the config are removed.
    ///
    /// **Warning:** Enabling this will remove any manually-assigned org/team memberships
    /// that aren't part of the SSO provisioning config. This is useful for ensuring
    /// consistent access control but can be disruptive if users have additional
    /// memberships assigned through other means.
    ///
    /// This setting is independent of `sync_attributes_on_login`.
    #[serde(default)]
    pub sync_memberships_on_login: bool,
}

#[cfg(feature = "sso")]
fn default_member_role() -> String {
    "member".to_string()
}

#[cfg(feature = "sso")]
impl ProvisioningConfig {
    /// Validate the provisioning configuration.
    pub fn validate(&mut self) -> Result<(), ConfigError> {
        // Validate organization_id if set
        if let Some(org_id) = &self.organization_id
            && org_id.trim().is_empty()
        {
            return Err(ConfigError::Validation(
                "provisioning.organization_id cannot be empty".into(),
            ));
        }

        // default_team_id requires organization_id
        if self.default_team_id.is_some() && self.organization_id.is_none() {
            return Err(ConfigError::Validation(
                "provisioning.default_team_id requires organization_id to be set".into(),
            ));
        }

        // Validate default_team_id if set
        if let Some(team_id) = &self.default_team_id
            && team_id.trim().is_empty()
        {
            return Err(ConfigError::Validation(
                "provisioning.default_team_id cannot be empty".into(),
            ));
        }

        Ok(())
    }
}

#[cfg(feature = "sso")]
impl std::fmt::Debug for OidcAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcAuthConfig")
            .field("issuer", &self.issuer)
            .field("discovery_url", &self.discovery_url)
            .field("client_id", &self.client_id)
            .field("client_secret", &"****")
            .field("redirect_uri", &self.redirect_uri)
            .field("scopes", &self.scopes)
            .field("identity_claim", &self.identity_claim)
            .field("org_claim", &self.org_claim)
            .field("groups_claim", &self.groups_claim)
            .field("session", &self.session)
            .field("provisioning", &self.provisioning)
            .finish()
    }
}

#[cfg(feature = "sso")]
impl OidcAuthConfig {
    #[allow(dead_code)] // Used for programmatic OIDC config validation, not from config file
    fn validate(&mut self) -> Result<(), ConfigError> {
        if self.issuer.is_empty() {
            return Err(ConfigError::Validation(
                "OIDC issuer cannot be empty".into(),
            ));
        }
        if self.client_id.is_empty() {
            return Err(ConfigError::Validation(
                "OIDC client_id cannot be empty".into(),
            ));
        }
        if self.client_secret.is_empty() {
            return Err(ConfigError::Validation(
                "OIDC client_secret cannot be empty".into(),
            ));
        }
        // Validate and compile provisioning config
        self.provisioning.validate()?;
        Ok(())
    }

    /// Get the base URL to use for OIDC discovery.
    /// Returns `discovery_url` if set, otherwise falls back to `issuer`.
    pub fn discovery_base_url(&self) -> &str {
        self.discovery_url.as_deref().unwrap_or(&self.issuer)
    }
}

#[cfg(feature = "sso")]
fn default_oidc_scopes() -> Vec<String> {
    vec!["openid".into(), "email".into(), "profile".into()]
}

/// Session cookie configuration.
#[cfg(feature = "sso")]
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SessionConfig {
    /// Cookie name.
    #[serde(default = "default_session_cookie")]
    pub cookie_name: String,

    /// Session duration in seconds.
    #[serde(default = "default_session_duration")]
    pub duration_secs: u64,

    /// Secure cookie (HTTPS only).
    #[serde(default = "default_true")]
    pub secure: bool,

    /// SameSite cookie attribute.
    #[serde(default)]
    pub same_site: SameSite,

    /// Secret key for signing session cookies.
    /// If not provided, a random key is generated on startup
    /// (sessions won't survive restarts).
    #[serde(default)]
    pub secret: Option<String>,

    /// Enhanced session management configuration.
    /// Enables session listing, device tracking, and user-to-sessions indexing.
    #[serde(default)]
    pub enhanced: EnhancedSessionConfig,
}

/// Enhanced session management configuration.
///
/// Enables opt-in features for enterprise session management including
/// session listing, device tracking, and user-to-sessions indexing.
#[cfg(feature = "sso")]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EnhancedSessionConfig {
    /// Master toggle for enhanced session features.
    /// When enabled, sessions are indexed by user ID for listing and management.
    #[serde(default)]
    pub enabled: bool,

    /// Track device information (user agent, IP address) with sessions.
    /// Requires `enabled = true`.
    #[serde(default)]
    pub track_devices: bool,

    /// Maximum concurrent sessions per user. 0 = unlimited.
    /// When exceeded, oldest sessions are automatically invalidated.
    /// Requires `enabled = true`. Enforcement is in Phase 2.
    #[serde(default)]
    pub max_concurrent_sessions: u32,

    /// Inactivity timeout in seconds. 0 = disabled.
    /// Sessions inactive for this duration are automatically invalidated.
    /// Requires `enabled = true`.
    #[serde(default)]
    pub inactivity_timeout_secs: u64,

    /// Minimum interval between last_activity updates in seconds.
    /// Reduces write load by only updating last_activity if the previous
    /// update was more than this many seconds ago.
    /// Defaults to 60 seconds. Set to 0 to update on every request.
    #[serde(default = "default_activity_update_interval")]
    pub activity_update_interval_secs: u64,
}

#[cfg(feature = "sso")]
fn default_activity_update_interval() -> u64 {
    60 // 1 minute
}

#[cfg(feature = "sso")]
impl std::fmt::Debug for SessionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionConfig")
            .field("cookie_name", &self.cookie_name)
            .field("duration_secs", &self.duration_secs)
            .field("secure", &self.secure)
            .field("same_site", &self.same_site)
            .field("secret", &self.secret.as_ref().map(|_| "****"))
            .field("enhanced", &self.enhanced)
            .finish()
    }
}

#[cfg(feature = "sso")]
impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            cookie_name: default_session_cookie(),
            duration_secs: default_session_duration(),
            secure: true,
            same_site: SameSite::default(),
            secret: None,
            enhanced: EnhancedSessionConfig::default(),
        }
    }
}

#[cfg(feature = "sso")]
impl SessionConfig {
    /// Validate the session configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.cookie_name.is_empty() {
            return Err(ConfigError::Validation(
                "Session cookie name cannot be empty".into(),
            ));
        }
        if self.duration_secs == 0 {
            return Err(ConfigError::Validation(
                "Session duration cannot be zero".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "sso")]
fn default_session_cookie() -> String {
    "__gw_session".to_string()
}

#[cfg(feature = "sso")]
fn default_session_duration() -> u64 {
    86400 * 7 // 7 days
}

#[cfg(feature = "sso")]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SameSite {
    #[default]
    Lax,
    Strict,
    None,
}

// ─────────────────────────────────────────────────────────────────────────────
// Bootstrap
// ─────────────────────────────────────────────────────────────────────────────

/// Bootstrap configuration for initial setup.
///
/// Provides mechanisms for bootstrapping a new deployment:
/// - `api_key`: Pre-shared key for admin API access before first user exists
/// - `auto_verify_domains`: Domains to auto-verify when SSO config is created
/// - `admin_identities`: Identity IDs to grant system admin role
///
/// The bootstrap API key uses a special `_system_bootstrap` role that:
/// - Is only valid when the database has no users (orgs can exist)
/// - Allows creating org + SSO config, then first IdP login disables bootstrap
/// - Cannot be assigned by IdPs (roles starting with `_` are reserved)
/// - Grants full admin access for initial setup via RBAC policy
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BootstrapConfig {
    /// Pre-shared API key for initial setup before first user exists.
    ///
    /// This key provides admin access ONLY when the database has no users.
    /// Organizations can exist - bootstrap remains active until first IdP login.
    ///
    /// Use this for:
    /// - Automated deployments (Terraform, Ansible, etc.)
    /// - E2E testing
    /// - Initial SSO configuration before users can authenticate
    ///
    /// Example: `api_key = "${HADRIAN_BOOTSTRAP_KEY}"`
    #[serde(default)]
    pub api_key: Option<String>,

    /// Domains to automatically verify when SSO config is created.
    ///
    /// When an SSO configuration is created with `allowed_email_domains` that
    /// match entries in this list, those domains are automatically marked as
    /// verified without requiring DNS TXT record verification.
    ///
    /// This is useful for:
    /// - E2E testing where DNS verification is impossible
    /// - Development environments
    /// - Pre-verified enterprise domains
    ///
    /// Example: `auto_verify_domains = ["university.edu", "example.com"]`
    #[serde(default)]
    pub auto_verify_domains: Vec<String>,

    /// Admin identity IDs that should be granted system admin role.
    /// These are the external identity IDs from your IdP.
    #[serde(default)]
    pub admin_identities: Vec<String>,

    /// Initial organization to create.
    #[serde(default)]
    pub initial_org: Option<BootstrapOrg>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BootstrapOrg {
    /// Organization slug (URL-safe identifier).
    pub slug: String,

    /// Organization display name.
    pub name: String,

    /// Identity IDs to add as org admins.
    #[serde(default)]
    pub admin_identities: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Emergency Access
// ─────────────────────────────────────────────────────────────────────────────

/// Emergency access configuration for break-glass admin access.
///
/// Provides a way for designated administrators to access Hadrian when SSO
/// is unavailable due to IdP outages or misconfigurations. This is a critical
/// disaster recovery feature.
///
/// **Security:**
/// - Emergency keys are compared using constant-time comparison
/// - The `_emergency_admin` role cannot be assigned by IdPs (reserved prefix)
/// - All access attempts are logged at WARN level
/// - IP restrictions and rate limiting provide defense in depth
/// - Config-only approach works even if database is corrupted
///
/// **Example:**
/// ```toml
/// [auth.emergency]
/// enabled = true
/// allowed_ips = ["10.0.0.0/8"]  # Optional: restrict to admin network
///
/// [[auth.emergency.accounts]]
/// id = "emergency-admin-1"
/// name = "Primary Emergency Admin"
/// key = "${EMERGENCY_KEY_1}"
/// email = "emergency@company.com"
/// roles = ["_emergency_admin", "super_admin"]
///
/// [auth.emergency.rate_limit]
/// max_attempts = 5
/// window_secs = 900
/// lockout_secs = 3600
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EmergencyAccessConfig {
    /// Whether emergency access is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Emergency admin accounts.
    /// Each account has a unique key for authentication.
    #[serde(default)]
    pub accounts: Vec<EmergencyAccount>,

    /// Rate limiting configuration for emergency access attempts.
    #[serde(default)]
    pub rate_limit: EmergencyRateLimit,

    /// Global IP allowlist for emergency access (CIDR notation).
    /// If specified, emergency access is only allowed from these IPs.
    /// Individual accounts can have additional IP restrictions.
    #[serde(default)]
    pub allowed_ips: Vec<String>,
}

impl EmergencyAccessConfig {
    /// Validate the emergency access configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.enabled {
            return Ok(());
        }

        // Validate that at least one account is configured
        if self.accounts.is_empty() {
            return Err(ConfigError::Validation(
                "Emergency access is enabled but no accounts are configured".into(),
            ));
        }

        // Check for duplicate account IDs
        let mut seen_ids = std::collections::HashSet::new();
        for account in &self.accounts {
            if !seen_ids.insert(&account.id) {
                return Err(ConfigError::Validation(format!(
                    "Duplicate emergency account ID: '{}'",
                    account.id
                )));
            }
        }

        // Validate each account
        for (i, account) in self.accounts.iter().enumerate() {
            account.validate().map_err(|e| {
                ConfigError::Validation(format!("Emergency account {} ({}): {}", i, account.id, e))
            })?;
        }

        // Validate rate limit
        self.rate_limit.validate()?;

        // Validate allowed_ips are valid CIDR
        for cidr in &self.allowed_ips {
            cidr.parse::<ipnet::IpNet>().map_err(|e| {
                ConfigError::Validation(format!(
                    "Invalid CIDR in emergency.allowed_ips '{}': {}",
                    cidr, e
                ))
            })?;
        }

        Ok(())
    }

    /// Check if emergency access is effectively enabled (enabled and has accounts).
    pub fn is_enabled(&self) -> bool {
        self.enabled && !self.accounts.is_empty()
    }

    /// Parse allowed_ips into IpNet for efficient matching.
    pub fn parsed_allowed_ips(&self) -> Vec<ipnet::IpNet> {
        self.allowed_ips
            .iter()
            .filter_map(|cidr| cidr.parse().ok())
            .collect()
    }
}

/// An emergency admin account for break-glass access.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EmergencyAccount {
    /// Unique identifier for this emergency account.
    /// Used in audit logs and rate limiting.
    pub id: String,

    /// Human-readable name for this emergency account.
    pub name: String,

    /// The emergency access key (secret).
    /// Should be stored in a secrets manager and referenced via environment variable.
    /// Example: `key = "${EMERGENCY_KEY_1}"`
    /// Must be at least 32 characters long for security.
    #[serde(skip_serializing)]
    pub key: String,

    /// Email address for audit logging and notifications.
    #[serde(default)]
    pub email: Option<String>,

    /// Roles granted when authenticating with this emergency key.
    /// Should include `_emergency_admin` plus any additional roles needed.
    #[serde(default)]
    pub roles: Vec<String>,

    /// Additional IP restrictions for this specific account (CIDR notation).
    /// These are in addition to the global `allowed_ips`.
    #[serde(default)]
    pub allowed_ips: Vec<String>,
}

/// Minimum length for emergency access keys.
const MIN_EMERGENCY_KEY_LENGTH: usize = 32;

impl EmergencyAccount {
    /// Validate the emergency account configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("Emergency account ID cannot be empty".into());
        }
        if self.key.is_empty() {
            return Err("Emergency account key cannot be empty".into());
        }
        if self.key.len() < MIN_EMERGENCY_KEY_LENGTH {
            return Err(format!(
                "Emergency account key must be at least {} characters (got {})",
                MIN_EMERGENCY_KEY_LENGTH,
                self.key.len()
            ));
        }
        if self.name.is_empty() {
            return Err("Emergency account name cannot be empty".into());
        }
        // Validate per-account allowed_ips
        for cidr in &self.allowed_ips {
            cidr.parse::<ipnet::IpNet>()
                .map_err(|e| format!("Invalid CIDR in allowed_ips '{}': {}", cidr, e))?;
        }
        Ok(())
    }

    /// Parse per-account allowed_ips into IpNet.
    pub fn parsed_allowed_ips(&self) -> Vec<ipnet::IpNet> {
        self.allowed_ips
            .iter()
            .filter_map(|cidr| cidr.parse().ok())
            .collect()
    }
}

impl std::fmt::Debug for EmergencyAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmergencyAccount")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("key", &"****")
            .field("email", &self.email)
            .field("roles", &self.roles)
            .field("allowed_ips", &self.allowed_ips)
            .finish()
    }
}

/// Rate limiting configuration for emergency access attempts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EmergencyRateLimit {
    /// Maximum failed attempts before lockout.
    #[serde(default = "default_emergency_max_attempts")]
    pub max_attempts: u32,

    /// Time window in seconds for counting attempts.
    #[serde(default = "default_emergency_window_secs")]
    pub window_secs: u64,

    /// Lockout duration in seconds after exceeding max_attempts.
    #[serde(default = "default_emergency_lockout_secs")]
    pub lockout_secs: u64,
}

impl Default for EmergencyRateLimit {
    fn default() -> Self {
        Self {
            max_attempts: default_emergency_max_attempts(),
            window_secs: default_emergency_window_secs(),
            lockout_secs: default_emergency_lockout_secs(),
        }
    }
}

impl EmergencyRateLimit {
    /// Validate the rate limit configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_attempts == 0 {
            return Err(ConfigError::Validation(
                "Emergency rate_limit.max_attempts must be > 0".into(),
            ));
        }
        if self.window_secs == 0 {
            return Err(ConfigError::Validation(
                "Emergency rate_limit.window_secs must be > 0".into(),
            ));
        }
        if self.lockout_secs == 0 {
            return Err(ConfigError::Validation(
                "Emergency rate_limit.lockout_secs must be > 0".into(),
            ));
        }
        Ok(())
    }
}

fn default_emergency_max_attempts() -> u32 {
    5
}

fn default_emergency_window_secs() -> u64 {
    900 // 15 minutes
}

fn default_emergency_lockout_secs() -> u64 {
    3600 // 1 hour
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Types
// ─────────────────────────────────────────────────────────────────────────────

/// A value that can be either a single item or a list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    pub fn into_vec(self) -> Vec<T> {
        match self {
            OneOrMany::One(v) => vec![v],
            OneOrMany::Many(v) => v,
        }
    }
}

impl<T: Clone> OneOrMany<T> {
    pub fn to_vec(&self) -> Vec<T> {
        match self {
            OneOrMany::One(v) => vec![v.clone()],
            OneOrMany::Many(v) => v.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "sso")]
    #[test]
    fn test_oidc_auth_config_debug_redacts_client_secret() {
        let config = OidcAuthConfig {
            issuer: "https://auth.example.com".to_string(),
            discovery_url: None,
            client_id: "my-client-id".to_string(),
            client_secret: "super-secret-client-secret-xyz".to_string(),
            redirect_uri: "https://app.example.com/callback".to_string(),
            scopes: vec!["openid".to_string()],
            identity_claim: "sub".to_string(),
            org_claim: None,
            groups_claim: None,
            session: SessionConfig::default(),
            provisioning: ProvisioningConfig::default(),
        };

        let debug_output = format!("{:?}", config);
        assert!(
            debug_output.contains("****"),
            "Debug output should contain redacted marker"
        );
        assert!(
            !debug_output.contains("super-secret-client-secret-xyz"),
            "Debug output must NOT contain client secret"
        );
        // Non-sensitive fields should still be visible
        assert!(
            debug_output.contains("my-client-id"),
            "Client ID should be visible"
        );
        assert!(
            debug_output.contains("https://auth.example.com"),
            "Issuer should be visible"
        );
    }

    #[cfg(feature = "sso")]
    #[test]
    fn test_session_config_debug_redacts_secret() {
        let config = SessionConfig {
            cookie_name: "__gw_session".to_string(),
            duration_secs: 86400,
            secure: true,
            same_site: SameSite::Lax,
            secret: Some("my-super-secret-session-key".to_string()),
            enhanced: EnhancedSessionConfig::default(),
        };

        let debug_output = format!("{:?}", config);
        assert!(
            debug_output.contains("****"),
            "Debug output should contain redacted marker"
        );
        assert!(
            !debug_output.contains("my-super-secret-session-key"),
            "Debug output must NOT contain session secret"
        );
        // Non-sensitive fields should still be visible
        assert!(
            debug_output.contains("__gw_session"),
            "Cookie name should be visible"
        );
    }

    #[cfg(feature = "sso")]
    #[test]
    fn test_session_config_debug_no_secret() {
        // When secret is None, should show None not ****
        let config = SessionConfig {
            cookie_name: "__gw_session".to_string(),
            duration_secs: 86400,
            secure: true,
            same_site: SameSite::Lax,
            secret: None,
            enhanced: EnhancedSessionConfig::default(),
        };

        let debug_output = format!("{:?}", config);
        assert!(
            debug_output.contains("secret: None"),
            "Debug output should show None for missing secret"
        );
    }

    #[test]
    fn test_emergency_account_key_minimum_length() {
        // Key too short (less than 32 characters)
        let account = EmergencyAccount {
            id: "test".to_string(),
            name: "Test Admin".to_string(),
            key: "short-key".to_string(), // Only 9 chars
            email: None,
            roles: vec![],
            allowed_ips: vec![],
        };
        let result = account.validate();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("must be at least 32 characters")
        );

        // Key exactly 32 characters (should pass)
        let account = EmergencyAccount {
            id: "test".to_string(),
            name: "Test Admin".to_string(),
            key: "12345678901234567890123456789012".to_string(), // Exactly 32 chars
            email: None,
            roles: vec![],
            allowed_ips: vec![],
        };
        let result = account.validate();
        assert!(result.is_ok());

        // Key longer than 32 characters (should pass)
        let account = EmergencyAccount {
            id: "test".to_string(),
            name: "Test Admin".to_string(),
            key: "this-is-a-very-long-emergency-key-for-testing".to_string(),
            email: None,
            roles: vec![],
            allowed_ips: vec![],
        };
        let result = account.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_emergency_config_duplicate_account_ids() {
        let config = EmergencyAccessConfig {
            enabled: true,
            accounts: vec![
                EmergencyAccount {
                    id: "admin1".to_string(),
                    name: "Admin One".to_string(),
                    key: "key-admin-1-at-least-32-characters-long".to_string(),
                    email: None,
                    roles: vec![],
                    allowed_ips: vec![],
                },
                EmergencyAccount {
                    id: "admin1".to_string(), // Duplicate ID
                    name: "Admin One Copy".to_string(),
                    key: "key-admin-2-at-least-32-characters-long".to_string(),
                    email: None,
                    roles: vec![],
                    allowed_ips: vec![],
                },
            ],
            rate_limit: EmergencyRateLimit::default(),
            allowed_ips: vec![],
        };

        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Duplicate emergency account ID"));
        assert!(err.contains("admin1"));
    }

    #[test]
    fn test_emergency_account_key_not_serialized() {
        let account = EmergencyAccount {
            id: "test".to_string(),
            name: "Test Admin".to_string(),
            key: "secret-key-that-should-not-appear-in-output".to_string(),
            email: None,
            roles: vec![],
            allowed_ips: vec![],
        };

        let json = serde_json::to_string(&account).unwrap();
        assert!(
            !json.contains("secret-key-that-should-not-appear-in-output"),
            "Key should not be serialized in JSON output"
        );
        assert!(
            !json.contains("\"key\""),
            "Key field should not appear in JSON output"
        );
    }
}
