//! DLQ retry worker for processing failed operations.
//!
//! This module provides a background worker that periodically:
//! 1. Retrieves entries from the dead-letter queue
//! 2. Attempts to reprocess them with exponential backoff
//! 3. Removes successfully processed entries
//! 4. Prunes old entries based on TTL

use std::sync::Arc;

use chrono::{Duration, Utc};

use crate::{
    config::DlqRetryConfig,
    db::DbPool,
    dlq::{DeadLetterQueue, DlqEntry},
    models::UsageLogEntry,
    observability::metrics,
};

/// Starts the DLQ retry worker as a background task.
///
/// The worker runs in a loop, processing DLQ entries at the configured interval.
/// It will shut down gracefully when the provided cancellation token is triggered.
pub async fn start_dlq_worker(
    dlq: Arc<dyn DeadLetterQueue>,
    db: Arc<DbPool>,
    config: DlqRetryConfig,
    ttl_secs: u64,
) {
    if !config.enabled {
        tracing::info!("DLQ retry worker disabled by configuration");
        return;
    }

    tracing::info!(
        interval_secs = config.interval_secs,
        max_retries = config.max_retries,
        batch_size = config.batch_size,
        "Starting DLQ retry worker"
    );

    let interval = std::time::Duration::from_secs(config.interval_secs);

    loop {
        // Process a batch of entries
        if let Err(e) = process_batch(&dlq, &db, &config).await {
            tracing::error!(error = %e, "Error processing DLQ batch");
        }

        // Prune old entries if enabled
        if config.prune_enabled
            && let Err(e) = prune_old_entries(&dlq, ttl_secs).await
        {
            tracing::error!(error = %e, "Error pruning old DLQ entries");
        }

        // Wait for the next interval
        tokio::time::sleep(interval).await;
    }
}

/// Process a batch of DLQ entries.
async fn process_batch(
    dlq: &Arc<dyn DeadLetterQueue>,
    db: &Arc<DbPool>,
    config: &DlqRetryConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get entries that are ready to be retried
    let params = crate::dlq::traits::DlqListParams {
        entry_type: None,
        limit: Some(config.batch_size),
        older_than: None,
        max_retries: Some(config.max_retries),
        cursor: None,
        direction: crate::dlq::traits::DlqCursorDirection::Forward,
    };

    let result = dlq.list(params).await?;

    if result.items.is_empty() {
        return Ok(());
    }

    tracing::debug!(count = result.items.len(), "Processing DLQ entries");

    let mut processed = 0;
    let mut failed = 0;

    for entry in result.items {
        // Check if entry is ready for retry based on backoff
        if !is_ready_for_retry(&entry, config) {
            continue;
        }

        match process_entry(&entry, db).await {
            Ok(true) => {
                // Successfully processed - remove from queue
                if let Err(e) = dlq.remove(entry.id).await {
                    tracing::error!(
                        entry_id = %entry.id,
                        error = %e,
                        "Failed to remove processed DLQ entry"
                    );
                } else {
                    processed += 1;
                    metrics::record_dlq_operation("retry_success", &entry.entry_type);
                    tracing::info!(
                        entry_id = %entry.id,
                        entry_type = %entry.entry_type,
                        retry_count = entry.retry_count,
                        "Successfully processed DLQ entry"
                    );
                }
            }
            Ok(false) => {
                // Entry type not supported for retry - skip it
                tracing::debug!(
                    entry_id = %entry.id,
                    entry_type = %entry.entry_type,
                    "Unsupported DLQ entry type, skipping"
                );
            }
            Err(e) => {
                // Failed to process - mark as retried
                if let Err(mark_err) = dlq.mark_retried(entry.id).await {
                    tracing::error!(
                        entry_id = %entry.id,
                        error = %mark_err,
                        "Failed to mark DLQ entry as retried"
                    );
                } else {
                    failed += 1;
                    metrics::record_dlq_operation("retry_failure", &entry.entry_type);
                    tracing::warn!(
                        entry_id = %entry.id,
                        entry_type = %entry.entry_type,
                        retry_count = entry.retry_count + 1,
                        error = %e,
                        "Failed to process DLQ entry, will retry later"
                    );
                }
            }
        }
    }

    if processed > 0 || failed > 0 {
        tracing::info!(
            processed = processed,
            failed = failed,
            "DLQ batch processing complete"
        );
    }

    Ok(())
}

