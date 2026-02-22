use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{
    error::{DlqError, DlqResult},
    traits::{DeadLetterQueue, DlqCursorDirection, DlqEntry, DlqListParams, DlqListResult},
};
use crate::db::{DbPool, DbPoolRef};

/// Database-based dead-letter queue implementation.
///
/// Uses a dedicated table with automatic cleanup based on TTL and max entries.
pub struct DatabaseDlq {
    pool: Arc<DbPool>,
    table_name: String,
    max_entries: u64,
    /// TTL for entries in seconds (used for automatic pruning).
    #[allow(dead_code)] // Set via constructor; reserved for TTL-based pruning
    ttl_secs: u64,
}

impl DatabaseDlq {
    /// Create a new database-based DLQ.
    pub fn new(pool: Arc<DbPool>, table_name: String, max_entries: u64, ttl_secs: u64) -> Self {
        Self {
            pool,
            table_name,
            max_entries,
            ttl_secs,
        }
    }
}

#[async_trait]
impl DeadLetterQueue for DatabaseDlq {
    async fn push(&self, entry: DlqEntry) -> DlqResult<()> {
        let metadata = serde_json::to_string(&entry.metadata)
            .map_err(|e| DlqError::Serialization(e.to_string()))?;

        match self.pool.pool() {
            #[cfg(feature = "database-sqlite")]
            DbPoolRef::Sqlite(pool) => {
                sqlx::query(&format!(
                    r#"
                    INSERT INTO {} (id, entry_type, payload, error, retry_count, created_at, last_retry_at, metadata)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                    self.table_name
                ))
                .bind(entry.id.to_string())
                .bind(&entry.entry_type)
                .bind(&entry.payload)
                .bind(&entry.error)
                .bind(entry.retry_count)
                .bind(entry.created_at)
                .bind(entry.last_retry_at)
                .bind(&metadata)
                .execute(pool)
                .await?;
            }
            #[cfg(feature = "database-postgres")]
            DbPoolRef::Postgres(pools) => {
                sqlx::query(&format!(
                    r#"
                    INSERT INTO {} (id, entry_type, payload, error, retry_count, created_at, last_retry_at, metadata)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    "#,
                    self.table_name
                ))
                .bind(entry.id)
                .bind(&entry.entry_type)
                .bind(&entry.payload)
                .bind(&entry.error)
                .bind(entry.retry_count)
                .bind(entry.created_at)
                .bind(entry.last_retry_at)
                .bind(&metadata)
                .execute(pools.write_pool())
                .await?;
            }
        }

        // Enforce max entries (delete oldest if over limit)
        self.enforce_max_entries().await?;

        Ok(())
    }

    async fn pop(&self) -> DlqResult<Option<DlqEntry>> {
        // Get the oldest entry
        let entry = match self.pool.pool() {
            #[cfg(feature = "database-sqlite")]
            DbPoolRef::Sqlite(pool) => {
                let row = sqlx::query_as::<_, DlqRow>(&format!(
                    r#"SELECT id, entry_type, payload, error, retry_count, created_at, last_retry_at, metadata
                       FROM {} ORDER BY created_at ASC LIMIT 1"#,
                    self.table_name
                ))
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    // Delete it
                    sqlx::query(&format!("DELETE FROM {} WHERE id = ?", self.table_name))
                        .bind(&row.id)
                        .execute(pool)
                        .await?;

                    Some(row.into_entry()?)
                } else {
                    None
                }
            }
            #[cfg(feature = "database-postgres")]
            DbPoolRef::Postgres(pools) => {
                let row = sqlx::query_as::<_, DlqRowPg>(&format!(
                    r#"SELECT id, entry_type, payload, error, retry_count, created_at, last_retry_at, metadata
                       FROM {} ORDER BY created_at ASC LIMIT 1"#,
                    self.table_name
                ))
                .fetch_optional(pools.write_pool())
                .await?;

                if let Some(row) = row {
                    // Delete it
                    sqlx::query(&format!("DELETE FROM {} WHERE id = $1", self.table_name))
                        .bind(row.id)
                        .execute(pools.write_pool())
                        .await?;

                    Some(row.into_entry()?)
                } else {
                    None
                }
            }
        };

