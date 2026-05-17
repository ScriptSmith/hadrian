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

    /// Patch a response's lifecycle fields. Removes the cancellation
    /// sender when the response enters a terminal state.
    pub async fn update(
        &self,
        id: &str,
        mut patch: ResponseCompletion,
    ) -> ResponsesStoreResult<ResponseRecord> {
        // When transitioning to a terminal state we also stamp the
        // retention expiry so the cleanup worker has a deadline.
        if let Some(status) = patch.status
            && status.is_terminal()
            && patch.retention_expires_at.is_none()
        {
            patch.retention_expires_at = Some(self.retention_expires_at(Utc::now()));
        }
        let record = self
            .repo
            .update(id, patch)
            .await?
            .ok_or(ResponsesStoreError::NotFound)?;
        if record.status.is_terminal() {
            self.cancel_senders.lock().await.remove(id);
            if let Some(ref webhook) = self.webhook {
                webhook.enqueue(record.id.clone(), record.status, record.background);
            }
        }
        Ok(record)
    }

    /// Org-scoped fetch.
    pub async fn get(
        &self,
        id: &str,
        org_id: Option<Uuid>,
    ) -> ResponsesStoreResult<ResponseRecord> {
        self.repo
            .get_by_id_and_org(id, org_id)
            .await?
            .ok_or(ResponsesStoreError::NotFound)
    }

    /// Org-scoped delete. Idempotent — succeeds even when the row is
    /// already gone (we still return Ok so `DELETE` is idempotent per
    /// REST convention).
    pub async fn delete(&self, id: &str, org_id: Option<Uuid>) -> ResponsesStoreResult<bool> {
        let removed = self.repo.delete_by_id_and_org(id, org_id).await?;
        if removed {
            self.cancel_senders.lock().await.remove(id);
        }
        Ok(removed)
    }

    /// Trip the cancel signal AND mark the row cancelled.
    ///
    /// Per OpenAI's spec, cancel only succeeds when `background=true`.
    /// Returns the updated record.
    pub async fn cancel(
        &self,
        id: &str,
        org_id: Option<Uuid>,
    ) -> ResponsesStoreResult<ResponseRecord> {
        let record = self.get(id, org_id).await?;
        if !record.background {
            return Err(ResponsesStoreError::NotBackground);
        }
        // Trip the in-process flag first so any in-flight stream sees
        // it before we update the DB.
        if let Some(tx) = self.cancel_senders.lock().await.get(id) {
            let _ = tx.send(true);
        }
        self.update(
            id,
            ResponseCompletion {
                status: Some(ResponseStatus::Cancelled),
                completed_at: Some(Utc::now()),
                ..Default::default()
            },
        )
        .await
    }

    /// Run by the retention worker — delete records past their
    /// expiry. Returns the number of rows removed.
    pub async fn prune_expired(&self, before: DateTime<Utc>) -> ResponsesStoreResult<u64> {
        Ok(self.repo.delete_expired(before).await?)
    }
}
