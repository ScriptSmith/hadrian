//! SQLite implementation of [`ResponseEventsRepo`].

use async_trait::async_trait;
use chrono::DateTime;

use super::backend::{Pool, RowExt, query};
use crate::db::{
    error::DbResult,
    repos::{NewResponseEvent, ResponseEvent, ResponseEventsRepo, truncate_to_millis},
};

pub struct SqliteResponseEventsRepo {
    pool: Pool,
}

impl SqliteResponseEventsRepo {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ResponseEventsRepo for SqliteResponseEventsRepo {
    async fn insert_batch(&self, events: Vec<NewResponseEvent>) -> DbResult<u64> {
        if events.is_empty() {
            return Ok(0);
        }
        // Each row uses 5 params. SQLite caps at 999 — stay under
        // with a chunk of 100 (500 params).
        const CHUNK: usize = 100;
        let mut total = 0u64;
        for chunk in events.chunks(CHUNK) {
            let placeholders: Vec<&str> = chunk.iter().map(|_| "(?, ?, ?, ?, ?)").collect();
            let sql = format!(
                "INSERT OR IGNORE INTO response_events \
                 (response_id, sequence_number, event_type, payload, created_at) \
                 VALUES {}",
                placeholders.join(", ")
            );
            let mut q = query(&sql);
            for ev in chunk {
                q = q
                    .bind(&ev.response_id)
                    .bind(ev.sequence_number)
                    .bind(&ev.event_type)
                    .bind(serde_json::to_string(&ev.payload)?)
                    .bind(truncate_to_millis(ev.created_at));
            }
            let result = q.execute(&self.pool).await?;
            total += result.rows_affected();
        }
        Ok(total)
    }

    async fn list_after(
        &self,
        response_id: &str,
        after: i64,
        limit: i64,
    ) -> DbResult<Vec<ResponseEvent>> {
        let rows = query(
            r#"
            SELECT response_id, sequence_number, event_type, payload, created_at
            FROM response_events
            WHERE response_id = ? AND sequence_number > ?
            ORDER BY sequence_number ASC
            LIMIT ?
            "#,
        )
        .bind(response_id)
        .bind(after)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let payload_str: String = row.col("payload");
            out.push(ResponseEvent {
                response_id: row.col("response_id"),
                sequence_number: row.col("sequence_number"),
                event_type: row.col("event_type"),
                payload: serde_json::from_str(&payload_str)?,
                created_at: row.col::<DateTime<chrono::Utc>>("created_at"),
            });
        }
        Ok(out)
    }

    async fn set_last_sequence(&self, response_id: &str, seq: i64) -> DbResult<()> {
        // Only ratchet upward — concurrent batches may complete out of
        // order; we never want a smaller sequence to overwrite a larger.
        query(
            r#"
            UPDATE responses
            SET last_sequence_number = ?
            WHERE id = ? AND last_sequence_number < ?
            "#,
        )
        .bind(seq)
        .bind(response_id)
        .bind(seq)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
