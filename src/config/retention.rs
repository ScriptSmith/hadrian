//! Data retention configuration.
//!
//! Configures automatic purging of old data to manage database size and
//! comply with data retention policies.
//!
//! # Example
//!
//! ```toml
//! [retention]
//! enabled = true
//! interval_hours = 24
//!
//! [retention.periods]
//! usage_records_days = 90
//! daily_spend_days = 365
//! audit_logs_days = 730
//! conversations_deleted_days = 30
//!
//! [retention.safety]
//! dry_run = false
//! max_deletes_per_run = 100000
//! ```

use serde::{Deserialize, Serialize};

/// Data retention configuration.
///
/// Controls automatic purging of old data from the database.
/// When enabled, a background worker periodically deletes records
/// older than their configured retention period.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RetentionConfig {
    /// Whether retention purging is enabled.
    /// Default: false (must be explicitly enabled)
    #[serde(default)]
    pub enabled: bool,

    /// How often to run the retention worker (in hours).
    /// Default: 24 (once per day)
    #[serde(default = "default_interval_hours")]
    pub interval_hours: u64,

    /// Retention periods for different data types.
    #[serde(default)]
    pub periods: RetentionPeriods,

    /// Safety settings to prevent accidental data loss.
    #[serde(default)]
    pub safety: RetentionSafety,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_hours: default_interval_hours(),
            periods: RetentionPeriods::default(),
            safety: RetentionSafety::default(),
        }
    }
}

fn default_interval_hours() -> u64 {
    24
}

/// Retention periods for different data types.
///
/// Each field specifies the number of days to keep records.
/// Set to 0 to disable retention for that data type (keep forever).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RetentionPeriods {
    /// Days to keep individual usage records.
    /// These are high-volume records (one per API request).
    /// Default: 90 days
    #[serde(default = "default_usage_records_days")]
    pub usage_records_days: u32,

    /// Days to keep aggregated daily spend records.
    /// These are lower-volume summary records (one per API key per model per day).
    /// Default: 365 days
    #[serde(default = "default_daily_spend_days")]
    pub daily_spend_days: u32,

    /// Days to keep audit log entries.
    /// Audit logs track admin operations and may be required for compliance.
    /// Default: 730 days (2 years)
    #[serde(default = "default_audit_logs_days")]
    pub audit_logs_days: u32,

    /// Days to keep soft-deleted conversations before hard deleting.
    /// Conversations are first soft-deleted, then permanently removed
    /// after this period.
    /// Default: 30 days
    #[serde(default = "default_conversations_deleted_days")]
    pub conversations_deleted_days: u32,
}

impl Default for RetentionPeriods {
    fn default() -> Self {
        Self {
            usage_records_days: default_usage_records_days(),
            daily_spend_days: default_daily_spend_days(),
            audit_logs_days: default_audit_logs_days(),
            conversations_deleted_days: default_conversations_deleted_days(),
        }
    }
}

fn default_usage_records_days() -> u32 {
    90
}

fn default_daily_spend_days() -> u32 {
    365
}

fn default_audit_logs_days() -> u32 {
    730 // 2 years
}

fn default_conversations_deleted_days() -> u32 {
    30
}

/// Safety settings for retention operations.
///
/// These settings help prevent accidental data loss and allow
/// testing retention policies before enabling them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RetentionSafety {
    /// If true, log what would be deleted without actually deleting.
    /// Useful for testing retention policies.
    /// Default: false
    #[serde(default)]
    pub dry_run: bool,

    /// Maximum number of records to delete per run per table.
    /// Prevents long-running delete operations that could impact performance.
    /// Set to 0 for unlimited.
    /// Default: 100000
    #[serde(default = "default_max_deletes_per_run")]
    pub max_deletes_per_run: u64,

    /// Batch size for delete operations.
    /// Records are deleted in batches to avoid locking the database.
    /// Default: 1000
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
}

impl Default for RetentionSafety {
    fn default() -> Self {
        Self {
            dry_run: false,
            max_deletes_per_run: default_max_deletes_per_run(),
            batch_size: default_batch_size(),
        }
    }
}

