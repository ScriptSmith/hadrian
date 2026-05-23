//! Service layer for persisted Responses API records.
//!
//! Wraps `ResponsesRepo` with:
//! - Response ID generation (`resp_<base32>` to match OpenAI's pattern)
//! - Retention-window computation from config
//! - In-process cancellation signaling — each persisted response maps
//!   to a `tokio::sync::watch` channel that the streaming pipeline can
//!   listen to so `POST /v1/responses/{id}/cancel` can abort in-flight
//!   execution as well as updating the DB row.
//!
//! Naming this module `responses_store` (not just `responses`) avoids a
//! collision with the existing `api_types::responses` import surface.

use std::{collections::HashMap, sync::Arc, time::Duration as StdDuration};

use chrono::{DateTime, Duration, Utc};
use thiserror::Error;
use tokio::sync::{Mutex, watch};
use uuid::Uuid;

use crate::{
    db::{
        DbError, DbPool,
        repos::{NewResponse, ResponseCompletion, ResponseRecord, ResponseStatus, ResponsesRepo},
    },
    services::responses_webhook::ResponsesWebhookDispatcher,
};

#[derive(Debug, Error)]
pub enum ResponsesStoreError {
    #[error("response not found")]
    NotFound,
    #[error("response is not background and cannot be cancelled")]
    NotBackground,
    #[error("database error: {0}")]
    Database(#[from] DbError),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type ResponsesStoreResult<T> = Result<T, ResponsesStoreError>;

/// Cancellation signal for a single in-flight response. The streaming
/// pipeline holds a `watch::Receiver<bool>` and polls it; the cancel
/// route handler flips it via the matching `Sender`.
///
/// # Cross-replica cancel protocol
///
/// A single response can be created on replica A, executed on replica
/// B (by the background worker after `claim_queued`), and cancelled
/// on replica C (because the user retried the cancel POST against a
/// different load-balancer target). The in-process `cancel_senders`
/// map only knows about its local replica's executions, so the cancel
/// has to flow through the database. The protocol is:
///
/// 1. Replica C's `cancel()` handler flips the in-process flag (no-op
///    on C if the row isn't executing there) AND writes
///    `status='cancelled'` to the DB.
/// 2. On every replica, [`jobs::responses_cancel_poller`] periodically
///    queries `WHERE status='cancelled' AND id IN (active set)` against
///    the `responses` table and trips the matching local cancel sender.
/// 3. On replica B (the executor), the cancel sender fires, the
///    persister's `tokio::select!` on `cancel_rx.changed()` wins, the
///    stream terminates, and a synthetic `response.cancelled` event is
///    appended to the log so polling clients converge to the same view.
///
/// **Why a poller and not a notify/listen channel?** Hadrian deploys
/// equally on SQLite (no LISTEN/NOTIFY) and Postgres; one batched
/// query per cycle is one DB round-trip regardless of in-flight count
/// and avoids per-execution polling tasks. The cancel latency floor is
/// `[features.responses]` cleanup_interval bounded by `POLL_INTERVAL`
/// in the poller job (currently 5s).
///
/// **What happens between the cancel and the poller tick?** The DB
/// row already says `cancelled`; new `GET /v1/responses/{id}` calls
/// see the terminal state immediately. The executor on replica B keeps
/// streaming until the poller fires, at which point the persister
/// truncates the stream and writes the synthetic terminal event. The
/// row's `status` doesn't flip back to in_progress in the interim.
pub type CancelSignal = watch::Receiver<bool>;

/// Service for persisted Responses API records.
#[derive(Clone)]
pub struct ResponsesStore {
    repo: Arc<dyn ResponsesRepo>,
    retention: StdDuration,
    /// Per-response cancellation senders. Removed when the response
    /// reaches a terminal state.
    cancel_senders: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    /// Optional webhook fired on terminal-state transitions.
    webhook: Option<ResponsesWebhookDispatcher>,
}

impl ResponsesStore {
    pub fn new(db: Arc<DbPool>, retention: StdDuration) -> Self {
        Self {
            repo: db.responses(),
            retention,
            cancel_senders: Arc::new(Mutex::new(HashMap::new())),
            webhook: None,
        }
    }

    /// Attach a webhook dispatcher. Fired once per response when the
    /// row first transitions into a terminal state.
    pub fn with_webhook(mut self, webhook: ResponsesWebhookDispatcher) -> Self {
        self.webhook = Some(webhook);
        self
    }

    /// Look up an active cancel signal for a given response. Returns
    /// `None` when the row has already reached a terminal state, was
    /// never created in this process (e.g. claimed across a restart),
    /// or doesn't exist. Background workers should fall back to
    /// status-polling in those cases.
    pub async fn subscribe_cancel(&self, response_id: &str) -> Option<CancelSignal> {
        self.cancel_senders
            .lock()
            .await
            .get(response_id)
            .map(|sender| sender.subscribe())
    }

    /// Register an in-flight execution that didn't go through
    /// `create()` on this replica — e.g., a background worker that
    /// just claimed a row created on another replica or before a
    /// restart. Returns a receiver that the local replica-wide cancel
    /// poller can trip via `trip_cancel_in_process`. Idempotent:
    /// returns a subscriber on the existing sender if the id is
    /// already registered.
    pub async fn register_external_execution(&self, response_id: &str) -> CancelSignal {
        let mut senders = self.cancel_senders.lock().await;
        if let Some(existing) = senders.get(response_id) {
            return existing.subscribe();
        }
        let (tx, rx) = watch::channel(false);
        senders.insert(response_id.to_string(), tx);
        rx
    }

    /// Generate a new response ID matching OpenAI's `resp_<random>`
    /// pattern. The random suffix is a base32-encoded UUID (no padding,
    /// lowercase) to be URL-safe.
    pub fn new_response_id() -> String {
        let bytes = Uuid::new_v4().into_bytes();
        let mut suffix = String::with_capacity(26);
        const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
        let mut buffer = 0u64;
        let mut bits = 0u8;
        for byte in &bytes {
            buffer = (buffer << 8) | (*byte as u64);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                let idx = ((buffer >> bits) & 0x1f) as usize;
                suffix.push(ALPHABET[idx] as char);
            }
        }
        if bits > 0 {
            let idx = ((buffer << (5 - bits)) & 0x1f) as usize;
            suffix.push(ALPHABET[idx] as char);
        }
        format!("resp_{suffix}")
    }

    /// Compute the retention timestamp from `now()` plus the configured
    /// retention window. Terminal records are eligible for deletion
    /// past this time.
    pub fn retention_expires_at(&self, from: DateTime<Utc>) -> DateTime<Utc> {
        from + Duration::from_std(self.retention).unwrap_or_else(|_| Duration::seconds(86_400))
    }

    /// Insert a fresh response row. Returns the record and a
    /// cancellation receiver the caller should pass into the streaming
    /// pipeline so `cancel()` can interrupt mid-flight execution.
    pub async fn create(
        &self,
        input: NewResponse,
    ) -> ResponsesStoreResult<(ResponseRecord, CancelSignal)> {
        let record = self.repo.insert(input.clone()).await?;
        let (tx, rx) = watch::channel(false);
        self.cancel_senders
            .lock()
            .await
            .insert(record.id.clone(), tx);
        Ok((record, rx))
    }

    /// Patch a response's lifecycle fields. Tenant-scoped: a wrong
    /// `org_id` returns `NotFound` rather than writing into someone
    /// else's row.
    ///
    /// Terminal transitions go through the DB-guarded
    /// [`ResponsesRepo::complete_within_org`], which only matches a
    /// non-terminal row. This makes the transition atomic: a late
    /// `completed` write from the persister can't clobber an earlier
    /// `cancelled` (lost-cancel), and exactly-once webhook firing is
    /// driven off whether *this* call actually flipped the row — never
    /// a read-then-write that two racing writers could both pass.
    pub async fn update_within_org(
        &self,
        id: &str,
        org_id: Uuid,
        mut patch: ResponseCompletion,
    ) -> ResponsesStoreResult<ResponseRecord> {
        let to_terminal = patch.status.is_some_and(|s| s.is_terminal());
        if !to_terminal {
            // Non-terminal patch (started_at, container_id, …): plain update.
            return self
                .repo
                .update_within_org(id, org_id, patch)
                .await?
                .ok_or(ResponsesStoreError::NotFound);
        }

        // When transitioning to a terminal state we also stamp the
        // retention expiry so the cleanup worker has a deadline.
        if patch.retention_expires_at.is_none() {
            patch.retention_expires_at = Some(self.retention_expires_at(Utc::now()));
        }
        match self.repo.complete_within_org(id, org_id, patch).await? {
            // We won the race to flip the row terminal: drop the cancel
            // sender and fire the webhook exactly once.
            Some(record) => {
                self.cancel_senders.lock().await.remove(id);
                if let Some(ref webhook) = self.webhook {
                    webhook.enqueue(record.id.clone(), record.status, record.background);
                }
                Ok(record)
            }
            // Row was already terminal (lost race) or wrong org. Don't
            // overwrite, don't fire the webhook — return the current
            // record (NotFound if it doesn't exist in this org).
            None => self
                .repo
                .get_by_id_and_org(id, org_id)
                .await?
                .ok_or(ResponsesStoreError::NotFound),
        }
    }

    /// Org-scoped fetch.
    pub async fn get(&self, id: &str, org_id: Uuid) -> ResponsesStoreResult<ResponseRecord> {
        self.repo
            .get_by_id_and_org(id, org_id)
            .await?
            .ok_or(ResponsesStoreError::NotFound)
    }

    /// Org-scoped delete. Idempotent — succeeds even when the row is
    /// already gone (we still return Ok so `DELETE` is idempotent per
    /// REST convention).
    pub async fn delete(&self, id: &str, org_id: Uuid) -> ResponsesStoreResult<bool> {
        let removed = self.repo.delete_by_id_and_org(id, org_id).await?;
        if removed {
            self.cancel_senders.lock().await.remove(id);
        }
        Ok(removed)
    }

    /// Trip the cancel signal AND mark the row cancelled.
    ///
    /// Per OpenAI's spec, cancel only succeeds when `background=true`.
    /// Idempotent for already-terminal rows: returns the existing
    /// record without re-patching (which would otherwise churn
    /// completed_at and risk a stray webhook fire).
    pub async fn cancel(&self, id: &str, org_id: Uuid) -> ResponsesStoreResult<ResponseRecord> {
        let record = self.get(id, org_id).await?;
        if !record.background {
            return Err(ResponsesStoreError::NotBackground);
        }
        if record.status.is_terminal() {
            return Ok(record);
        }
        // Trip the in-process flag first so any in-flight stream sees
        // it before we update the DB.
        if let Some(tx) = self.cancel_senders.lock().await.get(id) {
            let _ = tx.send(true);
        }
        self.update_within_org(
            id,
            org_id,
            ResponseCompletion {
                status: Some(ResponseStatus::Cancelled),
                completed_at: Some(Utc::now()),
                ..Default::default()
            },
        )
        .await
    }

    /// Stamp the liveness heartbeat for an in-progress response. Called
    /// periodically by the persister while a response streams so the
    /// in-progress reaper can tell a healthy long-running response from a
    /// dead worker. Best-effort: a failure is logged, not propagated.
    pub async fn touch_heartbeat(&self, id: &str) {
        if let Err(e) = self.repo.touch_heartbeat(id, Utc::now()).await {
            tracing::debug!(response_id = %id, error = %e, "heartbeat stamp failed");
        }
    }

    /// Run by the retention worker — delete records past their
    /// expiry. Returns the number of rows removed.
    pub async fn prune_expired(&self, before: DateTime<Utc>) -> ResponsesStoreResult<u64> {
        Ok(self.repo.delete_expired(before).await?)
    }

    /// Reap rows stuck in `in_progress`. Used by the retention
    /// worker when a worker that claimed a row died mid-execution
    /// (claim_queued only picks rows in `queued`, so without this
    /// they'd linger forever). Marks them `failed` with a
    /// `worker_lost` error payload and stamps a fresh
    /// retention_expires_at so they're pruned on the normal cycle.
    pub async fn reap_stuck(&self, max_age: StdDuration) -> ResponsesStoreResult<u64> {
        let now = Utc::now();
        let cutoff = match Duration::from_std(max_age) {
            Ok(d) => now - d,
            Err(_) => now - Duration::hours(1),
        };
        let retention = self.retention_expires_at(now);
        Ok(self
            .repo
            .reap_stuck_in_progress(cutoff, now, retention)
            .await?)
    }

    /// Snapshot of response IDs currently registered with an in-flight
    /// persister. Used by the cross-replica cancel poller to build the
    /// "active set" it polls each cycle.
    pub async fn active_response_ids(&self) -> Vec<String> {
        self.cancel_senders.lock().await.keys().cloned().collect()
    }

    /// Trip the in-process cancel signal for a response without
    /// touching the DB. Called by the cross-replica cancel poller
    /// when another replica has flipped `status='cancelled'`; the
    /// row update already happened on the originating replica.
    pub async fn trip_cancel_in_process(&self, id: &str) {
        if let Some(tx) = self.cancel_senders.lock().await.get(id) {
            let _ = tx.send(true);
        }
    }

    /// Poll the DB once for any `status='cancelled'` rows in the
    /// in-flight set and trip their watch signals. Designed to be
    /// driven by a single replica-wide task (see
    /// `jobs::responses_cancel_poller`), replacing per-execution
    /// pollers.
    pub async fn poll_external_cancels(&self) -> ResponsesStoreResult<usize> {
        let active = self.active_response_ids().await;
        if active.is_empty() {
            return Ok(0);
        }
        let cancelled = self.repo.list_cancelled_among(&active).await?;
        let mut tripped = 0;
        for id in &cancelled {
            self.trip_cancel_in_process(id).await;
            tripped += 1;
        }
        Ok(tripped)
    }
}