        Ok(entry)
    }

    async fn list(&self, params: DlqListParams) -> DlqResult<DlqListResult> {
        let limit = params.limit.unwrap_or(100);

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self.list_with_cursor(&params, cursor, limit).await;
        }

        // First page (no cursor provided)
        self.list_first_page(&params, limit).await
    }

    async fn get(&self, id: Uuid) -> DlqResult<Option<DlqEntry>> {
        let entry = match self.pool.pool() {
            #[cfg(feature = "database-sqlite")]
            DbPoolRef::Sqlite(pool) => {
                let row = sqlx::query_as::<_, DlqRow>(&format!(
                    r#"SELECT id, entry_type, payload, error, retry_count, created_at, last_retry_at, metadata
                       FROM {} WHERE id = ?"#,
                    self.table_name
                ))
                .bind(id.to_string())
                .fetch_optional(pool)
                .await?;

                row.and_then(|r| r.into_entry().ok())
            }
            #[cfg(feature = "database-postgres")]
            DbPoolRef::Postgres(pools) => {
                let row = sqlx::query_as::<_, DlqRowPg>(&format!(
                    r#"SELECT id, entry_type, payload, error, retry_count, created_at, last_retry_at, metadata
                       FROM {} WHERE id = $1"#,
                    self.table_name
                ))
                .bind(id)
                .fetch_optional(pools.read_pool())
                .await?;

                row.and_then(|r| r.into_entry().ok())
            }
        };

        Ok(entry)
    }

    async fn remove(&self, id: Uuid) -> DlqResult<bool> {
        let rows_affected = match self.pool.pool() {
            #[cfg(feature = "database-sqlite")]
            DbPoolRef::Sqlite(pool) => {
                sqlx::query(&format!("DELETE FROM {} WHERE id = ?", self.table_name))
                    .bind(id.to_string())
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
            #[cfg(feature = "database-postgres")]
            DbPoolRef::Postgres(pools) => {
                sqlx::query(&format!("DELETE FROM {} WHERE id = $1", self.table_name))
                    .bind(id)
                    .execute(pools.write_pool())
                    .await?
                    .rows_affected()
            }
        };

        Ok(rows_affected > 0)
    }

    async fn mark_retried(&self, id: Uuid) -> DlqResult<()> {
        let now = Utc::now();

        match self.pool.pool() {
            #[cfg(feature = "database-sqlite")]
            DbPoolRef::Sqlite(pool) => {
                sqlx::query(&format!(
                    "UPDATE {} SET retry_count = retry_count + 1, last_retry_at = ? WHERE id = ?",
                    self.table_name
                ))
                .bind(now)
                .bind(id.to_string())
                .execute(pool)
                .await?;
            }
            #[cfg(feature = "database-postgres")]
            DbPoolRef::Postgres(pools) => {
                sqlx::query(&format!(
                    "UPDATE {} SET retry_count = retry_count + 1, last_retry_at = $1 WHERE id = $2",
                    self.table_name
                ))
                .bind(now)
                .bind(id)
                .execute(pools.write_pool())
                .await?;
            }
        }

        Ok(())
    }

    async fn len(&self) -> DlqResult<u64> {
        let count: i64 = match self.pool.pool() {
            #[cfg(feature = "database-sqlite")]
            DbPoolRef::Sqlite(pool) => {
                sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", self.table_name))
                    .fetch_one(pool)
                    .await?
            }
            #[cfg(feature = "database-postgres")]
            DbPoolRef::Postgres(pools) => {
                sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", self.table_name))
                    .fetch_one(pools.read_pool())
                    .await?
            }
        };

        Ok(count as u64)
    }

    async fn prune(&self, older_than: DateTime<Utc>) -> DlqResult<u64> {
        let rows_affected = match self.pool.pool() {
            #[cfg(feature = "database-sqlite")]
            DbPoolRef::Sqlite(pool) => sqlx::query(&format!(
                "DELETE FROM {} WHERE created_at < ?",
                self.table_name
            ))
            .bind(older_than)
            .execute(pool)
            .await?
            .rows_affected(),
            #[cfg(feature = "database-postgres")]
            DbPoolRef::Postgres(pools) => sqlx::query(&format!(
                "DELETE FROM {} WHERE created_at < $1",
                self.table_name
            ))
            .bind(older_than)
            .execute(pools.write_pool())
            .await?
            .rows_affected(),
        };

        Ok(rows_affected)
    }

    async fn clear(&self) -> DlqResult<u64> {
        let rows_affected = match self.pool.pool() {
            #[cfg(feature = "database-sqlite")]
            DbPoolRef::Sqlite(pool) => sqlx::query(&format!("DELETE FROM {}", self.table_name))
                .execute(pool)
                .await?
                .rows_affected(),
            #[cfg(feature = "database-postgres")]
            DbPoolRef::Postgres(pools) => sqlx::query(&format!("DELETE FROM {}", self.table_name))
                .execute(pools.write_pool())
                .await?
                .rows_affected(),
        };

        Ok(rows_affected)
    }
}

