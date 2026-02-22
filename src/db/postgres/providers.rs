use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, DynamicProviderRepo, ListParams, ListResult, PageCursors, cursor_from_row,
        },
    },
    models::{CreateDynamicProvider, DynamicProvider, ProviderOwner, UpdateDynamicProvider},
};

pub struct PostgresDynamicProviderRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresDynamicProviderRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    fn parse_owner(owner_type: &str, owner_id: Uuid) -> DbResult<ProviderOwner> {
        match owner_type {
            "organization" => Ok(ProviderOwner::Organization { org_id: owner_id }),
            "team" => Ok(ProviderOwner::Team { team_id: owner_id }),
            "project" => Ok(ProviderOwner::Project {
                project_id: owner_id,
            }),
            "user" => Ok(ProviderOwner::User { user_id: owner_id }),
            _ => Err(DbError::Internal(format!(
                "Invalid owner type: {}",
                owner_type
            ))),
        }
    }

    fn owner_to_parts(owner: &ProviderOwner) -> (&'static str, Uuid) {
        match owner {
            ProviderOwner::Organization { org_id } => ("organization", *org_id),
            ProviderOwner::Team { team_id } => ("team", *team_id),
            ProviderOwner::Project { project_id } => ("project", *project_id),
            ProviderOwner::User { user_id } => ("user", *user_id),
        }
    }

    fn parse_provider(row: &sqlx::postgres::PgRow) -> DbResult<DynamicProvider> {
        let owner = Self::parse_owner(row.get("owner_type"), row.get("owner_id"))?;
        let models_value: serde_json::Value = row.get("models");
        let models: Vec<String> =
            serde_json::from_value(models_value).map_err(|e| DbError::Internal(e.to_string()))?;

        Ok(DynamicProvider {
            id: row.get("id"),
            name: row.get("name"),
            owner,
            provider_type: row.get("provider_type"),
            base_url: row.get("base_url"),
            api_key_secret_ref: row.get("api_key_secret_ref"),
            config: row.get("config"),
            models,
            is_enabled: row.get("is_enabled"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Helper method for cursor-based pagination of providers by organization.
    async fn list_by_org_with_cursor(
        &self,
        org_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'organization' AND owner_id = $1
            AND ROW(created_at, id) {} ROW($2, $3)
            ORDER BY created_at {}, id {}
            LIMIT $4
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(org_id)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |p| {
                cursor_from_row(p.created_at, p.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of providers by project.
    async fn list_by_project_with_cursor(
        &self,
        project_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'project' AND owner_id = $1
            AND ROW(created_at, id) {} ROW($2, $3)
            ORDER BY created_at {}, id {}
            LIMIT $4
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(project_id)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |p| {
                cursor_from_row(p.created_at, p.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of providers by team.
    async fn list_by_team_with_cursor(
        &self,
        team_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'team' AND owner_id = $1
            AND ROW(created_at, id) {} ROW($2, $3)
            ORDER BY created_at {}, id {}
            LIMIT $4
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(team_id)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |p| {
                cursor_from_row(p.created_at, p.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of providers by user.
    async fn list_by_user_with_cursor(
        &self,
        user_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'user' AND owner_id = $1
            AND ROW(created_at, id) {} ROW($2, $3)
            ORDER BY created_at {}, id {}
            LIMIT $4
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(user_id)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |p| {
                cursor_from_row(p.created_at, p.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[async_trait]
impl DynamicProviderRepo for PostgresDynamicProviderRepo {
    async fn create(&self, id: Uuid, input: CreateDynamicProvider) -> DbResult<DynamicProvider> {
        let (owner_type, owner_id) = Self::owner_to_parts(&input.owner);
        let models = input.models.clone().unwrap_or_default();
        let models_json =
            serde_json::to_value(&models).map_err(|e| DbError::Internal(e.to_string()))?;

        let row = sqlx::query(
            r#"
            INSERT INTO dynamic_providers (
                id, owner_type, owner_id, name, provider_type, base_url,
                api_key_secret_ref, config, models, is_enabled
            )
            VALUES ($1, $2::dynamic_provider_owner_type, $3, $4, $5, $6, $7, $8, $9, true)
            RETURNING created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(owner_type)
        .bind(owner_id)
        .bind(&input.name)
        .bind(&input.provider_type)
        .bind(&input.base_url)
        .bind(&input.api_key)
        .bind(&input.config)
        .bind(&models_json)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => DbError::Conflict(
                format!("Provider '{}' already exists for this owner", input.name),
            ),
            _ => DbError::from(e),
        })?;

        Ok(DynamicProvider {
            id,
            name: input.name,
            owner: input.owner,
            provider_type: input.provider_type,
            base_url: input.base_url,
            api_key_secret_ref: input.api_key,
            config: input.config,
            models,
            is_enabled: true,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<DynamicProvider>> {
        let row = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        row.as_ref().map(Self::parse_provider).transpose()
    }

    async fn get_by_name(
        &self,
        owner: &ProviderOwner,
        name: &str,
    ) -> DbResult<Option<DynamicProvider>> {
        let (owner_type, owner_id) = Self::owner_to_parts(owner);

        let row = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = $1::dynamic_provider_owner_type AND owner_id = $2 AND name = $3
            "#,
        )
        .bind(owner_type)
        .bind(owner_id)
        .bind(name)
        .fetch_optional(&self.read_pool)
        .await?;

        row.as_ref().map(Self::parse_provider).transpose()
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1; // Fetch one extra to detect has_more

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_org_with_cursor(org_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'organization' AND owner_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(org_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(&items, has_more, params.direction, None, |p| {
            cursor_from_row(p.created_at, p.id)
        });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM dynamic_providers
            WHERE owner_type = 'organization' AND owner_id = $1
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn list_by_team(
        &self,
        team_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_team_with_cursor(team_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'team' AND owner_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(team_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(&items, has_more, params.direction, None, |p| {
            cursor_from_row(p.created_at, p.id)
        });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_team(&self, team_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM dynamic_providers
            WHERE owner_type = 'team' AND owner_id = $1
            "#,
        )
        .bind(team_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn list_by_project(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1; // Fetch one extra to detect has_more

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_project_with_cursor(project_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'project' AND owner_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(project_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(&items, has_more, params.direction, None, |p| {
            cursor_from_row(p.created_at, p.id)
        });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_project(&self, project_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM dynamic_providers
            WHERE owner_type = 'project' AND owner_id = $1
            "#,
        )
        .bind(project_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn list_by_user(
        &self,
        user_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1; // Fetch one extra to detect has_more

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_user_with_cursor(user_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'user' AND owner_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(&items, has_more, params.direction, None, |p| {
            cursor_from_row(p.created_at, p.id)
        });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_user(&self, user_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM dynamic_providers
            WHERE owner_type = 'user' AND owner_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn list_enabled_by_user(
        &self,
        user_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            let (comparison, order, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let query = format!(
                r#"
                SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                       api_key_secret_ref, config, models, is_enabled, created_at, updated_at
                FROM dynamic_providers
                WHERE owner_type = 'user' AND owner_id = $1 AND is_enabled = true
                AND ROW(created_at, id) {} ROW($2, $3)
                ORDER BY created_at {}, id {}
                LIMIT $4
                "#,
                comparison, order, order
            );

            let rows = sqlx::query(&query)
                .bind(user_id)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
                .await?;

            let has_more = rows.len() as i64 > limit;
            let mut items: Vec<DynamicProvider> = rows
                .iter()
                .take(limit as usize)
                .map(Self::parse_provider)
                .collect::<DbResult<Vec<_>>>()?;

            if should_reverse {
                items.reverse();
            }

            let cursors =
                PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |p| {
                    cursor_from_row(p.created_at, p.id)
                });

            return Ok(ListResult::new(items, has_more, cursors));
        }

        let rows = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'user' AND owner_id = $1 AND is_enabled = true
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(&items, has_more, params.direction, None, |p| {
            cursor_from_row(p.created_at, p.id)
        });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn list_enabled_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            let (comparison, order, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let query = format!(
                r#"
                SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                       api_key_secret_ref, config, models, is_enabled, created_at, updated_at
                FROM dynamic_providers
                WHERE owner_type = 'organization' AND owner_id = $1 AND is_enabled = true
                AND ROW(created_at, id) {} ROW($2, $3)
                ORDER BY created_at {}, id {}
                LIMIT $4
                "#,
                comparison, order, order
            );

            let rows = sqlx::query(&query)
                .bind(org_id)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
                .await?;

            let has_more = rows.len() as i64 > limit;
            let mut items: Vec<DynamicProvider> = rows
                .iter()
                .take(limit as usize)
                .map(Self::parse_provider)
                .collect::<DbResult<Vec<_>>>()?;

            if should_reverse {
                items.reverse();
            }

            let cursors =
                PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |p| {
                    cursor_from_row(p.created_at, p.id)
                });

            return Ok(ListResult::new(items, has_more, cursors));
        }

        let rows = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'organization' AND owner_id = $1 AND is_enabled = true
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(org_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(&items, has_more, params.direction, None, |p| {
            cursor_from_row(p.created_at, p.id)
        });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn list_enabled_by_project(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            let (comparison, order, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let query = format!(
                r#"
                SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                       api_key_secret_ref, config, models, is_enabled, created_at, updated_at
                FROM dynamic_providers
                WHERE owner_type = 'project' AND owner_id = $1 AND is_enabled = true
                AND ROW(created_at, id) {} ROW($2, $3)
                ORDER BY created_at {}, id {}
                LIMIT $4
                "#,
                comparison, order, order
            );

            let rows = sqlx::query(&query)
                .bind(project_id)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
                .await?;

            let has_more = rows.len() as i64 > limit;
            let mut items: Vec<DynamicProvider> = rows
                .iter()
                .take(limit as usize)
                .map(Self::parse_provider)
                .collect::<DbResult<Vec<_>>>()?;

            if should_reverse {
                items.reverse();
            }

            let cursors =
                PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |p| {
                    cursor_from_row(p.created_at, p.id)
                });

            return Ok(ListResult::new(items, has_more, cursors));
        }

        let rows = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'project' AND owner_id = $1 AND is_enabled = true
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(project_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(&items, has_more, params.direction, None, |p| {
            cursor_from_row(p.created_at, p.id)
        });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn list_enabled_by_team(
        &self,
        team_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            let (comparison, order, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let query = format!(
                r#"
                SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                       api_key_secret_ref, config, models, is_enabled, created_at, updated_at
                FROM dynamic_providers
                WHERE owner_type = 'team' AND owner_id = $1 AND is_enabled = true
                AND ROW(created_at, id) {} ROW($2, $3)
                ORDER BY created_at {}, id {}
                LIMIT $4
                "#,
                comparison, order, order
            );

            let rows = sqlx::query(&query)
                .bind(team_id)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
                .await?;

            let has_more = rows.len() as i64 > limit;
            let mut items: Vec<DynamicProvider> = rows
                .iter()
                .take(limit as usize)
                .map(Self::parse_provider)
                .collect::<DbResult<Vec<_>>>()?;

            if should_reverse {
                items.reverse();
            }

            let cursors =
                PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |p| {
                    cursor_from_row(p.created_at, p.id)
                });

            return Ok(ListResult::new(items, has_more, cursors));
        }

        let rows = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'team' AND owner_id = $1 AND is_enabled = true
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(team_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<DynamicProvider> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_provider)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(&items, has_more, params.direction, None, |p| {
            cursor_from_row(p.created_at, p.id)
        });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn update(&self, id: Uuid, input: UpdateDynamicProvider) -> DbResult<DynamicProvider> {
        // Build dynamic update query
        let mut updates = vec![];
        let mut param_count = 1;

        if input.base_url.is_some() {
            updates.push(format!("base_url = ${}", param_count));
            param_count += 1;
        }
        if input.api_key.is_some() {
            updates.push(format!("api_key_secret_ref = ${}", param_count));
            param_count += 1;
        }
        if input.config.is_some() {
            updates.push(format!("config = ${}", param_count));
            param_count += 1;
        }
        if input.models.is_some() {
            updates.push(format!("models = ${}", param_count));
            param_count += 1;
        }
        if input.is_enabled.is_some() {
            updates.push(format!("is_enabled = ${}", param_count));
            param_count += 1;
        }

        if updates.is_empty() {
            // No updates, just return current record
            return self.get_by_id(id).await?.ok_or(DbError::NotFound);
        }

        let query_str = format!(
            "UPDATE dynamic_providers SET {} WHERE id = ${}",
            updates.join(", "),
            param_count
        );

        let mut query = sqlx::query(&query_str);
        if let Some(ref base_url) = input.base_url {
            query = query.bind(base_url);
        }
        if let Some(ref api_key) = input.api_key {
            query = query.bind(api_key);
        }
        if let Some(ref config) = input.config {
            query = query.bind(config);
        }
        if let Some(ref models) = input.models {
            let models_json =
                serde_json::to_value(models).map_err(|e| DbError::Internal(e.to_string()))?;
            query = query.bind(models_json);
        }
        if let Some(is_enabled) = input.is_enabled {
            query = query.bind(is_enabled);
        }
        query = query.bind(id);

        query.execute(&self.write_pool).await?;

        // Fetch and return updated record
        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM dynamic_providers WHERE id = $1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;

        Ok(())
    }
}
