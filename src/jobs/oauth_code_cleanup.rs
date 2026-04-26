//! Background cleanup for stale OAuth PKCE authorization codes.
//!
//! Codes are short-lived (default TTL = 10 minutes) and become useless the
//! moment they're consumed or expire. This worker periodically removes them
//! to keep `oauth_authorization_codes` from growing without bound. It does
//! NOT enforce a retention policy — there is nothing to retain — so it
//! always runs whenever the database is configured.

use std::{sync::Arc, time::Duration as StdDuration};

use chrono::Utc;
use tokio::time::sleep;

use crate::{
    db::DbPool,
    jobs::leader_lock::{self, LeadershipOutcome, keys},
};

/// How often to run the cleanup pass. The query is a single indexed DELETE,
/// so a 10-minute cadence is cheap and keeps the table near-empty even
/// under heavy OAuth traffic.
const CLEANUP_INTERVAL: StdDuration = StdDuration::from_secs(600);

/// Spawnable entry point. Loops indefinitely; intended to run under
/// `tokio::spawn`.
pub async fn start_oauth_code_cleanup_worker(db: Arc<DbPool>) {
    tracing::info!(
        interval_secs = CLEANUP_INTERVAL.as_secs(),
        "Starting OAuth authorization code cleanup worker"
    );

    loop {
        // Sleep first so we don't race the rest of startup.
        sleep(CLEANUP_INTERVAL).await;

        // Multi-replica deployments would otherwise have every replica fire
        // this same DELETE every interval; the advisory lock makes one
        // replica per tick the leader, the rest skip.
        let _guard = match leader_lock::try_acquire(&db, keys::OAUTH_CODE_CLEANUP).await {
            LeadershipOutcome::Leader(g) => Some(g),
            LeadershipOutcome::NotLeader => {
                tracing::trace!("oauth_code_cleanup: not leader this tick, skipping");
                continue;
            }
            LeadershipOutcome::NoCoordination => None,
        };

        let now = Utc::now();
        match db.oauth_authorization_codes().delete_stale(now).await {
            Ok(0) => {}
            Ok(n) => {
                tracing::debug!(deleted = n, "Cleaned up OAuth authorization codes");
            }
            Err(err) => {
                tracing::warn!(error = %err, "OAuth authorization code cleanup failed");
            }
        }
    }
}
