//! Admin API endpoints for Dead Letter Queue management.

use std::collections::HashMap;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AdminError;
use crate::{
    AppState,
    dlq::{DeadLetterQueue, DlqCursor, DlqCursorDirection, DlqEntry, DlqListParams},
    middleware::AuthzContext,
    models::UsageLogEntry,
    observability::metrics,
    openapi::PaginationMeta,
};

/// Query parameters for listing DLQ entries with cursor-based pagination.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
pub struct DlqListQuery {
    /// Filter by entry type (e.g., "usage_log").
    pub entry_type: Option<String>,
    /// Maximum number of entries to return (default: 100).
    pub limit: Option<i64>,
    /// Only return entries with fewer than this many retries.
    pub max_retries: Option<i32>,
    /// Cursor for keyset pagination. Encoded as base64 string.
    #[cfg_attr(
        feature = "utoipa",
        schema(example = "MTczMzU4MDgwMDAwMDphYmMxMjM0NS02Nzg5LTAxMjMtNDU2Ny0wMTIzNDU2Nzg5YWI")
    )]
    pub cursor: Option<String>,
    /// Pagination direction: "forward" (default) or "backward".
    #[serde(default)]
    pub direction: Option<String>,
}

impl DlqListQuery {
    /// Convert to DlqListParams with cursor support, returning an error for invalid cursors.
    pub fn try_into_params(self) -> Result<DlqListParams, AdminError> {
        let cursor = match &self.cursor {
            Some(c) => Some(
                DlqCursor::decode(c)
                    .ok_or_else(|| AdminError::BadRequest(format!("Invalid cursor: {}", c)))?,
            ),
            None => None,
        };

        let direction = match self.direction.as_deref() {
            Some("backward") => DlqCursorDirection::Backward,
            Some("forward") | None => DlqCursorDirection::Forward,
            Some(other) => {
                return Err(AdminError::BadRequest(format!(
                    "Invalid direction '{}': must be 'forward' or 'backward'",
                    other
                )));
            }
        };

        Ok(DlqListParams {
            entry_type: self.entry_type,
            limit: self.limit.or(Some(100)),
            older_than: None,
            max_retries: self.max_retries,
            cursor,
            direction,
        })
    }
}

/// Paginated list of DLQ entries.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DlqListResponse {
    /// List of DLQ entries.
    pub data: Vec<DlqEntryResponse>,
    /// Pagination metadata.
    pub pagination: PaginationMeta,
}

/// DLQ entry response.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DlqEntryResponse {
    /// Unique entry ID.
    pub id: Uuid,
    /// Type of the failed operation.
    pub entry_type: String,
    /// The serialized payload that failed.
    pub payload: serde_json::Value,
    /// Error message from the failed operation.
    pub error: String,
    /// Number of retry attempts.
    pub retry_count: i32,
    /// When the entry was created (RFC3339).
    pub created_at: String,
    /// When the entry was last retried (RFC3339).
    pub last_retry_at: Option<String>,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

impl From<DlqEntry> for DlqEntryResponse {
    fn from(entry: DlqEntry) -> Self {
        // Try to parse payload as JSON, fall back to string representation
        let payload = serde_json::from_str(&entry.payload)
            .unwrap_or(serde_json::Value::String(entry.payload));

        Self {
            id: entry.id,
            entry_type: entry.entry_type,
            payload,
            error: entry.error,
            retry_count: entry.retry_count,
            created_at: entry.created_at.to_rfc3339(),
            last_retry_at: entry.last_retry_at.map(|dt| dt.to_rfc3339()),
            metadata: entry.metadata,
        }
    }
}

/// DLQ statistics response.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DlqStatsResponse {
    /// Total number of entries in the queue.
    pub total_entries: u64,
    /// Whether the queue is empty.
    pub is_empty: bool,
    /// Breakdown by entry type.
    pub by_type: HashMap<String, u64>,
    /// Breakdown by retry count.
    pub by_retry_count: HashMap<i32, u64>,
}

/// Result of a retry operation.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DlqRetryResponse {
    /// Whether the retry was successful.
    pub success: bool,
    /// Message describing the result.
    pub message: String,
}

fn get_dlq(state: &AppState) -> Result<&std::sync::Arc<dyn DeadLetterQueue>, AdminError> {
    state
        .dlq
        .as_ref()
        .ok_or_else(|| AdminError::BadRequest("Dead letter queue is not configured".to_string()))
}

