//! Append-only event log for in-flight Responses API streams.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::db::error::DbResult;

/// One row in `response_events`. Owned representation; the wire
/// format emitted by the replay endpoint is the same JSON as the
/// live SSE stream, so callers serialize `payload` directly.
#[derive(Debug, Clone)]
pub struct ResponseEvent {
    pub response_id: String,
    pub sequence_number: i64,
    pub event_type: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

/// Fields supplied by the streaming pipeline when persisting one
/// event. `sequence_number` is gateway-authoritative — assigned in
/// order as events flow to the client, so retries don't create gaps.
#[derive(Debug, Clone)]
pub struct NewResponseEvent {
    pub response_id: String,
    pub sequence_number: i64,
    pub event_type: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ResponseEventsRepo: Send + Sync {
    /// Batch-insert events. Conflicts on the (response_id,
    /// sequence_number) PK are ignored — the buffer may double-flush
    /// on retry and the DB enforces idempotency.
    async fn insert_batch(&self, events: Vec<NewResponseEvent>) -> DbResult<u64>;

    /// Return events for a response with sequence_number > `after`,
    /// in ascending order, up to `limit`.
    async fn list_after(
        &self,
        response_id: &str,
        after: i64,
        limit: i64,
    ) -> DbResult<Vec<ResponseEvent>>;

    /// Update the parent `responses.last_sequence_number` so the
    /// replay endpoint knows when there's nothing more coming.
    async fn set_last_sequence(&self, response_id: &str, seq: i64) -> DbResult<()>;
}
