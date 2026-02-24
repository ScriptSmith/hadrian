use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Default limits configuration.
///
/// These limits are applied when no specific limits are set at the
/// org, project, or user level.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LimitsConfig {
    /// Rate limiting defaults.
    #[serde(default)]
    pub rate_limits: RateLimitDefaults,

    /// Budget defaults.
    #[serde(default)]
    pub budgets: BudgetDefaults,

    /// Token limits.
    #[serde(default)]
    pub tokens: TokenLimitDefaults,

    /// Resource limits for entity counts.
    #[serde(default)]
    pub resource_limits: ResourceLimits,
}

/// Resource limits for entity counts.
///
/// These limits prevent unbounded growth of resources that could cause
/// performance issues or resource exhaustion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResourceLimits {
    /// Maximum RBAC policies per organization.
    /// Set to 0 for unlimited. Default: 100 policies per org.
    ///
    /// This limit prevents resource exhaustion from unbounded policy growth.
    /// Organizations hitting this limit must delete or disable existing policies
    /// before creating new ones.
    #[serde(default = "default_max_policies_per_org")]
    pub max_policies_per_org: u32,

    /// Maximum dynamic providers per user (BYOK).
    /// Set to 0 for unlimited. Default: 10 providers per user.
    #[serde(default = "default_max_providers_per_user")]
    pub max_providers_per_user: u32,

    /// Maximum API keys per user (self-service).
    /// Set to 0 for unlimited. Default: 25 keys per user.
    #[serde(default = "default_max_api_keys_per_user")]
    pub max_api_keys_per_user: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_policies_per_org: default_max_policies_per_org(),
            max_providers_per_user: default_max_providers_per_user(),
            max_api_keys_per_user: default_max_api_keys_per_user(),
        }
    }
}

fn default_max_policies_per_org() -> u32 {
    100
}

fn default_max_providers_per_user() -> u32 {
    10
}

fn default_max_api_keys_per_user() -> u32 {
    25
}

/// Rate limiting defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RateLimitDefaults {
    /// Requests per minute per identity.
    #[serde(default = "default_rpm")]
    pub requests_per_minute: u32,

    /// Requests per day per identity.
    #[serde(default)]
    pub requests_per_day: Option<u32>,

    /// Tokens per minute per identity.
    #[serde(default = "default_tpm")]
    pub tokens_per_minute: u32,

    /// Tokens per day per identity.
    #[serde(default)]
    pub tokens_per_day: Option<u32>,

    /// Concurrent request limit per identity.
    #[serde(default = "default_concurrent")]
    pub concurrent_requests: u32,

    /// Rate limit window type.
    #[serde(default)]
    pub window_type: RateLimitWindowType,

    /// Estimated tokens per request for atomic token rate limit reservation.
    /// This is reserved before the request is processed to prevent race conditions.
    /// After the request completes, the actual token count replaces the estimate.
    /// Default is 1000 tokens which is conservative for most prompts.
    #[serde(default = "default_estimated_tokens")]
    pub estimated_tokens_per_request: i64,

    /// IP-based rate limiting for unauthenticated requests.
    /// Protects public endpoints (health, auth) from abuse.
    #[serde(default)]
    pub ip_rate_limits: IpRateLimitConfig,

    /// Allow per-API-key rate limits to exceed global defaults.
    /// When false (default), API keys cannot have higher rate limits than the global config.
    /// When true, API keys can have any positive rate limit value.
    #[serde(default)]
    pub allow_per_key_above_global: bool,
}

/// IP-based rate limiting configuration for unauthenticated traffic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IpRateLimitConfig {
    /// Enable IP-based rate limiting for unauthenticated requests.
    #[serde(default = "default_ip_rate_limit_enabled")]
    pub enabled: bool,

    /// Requests per minute per IP address.
    #[serde(default = "default_ip_rpm")]
    pub requests_per_minute: u32,

    /// Requests per hour per IP address.
    /// Provides longer-term protection against sustained abuse.
    #[serde(default)]
    pub requests_per_hour: Option<u32>,
}

impl Default for IpRateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: default_ip_rate_limit_enabled(),
            requests_per_minute: default_ip_rpm(),
            requests_per_hour: None,
        }
    }
}

