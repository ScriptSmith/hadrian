use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Observability configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ObservabilityConfig {
    /// Logging configuration.
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Tracing configuration (OpenTelemetry).
    #[serde(default)]
    pub tracing: TracingConfig,

    /// Metrics configuration.
    #[serde(default)]
    pub metrics: MetricsConfig,

    /// Request/response logging.
    #[serde(default)]
    pub request_logging: RequestLoggingConfig,

    /// Usage logging configuration.
    #[serde(default)]
    pub usage: UsageConfig,

    /// Dead-letter queue for failed operations (usage logging, etc.).
    #[serde(default)]
    pub dead_letter_queue: Option<DeadLetterQueueConfig>,

    /// Response schema validation configuration.
    /// Validates API responses against the OpenAI OpenAPI specification.
    #[serde(default)]
    pub response_validation: ResponseValidationConfig,
}

// ─────────────────────────────────────────────────────────────────────────────
// Logging
// ─────────────────────────────────────────────────────────────────────────────

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    /// Log level.
    #[serde(default = "default_log_level")]
    pub level: LogLevel,

    /// Log format.
    #[serde(default)]
    pub format: LogFormat,

    /// Include timestamps.
    #[serde(default = "default_true")]
    pub timestamps: bool,

    /// Include file/line information.
    #[serde(default)]
    pub file_line: bool,

    /// Include span information for tracing integration.
    #[serde(default = "default_true")]
    pub include_spans: bool,

    /// Filter directives (e.g., "tower_http=debug,sqlx=warn").
    #[serde(default)]
    pub filter: Option<String>,

    /// SIEM-specific configuration (for CEF, LEEF, Syslog formats).
    #[serde(default)]
    pub siem: SiemConfig,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: LogFormat::default(),
            timestamps: true,
            file_line: false,
            include_spans: true,
            filter: None,
            siem: SiemConfig::default(),
        }
    }
}

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn to_tracing_level(&self) -> tracing::Level {
        match self {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    /// Human-readable multi-line format.
    Pretty,
    /// Compact single-line format.
    #[default]
    Compact,
    /// JSON format (for log aggregation).
    Json,
    /// CEF (Common Event Format) for ArcSight, Splunk, and most SIEMs.
    Cef,
    /// LEEF (Log Event Extended Format) for IBM QRadar.
    Leef,
    /// Syslog (RFC 5424) format for standard syslog servers.
    Syslog,
}

impl LogFormat {
    /// Returns true if this format is a SIEM format (CEF, LEEF, or Syslog).
    pub fn is_siem_format(&self) -> bool {
        matches!(self, LogFormat::Cef | LogFormat::Leef | LogFormat::Syslog)
    }
}

/// SIEM-specific configuration for CEF, LEEF, and Syslog formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SiemConfig {
    /// Device vendor name for CEF/LEEF headers.
    #[serde(default = "default_device_vendor")]
    pub device_vendor: String,

    /// Device product name for CEF/LEEF headers.
    #[serde(default = "default_device_product")]
    pub device_product: String,

    /// Device version for CEF/LEEF headers.
    /// If not specified, uses the crate version from Cargo.toml.
    #[serde(default)]
    pub device_version: Option<String>,

    /// Syslog facility (only used for Syslog format).
    #[serde(default)]
    pub facility: SyslogFacility,

    /// Override hostname for Syslog/CEF/LEEF.
    /// If not specified, uses the system hostname.
    #[serde(default)]
    pub hostname: Option<String>,

    /// Application name for Syslog APP-NAME field.
    #[serde(default = "default_app_name")]
    pub app_name: String,

    /// LEEF format version (1.0 or 2.0).
    #[serde(default)]
    pub leef_version: LeefVersion,
}

impl Default for SiemConfig {
    fn default() -> Self {
        Self {
            device_vendor: default_device_vendor(),
            device_product: default_device_product(),
            device_version: None,
            facility: SyslogFacility::default(),
            hostname: None,
            app_name: default_app_name(),
            leef_version: LeefVersion::default(),
        }
    }
}

