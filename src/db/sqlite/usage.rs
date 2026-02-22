use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::{
    db::{
        error::DbResult,
        repos::{DateRange, UsageRepo, UsageStats},
    },
    models::{
        DailyModelSpend, DailyOrgSpend, DailyPricingSourceSpend, DailyProjectSpend,
        DailyProviderSpend, DailySpend, DailyTeamSpend, DailyUserSpend, ModelSpend, OrgSpend,
        PricingSourceSpend, ProjectSpend, ProviderSpend, RefererSpend, TeamSpend, UsageLogEntry,
        UsageSummary, UserSpend,
    },
};

/// Common media-tracking columns for aggregation queries.
const MEDIA_AGGREGATE_COLS: &str = "\
    COALESCE(SUM(image_count), 0) as image_count, \
    COALESCE(SUM(audio_seconds), 0) as audio_seconds, \
    COALESCE(SUM(character_count), 0) as character_count";

pub struct SqliteUsageRepo {
    pool: SqlitePool,
}

impl SqliteUsageRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn media_fields(row: &sqlx::sqlite::SqliteRow) -> (i64, i64, i64) {
        (
            row.get("image_count"),
            row.get("audio_seconds"),
            row.get("character_count"),
        )
    }
}

#[async_trait]
impl UsageRepo for SqliteUsageRepo {
    async fn log(&self, entry: UsageLogEntry) -> DbResult<()> {
        let id = Uuid::new_v4();
        let total_tokens = entry.input_tokens + entry.output_tokens;

        // Use INSERT OR IGNORE for idempotency - duplicate request_ids are silently skipped
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO usage_records (
                id, request_id, api_key_id, user_id, org_id, project_id, team_id,
                service_account_id, model, provider, input_tokens, output_tokens,
                total_tokens, cost_microcents, http_referer, recorded_at,
                streamed, cached_tokens, reasoning_tokens, finish_reason,
                latency_ms, cancelled, status_code, pricing_source,
                image_count, audio_seconds, character_count, provider_source
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&entry.request_id)
        .bind(entry.api_key_id.map(|id| id.to_string()))
        .bind(entry.user_id.map(|id| id.to_string()))
        .bind(entry.org_id.map(|id| id.to_string()))
        .bind(entry.project_id.map(|id| id.to_string()))
        .bind(entry.team_id.map(|id| id.to_string()))
        .bind(entry.service_account_id.map(|id| id.to_string()))
        .bind(&entry.model)
        .bind(&entry.provider)
        .bind(entry.input_tokens)
        .bind(entry.output_tokens)
        .bind(total_tokens)
        .bind(entry.cost_microcents.unwrap_or(0))
        .bind(&entry.http_referer)
        .bind(entry.request_at)
        .bind(entry.streamed)
        .bind(entry.cached_tokens)
        .bind(entry.reasoning_tokens)
        .bind(&entry.finish_reason)
        .bind(entry.latency_ms)
        .bind(entry.cancelled)
        .bind(entry.status_code)
        .bind(entry.pricing_source.as_str())
        .bind(entry.image_count)
        .bind(entry.audio_seconds)
        .bind(entry.character_count)
        .bind(&entry.provider_source)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn log_batch(&self, entries: Vec<UsageLogEntry>) -> DbResult<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        // SQLite has a limit of 999 parameters per query (SQLITE_LIMIT_VARIABLE_NUMBER)
        // Each entry uses 28 parameters. Use 35 entries (28*35=980) to leave headroom
        // for future columns.
        const MAX_ENTRIES_PER_BATCH: usize = 35;

        let mut total_inserted = 0;

        // Wrap all chunks in a single transaction for atomicity.
        // On failure, the caller can safely retry the entire batch since
        // INSERT OR IGNORE makes re-insertion idempotent.
        let mut tx = self.pool.begin().await?;

        // Process in chunks to stay within SQLite's parameter limit
        for chunk in entries.chunks(MAX_ENTRIES_PER_BATCH) {
            // Build dynamic multi-row INSERT query
            let placeholders: Vec<&str> = chunk
                .iter()
                .map(|_| "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                .collect();

            let query = format!(
                r#"
                INSERT OR IGNORE INTO usage_records (
                    id, request_id, api_key_id, user_id, org_id, project_id, team_id,
                    service_account_id, model, provider, input_tokens, output_tokens,
                    total_tokens, cost_microcents, http_referer, recorded_at,
                    streamed, cached_tokens, reasoning_tokens, finish_reason,
                    latency_ms, cancelled, status_code, pricing_source,
                    image_count, audio_seconds, character_count, provider_source
                )
                VALUES {}
                "#,
                placeholders.join(", ")
            );

            let mut query_builder = sqlx::query(&query);

            for entry in chunk {
                let id = Uuid::new_v4();
                let total_tokens = entry.input_tokens + entry.output_tokens;

                query_builder = query_builder
                    .bind(id.to_string())
                    .bind(&entry.request_id)
                    .bind(entry.api_key_id.map(|id| id.to_string()))
                    .bind(entry.user_id.map(|id| id.to_string()))
                    .bind(entry.org_id.map(|id| id.to_string()))
                    .bind(entry.project_id.map(|id| id.to_string()))
                    .bind(entry.team_id.map(|id| id.to_string()))
                    .bind(entry.service_account_id.map(|id| id.to_string()))
                    .bind(&entry.model)
                    .bind(&entry.provider)
                    .bind(entry.input_tokens)
                    .bind(entry.output_tokens)
                    .bind(total_tokens)
                    .bind(entry.cost_microcents.unwrap_or(0))
                    .bind(&entry.http_referer)
                    .bind(entry.request_at)
                    .bind(entry.streamed)
                    .bind(entry.cached_tokens)
                    .bind(entry.reasoning_tokens)
                    .bind(&entry.finish_reason)
                    .bind(entry.latency_ms)
                    .bind(entry.cancelled)
                    .bind(entry.status_code)
                    .bind(entry.pricing_source.as_str())
                    .bind(entry.image_count)
                    .bind(entry.audio_seconds)
                    .bind(entry.character_count)
                    .bind(&entry.provider_source);
            }

            let result = query_builder.execute(&mut *tx).await?;
            total_inserted += result.rows_affected() as usize;
        }

        tx.commit().await?;
        Ok(total_inserted)
    }

    async fn get_summary(&self, api_key_id: Uuid, range: DateRange) -> DbResult<UsageSummary> {
        // Use range query instead of date casting to allow index usage on recorded_at
        let row = sqlx::query(&format!(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                MIN(recorded_at) as first_request_at,
                MAX(recorded_at) as last_request_at,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE api_key_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            "#,
        ))
        .bind(api_key_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_one(&self.pool)
        .await?;

        let (image_count, audio_seconds, character_count) = Self::media_fields(&row);
        Ok(UsageSummary {
            total_cost_microcents: row.get("total_cost_microcents"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            total_tokens: row.get("total_tokens"),
            request_count: row.get("request_count"),
            first_request_at: row.get("first_request_at"),
            last_request_at: row.get("last_request_at"),
            image_count,
            audio_seconds,
            character_count,
        })
    }

    async fn get_by_date(&self, api_key_id: Uuid, range: DateRange) -> DbResult<Vec<DailySpend>> {
        // Use range query in WHERE for index usage; date cast only needed in SELECT/GROUP BY
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE api_key_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            ORDER BY date(recorded_at) DESC
            "#,
        ))
        .bind(api_key_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailySpend {
                    date: row.get("date"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_by_model(&self, api_key_id: Uuid, range: DateRange) -> DbResult<Vec<ModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE api_key_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY model
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(api_key_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ModelSpend {
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_by_referer(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<RefererSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                http_referer as referer,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE api_key_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY http_referer
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(api_key_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                RefererSpend {
                    referer: row.get("referer"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_usage_stats(&self, api_key_id: Uuid, range: DateRange) -> DbResult<UsageStats> {
        // Get daily totals first, then compute stats in Rust
        // This avoids SQLite's lack of native STDDEV function
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as daily_cost
            FROM usage_records
            WHERE api_key_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            "#,
        )
        .bind(api_key_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        compute_stats_from_daily_costs(&rows)
    }

    async fn get_current_period_spend(&self, api_key_id: Uuid, period: &str) -> DbResult<i64> {
        // Use range queries to allow index usage on recorded_at
        let query = match period {
            "daily" => {
                r#"
                SELECT COALESCE(SUM(cost_microcents), 0) as total
                FROM usage_records
                WHERE api_key_id = ?
                    AND recorded_at >= date('now')
                    AND recorded_at < date('now', '+1 day')
                "#
            }
            "monthly" => {
                r#"
                SELECT COALESCE(SUM(cost_microcents), 0) as total
                FROM usage_records
                WHERE api_key_id = ?
                    AND recorded_at >= date('now', 'start of month')
                    AND recorded_at < date('now', 'start of month', '+1 month')
                "#
            }
            _ => {
                return Ok(0);
            }
        };

        let row = sqlx::query(query)
            .bind(api_key_id.to_string())
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get("total"))
    }

    // ==================== Aggregated Usage Queries ====================

    async fn get_daily_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailySpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE org_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            ORDER BY date(recorded_at) ASC
            "#,
        ))
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailySpend {
                    date: row.get("date"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailySpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE project_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            ORDER BY date(recorded_at) ASC
            "#,
        ))
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailySpend {
                    date: row.get("date"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailySpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE user_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            ORDER BY date(recorded_at) ASC
            "#,
        ))
        .bind(user_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailySpend {
                    date: row.get("date"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailySpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE team_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            ORDER BY date(recorded_at) ASC
            "#,
        ))
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailySpend {
                    date: row.get("date"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_usage_by_provider(
        &self,
        provider: &str,
        range: DateRange,
    ) -> DbResult<Vec<DailySpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE provider = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            ORDER BY date(recorded_at) ASC
            "#,
        ))
        .bind(provider)
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailySpend {
                    date: row.get("date"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_summary_by_provider(
        &self,
        provider: &str,
        range: DateRange,
    ) -> DbResult<UsageSummary> {
        let row = sqlx::query(&format!(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                MIN(recorded_at) as first_request_at,
                MAX(recorded_at) as last_request_at,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE provider = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            "#,
        ))
        .bind(provider)
        .bind(range.start)
        .bind(range.end)
        .fetch_one(&self.pool)
        .await?;

        let (image_count, audio_seconds, character_count) = Self::media_fields(&row);
        Ok(UsageSummary {
            total_cost_microcents: row.get("total_cost_microcents"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            total_tokens: row.get("total_tokens"),
            request_count: row.get("request_count"),
            first_request_at: row.get("first_request_at"),
            last_request_at: row.get("last_request_at"),
            image_count,
            audio_seconds,
            character_count,
        })
    }

    async fn get_model_usage_by_provider(
        &self,
        provider: &str,
        range: DateRange,
    ) -> DbResult<Vec<ModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE provider = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY model
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(provider)
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ModelSpend {
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_usage_stats_by_provider(
        &self,
        provider: &str,
        range: DateRange,
    ) -> DbResult<UsageStats> {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as daily_cost
            FROM usage_records
            WHERE provider = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            "#,
        )
        .bind(provider)
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        compute_stats_from_daily_costs(&rows)
    }

    async fn get_model_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE org_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY model
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ModelSpend {
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_model_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE project_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY model
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ModelSpend {
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_model_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE user_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY model
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(user_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ModelSpend {
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_model_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE team_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY model
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ModelSpend {
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_provider_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE org_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY provider
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ProviderSpend {
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_provider_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE team_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY provider
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ProviderSpend {
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_summary_by_org(&self, org_id: Uuid, range: DateRange) -> DbResult<UsageSummary> {
        let row = sqlx::query(&format!(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                MIN(recorded_at) as first_request_at,
                MAX(recorded_at) as last_request_at,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE org_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            "#,
        ))
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_one(&self.pool)
        .await?;

        let (image_count, audio_seconds, character_count) = Self::media_fields(&row);
        Ok(UsageSummary {
            total_cost_microcents: row.get("total_cost_microcents"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            total_tokens: row.get("total_tokens"),
            request_count: row.get("request_count"),
            first_request_at: row.get("first_request_at"),
            last_request_at: row.get("last_request_at"),
            image_count,
            audio_seconds,
            character_count,
        })
    }

    async fn get_summary_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<UsageSummary> {
        let row = sqlx::query(&format!(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                MIN(recorded_at) as first_request_at,
                MAX(recorded_at) as last_request_at,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE project_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            "#,
        ))
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_one(&self.pool)
        .await?;

        let (image_count, audio_seconds, character_count) = Self::media_fields(&row);
        Ok(UsageSummary {
            total_cost_microcents: row.get("total_cost_microcents"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            total_tokens: row.get("total_tokens"),
            request_count: row.get("request_count"),
            first_request_at: row.get("first_request_at"),
            last_request_at: row.get("last_request_at"),
            image_count,
            audio_seconds,
            character_count,
        })
    }

    async fn get_summary_by_user(&self, user_id: Uuid, range: DateRange) -> DbResult<UsageSummary> {
        let row = sqlx::query(&format!(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                MIN(recorded_at) as first_request_at,
                MAX(recorded_at) as last_request_at,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE user_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            "#,
        ))
        .bind(user_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_one(&self.pool)
        .await?;

        let (image_count, audio_seconds, character_count) = Self::media_fields(&row);
        Ok(UsageSummary {
            total_cost_microcents: row.get("total_cost_microcents"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            total_tokens: row.get("total_tokens"),
            request_count: row.get("request_count"),
            first_request_at: row.get("first_request_at"),
            last_request_at: row.get("last_request_at"),
            image_count,
            audio_seconds,
            character_count,
        })
    }

    async fn get_summary_by_team(&self, team_id: Uuid, range: DateRange) -> DbResult<UsageSummary> {
        let row = sqlx::query(&format!(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                MIN(recorded_at) as first_request_at,
                MAX(recorded_at) as last_request_at,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE team_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            "#,
        ))
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_one(&self.pool)
        .await?;

        let (image_count, audio_seconds, character_count) = Self::media_fields(&row);
        Ok(UsageSummary {
            total_cost_microcents: row.get("total_cost_microcents"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            total_tokens: row.get("total_tokens"),
            request_count: row.get("request_count"),
            first_request_at: row.get("first_request_at"),
            last_request_at: row.get("last_request_at"),
            image_count,
            audio_seconds,
            character_count,
        })
    }

    async fn get_usage_stats_by_org(&self, org_id: Uuid, range: DateRange) -> DbResult<UsageStats> {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as daily_cost
            FROM usage_records
            WHERE org_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            "#,
        )
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        compute_stats_from_daily_costs(&rows)
    }

    async fn get_usage_stats_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<UsageStats> {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as daily_cost
            FROM usage_records
            WHERE project_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            "#,
        )
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        compute_stats_from_daily_costs(&rows)
    }

    async fn get_usage_stats_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<UsageStats> {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as daily_cost
            FROM usage_records
            WHERE user_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            "#,
        )
        .bind(user_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        compute_stats_from_daily_costs(&rows)
    }

    async fn get_usage_stats_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<UsageStats> {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as daily_cost
            FROM usage_records
            WHERE team_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            "#,
        )
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        compute_stats_from_daily_costs(&rows)
    }

    async fn get_provider_usage(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE api_key_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY provider
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(api_key_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ProviderSpend {
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_provider_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE project_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY provider
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ProviderSpend {
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_provider_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE user_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY provider
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(user_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ProviderSpend {
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_model_usage(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE api_key_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), model
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(api_key_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyModelSpend {
                    date: row.get("date"),
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_model_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE org_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), model
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyModelSpend {
                    date: row.get("date"),
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_model_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE project_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), model
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyModelSpend {
                    date: row.get("date"),
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_model_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE user_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), model
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(user_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyModelSpend {
                    date: row.get("date"),
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_model_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE team_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), model
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyModelSpend {
                    date: row.get("date"),
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_provider_usage(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE api_key_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), provider
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(api_key_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyProviderSpend {
                    date: row.get("date"),
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_provider_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE org_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), provider
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyProviderSpend {
                    date: row.get("date"),
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_provider_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE project_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), provider
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyProviderSpend {
                    date: row.get("date"),
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_provider_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE user_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), provider
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(user_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyProviderSpend {
                    date: row.get("date"),
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_provider_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE team_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), provider
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyProviderSpend {
                    date: row.get("date"),
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    // ==================== Pricing Source Aggregated Queries ====================

    async fn get_pricing_source_usage(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE api_key_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY pricing_source
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(api_key_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                PricingSourceSpend {
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_pricing_source_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE org_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY pricing_source
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                PricingSourceSpend {
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_pricing_source_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE project_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY pricing_source
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                PricingSourceSpend {
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_pricing_source_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE user_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY pricing_source
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(user_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                PricingSourceSpend {
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_pricing_source_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE team_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY pricing_source
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                PricingSourceSpend {
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_pricing_source_usage(
        &self,
        api_key_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE api_key_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), pricing_source
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(api_key_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyPricingSourceSpend {
                    date: row.get("date"),
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_pricing_source_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE org_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), pricing_source
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyPricingSourceSpend {
                    date: row.get("date"),
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_pricing_source_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE project_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), pricing_source
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyPricingSourceSpend {
                    date: row.get("date"),
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_pricing_source_usage_by_user(
        &self,
        user_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE user_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), pricing_source
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(user_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyPricingSourceSpend {
                    date: row.get("date"),
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_pricing_source_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE team_id = ?
                AND recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), pricing_source
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyPricingSourceSpend {
                    date: row.get("date"),
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    // ==================== Entity Breakdown Queries ====================

    // --- Project scope: by user ---

    async fn get_user_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<UserSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT u.user_id, users.name as user_name, users.email as user_email,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN users ON u.user_id = users.id
            WHERE u.project_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY u.user_id, users.name, users.email
            ORDER BY total_cost_microcents DESC
            "#,
        )
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                UserSpend {
                    user_id: row
                        .get::<Option<String>, _>("user_id")
                        .and_then(|s| s.parse().ok()),
                    user_name: row.get("user_name"),
                    user_email: row.get("user_email"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_user_usage_by_project(
        &self,
        project_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyUserSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT date(u.recorded_at) as date,
                u.user_id, users.name as user_name, users.email as user_email,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN users ON u.user_id = users.id
            WHERE u.project_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY date(u.recorded_at), u.user_id, users.name, users.email
            ORDER BY date(u.recorded_at) ASC, total_cost_microcents DESC
            "#,
        )
        .bind(project_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyUserSpend {
                    date: row.get("date"),
                    user_id: row
                        .get::<Option<String>, _>("user_id")
                        .and_then(|s| s.parse().ok()),
                    user_name: row.get("user_name"),
                    user_email: row.get("user_email"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    // --- Team scope: by user, by project ---

    async fn get_user_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<UserSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT u.user_id, users.name as user_name, users.email as user_email,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN users ON u.user_id = users.id
            WHERE u.team_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY u.user_id, users.name, users.email
            ORDER BY total_cost_microcents DESC
            "#,
        )
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                UserSpend {
                    user_id: row
                        .get::<Option<String>, _>("user_id")
                        .and_then(|s| s.parse().ok()),
                    user_name: row.get("user_name"),
                    user_email: row.get("user_email"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_user_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyUserSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT date(u.recorded_at) as date,
                u.user_id, users.name as user_name, users.email as user_email,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN users ON u.user_id = users.id
            WHERE u.team_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY date(u.recorded_at), u.user_id, users.name, users.email
            ORDER BY date(u.recorded_at) ASC, total_cost_microcents DESC
            "#,
        )
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyUserSpend {
                    date: row.get("date"),
                    user_id: row
                        .get::<Option<String>, _>("user_id")
                        .and_then(|s| s.parse().ok()),
                    user_name: row.get("user_name"),
                    user_email: row.get("user_email"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_project_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProjectSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT u.project_id, projects.name as project_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN projects ON u.project_id = projects.id
            WHERE u.team_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY u.project_id, projects.name
            ORDER BY total_cost_microcents DESC
            "#,
        )
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ProjectSpend {
                    project_id: row
                        .get::<Option<String>, _>("project_id")
                        .and_then(|s| s.parse().ok()),
                    project_name: row.get("project_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_project_usage_by_team(
        &self,
        team_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProjectSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT date(u.recorded_at) as date,
                u.project_id, projects.name as project_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN projects ON u.project_id = projects.id
            WHERE u.team_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY date(u.recorded_at), u.project_id, projects.name
            ORDER BY date(u.recorded_at) ASC, total_cost_microcents DESC
            "#,
        )
        .bind(team_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyProjectSpend {
                    date: row.get("date"),
                    project_id: row
                        .get::<Option<String>, _>("project_id")
                        .and_then(|s| s.parse().ok()),
                    project_name: row.get("project_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    // --- Org scope: by user, by project, by team ---

    async fn get_user_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<UserSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT u.user_id, users.name as user_name, users.email as user_email,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN users ON u.user_id = users.id
            WHERE u.org_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY u.user_id, users.name, users.email
            ORDER BY total_cost_microcents DESC
            "#,
        )
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                UserSpend {
                    user_id: row
                        .get::<Option<String>, _>("user_id")
                        .and_then(|s| s.parse().ok()),
                    user_name: row.get("user_name"),
                    user_email: row.get("user_email"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_user_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyUserSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT date(u.recorded_at) as date,
                u.user_id, users.name as user_name, users.email as user_email,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN users ON u.user_id = users.id
            WHERE u.org_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY date(u.recorded_at), u.user_id, users.name, users.email
            ORDER BY date(u.recorded_at) ASC, total_cost_microcents DESC
            "#,
        )
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyUserSpend {
                    date: row.get("date"),
                    user_id: row
                        .get::<Option<String>, _>("user_id")
                        .and_then(|s| s.parse().ok()),
                    user_name: row.get("user_name"),
                    user_email: row.get("user_email"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_project_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<ProjectSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT u.project_id, projects.name as project_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN projects ON u.project_id = projects.id
            WHERE u.org_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY u.project_id, projects.name
            ORDER BY total_cost_microcents DESC
            "#,
        )
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ProjectSpend {
                    project_id: row
                        .get::<Option<String>, _>("project_id")
                        .and_then(|s| s.parse().ok()),
                    project_name: row.get("project_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_project_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyProjectSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT date(u.recorded_at) as date,
                u.project_id, projects.name as project_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN projects ON u.project_id = projects.id
            WHERE u.org_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY date(u.recorded_at), u.project_id, projects.name
            ORDER BY date(u.recorded_at) ASC, total_cost_microcents DESC
            "#,
        )
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyProjectSpend {
                    date: row.get("date"),
                    project_id: row
                        .get::<Option<String>, _>("project_id")
                        .and_then(|s| s.parse().ok()),
                    project_name: row.get("project_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_team_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<TeamSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT u.team_id, teams.name as team_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN teams ON u.team_id = teams.id
            WHERE u.org_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY u.team_id, teams.name
            ORDER BY total_cost_microcents DESC
            "#,
        )
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                TeamSpend {
                    team_id: row
                        .get::<Option<String>, _>("team_id")
                        .and_then(|s| s.parse().ok()),
                    team_name: row.get("team_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_team_usage_by_org(
        &self,
        org_id: Uuid,
        range: DateRange,
    ) -> DbResult<Vec<DailyTeamSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT date(u.recorded_at) as date,
                u.team_id, teams.name as team_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN teams ON u.team_id = teams.id
            WHERE u.org_id = ?
                AND u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY date(u.recorded_at), u.team_id, teams.name
            ORDER BY date(u.recorded_at) ASC, total_cost_microcents DESC
            "#,
        )
        .bind(org_id.to_string())
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyTeamSpend {
                    date: row.get("date"),
                    team_id: row
                        .get::<Option<String>, _>("team_id")
                        .and_then(|s| s.parse().ok()),
                    team_name: row.get("team_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    // --- Global scope: base queries ---

    async fn get_summary_global(&self, range: DateRange) -> DbResult<UsageSummary> {
        let row = sqlx::query(&format!(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                MIN(recorded_at) as first_request_at,
                MAX(recorded_at) as last_request_at,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            "#,
        ))
        .bind(range.start)
        .bind(range.end)
        .fetch_one(&self.pool)
        .await?;

        let (image_count, audio_seconds, character_count) = Self::media_fields(&row);
        Ok(UsageSummary {
            total_cost_microcents: row.get("total_cost_microcents"),
            input_tokens: row.get("input_tokens"),
            output_tokens: row.get("output_tokens"),
            total_tokens: row.get("total_tokens"),
            request_count: row.get("request_count"),
            first_request_at: row.get("first_request_at"),
            last_request_at: row.get("last_request_at"),
            image_count,
            audio_seconds,
            character_count,
        })
    }

    async fn get_daily_usage_global(&self, range: DateRange) -> DbResult<Vec<DailySpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            ORDER BY date(recorded_at) ASC
            "#,
        ))
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailySpend {
                    date: row.get("date"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_model_usage_global(&self, range: DateRange) -> DbResult<Vec<ModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY model
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ModelSpend {
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_provider_usage_global(&self, range: DateRange) -> DbResult<Vec<ProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY provider
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ProviderSpend {
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_pricing_source_usage_global(
        &self,
        range: DateRange,
    ) -> DbResult<Vec<PricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY pricing_source
            ORDER BY total_cost_microcents DESC
            "#,
        ))
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                PricingSourceSpend {
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_model_usage_global(
        &self,
        range: DateRange,
    ) -> DbResult<Vec<DailyModelSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                model,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), model
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyModelSpend {
                    date: row.get("date"),
                    model: row.get("model"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_provider_usage_global(
        &self,
        range: DateRange,
    ) -> DbResult<Vec<DailyProviderSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                provider,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), provider
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyProviderSpend {
                    date: row.get("date"),
                    provider: row.get("provider"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_pricing_source_usage_global(
        &self,
        range: DateRange,
    ) -> DbResult<Vec<DailyPricingSourceSpend>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT
                date(recorded_at) as date,
                pricing_source,
                COALESCE(SUM(cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(input_tokens), 0) as input_tokens,
                COALESCE(SUM(output_tokens), 0) as output_tokens,
                COALESCE(SUM(total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                {MEDIA_AGGREGATE_COLS}
            FROM usage_records
            WHERE recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at), pricing_source
            ORDER BY date(recorded_at) ASC, total_cost_microcents DESC
            "#,
        ))
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyPricingSourceSpend {
                    date: row.get("date"),
                    pricing_source: row.get("pricing_source"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_usage_stats_global(&self, range: DateRange) -> DbResult<UsageStats> {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(cost_microcents), 0) as daily_cost
            FROM usage_records
            WHERE recorded_at >= ?
                AND recorded_at < date(?, '+1 day')
            GROUP BY date(recorded_at)
            "#,
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        compute_stats_from_daily_costs(&rows)
    }

    // --- Global scope: entity breakdowns ---

    async fn get_user_usage_global(&self, range: DateRange) -> DbResult<Vec<UserSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT u.user_id, users.name as user_name, users.email as user_email,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN users ON u.user_id = users.id
            WHERE u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY u.user_id, users.name, users.email
            ORDER BY total_cost_microcents DESC
            "#,
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                UserSpend {
                    user_id: row
                        .get::<Option<String>, _>("user_id")
                        .and_then(|s| s.parse().ok()),
                    user_name: row.get("user_name"),
                    user_email: row.get("user_email"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_user_usage_global(&self, range: DateRange) -> DbResult<Vec<DailyUserSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT date(u.recorded_at) as date,
                u.user_id, users.name as user_name, users.email as user_email,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN users ON u.user_id = users.id
            WHERE u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY date(u.recorded_at), u.user_id, users.name, users.email
            ORDER BY date(u.recorded_at) ASC, total_cost_microcents DESC
            "#,
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyUserSpend {
                    date: row.get("date"),
                    user_id: row
                        .get::<Option<String>, _>("user_id")
                        .and_then(|s| s.parse().ok()),
                    user_name: row.get("user_name"),
                    user_email: row.get("user_email"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_project_usage_global(&self, range: DateRange) -> DbResult<Vec<ProjectSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT u.project_id, projects.name as project_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN projects ON u.project_id = projects.id
            WHERE u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY u.project_id, projects.name
            ORDER BY total_cost_microcents DESC
            "#,
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                ProjectSpend {
                    project_id: row
                        .get::<Option<String>, _>("project_id")
                        .and_then(|s| s.parse().ok()),
                    project_name: row.get("project_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_project_usage_global(
        &self,
        range: DateRange,
    ) -> DbResult<Vec<DailyProjectSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT date(u.recorded_at) as date,
                u.project_id, projects.name as project_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN projects ON u.project_id = projects.id
            WHERE u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY date(u.recorded_at), u.project_id, projects.name
            ORDER BY date(u.recorded_at) ASC, total_cost_microcents DESC
            "#,
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyProjectSpend {
                    date: row.get("date"),
                    project_id: row
                        .get::<Option<String>, _>("project_id")
                        .and_then(|s| s.parse().ok()),
                    project_name: row.get("project_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_team_usage_global(&self, range: DateRange) -> DbResult<Vec<TeamSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT u.team_id, teams.name as team_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN teams ON u.team_id = teams.id
            WHERE u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY u.team_id, teams.name
            ORDER BY total_cost_microcents DESC
            "#,
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                TeamSpend {
                    team_id: row
                        .get::<Option<String>, _>("team_id")
                        .and_then(|s| s.parse().ok()),
                    team_name: row.get("team_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_team_usage_global(&self, range: DateRange) -> DbResult<Vec<DailyTeamSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT date(u.recorded_at) as date,
                u.team_id, teams.name as team_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN teams ON u.team_id = teams.id
            WHERE u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY date(u.recorded_at), u.team_id, teams.name
            ORDER BY date(u.recorded_at) ASC, total_cost_microcents DESC
            "#,
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyTeamSpend {
                    date: row.get("date"),
                    team_id: row
                        .get::<Option<String>, _>("team_id")
                        .and_then(|s| s.parse().ok()),
                    team_name: row.get("team_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_org_usage_global(&self, range: DateRange) -> DbResult<Vec<OrgSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT u.org_id, organizations.name as org_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN organizations ON u.org_id = organizations.id
            WHERE u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY u.org_id, organizations.name
            ORDER BY total_cost_microcents DESC
            "#,
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                OrgSpend {
                    org_id: row
                        .get::<Option<String>, _>("org_id")
                        .and_then(|s| s.parse().ok()),
                    org_name: row.get("org_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    async fn get_daily_org_usage_global(&self, range: DateRange) -> DbResult<Vec<DailyOrgSpend>> {
        let rows = sqlx::query(
            r#"
            SELECT date(u.recorded_at) as date,
                u.org_id, organizations.name as org_name,
                COALESCE(SUM(u.cost_microcents), 0) as total_cost_microcents,
                COALESCE(SUM(u.input_tokens), 0) as input_tokens,
                COALESCE(SUM(u.output_tokens), 0) as output_tokens,
                COALESCE(SUM(u.total_tokens), 0) as total_tokens,
                COUNT(*) as request_count,
                COALESCE(SUM(u.image_count), 0) as image_count,
                COALESCE(SUM(u.audio_seconds), 0) as audio_seconds,
                COALESCE(SUM(u.character_count), 0) as character_count
            FROM usage_records u
            LEFT JOIN organizations ON u.org_id = organizations.id
            WHERE u.recorded_at >= ?
                AND u.recorded_at < date(?, '+1 day')
            GROUP BY date(u.recorded_at), u.org_id, organizations.name
            ORDER BY date(u.recorded_at) ASC, total_cost_microcents DESC
            "#,
        )
        .bind(range.start)
        .bind(range.end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let (image_count, audio_seconds, character_count) = Self::media_fields(row);
                DailyOrgSpend {
                    date: row.get("date"),
                    org_id: row
                        .get::<Option<String>, _>("org_id")
                        .and_then(|s| s.parse().ok()),
                    org_name: row.get("org_name"),
                    total_cost_microcents: row.get("total_cost_microcents"),
                    input_tokens: row.get("input_tokens"),
                    output_tokens: row.get("output_tokens"),
                    total_tokens: row.get("total_tokens"),
                    request_count: row.get("request_count"),
                    image_count,
                    audio_seconds,
                    character_count,
                }
            })
            .collect())
    }

    // ==================== Retention Operations ====================

    async fn delete_usage_records_before(
        &self,
        cutoff: DateTime<Utc>,
        batch_size: u32,
        max_deletes: u64,
    ) -> DbResult<u64> {
        let mut total_deleted: u64 = 0;

        loop {
            // Check if we've hit the max deletes limit
            if total_deleted >= max_deletes {
                break;
            }

            // Calculate remaining deletes allowed
            let remaining = max_deletes - total_deleted;
            let limit = std::cmp::min(batch_size as u64, remaining) as i64;

            // Delete a batch using subquery to select IDs (SQLite doesn't support LIMIT in DELETE directly)
            let result = sqlx::query(
                r#"
                DELETE FROM usage_records
                WHERE id IN (
                    SELECT id FROM usage_records
                    WHERE recorded_at < ?
                    LIMIT ?
                )
                "#,
            )
            .bind(cutoff)
            .bind(limit)
            .execute(&self.pool)
            .await?;

            let rows_deleted = result.rows_affected();
            total_deleted += rows_deleted;

            // If we deleted fewer rows than the batch size, we're done
            if rows_deleted < limit as u64 {
                break;
            }
        }

        Ok(total_deleted)
    }

    async fn delete_daily_spend_before(
        &self,
        cutoff: DateTime<Utc>,
        batch_size: u32,
        max_deletes: u64,
    ) -> DbResult<u64> {
        let mut total_deleted: u64 = 0;
        // daily_spend.date is stored as TEXT in 'YYYY-MM-DD' format
        let cutoff_date = cutoff.format("%Y-%m-%d").to_string();

        loop {
            if total_deleted >= max_deletes {
                break;
            }

            let remaining = max_deletes - total_deleted;
            let limit = std::cmp::min(batch_size as u64, remaining) as i64;

            // daily_spend uses composite primary key (api_key_id, date, model), use rowid for deletion
            let result = sqlx::query(
                r#"
                DELETE FROM daily_spend
                WHERE rowid IN (
                    SELECT rowid FROM daily_spend
                    WHERE date < ?
                    LIMIT ?
                )
                "#,
            )
            .bind(&cutoff_date)
            .bind(limit)
            .execute(&self.pool)
            .await?;

            let rows_deleted = result.rows_affected();
            total_deleted += rows_deleted;

            if rows_deleted < limit as u64 {
                break;
            }
        }

        Ok(total_deleted)
    }
}

/// Helper function to compute usage stats from daily cost rows.
/// This avoids duplicating the statistics calculation logic.
fn compute_stats_from_daily_costs(rows: &[sqlx::sqlite::SqliteRow]) -> DbResult<UsageStats> {
    let daily_costs: Vec<i64> = rows.iter().map(|row| row.get("daily_cost")).collect();
    let sample_days = daily_costs.len() as i32;

    if sample_days == 0 {
        return Ok(UsageStats {
            avg_daily_spend_microcents: 0,
            std_dev_daily_spend_microcents: 0,
            sample_days: 0,
        });
    }

    let total: i64 = daily_costs.iter().sum();
    let avg = total / sample_days as i64;

    // Calculate standard deviation
    let variance: f64 = if sample_days > 1 {
        let sum_sq_diff: f64 = daily_costs
            .iter()
            .map(|&cost| {
                let diff = cost as f64 - avg as f64;
                diff * diff
            })
            .sum();
        sum_sq_diff / (sample_days - 1) as f64
    } else {
        0.0
    };

    let std_dev = variance.sqrt() as i64;

    Ok(UsageStats {
        avg_daily_spend_microcents: avg,
        std_dev_daily_spend_microcents: std_dev,
        sample_days,
    })
}