fn default_max_deletes_per_run() -> u64 {
    100_000
}

fn default_batch_size() -> u32 {
    1000
}

impl RetentionConfig {
    /// Check if any retention periods are configured (non-zero).
    pub fn has_any_retention(&self) -> bool {
        self.periods.usage_records_days > 0
            || self.periods.daily_spend_days > 0
            || self.periods.audit_logs_days > 0
            || self.periods.conversations_deleted_days > 0
    }

    /// Get the interval as a Duration.
    pub fn interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.interval_hours * 3600)
    }
}

impl RetentionPeriods {
    /// Check if usage records retention is enabled.
    pub fn should_retain_usage_records(&self) -> bool {
        self.usage_records_days > 0
    }

    /// Check if daily spend retention is enabled.
    pub fn should_retain_daily_spend(&self) -> bool {
        self.daily_spend_days > 0
    }

    /// Check if audit logs retention is enabled.
    pub fn should_retain_audit_logs(&self) -> bool {
        self.audit_logs_days > 0
    }

    /// Check if conversation hard-delete is enabled.
    pub fn should_retain_conversations(&self) -> bool {
        self.conversations_deleted_days > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RetentionConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.interval_hours, 24);
        assert_eq!(config.periods.usage_records_days, 90);
        assert_eq!(config.periods.daily_spend_days, 365);
        assert_eq!(config.periods.audit_logs_days, 730);
        assert_eq!(config.periods.conversations_deleted_days, 30);
        assert!(!config.safety.dry_run);
        assert_eq!(config.safety.max_deletes_per_run, 100_000);
        assert_eq!(config.safety.batch_size, 1000);
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
            enabled = true
        "#;
        let config: RetentionConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.interval_hours, 24);
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
            enabled = true
            interval_hours = 12

            [periods]
            usage_records_days = 60
            daily_spend_days = 180
            audit_logs_days = 365
            conversations_deleted_days = 7

            [safety]
            dry_run = true
            max_deletes_per_run = 50000
            batch_size = 500
        "#;
        let config: RetentionConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.interval_hours, 12);
        assert_eq!(config.periods.usage_records_days, 60);
        assert_eq!(config.periods.daily_spend_days, 180);
        assert_eq!(config.periods.audit_logs_days, 365);
        assert_eq!(config.periods.conversations_deleted_days, 7);
        assert!(config.safety.dry_run);
        assert_eq!(config.safety.max_deletes_per_run, 50000);
        assert_eq!(config.safety.batch_size, 500);
    }

    #[test]
    fn test_parse_disabled_periods() {
        let toml = r#"
            enabled = true

            [periods]
            usage_records_days = 0
            daily_spend_days = 0
            audit_logs_days = 0
            conversations_deleted_days = 0
        "#;
        let config: RetentionConfig = toml::from_str(toml).unwrap();
        assert!(!config.periods.should_retain_usage_records());
        assert!(!config.periods.should_retain_daily_spend());
        assert!(!config.periods.should_retain_audit_logs());
        assert!(!config.periods.should_retain_conversations());
        assert!(!config.has_any_retention());
    }

    #[test]
    fn test_has_any_retention() {
        let mut config = RetentionConfig::default();
        assert!(config.has_any_retention()); // Defaults have retention

        config.periods.usage_records_days = 0;
        config.periods.daily_spend_days = 0;
        config.periods.audit_logs_days = 0;
        config.periods.conversations_deleted_days = 0;
        assert!(!config.has_any_retention());

        config.periods.usage_records_days = 30;
        assert!(config.has_any_retention());
    }

    #[test]
    fn test_interval_duration() {
        let mut config = RetentionConfig::default();
        assert_eq!(config.interval(), std::time::Duration::from_secs(24 * 3600));

        config.interval_hours = 6;
        assert_eq!(config.interval(), std::time::Duration::from_secs(6 * 3600));
    }

    #[test]
    fn test_unlimited_deletes() {
        let toml = r#"
            enabled = true

            [safety]
            max_deletes_per_run = 0
        "#;
        let config: RetentionConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.safety.max_deletes_per_run, 0);
    }
}
