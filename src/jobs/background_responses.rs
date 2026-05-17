//! Background worker that runs `status=queued` responses.
//!
//! When a request arrives with `background=true`, the handler inserts
//! a row with status `queued` and returns immediately. This worker
//! polls for those rows and runs them through the LLM pipeline,
//! letting the persister capture events along the way.
//!
//! Multi-worker safety: `ResponsesRepo::claim_queued` uses
//! `SELECT FOR UPDATE SKIP LOCKED` on Postgres so multiple workers
//! across replicas never claim the same row. SQLite's writer
//! serialization gives equivalent semantics for single-node use.

use std::time::Duration as StdDuration;

use chrono::Utc;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::{
    AppState,
    services::background_executor::{execute_persisted_response, mark_background_failure},
};

/// How often to check for queued work when the previous tick found
/// no rows. We back off slightly so an idle gateway isn't constantly
/// hitting the DB; with work queued, the loop runs in a tight cycle
/// after each row completes.
const IDLE_INTERVAL: StdDuration = StdDuration::from_secs(1);

/// Spawnable entry point. Runs forever under `tokio::spawn`.
pub async fn start_background_response_worker(state: AppState) {
    info!("Starting background response worker");
    loop {
        let Some(ref store) = state.responses_store else {
            // No DB — worker has nothing to do.
            sleep(IDLE_INTERVAL).await;
            continue;
        };
        let Some(ref db) = state.db else {
            sleep(IDLE_INTERVAL).await;
            continue;
        };

        let claim = db.responses().claim_queued(Utc::now()).await;
        match claim {
            Ok(Some(record)) => {
                debug!(
                    response_id = %record.id,
                    model = %record.model,
                    "Claimed queued response"
                );
                let response_id = record.id.clone();
                let exec_state = state.clone();
                let store = store.clone();
                // Spawn the actual work so we can immediately go look
                // for the next claim. Concurrency cap comes from
                // tokio's runtime budget — set
                // [features.responses] worker_concurrency in the
                // future if we need finer control.
                tokio::spawn(async move {
                    if let Err(e) = execute_persisted_response(exec_state, record).await {
                        warn!(
                            response_id = %response_id,
                            error = %e,
                            "Background response execution failed"
                        );
                        mark_background_failure(&store, &response_id, &e).await;
                    }
                });
            }
            Ok(None) => {
                sleep(IDLE_INTERVAL).await;
            }
            Err(e) => {
                error!(error = %e, "claim_queued failed");
                sleep(IDLE_INTERVAL).await;
            }
        }
    }
}
