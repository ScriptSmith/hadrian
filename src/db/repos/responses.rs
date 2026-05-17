//! Persistence repo for the Responses API.
//!
//! Stores rows the gateway emits per request when `store=true` (the
//! default per OpenAI's spec). Powers `GET /v1/responses/{id}`,
//! `POST /v1/responses/{id}/cancel`, `DELETE /v1/responses/{id}`, and
//! the background-mode poll/replay flows that arrive in Phase 3.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::db::error::DbResult;

/// Lifecycle states for a stored response, mirroring OpenAI's
/// `ResponsesResponseStatus`. The wire-format strings match exactly so
/// the column can be deserialized directly into the API type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseStatus {
    Queued,
    InProgress,
    Completed,
    Failed,
    Cancelled,
    Incomplete,
}

impl ResponseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Incomplete => "incomplete",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(Self::Queued),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "incomplete" => Some(Self::Incomplete),
            _ => None,
        }
    }

    /// Terminal states no longer accept status transitions and are
    /// eligible for retention-based cleanup.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Incomplete
        )
    }
}

/// A persisted Responses API record.
///
/// `request_payload`, `output`, `usage`, and `error` are kept as
/// opaque JSON values so the API surface can evolve without further
/// migrations. The route handler is responsible for stitching these
/// back into a `CreateResponsesResponse` for the wire.
#[derive(Debug, Clone)]
pub struct ResponseRecord {
    pub id: String,
    pub org_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub status: ResponseStatus,
    pub background: bool,
    pub model: String,
    pub provider: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub request_payload: Value,
    pub output: Option<Value>,
    pub usage: Option<Value>,
    pub error: Option<Value>,
    pub retention_expires_at: DateTime<Utc>,
    /// Highest event sequence_number persisted. Set by the event
    /// buffer drainer on each batch flush.
    pub last_sequence_number: i64,
}

/// Fields needed to create a new response row at request-start time.
#[derive(Debug, Clone)]
pub struct NewResponse {
    pub id: String,
    pub org_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    pub status: ResponseStatus,
    pub background: bool,
    pub model: String,
    pub provider: Option<String>,
    pub created_at: DateTime<Utc>,
    pub request_payload: Value,
    pub retention_expires_at: DateTime<Utc>,
}

/// Fields to patch into an existing row when the response reaches a
/// terminal state.
#[derive(Debug, Clone, Default)]
pub struct ResponseCompletion {
    pub status: Option<ResponseStatus>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub output: Option<Value>,
    pub usage: Option<Value>,
    pub error: Option<Value>,
    pub retention_expires_at: Option<DateTime<Utc>>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ResponsesRepo: Send + Sync {
    /// Insert a new response row at request-start. Status is typically
    /// `Queued` (for background) or `InProgress` (synchronous).
    async fn insert(&self, input: NewResponse) -> DbResult<ResponseRecord>;

    /// Org-scoped fetch by ID. Returns `None` when the row is missing
    /// **or** belongs to a different org — the caller can't distinguish
    /// the two cases, which prevents enumeration attacks. Pass
    /// `org_id = None` to look up org-less rows (rare; mainly tests).
    async fn get_by_id_and_org(
        &self,
        id: &str,
        org_id: Option<Uuid>,
    ) -> DbResult<Option<ResponseRecord>>;

    /// Patch lifecycle fields. The repo applies only the `Some` fields
    /// in `patch`, so callers can advance status and set
    /// completed_at/output/usage in one call.
    async fn update(&self, id: &str, patch: ResponseCompletion)
    -> DbResult<Option<ResponseRecord>>;

    /// Org-scoped delete. Returns true if a row was removed.
    async fn delete_by_id_and_org(&self, id: &str, org_id: Option<Uuid>) -> DbResult<bool>;

    /// Delete all rows past `before` whose status is terminal. Run by
    /// the retention worker.
    async fn delete_expired(&self, before: DateTime<Utc>) -> DbResult<u64>;

    /// Atomically claim the next `queued` row, transitioning it to
    /// `in_progress` and stamping `started_at`. Returns `None` when
    /// nothing is queued. Multi-worker-safe: SQLite uses
    /// `UPDATE … RETURNING` against the rowid SELECT; Postgres uses
    /// `SELECT … FOR UPDATE SKIP LOCKED` so concurrent workers never
    /// claim the same row.
    async fn claim_queued(&self, now: DateTime<Utc>) -> DbResult<Option<ResponseRecord>>;
}