use super::traits::DlqCursor;

impl DatabaseDlq {
    /// List entries with cursor-based pagination.
    async fn list_with_cursor(
        &self,
        params: &DlqListParams,
        cursor: &DlqCursor,
        limit: i64,
    ) -> DlqResult<DlqListResult> {
        let is_backward = params.direction == DlqCursorDirection::Backward;
        // Fetch one extra to determine has_more
        let fetch_limit = limit + 1;

        let mut entries = match self.pool.pool() {
            #[cfg(feature = "database-sqlite")]
            DbPoolRef::Sqlite(pool) => {
                // Match repos cursor pattern: default DESC, Forward=older items, Backward=newer items
                let (cursor_condition, order_direction) = if is_backward {
                    // Backward: get entries AFTER the cursor (newer), sorted ASC, then reverse
                    ("(created_at, id) > (?, ?)", "ASC")
                } else {
                    // Forward: get entries BEFORE the cursor (older), sorted DESC
                    ("(created_at, id) < (?, ?)", "DESC")
                };

                let mut query = format!(
                    r#"SELECT id, entry_type, payload, error, retry_count, created_at, last_retry_at, metadata
                       FROM {} WHERE {}"#,
                    self.table_name, cursor_condition
                );

                if params.entry_type.is_some() {
                    query.push_str(" AND entry_type = ?");
                }
                if params.older_than.is_some() {
                    query.push_str(" AND created_at < ?");
                }
                if params.max_retries.is_some() {
                    query.push_str(" AND retry_count < ?");
                }

                query.push_str(&format!(
                    " ORDER BY created_at {}, id {} LIMIT ?",
                    order_direction, order_direction
                ));

                let mut q = sqlx::query_as::<_, DlqRow>(&query);

                // Bind cursor values
                q = q.bind(cursor.created_at).bind(cursor.id.to_string());

                if let Some(ref entry_type) = params.entry_type {
                    q = q.bind(entry_type);
                }
                if let Some(older_than) = params.older_than {
                    q = q.bind(older_than);
                }
                if let Some(max_retries) = params.max_retries {
                    q = q.bind(max_retries);
                }

                q = q.bind(fetch_limit);

                let rows = q.fetch_all(pool).await?;
                rows.into_iter()
                    .filter_map(|r| r.into_entry().ok())
                    .collect::<Vec<_>>()
            }
            #[cfg(feature = "database-postgres")]
            DbPoolRef::Postgres(pools) => {
                // Match repos cursor pattern: default DESC, Forward=older items, Backward=newer items
                let (cursor_condition, order_direction) = if is_backward {
                    // Backward: get entries AFTER the cursor (newer), sorted ASC, then reverse
                    ("ROW(created_at, id) > ROW($1, $2)", "ASC")
                } else {
                    // Forward: get entries BEFORE the cursor (older), sorted DESC
                    ("ROW(created_at, id) < ROW($1, $2)", "DESC")
                };

                let mut query = format!(
                    r#"SELECT id, entry_type, payload, error, retry_count, created_at, last_retry_at, metadata
                       FROM {} WHERE {}"#,
                    self.table_name, cursor_condition
                );

                let mut param_idx = 3;
                if params.entry_type.is_some() {
                    query.push_str(&format!(" AND entry_type = ${}", param_idx));
                    param_idx += 1;
                }
                if params.older_than.is_some() {
                    query.push_str(&format!(" AND created_at < ${}", param_idx));
                    param_idx += 1;
                }
                if params.max_retries.is_some() {
                    query.push_str(&format!(" AND retry_count < ${}", param_idx));
                    param_idx += 1;
                }

                query.push_str(&format!(
                    " ORDER BY created_at {}, id {} LIMIT ${}",
                    order_direction, order_direction, param_idx
                ));

                let mut q = sqlx::query_as::<_, DlqRowPg>(&query);

                // Bind cursor values
                q = q.bind(cursor.created_at).bind(cursor.id);

                if let Some(ref entry_type) = params.entry_type {
                    q = q.bind(entry_type);
                }
                if let Some(older_than) = params.older_than {
                    q = q.bind(older_than);
                }
                if let Some(max_retries) = params.max_retries {
                    q = q.bind(max_retries);
                }

                q = q.bind(fetch_limit);

                let rows = q.fetch_all(pools.read_pool()).await?;
                rows.into_iter()
                    .filter_map(|r| r.into_entry().ok())
                    .collect::<Vec<_>>()
            }
        };

        // Check if there are more entries
        let has_more = entries.len() as i64 > limit;
        if has_more {
            entries.pop(); // Remove the extra entry
        }

        // For backward pagination, reverse the results to maintain correct order
        if is_backward {
            entries.reverse();
        }

        Ok(DlqListResult::new(
            entries,
            has_more,
            params.direction,
            Some(cursor),
        ))
    }