impl SiemConfig {
    /// Get the device version, falling back to the crate version.
    pub fn get_device_version(&self) -> &str {
        self.device_version
            .as_deref()
            .unwrap_or(env!("CARGO_PKG_VERSION"))
    }

    /// Get the hostname, falling back to the system hostname.
    pub fn get_hostname(&self) -> String {
        self.hostname.clone().unwrap_or_else(|| {
            #[cfg(feature = "otlp")]
            {
                hostname::get()
                    .map(|h| h.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| "unknown".to_string())
            }
            #[cfg(not(feature = "otlp"))]
            {
                "unknown".to_string()
            }
        })
    }
}

fn default_device_vendor() -> String {
    "Hadrian".to_string()
}

fn default_device_product() -> String {
    "Gateway".to_string()
}

fn default_app_name() -> String {
    "hadrian".to_string()
}

/// Syslog facility as defined in RFC 5424.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SyslogFacility {
    /// Kernel messages (0).
    Kern,
    /// User-level messages (1).
    User,
    /// Mail system (2).
    Mail,
    /// System daemons (3).
    Daemon,
    /// Security/authorization messages (4).
    Auth,
    /// Messages generated internally by syslogd (5).
    Syslog,
    /// Line printer subsystem (6).
    Lpr,
    /// Network news subsystem (7).
    News,
    /// UUCP subsystem (8).
    Uucp,
    /// Clock daemon (9).
    Cron,
    /// Security/authorization messages (private) (10).
    Authpriv,
    /// FTP daemon (11).
    Ftp,
    /// NTP subsystem (12).
    Ntp,
    /// Log audit (13).
    Audit,
    /// Log alert (14).
    Alert,
    /// Clock daemon (15).
    Clock,
    /// Local use 0 (16).
    #[default]
    Local0,
    /// Local use 1 (17).
    Local1,
    /// Local use 2 (18).
    Local2,
    /// Local use 3 (19).
    Local3,
    /// Local use 4 (20).
    Local4,
    /// Local use 5 (21).
    Local5,
    /// Local use 6 (22).
    Local6,
    /// Local use 7 (23).
    Local7,
}

impl SyslogFacility {
    /// Returns the numeric facility code (0-23).
    pub fn code(&self) -> u8 {
        match self {
            SyslogFacility::Kern => 0,
            SyslogFacility::User => 1,
            SyslogFacility::Mail => 2,
            SyslogFacility::Daemon => 3,
            SyslogFacility::Auth => 4,
            SyslogFacility::Syslog => 5,
            SyslogFacility::Lpr => 6,
            SyslogFacility::News => 7,
            SyslogFacility::Uucp => 8,
            SyslogFacility::Cron => 9,
            SyslogFacility::Authpriv => 10,
            SyslogFacility::Ftp => 11,
            SyslogFacility::Ntp => 12,
            SyslogFacility::Audit => 13,
            SyslogFacility::Alert => 14,
            SyslogFacility::Clock => 15,
            SyslogFacility::Local0 => 16,
            SyslogFacility::Local1 => 17,
            SyslogFacility::Local2 => 18,
            SyslogFacility::Local3 => 19,
            SyslogFacility::Local4 => 20,
            SyslogFacility::Local5 => 21,
            SyslogFacility::Local6 => 22,
            SyslogFacility::Local7 => 23,
        }
    }
}

/// LEEF format version.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum LeefVersion {
    /// LEEF version 1.0 (original format).
    #[serde(rename = "1.0")]
    V1,
    /// LEEF version 2.0 (with delimiter specification).
    #[default]
    #[serde(rename = "2.0")]
    V2,
}

