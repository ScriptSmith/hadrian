//! Idle-container reaper for the shell-tool `/mnt/data` workspace.
//!
//! Every pass does two things with different scopes:
//! 1. **Leader only** — under the cluster-wide leader lock, mark
//!    `containers` rows whose `last_active_at + idle_ttl_secs` has
//!    elapsed as `expired`. Exactly one replica flips the rows.
//! 2. **Every replica** — reconcile this process's in-memory
//!    [`ContainerSessionRegistry`] against the DB: any locally-held
//!    session whose row is now terminal (`expired`/`deleted`) is
//!    removed so `ContainerSession::drop` detaches a terminate task and
//!    the underlying VM is torn down.
//!
//! The registry is process-local, so the reconcile step must NOT be
//! gated on leadership — only the replica hosting a VM can free it, and
//! that is usually not the replica that won the lock and flipped the
//! row.

use std::{sync::Arc, time::Duration as StdDuration};

use chrono::Utc;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::{
    jobs::leader_lock::{self, LeadershipOutcome, keys},
    services::{container_session::ContainerSessionRegistry, containers::ContainersService},
};

/// Run the reaper loop until `shutdown` fires.
pub async fn start_containers_reaper_worker(
    containers: Arc<ContainersService>,
    registry: Arc<ContainerSessionRegistry>,
    db: Arc<crate::db::DbPool>,
    interval: StdDuration,
    shutdown: CancellationToken,
) {
    tracing::info!(
        interval_secs = interval.as_secs(),
        "Starting containers reaper worker"
    );

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                tracing::info!("Containers reaper worker received shutdown signal");
                return;
            }
            _ = sleep(interval) => {}
        }

        // Only the leader flips DB rows; other replicas still need to
        // sweep their local registry of in-memory sessions for any
        // containers a previous leader pass already expired.
        let leader_guard = match leader_lock::try_acquire(&db, keys::CONTAINERS_REAPER).await {
            LeadershipOutcome::Leader(g) => Some(Some(g)),
            LeadershipOutcome::NotLeader => Some(None),
            LeadershipOutcome::NoCoordination => None,
        };
        let is_leader = !matches!(leader_guard, Some(None));

        // Step 1 (leader only): flip expired rows in the DB.
        if is_leader {
            match containers.mark_expired_idle(Utc::now()).await {
                Ok(expired_ids) if !expired_ids.is_empty() => {
                    tracing::info!(count = expired_ids.len(), "Marked idle containers expired");
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(error = %e, "Containers reaper DB pass failed");
                }
            }
        }

        // Step 2 (every replica): reconcile the process-local session
        // registry against terminal rows. This is what actually frees
        // VMs, and it must run regardless of leadership because only the
        // replica holding a session can drop it.
        let local_ids = registry.ids();
        if !local_ids.is_empty() {
            match containers.expired_among(&local_ids).await {
                Ok(expired) => {
                    let mut evicted = 0usize;
                    for id in &expired {
                        if registry.remove(id).is_some() {
                            evicted += 1;
                            tracing::debug!(
                                container_id = %id,
                                "Evicted expired container session from registry"
                            );
                        }
                    }
                    if evicted > 0 {
                        tracing::info!(count = evicted, "Evicted expired container sessions");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Containers reaper registry reconcile failed");
                }
            }
        }

        // `leader_guard` is held across the leader work above and
        // released here as it drops at end of scope, relinquishing
        // leadership before the next interval.
    }
}