    /// List entries for the first page (no cursor provided).
    async fn list_first_page(
        &self,
        params: &DlqListParams,
        limit: i64,
    ) -> DlqResult<DlqListResult> {
        // Fetch one extra to determine has_more
        let fetch_limit = limit + 1;

        let mut entries = match self.pool.pool() {
            #[cfg(feature = "database-sqlite")]
            DbPoolRef::Sqlite(pool) => {
                let mut query = format!(
                    r#"SELECT id, entry_type, payload, error, retry_count, created_at, last_retry_at, metadata
                       FROM {} WHERE 1=1"#,
                    self.table_name
                );

                if params.entry_type.is_some() {
                    query.push_str(" AND entry_type = ?");
                }
                if params.older_than.is_some() {
                    query.push_str(" AND created_at < ?");
                }
                if params.max_retries.is_some() {
                    query.push_str(" AND retry_count < ?");
                }

                // Match repos cursor pattern: default DESC (newest first)
                query.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");

                let mut q = sqlx::query_as::<_, DlqRow>(&query);

                if let Some(ref entry_type) = params.entry_type {
                    q = q.bind(entry_type);
                }
                if let Some(older_than) = params.older_than {
                    q = q.bind(older_than);
                }
                if let Some(max_retries) = params.max_retries {
                    q = q.bind(max_retries);
                }

                q = q.bind(fetch_limit);

                let rows = q.fetch_all(pool).await?;
                rows.into_iter()
                    .filter_map(|r| r.into_entry().ok())
                    .collect::<Vec<_>>()
            }
            #[cfg(feature = "database-postgres")]
            DbPoolRef::Postgres(pools) => {
                let mut query = format!(
                    r#"SELECT id, entry_type, payload, error, retry_count, created_at, last_retry_at, metadata
                       FROM {} WHERE true"#,
                    self.table_name
                );

                let mut param_idx = 1;
                if params.entry_type.is_some() {
                    query.push_str(&format!(" AND entry_type = ${}", param_idx));
                    param_idx += 1;
                }
                if params.older_than.is_some() {
                    query.push_str(&format!(" AND created_at < ${}", param_idx));
                    param_idx += 1;
                }
                if params.max_retries.is_some() {
                    query.push_str(&format!(" AND retry_count < ${}", param_idx));
                    param_idx += 1;
                }

                // Match repos cursor pattern: default DESC (newest first)
                query.push_str(&format!(
                    " ORDER BY created_at DESC, id DESC LIMIT ${}",
                    param_idx
                ));

                let mut q = sqlx::query_as::<_, DlqRowPg>(&query);

                if let Some(ref entry_type) = params.entry_type {
                    q = q.bind(entry_type);
                }
                if let Some(older_than) = params.older_than {
                    q = q.bind(older_than);
                }
                if let Some(max_retries) = params.max_retries {
                    q = q.bind(max_retries);
                }

                q = q.bind(fetch_limit);

                let rows = q.fetch_all(pools.read_pool()).await?;
                rows.into_iter()
                    .filter_map(|r| r.into_entry().ok())
                    .collect::<Vec<_>>()
            }
        };

        // Check if there are more entries
        let has_more = entries.len() as i64 > limit;
        if has_more {
            entries.pop(); // Remove the extra entry
        }

        Ok(DlqListResult::new(
            entries,
            has_more,
            DlqCursorDirection::Forward,
            None,
        ))
    }

