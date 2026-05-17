//! Retention worker for the `responses` table.
//!
//! Once a row reaches a terminal status (`completed`, `failed`,
//! `cancelled`, `incomplete`) and passes its `retention_expires_at`
//! timestamp, this worker deletes it. The window is configured via
//! `[features.responses] retention_secs`; scan cadence via
//! `cleanup_interval_secs`.

use std::{sync::Arc, time::Duration as StdDuration};

use chrono::Utc;
use tokio::time::sleep;

use crate::{
    jobs::leader_lock::{self, LeadershipOutcome, keys},
    services::ResponsesStore,
};

/// Loop forever; runs under `tokio::spawn`. Exits naturally if the
/// task is dropped.
pub async fn start_responses_retention_worker(
    store: Arc<ResponsesStore>,
    db: Arc<crate::db::DbPool>,
    cleanup_interval: StdDuration,
) {
    tracing::info!(
        interval_secs = cleanup_interval.as_secs(),
        "Starting responses retention worker"
    );

    loop {
        sleep(cleanup_interval).await;

        let _guard = match leader_lock::try_acquire(&db, keys::RESPONSES_RETENTION).await {
            LeadershipOutcome::Leader(g) => Some(g),
            LeadershipOutcome::NotLeader => {
                tracing::trace!("responses_retention: not leader, skipping");
                continue;
            }
            LeadershipOutcome::NoCoordination => None,
        };

        match store.prune_expired(Utc::now()).await {
            Ok(0) => {}
            Ok(n) => tracing::debug!(deleted = n, "Pruned expired response rows"),
            Err(e) => tracing::warn!(error = %e, "Responses retention pass failed"),
        }
    }
}