fn default_ip_rate_limit_enabled() -> bool {
    true
}

fn default_ip_rpm() -> u32 {
    120 // 2 requests per second average
}

impl Default for RateLimitDefaults {
    fn default() -> Self {
        Self {
            requests_per_minute: default_rpm(),
            requests_per_day: None,
            tokens_per_minute: default_tpm(),
            tokens_per_day: None,
            concurrent_requests: default_concurrent(),
            window_type: RateLimitWindowType::default(),
            estimated_tokens_per_request: default_estimated_tokens(),
            ip_rate_limits: IpRateLimitConfig::default(),
            allow_per_key_above_global: false,
        }
    }
}

fn default_estimated_tokens() -> i64 {
    1000 // Conservative estimate for most prompts
}

fn default_rpm() -> u32 {
    60
}

fn default_tpm() -> u32 {
    100_000
}

fn default_concurrent() -> u32 {
    10
}

/// Rate limit window type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RateLimitWindowType {
    /// Fixed window (resets at interval boundaries).
    Fixed,
    /// Sliding window (rolling count over the interval).
    #[default]
    Sliding,
}

/// Budget defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BudgetDefaults {
    /// Default monthly budget in USD. None means unlimited.
    #[serde(default)]
    #[cfg_attr(feature = "json-schema", schemars(with = "Option<String>"))]
    pub monthly_budget_usd: Option<Decimal>,

    /// Default daily budget in USD. None means unlimited.
    #[serde(default)]
    #[cfg_attr(feature = "json-schema", schemars(with = "Option<String>"))]
    pub daily_budget_usd: Option<Decimal>,

    /// Warning threshold as a percentage (0.0-1.0).
    /// Notifications are sent when this threshold is reached.
    #[serde(default = "default_warning_threshold")]
    pub warning_threshold: f64,

    /// Hard limit action when budget is exceeded.
    #[serde(default)]
    pub exceeded_action: BudgetExceededAction,

    /// Allow overage up to this percentage above the budget.
    /// E.g., 0.1 means 10% overage is allowed.
    #[serde(default)]
    pub allowed_overage: f64,

    /// Estimated cost per request in cents for budget reservation.
    /// This is reserved before the request is processed to prevent race conditions.
    /// After the request completes, the actual cost replaces the estimate.
    /// Default is 10 cents ($0.10) which is conservative for most models.
    #[serde(default = "default_estimated_cost_cents")]
    pub estimated_cost_cents: i64,
}

impl Default for BudgetDefaults {
    fn default() -> Self {
        Self {
            monthly_budget_usd: None,
            daily_budget_usd: None,
            warning_threshold: default_warning_threshold(),
            exceeded_action: BudgetExceededAction::default(),
            allowed_overage: 0.0,
            estimated_cost_cents: default_estimated_cost_cents(),
        }
    }
}

fn default_estimated_cost_cents() -> i64 {
    10 // $0.10 conservative estimate
}

fn default_warning_threshold() -> f64 {
    0.8 // 80%
}

/// Action to take when budget is exceeded.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BudgetExceededAction {
    /// Block the request.
    #[default]
    Block,
    /// Allow the request but log a warning.
    Warn,
    /// Allow but throttle (reduce rate limits).
    Throttle,
}

/// Token limit defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TokenLimitDefaults {
    /// Maximum input tokens per request.
    #[serde(default)]
    pub max_input_tokens: Option<u32>,

    /// Maximum output tokens per request.
    #[serde(default)]
    pub max_output_tokens: Option<u32>,

    /// Maximum total tokens per request (input + output).
    #[serde(default)]
    pub max_total_tokens: Option<u32>,

    /// Default max_tokens if not specified in the request.
    #[serde(default = "default_max_tokens")]
    pub default_max_tokens: u32,
}

impl Default for TokenLimitDefaults {
    fn default() -> Self {
        Self {
            max_input_tokens: None,
            max_output_tokens: None,
            max_total_tokens: None,
            default_max_tokens: default_max_tokens(),
        }
    }
}

fn default_max_tokens() -> u32 {
    4096
}