impl LeefVersion {
    /// Returns the version string for the LEEF header.
    pub fn as_str(&self) -> &'static str {
        match self {
            LeefVersion::V1 => "1.0",
            LeefVersion::V2 => "2.0",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tracing
// ─────────────────────────────────────────────────────────────────────────────

/// Tracing configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TracingConfig {
    /// Enable distributed tracing.
    #[serde(default)]
    pub enabled: bool,

    /// OTLP exporter configuration.
    #[serde(default)]
    pub otlp: Option<OtlpConfig>,

    /// Service name.
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Service version.
    #[serde(default)]
    pub service_version: Option<String>,

    /// Environment (e.g., "production", "staging").
    #[serde(default)]
    pub environment: Option<String>,

    /// Sampling configuration.
    #[serde(default)]
    pub sampling: SamplingConfig,

    /// Additional resource attributes.
    #[serde(default)]
    pub resource_attributes: HashMap<String, String>,

    /// Propagation format.
    #[serde(default)]
    pub propagation: PropagationFormat,
}

fn default_service_name() -> String {
    "ai-gateway".to_string()
}

/// OTLP exporter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OtlpConfig {
    /// OTLP endpoint URL.
    pub endpoint: String,

    /// Protocol (grpc or http).
    #[serde(default)]
    pub protocol: OtlpProtocol,

    /// Headers to include (e.g., for authentication).
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Timeout in seconds.
    #[serde(default = "default_otlp_timeout")]
    pub timeout_secs: u64,

    /// Enable compression.
    #[serde(default = "default_true")]
    pub compression: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OtlpProtocol {
    #[default]
    Grpc,
    Http,
}

fn default_otlp_timeout() -> u64 {
    10
}

/// Sampling configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SamplingConfig {
    /// Sampling strategy.
    #[serde(default)]
    pub strategy: SamplingStrategy,

    /// Sample rate for ratio-based sampling (0.0-1.0).
    #[serde(default = "default_sample_rate")]
    pub rate: f64,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            strategy: SamplingStrategy::default(),
            rate: default_sample_rate(),
        }
    }
}

fn default_sample_rate() -> f64 {
    1.0 // Sample everything
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SamplingStrategy {
    /// Sample all traces.
    #[default]
    AlwaysOn,
    /// Sample no traces.
    AlwaysOff,
    /// Sample a percentage of traces.
    Ratio,
    /// Parent-based sampling (inherit from parent span).
    ParentBased,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PropagationFormat {
    /// W3C Trace Context.
    #[default]
    TraceContext,
    /// B3 (Zipkin).
    B3,
    /// Jaeger.
    Jaeger,
    /// Multiple formats.
    Multi,
}

// ─────────────────────────────────────────────────────────────────────────────
// Metrics
// ─────────────────────────────────────────────────────────────────────────────

/// Metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    /// Enable metrics gathering.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Prometheus endpoint configuration.
    #[serde(default)]
    pub prometheus: Option<PrometheusConfig>,

    /// Prometheus server URL for querying aggregated metrics (e.g., "http://prometheus:9090").
    ///
    /// When configured, provider statistics are fetched from Prometheus using PromQL queries.
    /// This enables accurate metrics aggregation in multi-node deployments where each gateway
    /// instance exposes its own /metrics endpoint to Prometheus.
    ///
    /// When not configured (default), provider statistics are derived from the local /metrics
    /// endpoint. This works for single-node deployments but won't show aggregate metrics
    /// across multiple gateway instances.
    ///
    /// Note: Historical stats (time series data) require Prometheus to be configured.
    #[serde(default)]
    pub prometheus_query_url: Option<String>,

    /// OTLP metrics exporter.
    #[serde(default)]
    pub otlp: Option<OtlpConfig>,

    /// Histogram buckets for latency metrics (in milliseconds).
    #[serde(default = "default_latency_buckets")]
    pub latency_buckets_ms: Vec<f64>,

    /// Histogram buckets for token counts.
    #[serde(default = "default_token_buckets")]
    pub token_buckets: Vec<f64>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prometheus: None,
            prometheus_query_url: None,
            otlp: None,
            latency_buckets_ms: default_latency_buckets(),
            token_buckets: default_token_buckets(),
        }
    }
}

fn default_latency_buckets() -> Vec<f64> {
    vec![
        10.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
    ]
}

fn default_token_buckets() -> Vec<f64> {
    vec![
        10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, 10000.0, 50000.0, 100000.0,
    ]
}