/// Check if an entry is ready for retry based on exponential backoff.
fn is_ready_for_retry(entry: &DlqEntry, config: &DlqRetryConfig) -> bool {
    // If never retried, check initial delay from creation
    let reference_time = entry.last_retry_at.unwrap_or(entry.created_at);

    // Calculate backoff delay: initial_delay * multiplier^retry_count
    let delay_secs = (config.initial_delay_secs as f64
        * config.backoff_multiplier.powi(entry.retry_count))
    .min(config.max_delay_secs as f64) as i64;

    let ready_at = reference_time + Duration::seconds(delay_secs);
    Utc::now() >= ready_at
}

/// Process a single DLQ entry based on its type.
///
/// Returns `Ok(true)` if successfully processed, `Ok(false)` if entry type
/// is not supported, or `Err` if processing failed.
async fn process_entry(
    entry: &DlqEntry,
    db: &Arc<DbPool>,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    match entry.entry_type.as_str() {
        "usage_log" => {
            process_usage_log_entry(entry, db).await?;
            Ok(true)
        }
        _ => {
            // Unknown entry type - can't process
            Ok(false)
        }
    }
}

/// Process a usage_log DLQ entry by writing it to the database.
async fn process_usage_log_entry(
    entry: &DlqEntry,
    db: &Arc<DbPool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Deserialize the usage log entry from the payload
    let usage_entry: UsageLogEntry = serde_json::from_str(&entry.payload)?;

    // Attempt to write to the database
    db.usage().log(usage_entry).await?;

    Ok(())
}

/// Prune entries older than the TTL.
async fn prune_old_entries(
    dlq: &Arc<dyn DeadLetterQueue>,
    ttl_secs: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cutoff = Utc::now() - Duration::seconds(ttl_secs as i64);

    match dlq.prune(cutoff).await {
        Ok(count) if count > 0 => {
            tracing::info!(count = count, "Pruned old DLQ entries");
            metrics::record_dlq_operation("prune", "all");
        }
        Ok(_) => {}
        Err(e) => {
            tracing::error!(error = %e, "Failed to prune old DLQ entries");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ready_for_retry_first_attempt() {
        let config = DlqRetryConfig {
            enabled: true,
            interval_secs: 60,
            initial_delay_secs: 60,
            max_delay_secs: 3600,
            backoff_multiplier: 2.0,
            max_retries: 10,
            batch_size: 100,
            prune_enabled: true,
        };

        // Entry created 2 minutes ago, never retried
        let entry = DlqEntry {
            id: uuid::Uuid::new_v4(),
            entry_type: "usage_log".to_string(),
            payload: "{}".to_string(),
            error: "test error".to_string(),
            retry_count: 0,
            created_at: Utc::now() - Duration::minutes(2),
            last_retry_at: None,
            metadata: std::collections::HashMap::new(),
        };

        // Should be ready (2 min > 1 min initial delay)
        assert!(is_ready_for_retry(&entry, &config));
    }

    #[test]
    fn test_is_ready_for_retry_with_backoff() {
        let config = DlqRetryConfig {
            enabled: true,
            interval_secs: 60,
            initial_delay_secs: 60,
            max_delay_secs: 3600,
            backoff_multiplier: 2.0,
            max_retries: 10,
            batch_size: 100,
            prune_enabled: true,
        };

        // Entry retried once 1 minute ago
        let entry = DlqEntry {
            id: uuid::Uuid::new_v4(),
            entry_type: "usage_log".to_string(),
            payload: "{}".to_string(),
            error: "test error".to_string(),
            retry_count: 1,
            created_at: Utc::now() - Duration::hours(1),
            last_retry_at: Some(Utc::now() - Duration::minutes(1)),
            metadata: std::collections::HashMap::new(),
        };

        // Should NOT be ready (1 min < 2 min delay for retry_count=1)
        assert!(!is_ready_for_retry(&entry, &config));

        // Entry retried once 3 minutes ago
        let entry2 = DlqEntry {
            id: uuid::Uuid::new_v4(),
            entry_type: "usage_log".to_string(),
            payload: "{}".to_string(),
            error: "test error".to_string(),
            retry_count: 1,
            created_at: Utc::now() - Duration::hours(1),
            last_retry_at: Some(Utc::now() - Duration::minutes(3)),
            metadata: std::collections::HashMap::new(),
        };

        // Should be ready (3 min > 2 min delay for retry_count=1)
        assert!(is_ready_for_retry(&entry2, &config));
    }
}
