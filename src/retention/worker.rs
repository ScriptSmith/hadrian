//! Retention worker for processing data retention policies.
//!
//! This module provides a background worker that periodically purges old data
//! based on configured retention periods. It follows the same patterns as the
//! DLQ worker for consistency.

use std::sync::Arc;

use chrono::{Duration, Utc};

use crate::{config::RetentionConfig, db::DbPool, observability::metrics};

/// Results from a single retention run.
#[derive(Debug, Default)]
pub struct RetentionRunResult {
    /// Number of usage records deleted.
    pub usage_records_deleted: u64,
    /// Number of daily spend records deleted.
    pub daily_spend_deleted: u64,
    /// Number of audit log entries deleted.
    pub audit_logs_deleted: u64,
    /// Number of conversations hard-deleted.
    pub conversations_deleted: u64,
}

impl RetentionRunResult {
    /// Total number of records deleted across all tables.
    pub fn total(&self) -> u64 {
        self.usage_records_deleted
            + self.daily_spend_deleted
            + self.audit_logs_deleted
            + self.conversations_deleted
    }

    /// Check if any records were deleted.
    pub fn has_deletions(&self) -> bool {
        self.total() > 0
    }
}

/// Starts the retention worker as a background task.
///
/// The worker runs in a loop, purging old data at the configured interval.
/// It will run indefinitely until the task is cancelled.
pub async fn start_retention_worker(db: Arc<DbPool>, config: RetentionConfig) {
    if !config.enabled {
        tracing::info!("Retention worker disabled by configuration");
        return;
    }

    if !config.has_any_retention() {
        tracing::info!("Retention worker enabled but no retention periods configured");
        return;
    }

    let dry_run_msg = if config.safety.dry_run {
        " (DRY RUN)"
    } else {
        ""
    };

    tracing::info!(
        interval_hours = config.interval_hours,
        usage_records_days = config.periods.usage_records_days,
        daily_spend_days = config.periods.daily_spend_days,
        audit_logs_days = config.periods.audit_logs_days,
        conversations_deleted_days = config.periods.conversations_deleted_days,
        dry_run = config.safety.dry_run,
        "Starting retention worker{}",
        dry_run_msg
    );

    let interval = config.interval();

    loop {
        match run_retention(&db, &config).await {
            Ok(result) => {
                if result.has_deletions() {
                    tracing::info!(
                        usage_records = result.usage_records_deleted,
                        daily_spend = result.daily_spend_deleted,
                        audit_logs = result.audit_logs_deleted,
                        conversations = result.conversations_deleted,
                        total = result.total(),
                        dry_run = config.safety.dry_run,
                        "Retention run complete{}",
                        dry_run_msg
                    );
                } else {
                    tracing::debug!("Retention run complete, no records to delete");
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Error running retention");
            }
        }

        tokio::time::sleep(interval).await;
    }
}

/// Run a single retention pass, purging data from all configured tables.
async fn run_retention(
    db: &Arc<DbPool>,
    config: &RetentionConfig,
) -> Result<RetentionRunResult, Box<dyn std::error::Error + Send + Sync>> {
    let mut result = RetentionRunResult::default();

    // Delete usage records
    if config.periods.should_retain_usage_records() {
        let deleted = delete_usage_records(db, config).await?;
        result.usage_records_deleted = deleted;
    }

    // Delete daily spend records
    if config.periods.should_retain_daily_spend() {
        let deleted = delete_daily_spend(db, config).await?;
        result.daily_spend_deleted = deleted;
    }

    // Delete audit logs
    if config.periods.should_retain_audit_logs() {
        let deleted = delete_audit_logs(db, config).await?;
        result.audit_logs_deleted = deleted;
    }

    // Hard-delete soft-deleted conversations
    if config.periods.should_retain_conversations() {
        let deleted = delete_conversations(db, config).await?;
        result.conversations_deleted = deleted;
    }

    Ok(result)
}

/// Delete usage records older than the retention period.
async fn delete_usage_records(
    db: &Arc<DbPool>,
    config: &RetentionConfig,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let cutoff = Utc::now() - Duration::days(config.periods.usage_records_days as i64);

    if config.safety.dry_run {
        tracing::info!(
            cutoff = %cutoff,
            "DRY RUN: Would delete usage records before {}",
            cutoff
        );
        return Ok(0);
    }

    let max_deletes = if config.safety.max_deletes_per_run == 0 {
        u64::MAX
    } else {
        config.safety.max_deletes_per_run
    };

    let deleted = db
        .usage()
        .delete_usage_records_before(cutoff, config.safety.batch_size, max_deletes)
        .await?;

    if deleted > 0 {
        tracing::debug!(
            deleted = deleted,
            cutoff = %cutoff,
            "Deleted usage records"
        );
        metrics::record_retention_deletion("usage_records", deleted);
    }

    Ok(deleted)
}

/// Delete daily spend records older than the retention period.
async fn delete_daily_spend(
    db: &Arc<DbPool>,
    config: &RetentionConfig,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let cutoff = Utc::now() - Duration::days(config.periods.daily_spend_days as i64);

    if config.safety.dry_run {
        tracing::info!(
            cutoff = %cutoff,
            "DRY RUN: Would delete daily spend records before {}",
            cutoff
        );
        return Ok(0);
    }

    let max_deletes = if config.safety.max_deletes_per_run == 0 {
        u64::MAX
    } else {
        config.safety.max_deletes_per_run
    };

    let deleted = db
        .usage()
        .delete_daily_spend_before(cutoff, config.safety.batch_size, max_deletes)
        .await?;

    if deleted > 0 {
        tracing::debug!(
            deleted = deleted,
            cutoff = %cutoff,
            "Deleted daily spend records"
        );
        metrics::record_retention_deletion("daily_spend", deleted);
    }

    Ok(deleted)
}

/// Delete audit logs older than the retention period.
async fn delete_audit_logs(
    db: &Arc<DbPool>,
    config: &RetentionConfig,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let cutoff = Utc::now() - Duration::days(config.periods.audit_logs_days as i64);

    if config.safety.dry_run {
        tracing::info!(
            cutoff = %cutoff,
            "DRY RUN: Would delete audit logs before {}",
            cutoff
        );
        return Ok(0);
    }

    let max_deletes = if config.safety.max_deletes_per_run == 0 {
        u64::MAX
    } else {
        config.safety.max_deletes_per_run
    };

    let deleted = db
        .audit_logs()
        .delete_before(cutoff, config.safety.batch_size, max_deletes)
        .await?;

    if deleted > 0 {
        tracing::debug!(
            deleted = deleted,
            cutoff = %cutoff,
            "Deleted audit logs"
        );
        metrics::record_retention_deletion("audit_logs", deleted);
    }

    Ok(deleted)
}

/// Hard-delete conversations that were soft-deleted before the retention period.
async fn delete_conversations(
    db: &Arc<DbPool>,
    config: &RetentionConfig,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let cutoff = Utc::now() - Duration::days(config.periods.conversations_deleted_days as i64);

    if config.safety.dry_run {
        tracing::info!(
            cutoff = %cutoff,
            "DRY RUN: Would hard-delete soft-deleted conversations before {}",
            cutoff
        );
        return Ok(0);
    }

    let max_deletes = if config.safety.max_deletes_per_run == 0 {
        u64::MAX
    } else {
        config.safety.max_deletes_per_run
    };

    let deleted = db
        .conversations()
        .hard_delete_soft_deleted_before(cutoff, config.safety.batch_size, max_deletes)
        .await?;

    if deleted > 0 {
        tracing::debug!(
            deleted = deleted,
            cutoff = %cutoff,
            "Hard-deleted soft-deleted conversations"
        );
        metrics::record_retention_deletion("conversations", deleted);
    }

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_run_result_total() {
        let result = RetentionRunResult {
            usage_records_deleted: 100,
            daily_spend_deleted: 50,
            audit_logs_deleted: 25,
            conversations_deleted: 10,
        };
        assert_eq!(result.total(), 185);
    }

    #[test]
    fn test_retention_run_result_has_deletions() {
        let empty = RetentionRunResult::default();
        assert!(!empty.has_deletions());

        let with_deletions = RetentionRunResult {
            usage_records_deleted: 1,
            ..Default::default()
        };
        assert!(with_deletions.has_deletions());
    }

    #[test]
    fn test_retention_run_result_default() {
        let result = RetentionRunResult::default();
        assert_eq!(result.usage_records_deleted, 0);
        assert_eq!(result.daily_spend_deleted, 0);
        assert_eq!(result.audit_logs_deleted, 0);
        assert_eq!(result.conversations_deleted, 0);
        assert_eq!(result.total(), 0);
    }
}
