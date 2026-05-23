//! Persistence repo for the Responses API.
//!
//! Stores rows the gateway emits per request when `store=true` (the
//! default per OpenAI's spec). Powers `GET /v1/responses/{id}`,
//! `POST /v1/responses/{id}/cancel`, `DELETE /v1/responses/{id}`, and
//! the background-mode poll/replay flows.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::db::error::DbResult;

/// Ownership scope for a stored response. Mirrors the pattern used
/// by `skills`, `templates`, `conversations` etc.: every row is owned
/// by one of organization/team/project/user/service-account, and
/// reads cascade through the org-scope filter so any caller in the
/// same org with the right RBAC can retrieve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseOwnerType {
    Organization,
    Team,
    Project,
    User,
    ServiceAccount,
}

impl ResponseOwnerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Organization => "organization",
            Self::Team => "team",
            Self::Project => "project",
            Self::User => "user",
            Self::ServiceAccount => "service_account",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "organization" => Some(Self::Organization),
            "team" => Some(Self::Team),
            "project" => Some(Self::Project),
            "user" => Some(Self::User),
            "service_account" => Some(Self::ServiceAccount),
            _ => None,
        }
    }
}

/// Tagged owner specification — the canonical way to express
/// "<scope, id>" in code. Repos consume the flat
/// (`owner_type`, `owner_id`) pair; this enum is what handlers build
/// when deriving an owner from the calling principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseOwner {
    Organization(Uuid),
    Team(Uuid),
    Project(Uuid),
    User(Uuid),
    ServiceAccount(Uuid),
}

impl ResponseOwner {
    pub fn owner_type(&self) -> ResponseOwnerType {
        match self {
            Self::Organization(_) => ResponseOwnerType::Organization,
            Self::Team(_) => ResponseOwnerType::Team,
            Self::Project(_) => ResponseOwnerType::Project,
            Self::User(_) => ResponseOwnerType::User,
            Self::ServiceAccount(_) => ResponseOwnerType::ServiceAccount,
        }
    }

    pub fn owner_id(&self) -> Uuid {
        match self {
            Self::Organization(id)
            | Self::Team(id)
            | Self::Project(id)
            | Self::User(id)
            | Self::ServiceAccount(id) => *id,
        }
    }

    /// Rebuild an owner from the flat (`owner_type`, `owner_id`) pair a
    /// repo row stores.
    pub fn from_parts(owner_type: ResponseOwnerType, owner_id: Uuid) -> Self {
        match owner_type {
            ResponseOwnerType::Organization => Self::Organization(owner_id),
            ResponseOwnerType::Team => Self::Team(owner_id),
            ResponseOwnerType::Project => Self::Project(owner_id),
            ResponseOwnerType::User => Self::User(owner_id),
            ResponseOwnerType::ServiceAccount => Self::ServiceAccount(owner_id),
        }
    }

    /// Map to the Files / Vector-Store owner pair used for tenant
    /// scoping. Returns `None` for `ServiceAccount`, which has no
    /// file-owner equivalent (files are owned by org/team/project/user) —
    /// callers treat `None` as "no file is in this scope" and fail
    /// closed, so a service-account-owned request can't reference
    /// Files-API uploads by id.
    pub fn as_file_owner(&self) -> Option<(crate::models::VectorStoreOwnerType, Uuid)> {
        use crate::models::VectorStoreOwnerType as V;
        match self {
            Self::Organization(id) => Some((V::Organization, *id)),
            Self::Team(id) => Some((V::Team, *id)),
            Self::Project(id) => Some((V::Project, *id)),
            Self::User(id) => Some((V::User, *id)),
            Self::ServiceAccount(_) => None,
        }
    }
}

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
    pub org_id: Uuid,
    /// Ownership scope. Determines which scope's listing the row
    /// shows up in; reads / writes / deletes cascade through the org
    /// the owner belongs to.
    pub owner_type: ResponseOwnerType,
    pub owner_id: Uuid,
    /// Audit field: which project's API key (if any) made the call.
    /// Distinct from ownership — an API key bound to a project can
    /// submit a response owned by the user, for instance.
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
    /// Liveness heartbeat for `in_progress` rows, stamped periodically
    /// by the executing worker. `None` until the first heartbeat (the
    /// reaper falls back to `started_at`).
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    /// Container the shell-tool session for this response was attached
    /// to. `None` when the request had no shell tool, when containers
    /// persistence is disabled, or before the shell tool first runs.
    /// Drives `previous_response_id`-based reuse: a chained response
    /// looks here to find which container to reattach to.
    pub container_id: Option<String>,
}

