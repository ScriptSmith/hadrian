use async_trait::async_trait;
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, ModelPricingRepo, PageCursors,
            cursor_from_row,
        },
        wasm_sqlite::{WasmRow, WasmSqlitePool, query as wasm_query},
    },
    models::{CreateModelPricing, DbModelPricing, PricingOwner, PricingSource, UpdateModelPricing},
};

pub struct WasmSqliteModelPricingRepo {
    pool: WasmSqlitePool,
}

impl WasmSqliteModelPricingRepo {
    pub fn new(pool: WasmSqlitePool) -> Self {
        Self { pool }
    }

    fn parse_owner(owner_type: Option<&str>, owner_id: Option<&str>) -> DbResult<PricingOwner> {
        match (owner_type, owner_id) {
            (None, _) | (Some(""), _) => Ok(PricingOwner::Global),
            (Some("organization"), Some(id)) => Ok(PricingOwner::Organization {
                org_id: parse_uuid(id)?,
            }),
            (Some("team"), Some(id)) => Ok(PricingOwner::Team {
                team_id: parse_uuid(id)?,
            }),
            (Some("project"), Some(id)) => Ok(PricingOwner::Project {
                project_id: parse_uuid(id)?,
            }),
            (Some("user"), Some(id)) => Ok(PricingOwner::User {
                user_id: parse_uuid(id)?,
            }),
            _ => Err(DbError::Internal("Invalid pricing owner".to_string())),
        }
    }

    fn owner_to_parts(owner: &PricingOwner) -> (Option<&'static str>, Option<Uuid>) {
        match owner {
            PricingOwner::Global => (None, None),
            PricingOwner::Organization { org_id } => (Some("organization"), Some(*org_id)),
            PricingOwner::Team { team_id } => (Some("team"), Some(*team_id)),
            PricingOwner::Project { project_id } => (Some("project"), Some(*project_id)),
            PricingOwner::User { user_id } => (Some("user"), Some(*user_id)),
        }
    }