/// Prometheus configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PrometheusConfig {
    /// Enable Prometheus endpoint.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Path for the metrics endpoint.
    #[serde(default = "default_metrics_path")]
    pub path: String,

    /// Include default process metrics.
    #[serde(default = "default_true")]
    pub process_metrics: bool,
}

fn default_metrics_path() -> String {
    "/metrics".to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Request Logging
// ─────────────────────────────────────────────────────────────────────────────

/// Request/response logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RequestLoggingConfig {
    /// Enable request logging.
    #[serde(default)]
    pub enabled: bool,

    /// Log request bodies.
    #[serde(default)]
    pub log_request_body: bool,

    /// Log response bodies.
    #[serde(default)]
    pub log_response_body: bool,

    /// Maximum body size to log (in bytes).
    #[serde(default = "default_max_body_log")]
    pub max_body_size: usize,

    /// Redact sensitive fields.
    #[serde(default = "default_true")]
    pub redact_sensitive: bool,

    /// Fields to redact.
    #[serde(default = "default_redact_fields")]
    pub redact_fields: Vec<String>,

    /// Log to separate destination.
    #[serde(default)]
    pub destination: Option<LogDestination>,
}

impl Default for RequestLoggingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            log_request_body: false,
            log_response_body: false,
            max_body_size: default_max_body_log(),
            redact_sensitive: true,
            redact_fields: default_redact_fields(),
            destination: None,
        }
    }
}

fn default_max_body_log() -> usize {
    10 * 1024 // 10 KB
}

fn default_redact_fields() -> Vec<String> {
    vec![
        "api_key".into(),
        "password".into(),
        "secret".into(),
        "authorization".into(),
    ]
}

/// Log destination for request logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum LogDestination {
    /// Log to file.
    File {
        path: String,
        #[serde(default)]
        rotation: Option<LogRotation>,
    },
    /// Log to stdout/stderr (same as regular logs).
    Stdout,
    /// Send to external service.
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LogRotation {
    Daily,
    Hourly,
    Size { max_bytes: usize },
}

fn default_true() -> bool {
    true
}

// ─────────────────────────────────────────────────────────────────────────────
// Dead Letter Queue
// ─────────────────────────────────────────────────────────────────────────────

/// Dead-letter queue configuration for failed operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum DeadLetterQueueConfig {
    /// File-based dead-letter queue.
    File {
        /// Path to the dead-letter directory.
        path: String,
        /// Maximum file size in MB before rotation.
        #[serde(default = "default_dlq_max_file_size")]
        max_file_size_mb: u64,
        /// Maximum number of files to keep.
        #[serde(default = "default_dlq_max_files")]
        max_files: u32,
        /// Retry configuration.
        #[serde(default)]
        retry: DlqRetryConfig,
    },

    /// Redis-based dead-letter queue.
    Redis {
        /// Redis URL (can reuse cache URL).
        url: String,
        /// Key prefix for DLQ entries.
        #[serde(default = "default_dlq_key_prefix")]
        key_prefix: String,
        /// Maximum entries to keep.
        #[serde(default = "default_dlq_max_entries")]
        max_entries: u64,
        /// TTL for DLQ entries in seconds.
        #[serde(default = "default_dlq_ttl")]
        ttl_secs: u64,
        /// Retry configuration.
        #[serde(default)]
        retry: DlqRetryConfig,
    },

    /// Database-based dead-letter queue.
    Database {
        /// Table name for DLQ entries.
        #[serde(default = "default_dlq_table")]
        table_name: String,
        /// Maximum entries to keep.
        #[serde(default = "default_dlq_max_entries")]
        max_entries: u64,
        /// TTL for DLQ entries in seconds.
        #[serde(default = "default_dlq_ttl")]
        ttl_secs: u64,
        /// Retry configuration.
        #[serde(default)]
        retry: DlqRetryConfig,
    },
}

impl DeadLetterQueueConfig {
    /// Get the retry configuration for any DLQ type.
    pub fn retry(&self) -> &DlqRetryConfig {
        match self {
            DeadLetterQueueConfig::File { retry, .. } => retry,
            DeadLetterQueueConfig::Redis { retry, .. } => retry,
            DeadLetterQueueConfig::Database { retry, .. } => retry,
        }
    }