/// Fields needed to create a new response row at request-start time.
#[derive(Debug, Clone)]
pub struct NewResponse {
    pub id: String,
    pub org_id: Uuid,
    pub owner_type: ResponseOwnerType,
    pub owner_id: Uuid,
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
    /// Container attached to this response. Patched in by the
    /// streaming pipeline once a shell-tool session has been
    /// provisioned (or reattached), so a follow-up request that
    /// chains via `previous_response_id` can resolve the right
    /// container.
    pub container_id: Option<String>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ResponsesRepo: Send + Sync {
    /// Insert a new response row at request-start. Status is typically
    /// `Queued` (for background) or `InProgress` (synchronous).
    async fn insert(&self, input: NewResponse) -> DbResult<ResponseRecord>;

    /// Org-scoped fetch by ID. Returns `None` when the row is missing
    /// **or** belongs to a different org — the caller can't distinguish
    /// the two cases, which prevents enumeration attacks. The `org_id`
    /// is required: anonymous-mode deployments populate a synthetic
    /// default via the auth middleware, so all reachable rows have a
    /// non-NULL org.
    async fn get_by_id_and_org(&self, id: &str, org_id: Uuid) -> DbResult<Option<ResponseRecord>>;

    /// Patch lifecycle fields, scoped to a tenant. The repo applies
    /// only the `Some` fields in `patch`, so callers can advance status
    /// and set completed_at/output/usage in one call. Returns `None` if
    /// no row matches `id` AND `org_id` — protects against cross-tenant
    /// writes even when a caller has the ID.
    async fn update_within_org(
        &self,
        id: &str,
        org_id: Uuid,
        patch: ResponseCompletion,
    ) -> DbResult<Option<ResponseRecord>>;

    /// Like [`update_within_org`](Self::update_within_org) but the
    /// UPDATE additionally requires the row's *current* status to be
    /// non-terminal. Returns `Some(record)` only when this call actually
    /// performed the transition; `None` when the row was already
    /// terminal (a lost race) or doesn't match `org_id`.
    ///
    /// This makes terminal lifecycle transitions atomic at the DB layer:
    /// a late `completed` write from the persister cannot clobber an
    /// earlier `cancelled`, and the affected-row outcome is what drives
    /// exactly-once webhook firing — never a read-then-write.
    async fn complete_within_org(
        &self,
        id: &str,
        org_id: Uuid,
        patch: ResponseCompletion,
    ) -> DbResult<Option<ResponseRecord>>;

    /// Stamp the liveness heartbeat for an `in_progress` row. No-op for
    /// rows in any other status (so a late ping can't resurrect a
    /// terminal row's heartbeat). Best-effort: drives the in-progress
    /// reaper's dead-worker detection.
    async fn touch_heartbeat(&self, id: &str, now: DateTime<Utc>) -> DbResult<()>;

    /// Org-scoped delete. Returns true if a row was removed.
    async fn delete_by_id_and_org(&self, id: &str, org_id: Uuid) -> DbResult<bool>;

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

    /// Given a set of in-flight response IDs, return the subset whose
    /// `status='cancelled'`. Used by the cross-replica cancel poller
    /// to detect cancels issued on a *different* replica than the one
    /// executing the response. One batched query per poll cycle
    /// regardless of in-flight count.
    async fn list_cancelled_among(&self, ids: &[String]) -> DbResult<Vec<String>>;

    /// Mark rows in `in_progress` whose `started_at` is older than
    /// `started_before` as `failed`, stamping a `worker_lost` error
    /// payload. Returns the number of rows updated. Called by the
    /// reaper to recover from worker crashes — without this, a row
    /// claimed by a worker that died mid-execution would stay in
    /// `in_progress` forever (`claim_queued` only picks `queued`).
    async fn reap_stuck_in_progress(
        &self,
        started_before: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        retention_expires_at: DateTime<Utc>,
    ) -> DbResult<u64>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::VectorStoreOwnerType as V;

    #[test]
    fn response_owner_maps_to_file_owner_for_tenant_scoping() {
        let id = Uuid::new_v4();
        // Org/team/project/user map straight across.
        assert_eq!(
            ResponseOwner::Organization(id).as_file_owner(),
            Some((V::Organization, id))
        );
        assert_eq!(ResponseOwner::Team(id).as_file_owner(), Some((V::Team, id)));
        assert_eq!(
            ResponseOwner::Project(id).as_file_owner(),
            Some((V::Project, id))
        );
        assert_eq!(ResponseOwner::User(id).as_file_owner(), Some((V::User, id)));
        // Service accounts have no file-owner equivalent → fail closed:
        // an SA-owned request can't reference Files-API uploads by id.
        assert_eq!(ResponseOwner::ServiceAccount(id).as_file_owner(), None);
    }

    #[test]
    fn response_owner_from_parts_roundtrips() {
        let id = Uuid::new_v4();
        for owner in [
            ResponseOwner::Organization(id),
            ResponseOwner::Team(id),
            ResponseOwner::Project(id),
            ResponseOwner::User(id),
            ResponseOwner::ServiceAccount(id),
        ] {
            assert_eq!(
                ResponseOwner::from_parts(owner.owner_type(), owner.owner_id()),
                owner
            );
        }
    }
}
