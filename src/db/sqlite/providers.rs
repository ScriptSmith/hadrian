use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
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

pub struct SqliteDynamicProviderRepo {
    pool: SqlitePool,
}

impl SqliteDynamicProviderRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_owner(owner_type: &str, owner_id: &str) -> DbResult<ProviderOwner> {
        let owner_uuid = Uuid::parse_str(owner_id).map_err(|e| DbError::Internal(e.to_string()))?;
        match owner_type {
            "organization" => Ok(ProviderOwner::Organization { org_id: owner_uuid }),
            "team" => Ok(ProviderOwner::Team {
                team_id: owner_uuid,
            }),
            "project" => Ok(ProviderOwner::Project {
                project_id: owner_uuid,
            }),
            "user" => Ok(ProviderOwner::User {
                user_id: owner_uuid,
            }),
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

    fn parse_provider(row: &sqlx::sqlite::SqliteRow) -> DbResult<DynamicProvider> {
        let owner = Self::parse_owner(row.get("owner_type"), row.get("owner_id"))?;
        let models_json: String = row.get("models");
        let models: Vec<String> =
            serde_json::from_str(&models_json).map_err(|e| DbError::Internal(e.to_string()))?;

        let config: Option<serde_json::Value> = row
            .get::<Option<String>, _>("config")
            .and_then(|s| serde_json::from_str(&s).ok());

        Ok(DynamicProvider {
            id: Uuid::parse_str(row.get("id")).map_err(|e| DbError::Internal(e.to_string()))?,
            name: row.get("name"),
            owner,
            provider_type: row.get("provider_type"),
            base_url: row.get("base_url"),
            api_key_secret_ref: row.get("api_key_secret_ref"),
            config,
            models,
            is_enabled: row.get::<i32, _>("is_enabled") != 0,
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
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'organization' AND owner_id = ?
            AND (created_at, id) {} (?, ?)
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(org_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
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
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'project' AND owner_id = ?
            AND (created_at, id) {} (?, ?)
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(project_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
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
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'team' AND owner_id = ?
            AND (created_at, id) {} (?, ?)
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(team_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
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
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'user' AND owner_id = ?
            AND (created_at, id) {} (?, ?)
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(user_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
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
impl DynamicProviderRepo for SqliteDynamicProviderRepo {
    async fn create(&self, id: Uuid, input: CreateDynamicProvider) -> DbResult<DynamicProvider> {
        let now = chrono::Utc::now();
        let (owner_type, owner_id) = Self::owner_to_parts(&input.owner);
        let models = input.models.clone().unwrap_or_default();
        let models_json =
            serde_json::to_string(&models).map_err(|e| DbError::Internal(e.to_string()))?;
        let config_json = input
            .config
            .as_ref()
            .map(|c| serde_json::to_string(c).map_err(|e| DbError::Internal(e.to_string())))
            .transpose()?;

        sqlx::query(
            r#"
            INSERT INTO dynamic_providers (
                id, owner_type, owner_id, name, provider_type, base_url,
                api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(owner_type)
        .bind(owner_id.to_string())
        .bind(&input.name)
        .bind(&input.provider_type)
        .bind(&input.base_url)
        .bind(&input.api_key)
        .bind(&config_json)
        .bind(&models_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
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
            models: input.models.unwrap_or_default(),
            is_enabled: true,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<DynamicProvider>> {
        let row = sqlx::query(
            r#"
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
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
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = ? AND owner_id = ? AND name = ?
            "#,
        )
        .bind(owner_type)
        .bind(owner_id.to_string())
        .bind(name)
        .fetch_optional(&self.pool)
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

        // Cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_org_with_cursor(org_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'organization' AND owner_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(org_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
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
            WHERE owner_type = 'organization' AND owner_id = ?
            "#,
        )
        .bind(org_id.to_string())
        .fetch_one(&self.pool)
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
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'team' AND owner_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(team_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
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
            WHERE owner_type = 'team' AND owner_id = ?
            "#,
        )
        .bind(team_id.to_string())
        .fetch_one(&self.pool)
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

        // Cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_project_with_cursor(project_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'project' AND owner_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
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
            WHERE owner_type = 'project' AND owner_id = ?
            "#,
        )
        .bind(project_id.to_string())
        .fetch_one(&self.pool)
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

        // Cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_user_with_cursor(user_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'user' AND owner_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(user_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
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
            WHERE owner_type = 'user' AND owner_id = ?
            "#,
        )
        .bind(user_id.to_string())
        .fetch_one(&self.pool)
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
                SELECT id, owner_type, owner_id, name, provider_type, base_url,
                       api_key_secret_ref, config, models, is_enabled, created_at, updated_at
                FROM dynamic_providers
                WHERE owner_type = 'user' AND owner_id = ? AND is_enabled = 1
                AND (created_at, id) {} (?, ?)
                ORDER BY created_at {}, id {}
                LIMIT ?
                "#,
                comparison, order, order
            );

            let rows = sqlx::query(&query)
                .bind(user_id.to_string())
                .bind(cursor.created_at)
                .bind(cursor.id.to_string())
                .bind(fetch_limit)
                .fetch_all(&self.pool)
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
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'user' AND owner_id = ? AND is_enabled = 1
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(user_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
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
                SELECT id, owner_type, owner_id, name, provider_type, base_url,
                       api_key_secret_ref, config, models, is_enabled, created_at, updated_at
                FROM dynamic_providers
                WHERE owner_type = 'organization' AND owner_id = ? AND is_enabled = 1
                AND (created_at, id) {} (?, ?)
                ORDER BY created_at {}, id {}
                LIMIT ?
                "#,
                comparison, order, order
            );

            let rows = sqlx::query(&query)
                .bind(org_id.to_string())
                .bind(cursor.created_at)
                .bind(cursor.id.to_string())
                .bind(fetch_limit)
                .fetch_all(&self.pool)
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
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'organization' AND owner_id = ? AND is_enabled = 1
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(org_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
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
                SELECT id, owner_type, owner_id, name, provider_type, base_url,
                       api_key_secret_ref, config, models, is_enabled, created_at, updated_at
                FROM dynamic_providers
                WHERE owner_type = 'project' AND owner_id = ? AND is_enabled = 1
                AND (created_at, id) {} (?, ?)
                ORDER BY created_at {}, id {}
                LIMIT ?
                "#,
                comparison, order, order
            );

            let rows = sqlx::query(&query)
                .bind(project_id.to_string())
                .bind(cursor.created_at)
                .bind(cursor.id.to_string())
                .bind(fetch_limit)
                .fetch_all(&self.pool)
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
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'project' AND owner_id = ? AND is_enabled = 1
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
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
                SELECT id, owner_type, owner_id, name, provider_type, base_url,
                       api_key_secret_ref, config, models, is_enabled, created_at, updated_at
                FROM dynamic_providers
                WHERE owner_type = 'team' AND owner_id = ? AND is_enabled = 1
                AND (created_at, id) {} (?, ?)
                ORDER BY created_at {}, id {}
                LIMIT ?
                "#,
                comparison, order, order
            );

            let rows = sqlx::query(&query)
                .bind(team_id.to_string())
                .bind(cursor.created_at)
                .bind(cursor.id.to_string())
                .bind(fetch_limit)
                .fetch_all(&self.pool)
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
            SELECT id, owner_type, owner_id, name, provider_type, base_url,
                   api_key_secret_ref, config, models, is_enabled, created_at, updated_at
            FROM dynamic_providers
            WHERE owner_type = 'team' AND owner_id = ? AND is_enabled = 1
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(team_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
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
        let now = chrono::Utc::now();

        // Build dynamic update query
        let mut updates = vec!["updated_at = ?"];
        let mut has_base_url = false;
        let mut has_api_key = false;
        let mut has_config = false;
        let mut has_models = false;
        let mut has_is_enabled = false;

        if input.base_url.is_some() {
            updates.push("base_url = ?");
            has_base_url = true;
        }
        if input.api_key.is_some() {
            updates.push("api_key_secret_ref = ?");
            has_api_key = true;
        }
        if input.config.is_some() {
            updates.push("config = ?");
            has_config = true;
        }
        if input.models.is_some() {
            updates.push("models = ?");
            has_models = true;
        }
        if input.is_enabled.is_some() {
            updates.push("is_enabled = ?");
            has_is_enabled = true;
        }

        let query_str = format!(
            "UPDATE dynamic_providers SET {} WHERE id = ?",
            updates.join(", ")
        );

        let mut query = sqlx::query(&query_str);
        query = query.bind(now);
        if has_base_url {
            query = query.bind(&input.base_url);
        }
        if has_api_key {
            query = query.bind(&input.api_key);
        }
        if has_config {
            let config_json = input
                .config
                .as_ref()
                .map(|c| serde_json::to_string(c).map_err(|e| DbError::Internal(e.to_string())))
                .transpose()?;
            query = query.bind(config_json);
        }
        if has_models {
            let models_json = serde_json::to_string(&input.models.as_ref().unwrap())
                .map_err(|e| DbError::Internal(e.to_string()))?;
            query = query.bind(models_json);
        }
        if has_is_enabled {
            query = query.bind(input.is_enabled.map(|b| if b { 1 } else { 0 }));
        }
        query = query.bind(id.to_string());

        query.execute(&self.pool).await?;

        // Fetch and return updated record
        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM dynamic_providers WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