    async fn enforce_max_entries(&self) -> DlqResult<()> {
        let count = self.len().await?;

        if count > self.max_entries {
            let to_delete = count - self.max_entries;

            match self.pool.pool() {
                #[cfg(feature = "database-sqlite")]
                DbPoolRef::Sqlite(pool) => {
                    sqlx::query(&format!(
                        "DELETE FROM {} WHERE id IN (SELECT id FROM {} ORDER BY created_at ASC LIMIT ?)",
                        self.table_name, self.table_name
                    ))
                    .bind(to_delete as i64)
                    .execute(pool)
                    .await?;
                }
                #[cfg(feature = "database-postgres")]
                DbPoolRef::Postgres(pools) => {
                    sqlx::query(&format!(
                        "DELETE FROM {} WHERE id IN (SELECT id FROM {} ORDER BY created_at ASC LIMIT $1)",
                        self.table_name, self.table_name
                    ))
                    .bind(to_delete as i64)
                    .execute(pools.write_pool())
                    .await?;
                }
            }
        }

        Ok(())
    }
}

// Helper types for SQLite row mapping
#[cfg(feature = "database-sqlite")]
#[derive(sqlx::FromRow)]
struct DlqRow {
    id: String,
    entry_type: String,
    payload: String,
    error: String,
    retry_count: i32,
    created_at: DateTime<Utc>,
    last_retry_at: Option<DateTime<Utc>>,
    metadata: String,
}

#[cfg(feature = "database-sqlite")]
impl DlqRow {
    fn into_entry(self) -> DlqResult<DlqEntry> {
        let id = Uuid::parse_str(&self.id)
            .map_err(|e| DlqError::Deserialization(format!("Invalid UUID: {}", e)))?;

        let metadata = serde_json::from_str(&self.metadata)
            .map_err(|e| DlqError::Deserialization(format!("Invalid metadata JSON: {}", e)))?;

        Ok(DlqEntry {
            id,
            entry_type: self.entry_type,
            payload: self.payload,
            error: self.error,
            retry_count: self.retry_count,
            created_at: self.created_at,
            last_retry_at: self.last_retry_at,
            metadata,
        })
    }
}

// Helper types for PostgreSQL row mapping
#[cfg(feature = "database-postgres")]
#[derive(sqlx::FromRow)]
struct DlqRowPg {
    id: Uuid,
    entry_type: String,
    payload: String,
    error: String,
    retry_count: i32,
    created_at: DateTime<Utc>,
    last_retry_at: Option<DateTime<Utc>>,
    metadata: String,
}

#[cfg(feature = "database-postgres")]
impl DlqRowPg {
    fn into_entry(self) -> DlqResult<DlqEntry> {
        let metadata = serde_json::from_str(&self.metadata)
            .map_err(|e| DlqError::Deserialization(format!("Invalid metadata JSON: {}", e)))?;

        Ok(DlqEntry {
            id: self.id,
            entry_type: self.entry_type,
            payload: self.payload,
            error: self.error,
            retry_count: self.retry_count,
            created_at: self.created_at,
            last_retry_at: self.last_retry_at,
            metadata,
        })
    }
}
