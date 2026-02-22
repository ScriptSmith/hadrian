use serde::{Deserialize, Serialize};

use super::ConfigError;

/// Cache configuration.
///
/// The cache is used for:
/// - Rate limiting counters
/// - Budget enforcement (current spend)
/// - Session data
/// - API key lookups (to reduce database load)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum CacheConfig {
    /// No caching. Rate limiting and budget enforcement are disabled.
    /// Only suitable for local development.
    #[default]
    None,

    /// In-memory cache. Good for single-node deployments.
    /// Data is lost on restart. Not suitable for multi-node.
    Memory(MemoryCacheConfig),

    /// Redis cache. Required for multi-node deployments.
    Redis(RedisCacheConfig),
}

impl CacheConfig {
    pub fn is_none(&self) -> bool {
        matches!(self, CacheConfig::None)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        match self {
            CacheConfig::None => Ok(()),
            CacheConfig::Memory(c) => c.validate(),
            CacheConfig::Redis(c) => c.validate(),
        }
    }

    /// Get TTL configuration, using defaults if cache is not configured.
    pub fn ttl(&self) -> CacheTtlConfig {
        match self {
            CacheConfig::None => CacheTtlConfig::default(),
            CacheConfig::Memory(c) => c.ttl.clone(),
            CacheConfig::Redis(c) => c.ttl.clone(),
        }
    }
}

/// In-memory cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MemoryCacheConfig {
    /// Maximum number of entries in the cache.
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,

    /// Number of entries to evict when cache is full.
    /// Eviction removes expired entries first, then uses LRU.
    #[serde(default = "default_eviction_batch_size")]
    pub eviction_batch_size: usize,

    /// Default TTL for cache entries in seconds.
    #[serde(default = "default_ttl")]
    pub default_ttl_secs: u64,

    /// TTL settings for specific cache types.
    #[serde(default)]
    pub ttl: CacheTtlConfig,
}

impl Default for MemoryCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: default_max_entries(),
            eviction_batch_size: default_eviction_batch_size(),
            default_ttl_secs: default_ttl(),
            ttl: CacheTtlConfig::default(),
        }
    }
}

impl MemoryCacheConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.max_entries == 0 {
            return Err(ConfigError::Validation(
                "Memory cache max_entries must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}

fn default_max_entries() -> usize {
    100_000
}

fn default_eviction_batch_size() -> usize {
    100 // Evict 100 entries at a time when cache is full
}

fn default_ttl() -> u64 {
    3600 // 1 hour
}

/// Redis cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RedisCacheConfig {
    /// Redis connection URL.
    /// Format: redis://[user:password@]host:port[/database]
    /// For clusters: redis+cluster://host1:port1,host2:port2,...
    pub url: String,

    /// Connection timeout in seconds.
    #[serde(default = "default_redis_timeout")]
    pub connect_timeout_secs: u64,

    /// Key prefix for all cache keys.
    /// Useful when sharing a Redis instance with other applications.
    #[serde(default = "default_key_prefix")]
    pub key_prefix: String,

    /// Enable TLS for Redis connections.
    #[serde(default)]
    pub tls: bool,

    /// Cluster mode configuration.
    #[serde(default)]
    pub cluster: Option<RedisClusterConfig>,

    /// TTL settings for specific cache types.
    #[serde(default)]
    pub ttl: CacheTtlConfig,
}

impl RedisCacheConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.url.is_empty() {
            return Err(ConfigError::Validation("Redis URL cannot be empty".into()));
        }
        Ok(())
    }
}

/// Redis cluster configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RedisClusterConfig {
    /// Read from replicas for read operations.
    #[serde(default)]
    pub read_from_replicas: bool,

    /// Number of retries for cluster operations.
    #[serde(default = "default_cluster_retries")]
    pub retries: u32,

    /// Retry delay in milliseconds between retries.
    #[serde(default = "default_cluster_retry_delay")]
    pub retry_delay_ms: u64,

    /// Connection timeout for cluster nodes in seconds.
    #[serde(default = "default_cluster_connection_timeout")]
    pub connection_timeout_secs: u64,

    /// Response timeout for cluster operations in seconds.
    #[serde(default = "default_cluster_response_timeout")]
    pub response_timeout_secs: u64,
}

fn default_redis_timeout() -> u64 {
    5
}

fn default_key_prefix() -> String {
    "gw:".to_string()
}

fn default_cluster_retries() -> u32 {
    3
}

fn default_cluster_retry_delay() -> u64 {
    100 // 100ms
}

fn default_cluster_connection_timeout() -> u64 {
    5
}

fn default_cluster_response_timeout() -> u64 {
    1
}

/// TTL configuration for different cache types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CacheTtlConfig {
    /// TTL for API key cache in seconds.
    #[serde(default = "default_api_key_ttl")]
    pub api_key_secs: u64,

    /// TTL for rate limit counters in seconds.
    #[serde(default = "default_rate_limit_ttl")]
    pub rate_limit_secs: u64,

    /// TTL for dynamic provider cache in seconds.
    #[serde(default = "default_provider_ttl")]
    pub provider_secs: u64,

    /// TTL for daily spend cache in seconds.
    #[serde(default = "default_daily_spend_ttl")]
    pub daily_spend_secs: u64,

    /// TTL for monthly spend cache in seconds.
    #[serde(default = "default_monthly_spend_ttl")]
    pub monthly_spend_secs: u64,
}

impl Default for CacheTtlConfig {
    fn default() -> Self {
        Self {
            api_key_secs: default_api_key_ttl(),
            rate_limit_secs: default_rate_limit_ttl(),
            provider_secs: default_provider_ttl(),
            daily_spend_secs: default_daily_spend_ttl(),
            monthly_spend_secs: default_monthly_spend_ttl(),
        }
    }
}

fn default_api_key_ttl() -> u64 {
    300 // 5 minutes
}

fn default_rate_limit_ttl() -> u64 {
    60 // 1 minute
}

fn default_provider_ttl() -> u64 {
    300 // 5 minutes
}

fn default_daily_spend_ttl() -> u64 {
    86400 // 1 day
}

fn default_monthly_spend_ttl() -> u64 {
    86400 * 32 // ~32 days
}
