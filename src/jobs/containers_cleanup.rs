//! Container cleanup worker for hard-deleting terminal container rows.
//!
//! Containers move `active` → `expired` (the idle reaper) → `deleted`
//! (explicit `DELETE /v1/containers/{id}`). Both terminal transitions stamp
//! `expires_at` but never remove the row, so without this worker `expired` /
//! `deleted` containers (and their `container_files`) accumulate forever.
//!
//! This module provides a background worker that periodically:
//! 1. Finds terminal containers whose `expires_at` has passed the cleanup delay
//! 2. Hard deletes the container rows, cascading their `container_files`
//!
//! The cleanup process is designed to be safe and incremental:
//! - Cleanup is batched to avoid long-running operations
//! - A configurable delay between the terminal transition and hard delete
//!   gives clients time to download captured files
//! - Dry run mode allows testing the cleanup configuration

use std::{sync::Arc, time::Instant};

use chrono::{Duration, Utc};
use tokio_util::sync::CancellationToken;

use crate::{
    config::ContainersCleanupConfig,
    db::DbPool,
    jobs::leader_lock::{self, LeadershipOutcome, keys},
    observability::metrics,
    services::containers::ContainersService,
};

/// Results from a single cleanup run.
#[derive(Debug, Default)]
pub struct CleanupRunResult {
    /// Number of containers hard-deleted.
    pub containers_deleted: u64,
    /// Duration of the cleanup run in milliseconds.
    pub duration_ms: u64,
}

impl CleanupRunResult {
    /// Check if any records were deleted.
    pub fn has_deletions(&self) -> bool {
        self.containers_deleted > 0
    }
}

/// Starts the container cleanup worker as a background task.
///
/// The worker runs in a loop, hard-deleting terminal containers at the
/// configured interval. It runs until `shutdown` fires, so a SIGTERM
/// stops it promptly instead of letting it keep hitting the DB while the
/// rest of the process drains.
pub async fn start_containers_cleanup_worker(
    containers: Arc<ContainersService>,
    db: Arc<DbPool>,
    config: ContainersCleanupConfig,
    shutdown: CancellationToken,
) {
    if !config.enabled {
        tracing::info!("Container cleanup worker disabled by configuration");
        return;
    }

    let dry_run_msg = if config.dry_run { " (DRY RUN)" } else { "" };

    tracing::info!(
        interval_secs = config.interval_secs,
        cleanup_delay_secs = config.cleanup_delay_secs,
        batch_size = config.batch_size,
        max_duration_secs = config.max_duration_secs,
        dry_run = config.dry_run,
        "Starting container cleanup worker{}",
        dry_run_msg
    );

    let interval = config.interval();

    loop {
        if shutdown.is_cancelled() {
            tracing::info!("Container cleanup worker received shutdown signal");
            return;
        }
        // Skip ticks where another replica already holds the cleanup lock —
        // running deletes from two replicas would race on the same rows.
        let _guard = match leader_lock::try_acquire(&db, keys::CONTAINERS_CLEANUP).await {
            LeadershipOutcome::Leader(g) => Some(g),
            LeadershipOutcome::NotLeader => {
                tracing::trace!("containers_cleanup: not leader this tick, skipping");
                tokio::select! {
                    _ = shutdown.cancelled() => return,
                    _ = tokio::time::sleep(interval) => {}
                }
                continue;
            }
            LeadershipOutcome::NoCoordination => None,
        };

        match run_cleanup(&containers, &config).await {
            Ok(result) => {
                if result.has_deletions() {
                    tracing::info!(
                        stage = "complete",
                        containers = result.containers_deleted,
                        duration_ms = result.duration_ms,
                        dry_run = config.dry_run,
                        "Container cleanup run complete{}",
                        dry_run_msg
                    );
                } else {
                    tracing::debug!(
                        stage = "complete",
                        "Container cleanup run complete, nothing to clean up"
                    );
                }
            }
            Err(e) => {
                tracing::error!(stage = "error", error = %e, "Error running container cleanup");
                metrics::record_cleanup_error("containers");
            }
        }

        tokio::select! {
            _ = shutdown.cancelled() => return,
            _ = tokio::time::sleep(interval) => {}
        }
    }
}

/// Run a single cleanup pass, hard-deleting terminal containers and their
/// captured files.
async fn run_cleanup(
    containers: &Arc<ContainersService>,
    config: &ContainersCleanupConfig,
) -> Result<CleanupRunResult, Box<dyn std::error::Error + Send + Sync>> {
    let start = Instant::now();
    let mut result = CleanupRunResult::default();

    // Calculate cutoff time: containers that became terminal before this time
    // should be cleaned up.
    let cutoff = Utc::now() - Duration::seconds(config.cleanup_delay_secs as i64);
    let max_duration = config.max_duration();

    let mut remaining = config.batch_size as i64;
    loop {
        if remaining <= 0 {
            break;
        }
        // Check if we've exceeded max duration.
        if let Some(max_dur) = max_duration
            && start.elapsed() > max_dur
        {
            tracing::info!(
                stage = "max_duration",
                containers_processed = result.containers_deleted,
                "Max cleanup duration exceeded, stopping early"
            );
            break;
        }

        if config.dry_run {
            // Dry run can't delete, so there's nothing to iterate on without
            // racing the same rows forever. Report the candidate count once.
            let cutoff_ts = cutoff;
            tracing::info!(
                stage = "dry_run",
                cutoff = %cutoff_ts,
                "DRY RUN: would hard-delete terminal containers older than cutoff"
            );
            break;
        }

        let deleted = containers.hard_delete_expired(cutoff, remaining).await?;
        if deleted.is_empty() {
            break;
        }
        result.containers_deleted += deleted.len() as u64;
        remaining -= deleted.len() as i64;
        for id in &deleted {
            tracing::debug!(stage = "delete", container_id = %id, "Hard deleted terminal container");
        }
    }

    result.duration_ms = start.elapsed().as_millis() as u64;

    if result.containers_deleted > 0 {
        metrics::record_cleanup_deletion("containers", result.containers_deleted);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_run_result_default() {
        let result = CleanupRunResult::default();
        assert_eq!(result.containers_deleted, 0);
        assert_eq!(result.duration_ms, 0);
        assert!(!result.has_deletions());
    }

    #[test]
    fn test_cleanup_run_result_has_deletions() {
        let empty = CleanupRunResult::default();
        assert!(!empty.has_deletions());

        let with_containers = CleanupRunResult {
            containers_deleted: 1,
            ..Default::default()
        };
        assert!(with_containers.has_deletions());
    }
}
