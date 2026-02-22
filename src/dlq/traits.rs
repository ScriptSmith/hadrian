use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::DlqResult;

/// A dead-letter queue entry containing a failed operation and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqEntry {
    /// Unique ID for this entry.
    pub id: Uuid,
    /// Type of the failed operation (e.g., "usage_log", "webhook").
    pub entry_type: String,
    /// The serialized payload that failed to process.
    pub payload: String,
    /// Error message from the failed operation.
    pub error: String,
    /// Number of times this entry has been retried.
    pub retry_count: i32,
    /// When this entry was first created.
    pub created_at: DateTime<Utc>,
    /// When this entry was last retried.
    pub last_retry_at: Option<DateTime<Utc>>,
    /// Additional metadata (e.g., source, trace ID).
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

/// A cursor for DLQ keyset pagination, encoding a position in an ordered result set.
///
/// The cursor encodes both `created_at` timestamp and `id` to provide a unique,
/// stable ordering even when multiple records have the same timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlqCursor {
    /// The timestamp component of the cursor position.
    pub created_at: DateTime<Utc>,
    /// The UUID component of the cursor position.
    pub id: Uuid,
}

impl DlqCursor {
    /// Create a new cursor from a timestamp and ID.
    pub fn new(created_at: DateTime<Utc>, id: Uuid) -> Self {
        Self { created_at, id }
    }

    /// Encode the cursor as a URL-safe base64 string.
    pub fn encode(&self) -> String {
        let timestamp_millis = self.created_at.timestamp_millis();
        let raw = format!("{}:{}", timestamp_millis, self.id);
        URL_SAFE_NO_PAD.encode(raw.as_bytes())
    }

    /// Decode a cursor from a base64 string.
    pub fn decode(encoded: &str) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
        let raw = String::from_utf8(bytes).ok()?;
        let (timestamp_str, uuid_str) = raw.split_once(':')?;
        let timestamp_millis: i64 = timestamp_str.parse().ok()?;
        let created_at = DateTime::from_timestamp_millis(timestamp_millis)?;
        let id = Uuid::parse_str(uuid_str).ok()?;
        Some(Self { created_at, id })
    }

    /// Create a cursor from a DLQ entry.
    pub fn from_entry(entry: &DlqEntry) -> Self {
        Self::new(entry.created_at, entry.id)
    }
}

/// Direction for cursor-based pagination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DlqCursorDirection {
    /// Fetch items after the cursor (newer items).
    #[default]
    Forward,
    /// Fetch items before the cursor (older items).
    Backward,
}

/// Cursors for navigating DLQ paginated results.
#[derive(Debug, Clone, Default)]
pub struct DlqPageCursors {
    /// Cursor for the next page (if more items exist).
    pub next: Option<DlqCursor>,
    /// Cursor for the previous page (if not on first page).
    pub prev: Option<DlqCursor>,
}

impl DlqPageCursors {
    /// Create cursors from a list of DLQ entries.
    pub fn from_entries(
        entries: &[DlqEntry],
        has_more: bool,
        direction: DlqCursorDirection,
        cursor: Option<&DlqCursor>,
    ) -> Self {
        if entries.is_empty() {
            return Self::default();
        }

        let first = DlqCursor::from_entry(&entries[0]);
        let last = DlqCursor::from_entry(&entries[entries.len() - 1]);

        match direction {
            DlqCursorDirection::Forward => Self {
                next: if has_more { Some(last) } else { None },
                prev: cursor.map(|_| first),
            },
            DlqCursorDirection::Backward => Self {
                next: cursor.map(|_| first),
                prev: if has_more { Some(last) } else { None },
            },
        }
    }
}

/// Result of a paginated DLQ list query.
#[derive(Debug, Clone)]
pub struct DlqListResult {
    /// The entries returned for this page.
    pub items: Vec<DlqEntry>,
    /// Whether there are more items after this page.
    pub has_more: bool,
    /// Cursors for navigating to next/previous pages.
    pub cursors: DlqPageCursors,
}

impl DlqListResult {
    /// Create a new list result with cursor information.
    pub fn new(
        items: Vec<DlqEntry>,
        has_more: bool,
        direction: DlqCursorDirection,
        cursor: Option<&DlqCursor>,
    ) -> Self {
        let cursors = DlqPageCursors::from_entries(&items, has_more, direction, cursor);
        Self {
            items,
            has_more,
            cursors,
        }
    }
}

/// Truncate a DateTime to millisecond precision.
///
/// This is important for cursor-based pagination because cursors encode timestamps
/// as milliseconds. Without truncation, the cursor's timestamp (ms precision) won't
/// match the stored timestamp (ns precision), causing string comparison issues in SQLite.
fn truncate_to_millis(dt: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(dt.timestamp_millis()).unwrap_or(dt)
}

impl DlqEntry {
    /// Create a new DLQ entry.
    ///
    /// Note: The timestamp is truncated to millisecond precision to ensure consistent
    /// cursor-based pagination. See `truncate_to_millis` for details.
    pub fn new(
        entry_type: impl Into<String>,
        payload: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            entry_type: entry_type.into(),
            payload: payload.into(),
            error: error.into(),
            retry_count: 0,
            created_at: truncate_to_millis(Utc::now()),
            last_retry_at: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add metadata to the entry.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Parameters for listing DLQ entries with cursor-based pagination.
#[derive(Debug, Clone, Default)]
pub struct DlqListParams {
    /// Filter by entry type.
    pub entry_type: Option<String>,
    /// Maximum number of entries to return.
    pub limit: Option<i64>,
    /// Only return entries older than this time.
    pub older_than: Option<DateTime<Utc>>,
    /// Only return entries with fewer than this many retries.
    pub max_retries: Option<i32>,
    /// Cursor for keyset pagination.
    pub cursor: Option<DlqCursor>,
    /// Direction for cursor-based pagination.
    pub direction: DlqCursorDirection,
}

impl DlqListParams {
    /// Returns true if cursor-based pagination is being used.
    pub fn is_cursor_based(&self) -> bool {
        self.cursor.is_some()
    }
}

/// Dead-letter queue trait for storing failed operations.
///
/// Implementations must be thread-safe and support concurrent access.
#[async_trait]
pub trait DeadLetterQueue: Send + Sync {
    /// Push an entry to the dead-letter queue.
    async fn push(&self, entry: DlqEntry) -> DlqResult<()>;

    /// Pop the oldest entry from the queue (for reprocessing).
    /// Returns None if the queue is empty.
    async fn pop(&self) -> DlqResult<Option<DlqEntry>>;

    /// Peek at entries without removing them using cursor-based pagination.
    async fn list(&self, params: DlqListParams) -> DlqResult<DlqListResult>;

    /// Get a specific entry by ID.
    async fn get(&self, id: Uuid) -> DlqResult<Option<DlqEntry>>;

    /// Remove an entry by ID (after successful reprocessing).
    async fn remove(&self, id: Uuid) -> DlqResult<bool>;

    /// Update retry count and last_retry_at for an entry.
    async fn mark_retried(&self, id: Uuid) -> DlqResult<()>;

    /// Get the current queue size.
    async fn len(&self) -> DlqResult<u64>;

    /// Check if the queue is empty.
    async fn is_empty(&self) -> DlqResult<bool> {
        Ok(self.len().await? == 0)
    }

    /// Prune old entries (returns number of entries removed).
    async fn prune(&self, older_than: DateTime<Utc>) -> DlqResult<u64>;

    /// Clear all entries from the queue.
    async fn clear(&self) -> DlqResult<u64>;
}