    fn row_to_pricing(row: &WasmRow) -> DbResult<DbModelPricing> {
        let owner_type: Option<String> = row.get("owner_type");
        let owner_id: Option<String> = row.get("owner_id");
        let source_str: String = row.get("source");

        Ok(DbModelPricing {
            id: parse_uuid(&row.get::<String>("id"))?,
            owner: Self::parse_owner(owner_type.as_deref(), owner_id.as_deref())?,
            provider: row.get("provider"),
            model: row.get("model"),
            input_per_1m_tokens: row.get("input_per_1m_tokens"),
            output_per_1m_tokens: row.get("output_per_1m_tokens"),
            per_image: row.get("per_image"),
            per_request: row.get("per_request"),
            cached_input_per_1m_tokens: row.get("cached_input_per_1m_tokens"),
            cache_write_per_1m_tokens: row.get("cache_write_per_1m_tokens"),
            reasoning_per_1m_tokens: row.get("reasoning_per_1m_tokens"),
            per_second: row.get("per_second"),
            per_1m_characters: row.get("per_1m_characters"),
            source: PricingSource::from_str(&source_str),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Helper method for cursor-based pagination with a custom WHERE clause.
    async fn list_with_cursor(
        &self,
        where_clause: &str,
        binds: Vec<String>,
        params: &ListParams,
        cursor: &Cursor,
        limit: i64,
    ) -> DbResult<ListResult<DbModelPricing>> {
        let fetch_limit = limit + 1;
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let cursor_condition = if where_clause.is_empty() {
            format!("WHERE (created_at, id) {} (?, ?)", comparison)
        } else {
            format!(
                "{} AND (created_at, id) {} (?, ?)",
                where_clause, comparison
            )
        };

        let query = format!(
            r#"
            SELECT id, owner_type, owner_id, provider, model,
                   input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                   cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                   per_second, per_1m_characters, source, created_at, updated_at
            FROM model_pricing
            {}
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            cursor_condition, order, order
        );

        let mut query_builder = wasm_query(&query);
        for bind in &binds {
            query_builder = query_builder.bind(bind);
        }
        query_builder = query_builder
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit);

        let rows = query_builder.fetch_all(&self.pool).await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<DbModelPricing> = rows
            .iter()
            .take(limit as usize)
            .map(Self::row_to_pricing)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        // Generate cursors
        let cursors = PageCursors::from_items(
            &items,
            has_more,
            params.direction,
            Some(cursor),
            |pricing| cursor_from_row(pricing.created_at, pricing.id),
        );

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for first page (no cursor provided).
    async fn list_first_page(
        &self,
        where_clause: &str,
        binds: Vec<String>,
        limit: i64,
    ) -> DbResult<ListResult<DbModelPricing>> {
        let fetch_limit = limit + 1;

        let query = format!(
            r#"
            SELECT id, owner_type, owner_id, provider, model,
                   input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                   cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                   per_second, per_1m_characters, source, created_at, updated_at
            FROM model_pricing
            {}
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
            where_clause
        );

        let mut query_builder = wasm_query(&query);
        for bind in &binds {
            query_builder = query_builder.bind(bind);
        }
        query_builder = query_builder.bind(fetch_limit);

        let rows = query_builder.fetch_all(&self.pool).await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<DbModelPricing> = rows
            .iter()
            .take(limit as usize)
            .map(Self::row_to_pricing)
            .collect::<DbResult<Vec<_>>>()?;

        // Generate cursors for pagination
        let cursors = PageCursors::from_items(
            &items,
            has_more,
            CursorDirection::Forward,
            None,
            |pricing| cursor_from_row(pricing.created_at, pricing.id),
        );

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ModelPricingRepo for WasmSqliteModelPricingRepo {
    async fn create(&self, input: CreateModelPricing) -> DbResult<DbModelPricing> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let (owner_type, owner_id) = Self::owner_to_parts(&input.owner);

        wasm_query(
            r#"
            INSERT INTO model_pricing (
                id, owner_type, owner_id, provider, model,
                input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                per_second, per_1m_characters,
                source, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(owner_type)
        .bind(owner_id.map(|u| u.to_string()))
        .bind(&input.provider)
        .bind(&input.model)
        .bind(input.input_per_1m_tokens)
        .bind(input.output_per_1m_tokens)
        .bind(input.per_image)
        .bind(input.per_request)
        .bind(input.cached_input_per_1m_tokens)
        .bind(input.cache_write_per_1m_tokens)
        .bind(input.reasoning_per_1m_tokens)
        .bind(input.per_second)
        .bind(input.per_1m_characters)
        .bind(input.source.as_str())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict(format!(
                    "Pricing for provider '{}' model '{}' already exists",
                    input.provider, input.model
                ))
            } else {
                DbError::from(e)
            }
        })?;

        Ok(DbModelPricing {
            id,
            owner: input.owner,
            provider: input.provider,
            model: input.model,
            input_per_1m_tokens: input.input_per_1m_tokens,
            output_per_1m_tokens: input.output_per_1m_tokens,
            per_image: input.per_image,
            per_request: input.per_request,
            cached_input_per_1m_tokens: input.cached_input_per_1m_tokens,
            cache_write_per_1m_tokens: input.cache_write_per_1m_tokens,
            reasoning_per_1m_tokens: input.reasoning_per_1m_tokens,
            per_second: input.per_second,
            per_1m_characters: input.per_1m_characters,
            source: input.source,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<DbModelPricing>> {
        let row = wasm_query(
            r#"
            SELECT id, owner_type, owner_id, provider, model,
                   input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                   cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                   per_second, per_1m_characters, source, created_at, updated_at
            FROM model_pricing
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(Self::row_to_pricing).transpose()
    }

    async fn get_by_provider_model(
        &self,
        owner: &PricingOwner,
        provider: &str,
        model: &str,
    ) -> DbResult<Option<DbModelPricing>> {
        let (owner_type, owner_id) = Self::owner_to_parts(owner);

        let row = if owner_type.is_none() {
            wasm_query(
                r#"
                SELECT id, owner_type, owner_id, provider, model,
                       input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                       cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                       per_second, per_1m_characters, source, created_at, updated_at
                FROM model_pricing
                WHERE owner_type IS NULL AND provider = ? AND model = ?
                "#,
            )
            .bind(provider)
            .bind(model)
            .fetch_optional(&self.pool)
            .await?
        } else {
            wasm_query(
                r#"
                SELECT id, owner_type, owner_id, provider, model,
                       input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                       cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                       per_second, per_1m_characters, source, created_at, updated_at
                FROM model_pricing
                WHERE owner_type = ? AND owner_id = ? AND provider = ? AND model = ?
                "#,
            )
            .bind(owner_type)
            .bind(owner_id.map(|u| u.to_string()))
            .bind(provider)
            .bind(model)
            .fetch_optional(&self.pool)
            .await?
        };

        row.as_ref().map(Self::row_to_pricing).transpose()
    }

    async fn get_effective_pricing(
        &self,
        provider: &str,
        model: &str,
        user_id: Option<Uuid>,
        project_id: Option<Uuid>,
        org_id: Option<Uuid>,
    ) -> DbResult<Option<DbModelPricing>> {
        // Single query with priority ordering: user > project > org > global
        // Uses CASE expression to assign priority and LIMIT 1 to get highest priority match
        let row = wasm_query(
            r#"
            SELECT id, owner_type, owner_id, provider, model,
                   input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                   cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                   per_second, per_1m_characters, source, created_at, updated_at
            FROM model_pricing
            WHERE provider = ? AND model = ?
              AND (
                (? IS NOT NULL AND owner_type = 'user' AND owner_id = ?)
                OR (? IS NOT NULL AND owner_type = 'project' AND owner_id = ?)
                OR (? IS NOT NULL AND owner_type = 'organization' AND owner_id = ?)
                OR owner_type IS NULL
              )
            ORDER BY CASE
              WHEN owner_type = 'user' THEN 1
              WHEN owner_type = 'project' THEN 2
              WHEN owner_type = 'organization' THEN 3
              ELSE 4
            END
            LIMIT 1
            "#,
        )
        .bind(provider)
        .bind(model)
        .bind(user_id.map(|u| u.to_string()))
        .bind(user_id.map(|u| u.to_string()))
        .bind(project_id.map(|p| p.to_string()))
        .bind(project_id.map(|p| p.to_string()))
        .bind(org_id.map(|o| o.to_string()))
        .bind(org_id.map(|o| o.to_string()))
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(Self::row_to_pricing).transpose()
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DbModelPricing>> {
        let limit = params.limit.unwrap_or(100);
        let where_clause = "WHERE owner_type = 'organization' AND owner_id = ?";
        let binds = vec![org_id.to_string()];

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(where_clause, binds, &params, cursor, limit)
                .await;
        }

        // First page (no cursor provided)
        self.list_first_page(where_clause, binds, limit).await
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = wasm_query(
            r#"
            SELECT COUNT(*) as count
            FROM model_pricing
            WHERE owner_type = 'organization' AND owner_id = ?
            "#,
        )
        .bind(org_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64>("count"))
    }

    async fn list_by_project(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DbModelPricing>> {
        let limit = params.limit.unwrap_or(100);
        let where_clause = "WHERE owner_type = 'project' AND owner_id = ?";
        let binds = vec![project_id.to_string()];

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(where_clause, binds, &params, cursor, limit)
                .await;
        }

        // First page (no cursor provided)
        self.list_first_page(where_clause, binds, limit).await
    }

    async fn count_by_project(&self, project_id: Uuid) -> DbResult<i64> {
        let row = wasm_query(
            r#"
            SELECT COUNT(*) as count
            FROM model_pricing
            WHERE owner_type = 'project' AND owner_id = ?
            "#,
        )
        .bind(project_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64>("count"))
    }

    async fn list_by_user(
        &self,
        user_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DbModelPricing>> {
        let limit = params.limit.unwrap_or(100);
        let where_clause = "WHERE owner_type = 'user' AND owner_id = ?";
        let binds = vec![user_id.to_string()];

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(where_clause, binds, &params, cursor, limit)
                .await;
        }

        // First page (no cursor provided)
        self.list_first_page(where_clause, binds, limit).await
    }

    async fn count_by_user(&self, user_id: Uuid) -> DbResult<i64> {
        let row = wasm_query(
            r#"
            SELECT COUNT(*) as count
            FROM model_pricing
            WHERE owner_type = 'user' AND owner_id = ?
            "#,
        )
        .bind(user_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64>("count"))
    }

    async fn list_global(&self, params: ListParams) -> DbResult<ListResult<DbModelPricing>> {
        let limit = params.limit.unwrap_or(100);
        let where_clause = "WHERE owner_type IS NULL";
        let binds = vec![];

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(where_clause, binds, &params, cursor, limit)
                .await;
        }

        // First page (no cursor provided)
        self.list_first_page(where_clause, binds, limit).await
    }

    async fn count_global(&self) -> DbResult<i64> {
        let row = wasm_query(
            r#"
            SELECT COUNT(*) as count
            FROM model_pricing
            WHERE owner_type IS NULL
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64>("count"))
    }

    async fn list_by_provider(
        &self,
        provider: &str,
        params: ListParams,
    ) -> DbResult<ListResult<DbModelPricing>> {
        let limit = params.limit.unwrap_or(100);
        let where_clause = "WHERE provider = ?";
        let binds = vec![provider.to_string()];

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(where_clause, binds, &params, cursor, limit)
                .await;
        }

        // First page (no cursor provided)
        self.list_first_page(where_clause, binds, limit).await
    }

    async fn count_by_provider(&self, provider: &str) -> DbResult<i64> {
        let row = wasm_query(
            r#"
            SELECT COUNT(*) as count
            FROM model_pricing
            WHERE provider = ?
            "#,
        )
        .bind(provider)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateModelPricing) -> DbResult<DbModelPricing> {
        let now = chrono::Utc::now();

        wasm_query(
            r#"
            UPDATE model_pricing SET
                input_per_1m_tokens = COALESCE(?, input_per_1m_tokens),
                output_per_1m_tokens = COALESCE(?, output_per_1m_tokens),
                per_image = COALESCE(?, per_image),
                per_request = COALESCE(?, per_request),
                cached_input_per_1m_tokens = COALESCE(?, cached_input_per_1m_tokens),
                cache_write_per_1m_tokens = COALESCE(?, cache_write_per_1m_tokens),
                reasoning_per_1m_tokens = COALESCE(?, reasoning_per_1m_tokens),
                source = COALESCE(?, source),
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(input.input_per_1m_tokens)
        .bind(input.output_per_1m_tokens)
        .bind(input.per_image)
        .bind(input.per_request)
        .bind(input.cached_input_per_1m_tokens)
        .bind(input.cache_write_per_1m_tokens)
        .bind(input.reasoning_per_1m_tokens)
        .bind(input.source.map(|s| s.as_str()))
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        wasm_query("DELETE FROM model_pricing WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn upsert(&self, input: CreateModelPricing) -> DbResult<DbModelPricing> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let (owner_type, owner_id) = Self::owner_to_parts(&input.owner);

        // Use INSERT ... ON CONFLICT DO UPDATE for atomic upsert
        // Different conflict targets for global vs scoped pricing due to partial indexes
        if owner_type.is_none() {
            // Global pricing: conflict on (provider, model) where owner_type IS NULL
            wasm_query(
                r#"
                INSERT INTO model_pricing (
                    id, owner_type, owner_id, provider, model,
                    input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                    cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                    per_second, per_1m_characters, source, created_at, updated_at
                )
                VALUES (?, NULL, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT (provider, model) WHERE owner_type IS NULL
                DO UPDATE SET
                    input_per_1m_tokens = excluded.input_per_1m_tokens,
                    output_per_1m_tokens = excluded.output_per_1m_tokens,
                    per_image = excluded.per_image,
                    per_request = excluded.per_request,
                    cached_input_per_1m_tokens = excluded.cached_input_per_1m_tokens,
                    cache_write_per_1m_tokens = excluded.cache_write_per_1m_tokens,
                    reasoning_per_1m_tokens = excluded.reasoning_per_1m_tokens,
                    per_second = excluded.per_second,
                    per_1m_characters = excluded.per_1m_characters,
                    source = excluded.source,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(id.to_string())
            .bind(&input.provider)
            .bind(&input.model)
            .bind(input.input_per_1m_tokens)
            .bind(input.output_per_1m_tokens)
            .bind(input.per_image)
            .bind(input.per_request)
            .bind(input.cached_input_per_1m_tokens)
            .bind(input.cache_write_per_1m_tokens)
            .bind(input.reasoning_per_1m_tokens)
            .bind(input.per_second)
            .bind(input.per_1m_characters)
            .bind(input.source.as_str())
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        } else {
            // Scoped pricing: conflict on (owner_type, owner_id, provider, model) where owner_type IS NOT NULL
            wasm_query(
                r#"
                INSERT INTO model_pricing (
                    id, owner_type, owner_id, provider, model,
                    input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                    cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                    per_second, per_1m_characters, source, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT (owner_type, owner_id, provider, model) WHERE owner_type IS NOT NULL
                DO UPDATE SET
                    input_per_1m_tokens = excluded.input_per_1m_tokens,
                    output_per_1m_tokens = excluded.output_per_1m_tokens,
                    per_image = excluded.per_image,
                    per_request = excluded.per_request,
                    cached_input_per_1m_tokens = excluded.cached_input_per_1m_tokens,
                    cache_write_per_1m_tokens = excluded.cache_write_per_1m_tokens,
                    reasoning_per_1m_tokens = excluded.reasoning_per_1m_tokens,
                    per_second = excluded.per_second,
                    per_1m_characters = excluded.per_1m_characters,
                    source = excluded.source,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(id.to_string())
            .bind(owner_type)
            .bind(owner_id.map(|u| u.to_string()))
            .bind(&input.provider)
            .bind(&input.model)
            .bind(input.input_per_1m_tokens)
            .bind(input.output_per_1m_tokens)
            .bind(input.per_image)
            .bind(input.per_request)
            .bind(input.cached_input_per_1m_tokens)
            .bind(input.cache_write_per_1m_tokens)
            .bind(input.reasoning_per_1m_tokens)
            .bind(input.per_second)
            .bind(input.per_1m_characters)
            .bind(input.source.as_str())
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }

        // Fetch the result (either newly inserted or updated row)
        self.get_by_provider_model(&input.owner, &input.provider, &input.model)
            .await?
            .ok_or_else(|| DbError::Internal("Upsert failed to retrieve result".to_string()))
    }

    async fn bulk_upsert(&self, entries: Vec<CreateModelPricing>) -> DbResult<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let now = chrono::Utc::now();
        let count = entries.len();

        // Process all entries individually (no transaction support in WASM SQLite bridge)
        for entry in entries {
            let id = Uuid::new_v4();
            let (owner_type, owner_id) = Self::owner_to_parts(&entry.owner);

            if owner_type.is_none() {
                wasm_query(
                    r#"
                    INSERT INTO model_pricing (
                        id, owner_type, owner_id, provider, model,
                        input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                        cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                        per_second, per_1m_characters, source, created_at, updated_at
                    )
                    VALUES (?, NULL, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT (provider, model) WHERE owner_type IS NULL
                    DO UPDATE SET
                        input_per_1m_tokens = excluded.input_per_1m_tokens,
                        output_per_1m_tokens = excluded.output_per_1m_tokens,
                        per_image = excluded.per_image,
                        per_request = excluded.per_request,
                        cached_input_per_1m_tokens = excluded.cached_input_per_1m_tokens,
                        cache_write_per_1m_tokens = excluded.cache_write_per_1m_tokens,
                        reasoning_per_1m_tokens = excluded.reasoning_per_1m_tokens,
                        per_second = excluded.per_second,
                        per_1m_characters = excluded.per_1m_characters,
                        source = excluded.source,
                        updated_at = excluded.updated_at
                    "#,
                )
                .bind(id.to_string())
                .bind(&entry.provider)
                .bind(&entry.model)
                .bind(entry.input_per_1m_tokens)
                .bind(entry.output_per_1m_tokens)
                .bind(entry.per_image)
                .bind(entry.per_request)
                .bind(entry.cached_input_per_1m_tokens)
                .bind(entry.cache_write_per_1m_tokens)
                .bind(entry.reasoning_per_1m_tokens)
                .bind(entry.per_second)
                .bind(entry.per_1m_characters)
                .bind(entry.source.as_str())
                .bind(now)
                .bind(now)
                .execute(&self.pool)
                .await?;
            } else {
                wasm_query(
                    r#"
                    INSERT INTO model_pricing (
                        id, owner_type, owner_id, provider, model,
                        input_per_1m_tokens, output_per_1m_tokens, per_image, per_request,
                        cached_input_per_1m_tokens, cache_write_per_1m_tokens, reasoning_per_1m_tokens,
                        per_second, per_1m_characters, source, created_at, updated_at
                    )
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT (owner_type, owner_id, provider, model) WHERE owner_type IS NOT NULL
                    DO UPDATE SET
                        input_per_1m_tokens = excluded.input_per_1m_tokens,
                        output_per_1m_tokens = excluded.output_per_1m_tokens,
                        per_image = excluded.per_image,
                        per_request = excluded.per_request,
                        cached_input_per_1m_tokens = excluded.cached_input_per_1m_tokens,
                        cache_write_per_1m_tokens = excluded.cache_write_per_1m_tokens,
                        reasoning_per_1m_tokens = excluded.reasoning_per_1m_tokens,
                        per_second = excluded.per_second,
                        per_1m_characters = excluded.per_1m_characters,
                        source = excluded.source,
                        updated_at = excluded.updated_at
                    "#,
                )
                .bind(id.to_string())
                .bind(owner_type)
                .bind(owner_id.map(|u| u.to_string()))
                .bind(&entry.provider)
                .bind(&entry.model)
                .bind(entry.input_per_1m_tokens)
                .bind(entry.output_per_1m_tokens)
                .bind(entry.per_image)
                .bind(entry.per_request)
                .bind(entry.cached_input_per_1m_tokens)
                .bind(entry.cache_write_per_1m_tokens)
                .bind(entry.reasoning_per_1m_tokens)
                .bind(entry.per_second)
                .bind(entry.per_1m_characters)
                .bind(entry.source.as_str())
                .bind(now)
                .bind(now)
                .execute(&self.pool)
                .await?;
            }
        }

        Ok(count)
    }
}