/// List DLQ entries.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/dlq",
    tag = "dlq",
    operation_id = "dlq_list",
    params(DlqListQuery),
    responses(
        (status = 200, description = "List of DLQ entries", body = DlqListResponse),
        (status = 400, description = "DLQ not configured or invalid cursor", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Query(query): Query<DlqListQuery>,
) -> Result<Json<DlqListResponse>, AdminError> {
    authz.require("dlq", "list", None, None, None, None)?;
    let dlq = get_dlq(&state)?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_params()?;

    let result = dlq.list(params).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to list DLQ entries");
        AdminError::Internal(e.to_string())
    })?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(DlqListResponse {
        data: result.items.into_iter().map(Into::into).collect(),
        pagination,
    }))
}

/// Get a specific DLQ entry by ID.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/dlq/{id}",
    tag = "dlq",
    operation_id = "dlq_get",
    params(
        ("id" = Uuid, Path, description = "DLQ entry ID"),
    ),
    responses(
        (status = 200, description = "DLQ entry", body = DlqEntryResponse),
        (status = 400, description = "DLQ not configured", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Entry not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<DlqEntryResponse>, AdminError> {
    authz.require("dlq", "read", None, None, None, None)?;
    let dlq = get_dlq(&state)?;

    let entry = dlq.get(id).await.map_err(|e| {
        tracing::error!(error = %e, entry_id = %id, "Failed to get DLQ entry");
        AdminError::Internal(e.to_string())
    })?;

    match entry {
        Some(e) => Ok(Json(e.into())),
        None => Err(AdminError::NotFound("DLQ entry".to_string())),
    }
}

/// Delete a DLQ entry.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/dlq/{id}",
    tag = "dlq",
    operation_id = "dlq_delete",
    params(
        ("id" = Uuid, Path, description = "DLQ entry ID"),
    ),
    responses(
        (status = 200, description = "Entry deleted"),
        (status = 400, description = "DLQ not configured", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Entry not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AdminError> {
    authz.require("dlq", "delete", None, None, None, None)?;
    let dlq = get_dlq(&state)?;

    let removed = dlq.remove(id).await.map_err(|e| {
        tracing::error!(error = %e, entry_id = %id, "Failed to delete DLQ entry");
        AdminError::Internal(e.to_string())
    })?;

    if removed {
        metrics::record_dlq_operation("delete", "admin");
        Ok(Json(serde_json::json!({"deleted": true})))
    } else {
        Err(AdminError::NotFound("DLQ entry".to_string()))
    }
}

/// Retry a specific DLQ entry.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/dlq/{id}/retry",
    tag = "dlq",
    operation_id = "dlq_retry",
    params(
        ("id" = Uuid, Path, description = "DLQ entry ID"),
    ),
    responses(
        (status = 200, description = "Retry result", body = DlqRetryResponse),
        (status = 400, description = "DLQ not configured or invalid entry", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Entry not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn retry(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<DlqRetryResponse>, AdminError> {
    authz.require("dlq", "update", None, None, None, None)?;
    let dlq = get_dlq(&state)?;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| AdminError::BadRequest("Database is not configured".to_string()))?;

    // Get the entry
    let entry = dlq.get(id).await.map_err(|e| {
        tracing::error!(error = %e, entry_id = %id, "Failed to get DLQ entry for retry");
        AdminError::Internal(e.to_string())
    })?;

    let entry = match entry {
        Some(e) => e,
        None => return Err(AdminError::NotFound("DLQ entry".to_string())),
    };

    // Process based on entry type
    let result = match entry.entry_type.as_str() {
        "usage_log" => {
            // Parse the usage log entry
            let usage_entry: UsageLogEntry = serde_json::from_str(&entry.payload)
                .map_err(|e| AdminError::BadRequest(format!("Invalid usage_log payload: {}", e)))?;

            // Try to write to database
            match db.usage().log(usage_entry).await {
                Ok(_) => {
                    // Success - remove from queue
                    if let Err(e) = dlq.remove(id).await {
                        tracing::error!(error = %e, entry_id = %id, "Failed to remove successfully retried entry");
                    }
                    metrics::record_dlq_operation("manual_retry_success", &entry.entry_type);
                    DlqRetryResponse {
                        success: true,
                        message: "Entry processed and removed from queue".to_string(),
                    }
                }
                Err(e) => {
                    // Failed - mark as retried
                    if let Err(mark_err) = dlq.mark_retried(id).await {
                        tracing::error!(error = %mark_err, entry_id = %id, "Failed to mark entry as retried");
                    }
                    metrics::record_dlq_operation("manual_retry_failure", &entry.entry_type);
                    DlqRetryResponse {
                        success: false,
                        message: format!("Retry failed: {}", e),
                    }
                }
            }
        }
        _ => {
            return Err(AdminError::BadRequest(format!(
                "Unsupported entry type for manual retry: {}",
                entry.entry_type
            )));
        }
    };

    Ok(Json(result))
}

/// Get DLQ statistics.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/dlq/stats",
    tag = "dlq",
    operation_id = "dlq_stats",
    responses(
        (status = 200, description = "DLQ statistics", body = DlqStatsResponse),
        (status = 400, description = "DLQ not configured", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn stats(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<DlqStatsResponse>, AdminError> {
    authz.require("dlq", "read", None, None, None, None)?;
    let dlq = get_dlq(&state)?;

    // Get total count
    let total_entries = dlq.len().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to get DLQ length");
        AdminError::Internal(e.to_string())
    })?;

    let is_empty = total_entries == 0;

    // Get all entries for breakdown (limited for performance)
    let params = DlqListParams {
        entry_type: None,
        limit: Some(10000), // Reasonable limit for stats
        older_than: None,
        max_retries: None,
        cursor: None,
        direction: DlqCursorDirection::Forward,
    };

    let result = dlq.list(params).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to list DLQ entries for stats");
        AdminError::Internal(e.to_string())
    })?;

    // Calculate breakdowns
    let mut by_type: HashMap<String, u64> = HashMap::new();
    let mut by_retry_count: HashMap<i32, u64> = HashMap::new();

    for entry in result.items {
        *by_type.entry(entry.entry_type).or_insert(0) += 1;
        *by_retry_count.entry(entry.retry_count).or_insert(0) += 1;
    }

    Ok(Json(DlqStatsResponse {
        total_entries,
        is_empty,
        by_type,
        by_retry_count,
    }))
}

/// Purge all DLQ entries.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/dlq",
    tag = "dlq",
    operation_id = "dlq_purge",
    responses(
        (status = 200, description = "Purge result", body = serde_json::Value),
        (status = 400, description = "DLQ not configured", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn purge(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<serde_json::Value>, AdminError> {
    authz.require("dlq", "delete", None, None, None, None)?;
    let dlq = get_dlq(&state)?;

    let count = dlq.clear().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to purge DLQ");
        AdminError::Internal(e.to_string())
    })?;

    metrics::record_dlq_operation("purge", "all");
    tracing::info!(count = count, "DLQ purged via admin API");

    Ok(Json(serde_json::json!({
        "purged": count
    })))
}

/// Prune old DLQ entries based on age.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
pub struct PruneQuery {
    /// Prune entries older than this many seconds (default: TTL from config).
    pub older_than_secs: Option<u64>,
}

#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/dlq/prune",
    tag = "dlq",
    operation_id = "dlq_prune",
    params(PruneQuery),
    responses(
        (status = 200, description = "Prune result", body = serde_json::Value),
        (status = 400, description = "DLQ not configured", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn prune(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Query(query): Query<PruneQuery>,
) -> Result<Json<serde_json::Value>, AdminError> {
    authz.require("dlq", "delete", None, None, None, None)?;
    let dlq = get_dlq(&state)?;

    // Get TTL from config or use provided value
    let ttl_secs = query.older_than_secs.unwrap_or_else(|| {
        state
            .config
            .observability
            .dead_letter_queue
            .as_ref()
            .map(|c| c.ttl_secs())
            .unwrap_or(86400 * 7) // Default 7 days
    });

    let cutoff = Utc::now() - chrono::Duration::seconds(ttl_secs as i64);

    let count = dlq.prune(cutoff).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to prune DLQ");
        AdminError::Internal(e.to_string())
    })?;

    metrics::record_dlq_operation("manual_prune", "all");
    tracing::info!(
        count = count,
        older_than_secs = ttl_secs,
        "DLQ pruned via admin API"
    );

    Ok(Json(serde_json::json!({
        "pruned": count,
        "older_than_secs": ttl_secs
    })))
}