    /// Get the TTL in seconds (for pruning).
    pub fn ttl_secs(&self) -> u64 {
        match self {
            DeadLetterQueueConfig::File { .. } => default_dlq_ttl(),
            DeadLetterQueueConfig::Redis { ttl_secs, .. } => *ttl_secs,
            DeadLetterQueueConfig::Database { ttl_secs, .. } => *ttl_secs,
        }
    }
}

/// Configuration for DLQ retry processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DlqRetryConfig {
    /// Enable automatic retry processing.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Interval between retry processing runs in seconds.
    #[serde(default = "default_dlq_retry_interval")]
    pub interval_secs: u64,
    /// Initial delay before first retry in seconds.
    #[serde(default = "default_dlq_initial_delay")]
    pub initial_delay_secs: u64,
    /// Maximum delay between retries in seconds.
    #[serde(default = "default_dlq_max_delay")]
    pub max_delay_secs: u64,
    /// Backoff multiplier for exponential backoff.
    #[serde(default = "default_dlq_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// Maximum number of retry attempts before giving up.
    #[serde(default = "default_dlq_max_retries")]
    pub max_retries: i32,
    /// Batch size for retry processing.
    #[serde(default = "default_dlq_batch_size")]
    pub batch_size: i64,
    /// Enable automatic pruning of old entries.
    #[serde(default = "default_true")]
    pub prune_enabled: bool,
}

impl Default for DlqRetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: default_dlq_retry_interval(),
            initial_delay_secs: default_dlq_initial_delay(),
            max_delay_secs: default_dlq_max_delay(),
            backoff_multiplier: default_dlq_backoff_multiplier(),
            max_retries: default_dlq_max_retries(),
            batch_size: default_dlq_batch_size(),
            prune_enabled: true,
        }
    }
}

fn default_dlq_max_file_size() -> u64 {
    100 // 100 MB
}

fn default_dlq_max_files() -> u32 {
    10
}

fn default_dlq_key_prefix() -> String {
    "gw:dlq:".to_string()
}

fn default_dlq_max_entries() -> u64 {
    100_000
}

fn default_dlq_ttl() -> u64 {
    86400 * 7 // 7 days
}

fn default_dlq_table() -> String {
    "dead_letter_queue".to_string()
}

fn default_dlq_retry_interval() -> u64 {
    60 // 1 minute
}

fn default_dlq_initial_delay() -> u64 {
    60 // 1 minute
}

fn default_dlq_max_delay() -> u64 {
    3600 // 1 hour
}

fn default_dlq_backoff_multiplier() -> f64 {
    2.0
}

fn default_dlq_max_retries() -> i32 {
    10
}

fn default_dlq_batch_size() -> i64 {
    100
}

// ─────────────────────────────────────────────────────────────────────────────
// Usage Logging
// ─────────────────────────────────────────────────────────────────────────────

/// Usage logging configuration.
///
/// Controls where API usage data (tokens, costs, latency) is sent.
/// Multiple sinks can be enabled simultaneously.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UsageConfig {
    /// Enable database logging (default: true if database is configured).
    #[serde(default = "default_true")]
    pub database: bool,

    /// OTLP exporter for usage data.
    /// Sends usage records as OTLP log records to any OpenTelemetry-compatible backend.
    #[serde(default)]
    pub otlp: Option<UsageOtlpConfig>,

    /// Buffer configuration for batched writes.
    #[serde(default)]
    pub buffer: UsageBufferConfig,
}

impl Default for UsageConfig {
    fn default() -> Self {
        Self {
            database: true,
            otlp: None,
            buffer: UsageBufferConfig::default(),
        }
    }
}

