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

use std::{sync::Arc, time::Duration as StdDuration};

use chrono::Utc;
use tokio::{sync::Semaphore, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::{
    AppState,
    db::repos::ResponseCompletion,
    services::background_executor::{
        BackgroundExecuteError, execute_persisted_response, mark_background_failure,
    },
};

/// How often to check for queued work when the previous tick found
/// no rows. We back off slightly so an idle gateway isn't constantly
/// hitting the DB; with work queued, the loop runs in a tight cycle
/// after each row completes.
const IDLE_INTERVAL: StdDuration = StdDuration::from_secs(1);

/// Spawnable entry point. Runs forever under `tokio::spawn` until the
/// `shutdown_token` is cancelled. Bounded concurrency via a semaphore
/// sized at `[features.responses] worker_concurrency` — without a cap,
/// a flood of `background=true` requests would `tokio::spawn` one task
/// per claim and saturate the replica.
pub async fn start_background_response_worker(state: AppState, shutdown: CancellationToken) {
    let concurrency = state.config.features.responses.worker_concurrency.max(1);
    let semaphore = Arc::new(Semaphore::new(concurrency));
    info!(
        worker_concurrency = concurrency,
        "Starting background response worker"
    );
    loop {
        if shutdown.is_cancelled() {
            info!("Background response worker received shutdown signal");
            return;
        }
        let Some(ref store) = state.responses_store else {
            tokio::select! {
                _ = shutdown.cancelled() => return,
                _ = sleep(IDLE_INTERVAL) => continue,
            }
        };
        let Some(ref db) = state.db else {
            tokio::select! {
                _ = shutdown.cancelled() => return,
                _ = sleep(IDLE_INTERVAL) => continue,
            }
        };

        // Wait for a slot before claiming, so we don't pop a row off
        // the queue we have no capacity to execute.
        let permit = tokio::select! {
            _ = shutdown.cancelled() => return,
            permit = Arc::clone(&semaphore).acquire_owned() => permit,
        };
        let permit = match permit {
            Ok(p) => p,
            Err(_) => return, // semaphore closed
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
                let response_org = record.org_id;
                let exec_state = state.clone();
                let store = store.clone();
                let retry_config = state.config.features.responses.retry.clone();
                // Track the execution on the shared task tracker so a
                // SIGTERM drains an in-flight background response instead
                // of abandoning it mid-stream (later force-reaped as
                // `worker_lost`). The claim loop above stops on the
                // shutdown token; this keeps the work it already claimed.
                let tracker = exec_state.task_tracker.clone();
                tracker.spawn(async move {
                    let result =
                        run_with_retry(&exec_state, store.clone(), record, retry_config).await;
                    if let Err(e) = result {
                        warn!(
                            response_id = %response_id,
                            error = %e,
                            "Background response execution failed after retries"
                        );
                        mark_background_failure(&store, &response_id, response_org, &e).await;
                    }
                    drop(permit);
                });
            }
            Ok(None) => {
                drop(permit);
                tokio::select! {
                    _ = shutdown.cancelled() => return,
                    _ = sleep(IDLE_INTERVAL) => {}
                }
            }
            Err(e) => {
                drop(permit);
                error!(error = %e, "claim_queued failed");
                tokio::select! {
                    _ = shutdown.cancelled() => return,
                    _ = sleep(IDLE_INTERVAL) => {}
                }
            }
        }
    }
}

/// Execute a claimed response with retry on transient failures.
///
/// Permanent failures (BadPayload/Routing/Resolution/NoStore) return
/// immediately. Between attempts we re-load the row so the persister
/// resumes from the updated `last_sequence_number` and bump
/// `started_at` so the in-progress reaper doesn't kill an active
/// retry.
async fn run_with_retry(
    state: &AppState,
    store: Arc<crate::services::ResponsesStore>,
    initial_record: crate::db::repos::ResponseRecord,
    retry: crate::config::ResponsesRetryConfig,
) -> Result<(), BackgroundExecuteError> {
    let max_attempts = if retry.enabled { retry.max_attempts } else { 1 };
    let response_id = initial_record.id.clone();
    let org_id = initial_record.org_id;
    let mut record = initial_record;

    for attempt in 1..=max_attempts {
        let result = execute_persisted_response(state.clone(), record.clone()).await;
        match result {
            Ok(()) => return Ok(()),
            Err(e) if !e.is_transient() => return Err(e),
            Err(e) if attempt == max_attempts => return Err(e),
            Err(e) => {
                let backoff = retry.backoff_for_attempt(attempt);
                warn!(
                    response_id = %response_id,
                    attempt,
                    max_attempts,
                    backoff_ms = backoff.as_millis() as u64,
                    error = %e,
                    "Background response attempt failed; retrying"
                );
                sleep(backoff).await;

                // Re-fetch the row so the next attempt picks up the
                // up-to-date `last_sequence_number` (the previous
                // attempt's persister wrote partial events). Refresh
                // `started_at` while we're at it so the reaper sees
                // this as fresh work.
                if let Err(update_err) = store
                    .update_within_org(
                        &response_id,
                        org_id,
                        ResponseCompletion {
                            started_at: Some(Utc::now()),
                            ..Default::default()
                        },
                    )
                    .await
                {
                    warn!(
                        response_id = %response_id,
                        error = %update_err,
                        "Failed to bump started_at before retry; will retry anyway"
                    );
                }
                match store.get(&response_id, org_id).await {
                    Ok(r) => record = r,
                    Err(get_err) => {
                        warn!(
                            response_id = %response_id,
                            error = %get_err,
                            "Failed to reload record between retries"
                        );
                        return Err(BackgroundExecuteError::Execution(get_err.to_string()));
                    }
                }
            }
        }
    }
    // Unreachable: the loop always returns or hits `attempt ==
    // max_attempts`. The conditional construction keeps the compiler
    // happy without an explicit unreachable!().
    Ok(())
}
