//! Single replica-wide poller for cross-replica response cancels.
//!
//! When `POST /v1/responses/{id}/cancel` runs on replica A but the
//! row is executing on replica B, the in-process watch channel on
//! B doesn't see the flip. This worker polls every N seconds: one
//! batched query `WHERE status='cancelled' AND id IN (active set)`
//! that trips the matching watch sender on B. The previous
//! per-execution poller cost one task + one DB round-trip per
//! in-flight response; this is one task + one query for the whole
//! replica.

use std::{sync::Arc, time::Duration as StdDuration};

use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::services::ResponsesStore;

/// Default poll cadence. Matches the previous per-execution
/// polling interval so cancel latency doesn't regress.
const POLL_INTERVAL: StdDuration = StdDuration::from_secs(5);

/// Spawnable entry point. Exits when `shutdown` is cancelled.
pub async fn start_responses_cancel_poller(
    store: Arc<ResponsesStore>,
    shutdown: CancellationToken,
) {
    tracing::info!("Starting responses cancel poller");
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                tracing::info!("Responses cancel poller received shutdown signal");
                return;
            }
            _ = sleep(POLL_INTERVAL) => {}
        }

        match store.poll_external_cancels().await {
            Ok(0) => {}
            Ok(n) => {
                tracing::debug!(tripped = n, "Cross-replica cancels delivered");
            }
            Err(e) => {
                tracing::warn!(error = %e, "Cancel poll failed");
            }
        }
    }
}