/// OTLP configuration for usage logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UsageOtlpConfig {
    /// Enable OTLP usage export.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// OTLP endpoint URL.
    /// If not specified, uses the tracing OTLP endpoint.
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Protocol (grpc or http).
    #[serde(default)]
    pub protocol: OtlpProtocol,

    /// Headers to include (e.g., for authentication).
    #[serde(default)]
    pub headers: HashMap<String, String>,

    /// Timeout in seconds.
    #[serde(default = "default_otlp_timeout")]
    pub timeout_secs: u64,

    /// Enable compression.
    #[serde(default = "default_true")]
    pub compression: bool,

    /// Service name override (defaults to tracing service name).
    #[serde(default)]
    pub service_name: Option<String>,
}

/// Buffer configuration for usage logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UsageBufferConfig {
    /// Maximum entries to buffer before flushing.
    #[serde(default = "default_usage_buffer_size")]
    pub max_size: usize,

    /// Maximum time between flushes in milliseconds.
    #[serde(default = "default_usage_flush_interval_ms")]
    pub flush_interval_ms: u64,

    /// Maximum pending entries before dropping oldest.
    /// When the sink is slow or unavailable, entries accumulate in the buffer.
    /// If pending entries exceed this limit, the oldest entries are dropped
    /// to prevent unbounded memory growth (OOM). Set to 0 to disable (not recommended).
    /// Default: 10x max_size (10,000 entries at ~1KB each = ~10MB max memory).
    #[serde(default = "default_max_pending_entries")]
    pub max_pending_entries: usize,
}

impl Default for UsageBufferConfig {
    fn default() -> Self {
        Self {
            max_size: default_usage_buffer_size(),
            flush_interval_ms: default_usage_flush_interval_ms(),
            max_pending_entries: default_max_pending_entries(),
        }
    }
}

fn default_usage_buffer_size() -> usize {
    1000
}

fn default_usage_flush_interval_ms() -> u64 {
    1000 // 1 second
}

fn default_max_pending_entries() -> usize {
    10_000 // 10x default max_size
}

// ─────────────────────────────────────────────────────────────────────────────
// Response Validation
// ─────────────────────────────────────────────────────────────────────────────

/// Response schema validation configuration.
///
/// When enabled, validates API responses against the OpenAI OpenAPI specification
/// before sending them to clients. This helps catch response format issues early,
/// especially from non-OpenAI providers.
///
/// # Example
///
/// ```toml
/// [observability.response_validation]
/// enabled = true
/// mode = "warn"
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResponseValidationConfig {
    /// Enable response schema validation.
    /// When enabled, responses are validated against the OpenAI OpenAPI spec.
    #[serde(default)]
    pub enabled: bool,

    /// Validation mode.
    /// - `warn`: Log validation failures but return the response anyway.
    /// - `error`: Return a 500 error if validation fails.
    #[serde(default)]
    pub mode: ResponseValidationMode,
}

