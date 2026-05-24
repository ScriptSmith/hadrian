//! Postgres implementation of [`ResponseEventsRepo`].

use async_trait::async_trait;
use sqlx::{PgPool, Row};

use crate::db::{
    error::DbResult,
    repos::{NewResponseEvent, ResponseEvent, ResponseEventsRepo},
};

pub struct PostgresResponseEventsRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresResponseEventsRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ResponseEventsRepo for PostgresResponseEventsRepo {
    async fn insert_batch(&self, events: Vec<NewResponseEvent>) -> DbResult<u64> {
        if events.is_empty() {
            return Ok(0);
        }
        // Postgres allows up to 65535 bind params; 5 per row → 13000+ rows
        // per batch. Cap at 1000 for query-time and memory reasons.
        const CHUNK: usize = 1000;
        let mut total = 0u64;
        for chunk in events.chunks(CHUNK) {
            let placeholders: Vec<String> = chunk
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let o = i * 5;
                    format!(
                        "(${}, ${}, ${}, ${}, ${})",
                        o + 1,
                        o + 2,
                        o + 3,
                        o + 4,
                        o + 5
                    )
                })
                .collect();
            let sql = format!(
                "INSERT INTO response_events \
                 (response_id, sequence_number, event_type, payload, created_at) \
                 VALUES {} \
                 ON CONFLICT (response_id, sequence_number) DO NOTHING",
                placeholders.join(", ")
            );
            let mut q = sqlx::query(&sql);
            for ev in chunk {
                q = q
                    .bind(&ev.response_id)
                    .bind(ev.sequence_number)
                    .bind(&ev.event_type)
                    .bind(&ev.payload)
                    .bind(ev.created_at);
            }
            let result = q.execute(&self.write_pool).await?;
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
        let rows = sqlx::query(
            r#"
            SELECT response_id, sequence_number, event_type, payload, created_at
            FROM response_events
            WHERE response_id = $1 AND sequence_number > $2
            ORDER BY sequence_number ASC
            LIMIT $3
            "#,
        )
        .bind(response_id)
        .bind(after)
        .bind(limit)
        .fetch_all(&self.read_pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(ResponseEvent {
                response_id: row.get("response_id"),
                sequence_number: row.get("sequence_number"),
                event_type: row.get("event_type"),
                payload: row.get("payload"),
                created_at: row.get("created_at"),
            });
        }
        Ok(out)
    }

    async fn set_last_sequence(&self, response_id: &str, seq: i64) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE responses
            SET last_sequence_number = $1
            WHERE id = $2 AND last_sequence_number < $1
            "#,
        )
        .bind(seq)
        .bind(response_id)
        .execute(&self.write_pool)
        .await?;
        Ok(())
    }
}