/// Response validation mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ResponseValidationMode {
    /// Log validation failures but return the response anyway.
    #[default]
    Warn,
    /// Return a 500 error if validation fails.
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_is_siem_format() {
        assert!(!LogFormat::Pretty.is_siem_format());
        assert!(!LogFormat::Compact.is_siem_format());
        assert!(!LogFormat::Json.is_siem_format());
        assert!(LogFormat::Cef.is_siem_format());
        assert!(LogFormat::Leef.is_siem_format());
        assert!(LogFormat::Syslog.is_siem_format());
    }

    #[test]
    fn test_log_format_parsing() {
        assert_eq!(
            serde_json::from_str::<LogFormat>("\"pretty\"").unwrap(),
            LogFormat::Pretty
        );
        assert_eq!(
            serde_json::from_str::<LogFormat>("\"compact\"").unwrap(),
            LogFormat::Compact
        );
        assert_eq!(
            serde_json::from_str::<LogFormat>("\"json\"").unwrap(),
            LogFormat::Json
        );
        assert_eq!(
            serde_json::from_str::<LogFormat>("\"cef\"").unwrap(),
            LogFormat::Cef
        );
        assert_eq!(
            serde_json::from_str::<LogFormat>("\"leef\"").unwrap(),
            LogFormat::Leef
        );
        assert_eq!(
            serde_json::from_str::<LogFormat>("\"syslog\"").unwrap(),
            LogFormat::Syslog
        );
    }

    #[test]
    fn test_syslog_facility_codes() {
        assert_eq!(SyslogFacility::Kern.code(), 0);
        assert_eq!(SyslogFacility::User.code(), 1);
        assert_eq!(SyslogFacility::Mail.code(), 2);
        assert_eq!(SyslogFacility::Daemon.code(), 3);
        assert_eq!(SyslogFacility::Auth.code(), 4);
        assert_eq!(SyslogFacility::Syslog.code(), 5);
        assert_eq!(SyslogFacility::Local0.code(), 16);
        assert_eq!(SyslogFacility::Local7.code(), 23);
    }

    #[test]
    fn test_syslog_facility_parsing() {
        assert_eq!(
            serde_json::from_str::<SyslogFacility>("\"kern\"").unwrap(),
            SyslogFacility::Kern
        );
        assert_eq!(
            serde_json::from_str::<SyslogFacility>("\"local0\"").unwrap(),
            SyslogFacility::Local0
        );
        assert_eq!(
            serde_json::from_str::<SyslogFacility>("\"auth\"").unwrap(),
            SyslogFacility::Auth
        );
    }

    #[test]
    fn test_leef_version() {
        assert_eq!(LeefVersion::V1.as_str(), "1.0");
        assert_eq!(LeefVersion::V2.as_str(), "2.0");

        assert_eq!(
            serde_json::from_str::<LeefVersion>("\"1.0\"").unwrap(),
            LeefVersion::V1
        );
        assert_eq!(
            serde_json::from_str::<LeefVersion>("\"2.0\"").unwrap(),
            LeefVersion::V2
        );
    }

    #[test]
    fn test_siem_config_defaults() {
        let config = SiemConfig::default();
        assert_eq!(config.device_vendor, "Hadrian");
        assert_eq!(config.device_product, "Gateway");
        assert!(config.device_version.is_none());
        assert_eq!(config.facility, SyslogFacility::Local0);
        assert!(config.hostname.is_none());
        assert_eq!(config.app_name, "hadrian");
        assert_eq!(config.leef_version, LeefVersion::V2);
    }

    #[test]
    fn test_siem_config_get_device_version() {
        let mut config = SiemConfig::default();
        // Default should return crate version
        assert!(!config.get_device_version().is_empty());

        // Custom version should override
        config.device_version = Some("2.0.0".to_string());
        assert_eq!(config.get_device_version(), "2.0.0");
    }

    #[test]
    fn test_siem_config_get_hostname() {
        let mut config = SiemConfig::default();
        // Default should return system hostname (or "unknown")
        let hostname = config.get_hostname();
        assert!(!hostname.is_empty());

        // Custom hostname should override
        config.hostname = Some("custom-host".to_string());
        assert_eq!(config.get_hostname(), "custom-host");
    }

    #[test]
    fn test_siem_config_parsing() {
        let json = r#"{
            "device_vendor": "MyCompany",
            "device_product": "MyProduct",
            "device_version": "1.0.0",
            "facility": "local3",
            "hostname": "my-host",
            "app_name": "myapp",
            "leef_version": "1.0"
        }"#;

        let config: SiemConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.device_vendor, "MyCompany");
        assert_eq!(config.device_product, "MyProduct");
        assert_eq!(config.device_version, Some("1.0.0".to_string()));
        assert_eq!(config.facility, SyslogFacility::Local3);
        assert_eq!(config.hostname, Some("my-host".to_string()));
        assert_eq!(config.app_name, "myapp");
        assert_eq!(config.leef_version, LeefVersion::V1);
    }

    #[test]
    fn test_logging_config_with_siem() {
        let json = r#"{
            "level": "debug",
            "format": "cef",
            "siem": {
                "device_vendor": "TestVendor"
            }
        }"#;

        let config: LoggingConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.format, LogFormat::Cef);
        assert_eq!(config.siem.device_vendor, "TestVendor");
        // Other SIEM fields should have defaults
        assert_eq!(config.siem.device_product, "Gateway");
    }
}
