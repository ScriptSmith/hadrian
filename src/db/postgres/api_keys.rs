use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            ApiKeyRepo, Cursor, CursorDirection, ListParams, ListResult, PageCursors,
            cursor_from_row,
        },
    },
    models::{ApiKey, ApiKeyOwner, ApiKeyWithOwner, BudgetPeriod, CreateApiKey},
};

pub struct PostgresApiKeyRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresApiKeyRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    fn parse_owner(owner_type: &str, owner_id: Uuid) -> DbResult<ApiKeyOwner> {
        match owner_type {
            "organization" => Ok(ApiKeyOwner::Organization { org_id: owner_id }),
            "team" => Ok(ApiKeyOwner::Team { team_id: owner_id }),
            "project" => Ok(ApiKeyOwner::Project {
                project_id: owner_id,
            }),
            "user" => Ok(ApiKeyOwner::User { user_id: owner_id }),
            "service_account" => Ok(ApiKeyOwner::ServiceAccount {
                service_account_id: owner_id,
            }),
            _ => Err(DbError::Internal(format!(
                "Invalid owner type: {}",
                owner_type
            ))),
        }
    }

    fn owner_to_parts(owner: &ApiKeyOwner) -> (&'static str, Uuid) {
        match owner {
            ApiKeyOwner::Organization { org_id } => ("organization", *org_id),
            ApiKeyOwner::Team { team_id } => ("team", *team_id),
            ApiKeyOwner::Project { project_id } => ("project", *project_id),
            ApiKeyOwner::User { user_id } => ("user", *user_id),
            ApiKeyOwner::ServiceAccount { service_account_id } => {
                ("service_account", *service_account_id)
            }
        }
    }

    fn read_budget_amount(row: &sqlx::postgres::PgRow) -> Option<i64> {
        row.get("budget_amount")
    }

    fn parse_api_key(row: &sqlx::postgres::PgRow) -> DbResult<ApiKey> {
        let owner = Self::parse_owner(row.get("owner_type"), row.get("owner_id"))?;
        let budget_period: Option<String> = row.get("budget_period");

        // JSONB columns parse to serde_json::Value, then convert to Vec<String>
        let scopes: Option<Vec<String>> = row
            .get::<Option<serde_json::Value>, _>("scopes")
            .and_then(|v| serde_json::from_value(v).ok());
        let allowed_models: Option<Vec<String>> = row
            .get::<Option<serde_json::Value>, _>("allowed_models")
            .and_then(|v| serde_json::from_value(v).ok());
        let ip_allowlist: Option<Vec<String>> = row
            .get::<Option<serde_json::Value>, _>("ip_allowlist")
            .and_then(|v| serde_json::from_value(v).ok());

        Ok(ApiKey {
            id: row.get("id"),
            key_prefix: row.get("key_prefix"),
            name: row.get("name"),
            owner,
            budget_limit_cents: Self::read_budget_amount(row),
            budget_period: budget_period.and_then(|p| match p.as_str() {
                "daily" => Some(BudgetPeriod::Daily),
                "monthly" => Some(BudgetPeriod::Monthly),
                _ => None,
            }),
            created_at: row.get("created_at"),
            expires_at: row.get("expires_at"),
            revoked_at: row.get("revoked_at"),
            last_used_at: row.get("last_used_at"),
            scopes,
            allowed_models,
            ip_allowlist,
            rate_limit_rpm: row.get("rate_limit_rpm"),
            rate_limit_tpm: row.get("rate_limit_tpm"),
            rotated_from_key_id: row.get("rotated_from_key_id"),
            rotation_grace_until: row.get("rotation_grace_until"),
        })
    }

    /// Helper method for cursor-based pagination of API keys by organization.
    async fn list_by_org_with_cursor(
        &self,
        org_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<ApiKey>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, key_prefix, name, owner_type::TEXT, owner_id, budget_amount, budget_period::TEXT,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
        let mut items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_api_key)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |key| {
                cursor_from_row(key.created_at, key.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of API keys by project.
    async fn list_by_project_with_cursor(
        &self,
        project_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<ApiKey>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, key_prefix, name, owner_type::TEXT, owner_id, budget_amount, budget_period::TEXT,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
        let mut items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_api_key)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |key| {
                cursor_from_row(key.created_at, key.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of API keys by team.
    async fn list_by_team_with_cursor(
        &self,
        team_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<ApiKey>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, key_prefix, name, owner_type::TEXT, owner_id, budget_amount, budget_period::TEXT,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
        let mut items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_api_key)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |key| {
                cursor_from_row(key.created_at, key.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of API keys by user.
    async fn list_by_user_with_cursor(
        &self,
        user_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<ApiKey>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, key_prefix, name, owner_type::TEXT, owner_id, budget_amount, budget_period::TEXT,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
        let mut items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_api_key)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |key| {
                cursor_from_row(key.created_at, key.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of API keys by service account.
    async fn list_by_service_account_with_cursor(
        &self,
        service_account_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<ApiKey>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, key_prefix, name, owner_type::TEXT, owner_id, budget_amount, budget_period::TEXT,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
            WHERE owner_type = 'service_account' AND owner_id = $1
            AND ROW(created_at, id) {} ROW($2, $3)
            ORDER BY created_at {}, id {}
            LIMIT $4
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(service_account_id)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_api_key)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |key| {
                cursor_from_row(key.created_at, key.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[async_trait]
impl ApiKeyRepo for PostgresApiKeyRepo {
    async fn create(&self, input: CreateApiKey, key_hash: &str) -> DbResult<ApiKey> {
        let id = Uuid::new_v4();

        // Extract first 8 characters of hash as prefix
        let key_prefix = if key_hash.len() >= 8 {
            &key_hash[..8]
        } else {
            key_hash
        };

        let (owner_type, owner_id) = Self::owner_to_parts(&input.owner);

        let row = sqlx::query(
            r#"
            INSERT INTO api_keys (
                id, name, key_hash, key_prefix, owner_type, owner_id,
                budget_amount, budget_period, expires_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm
            )
            VALUES ($1, $2, $3, $4, $5::api_key_owner_type, $6, $7, $8::budget_period, $9, $10, $11, $12, $13, $14)
            RETURNING created_at
            "#,
        )
        .bind(id)
        .bind(&input.name)
        .bind(key_hash)
        .bind(key_prefix)
        .bind(owner_type)
        .bind(owner_id)
        .bind(input.budget_limit_cents)
        .bind(input.budget_period.map(|p| p.as_str()))
        .bind(input.expires_at)
        .bind(
            input
                .scopes
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok()),
        )
        .bind(
            input
                .allowed_models
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok()),
        )
        .bind(
            input
                .ip_allowlist
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok()),
        )
        .bind(input.rate_limit_rpm)
        .bind(input.rate_limit_tpm)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("API key with this hash already exists".to_string())
            }
            _ => DbError::from(e),
        })?;

        Ok(ApiKey {
            id,
            key_prefix: key_prefix.to_string(),
            name: input.name,
            owner: input.owner,
            budget_limit_cents: input.budget_limit_cents,
            budget_period: input.budget_period,
            created_at: row.get("created_at"),
            expires_at: input.expires_at,
            revoked_at: None,
            last_used_at: None,
            scopes: input.scopes,
            allowed_models: input.allowed_models,
            ip_allowlist: input.ip_allowlist,
            rate_limit_rpm: input.rate_limit_rpm,
            rate_limit_tpm: input.rate_limit_tpm,
            rotated_from_key_id: None,
            rotation_grace_until: None,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ApiKey>> {
        let row = sqlx::query(
            r#"
            SELECT
                id, key_prefix, name, owner_type::TEXT, owner_id,
                budget_amount, budget_period::TEXT, expires_at, last_used_at, created_at, revoked_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                rotated_from_key_id, rotation_grace_until
            FROM api_keys
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        Ok(Some(Self::parse_api_key(&row)?))
    }

    async fn get_by_hash(&self, key_hash: &str) -> DbResult<Option<ApiKeyWithOwner>> {
        let row = sqlx::query(
            r#"
            SELECT
                k.id, k.key_prefix, k.name, k.owner_type::TEXT, k.owner_id,
                k.budget_amount, k.budget_period::TEXT, k.expires_at, k.last_used_at, k.created_at,
                k.revoked_at,
                k.scopes, k.allowed_models, k.ip_allowlist, k.rate_limit_rpm, k.rate_limit_tpm,
                k.rotated_from_key_id, k.rotation_grace_until,
                CASE
                    WHEN k.owner_type = 'organization' THEN k.owner_id
                    WHEN k.owner_type = 'team' THEN t.org_id
                    WHEN k.owner_type = 'project' THEN p.org_id
                    WHEN k.owner_type = 'service_account' THEN sa.org_id
                    WHEN k.owner_type = 'user' THEN NULL
                END as org_id,
                CASE WHEN k.owner_type = 'team' THEN k.owner_id ELSE NULL END as team_id,
                CASE WHEN k.owner_type = 'project' THEN k.owner_id ELSE NULL END as project_id,
                CASE WHEN k.owner_type = 'user' THEN k.owner_id ELSE NULL END as user_id,
                CASE WHEN k.owner_type = 'service_account' THEN k.owner_id ELSE NULL END as service_account_id,
                sa.roles as service_account_roles
            FROM api_keys k
            LEFT JOIN projects p ON k.owner_type = 'project' AND k.owner_id = p.id
            LEFT JOIN teams t ON k.owner_type = 'team' AND k.owner_id = t.id
            LEFT JOIN service_accounts sa ON k.owner_type = 'service_account' AND k.owner_id = sa.id
            WHERE k.key_hash = $1 AND k.revoked_at IS NULL
              AND (k.rotation_grace_until IS NULL OR k.rotation_grace_until > NOW())
            "#,
        )
        .bind(key_hash)
        .fetch_optional(&self.read_pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let key = Self::parse_api_key(&row)?;

        // Parse service account roles from JSONB
        let service_account_roles: Option<Vec<String>> = row
            .get::<Option<serde_json::Value>, _>("service_account_roles")
            .and_then(|v| serde_json::from_value(v).ok());

        Ok(Some(ApiKeyWithOwner {
            key,
            org_id: row.get("org_id"),
            team_id: row.get("team_id"),
            project_id: row.get("project_id"),
            user_id: row.get("user_id"),
            service_account_id: row.get("service_account_id"),
            service_account_roles,
        }))
    }

    async fn list_by_org(&self, org_id: Uuid, params: ListParams) -> DbResult<ListResult<ApiKey>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_org_with_cursor(org_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT
                id, key_prefix, name, owner_type::TEXT, owner_id,
                budget_amount, budget_period::TEXT, expires_at, last_used_at, created_at, revoked_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
        let items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_api_key)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |key| {
                cursor_from_row(key.created_at, key.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_org(&self, org_id: Uuid, _include_deleted: bool) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'organization' AND owner_id = $1",
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
    ) -> DbResult<ListResult<ApiKey>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_team_with_cursor(team_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT
                id, key_prefix, name, owner_type::TEXT, owner_id,
                budget_amount, budget_period::TEXT, expires_at, last_used_at, created_at, revoked_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
        let items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_api_key)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |key| {
                cursor_from_row(key.created_at, key.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_team(&self, team_id: Uuid, _include_deleted: bool) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'team' AND owner_id = $1",
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
    ) -> DbResult<ListResult<ApiKey>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_project_with_cursor(project_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT
                id, key_prefix, name, owner_type::TEXT, owner_id,
                budget_amount, budget_period::TEXT, expires_at, last_used_at, created_at, revoked_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
        let items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_api_key)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |key| {
                cursor_from_row(key.created_at, key.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_project(&self, project_id: Uuid, _include_deleted: bool) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'project' AND owner_id = $1",
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
    ) -> DbResult<ListResult<ApiKey>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_user_with_cursor(user_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT
                id, key_prefix, name, owner_type::TEXT, owner_id,
                budget_amount, budget_period::TEXT, expires_at, last_used_at, created_at, revoked_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
        let items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_api_key)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |key| {
                cursor_from_row(key.created_at, key.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_user(&self, user_id: Uuid, _include_deleted: bool) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'user' AND owner_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.read_pool)
        .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn revoke(&self, id: Uuid) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE api_keys
            SET revoked_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.write_pool)
        .await?;

        Ok(())
    }

    async fn update_last_used(&self, id: Uuid) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE api_keys
            SET last_used_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.write_pool)
        .await?;

        Ok(())
    }

    async fn revoke_by_user(&self, user_id: Uuid) -> DbResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE api_keys
            SET revoked_at = NOW()
            WHERE owner_type = 'user' AND owner_id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(user_id)
        .execute(&self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }

    async fn list_by_service_account(
        &self,
        service_account_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ApiKey>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_by_service_account_with_cursor(
                    service_account_id,
                    &params,
                    cursor,
                    fetch_limit,
                    limit,
                )
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT
                id, key_prefix, name, owner_type::TEXT, owner_id,
                budget_amount, budget_period::TEXT, expires_at, last_used_at, created_at, revoked_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                rotated_from_key_id, rotation_grace_until
            FROM api_keys
            WHERE owner_type = 'service_account' AND owner_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(service_account_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<ApiKey> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_api_key)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |key| {
                cursor_from_row(key.created_at, key.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_service_account(
        &self,
        service_account_id: Uuid,
        include_revoked: bool,
    ) -> DbResult<i64> {
        let query = if include_revoked {
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'service_account' AND owner_id = $1"
        } else {
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'service_account' AND owner_id = $1 AND revoked_at IS NULL"
        };
        let row = sqlx::query(query)
            .bind(service_account_id)
            .fetch_one(&self.read_pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn revoke_by_service_account(&self, service_account_id: Uuid) -> DbResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE api_keys
            SET revoked_at = NOW()
            WHERE owner_type = 'service_account' AND owner_id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(service_account_id)
        .execute(&self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }

    async fn rotate(
        &self,
        old_key_id: Uuid,
        new_key_input: CreateApiKey,
        new_key_hash: &str,
        grace_until: DateTime<Utc>,
    ) -> DbResult<ApiKey> {
        let new_id = Uuid::new_v4();

        // Extract first 8 characters of hash as prefix
        let key_prefix = if new_key_hash.len() >= 8 {
            &new_key_hash[..8]
        } else {
            new_key_hash
        };

        let (owner_type, owner_id) = Self::owner_to_parts(&new_key_input.owner);

        // Use a transaction to ensure both operations succeed or fail together
        let mut tx = self.write_pool.begin().await?;

        // 1. Update old key with grace period
        sqlx::query(
            r#"
            UPDATE api_keys
            SET rotation_grace_until = $1
            WHERE id = $2
            "#,
        )
        .bind(grace_until)
        .bind(old_key_id)
        .execute(&mut *tx)
        .await?;

        // 2. Insert new key with rotated_from_key_id
        let row = sqlx::query(
            r#"
            INSERT INTO api_keys (
                id, name, key_hash, key_prefix, owner_type, owner_id,
                budget_amount, budget_period, expires_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                rotated_from_key_id
            )
            VALUES ($1, $2, $3, $4, $5::api_key_owner_type, $6, $7, $8::budget_period, $9, $10, $11, $12, $13, $14, $15)
            RETURNING created_at
            "#,
        )
        .bind(new_id)
        .bind(&new_key_input.name)
        .bind(new_key_hash)
        .bind(key_prefix)
        .bind(owner_type)
        .bind(owner_id)
        .bind(new_key_input.budget_limit_cents)
        .bind(new_key_input.budget_period.map(|p| p.as_str()))
        .bind(new_key_input.expires_at)
        .bind(
            new_key_input
                .scopes
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok()),
        )
        .bind(
            new_key_input
                .allowed_models
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok()),
        )
        .bind(
            new_key_input
                .ip_allowlist
                .as_ref()
                .and_then(|s| serde_json::to_value(s).ok()),
        )
        .bind(new_key_input.rate_limit_rpm)
        .bind(new_key_input.rate_limit_tpm)
        .bind(old_key_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("API key with this hash already exists".to_string())
            }
            _ => DbError::from(e),
        })?;

        tx.commit().await?;

        Ok(ApiKey {
            id: new_id,
            key_prefix: key_prefix.to_string(),
            name: new_key_input.name,
            owner: new_key_input.owner,
            budget_limit_cents: new_key_input.budget_limit_cents,
            budget_period: new_key_input.budget_period,
            created_at: row.get("created_at"),
            expires_at: new_key_input.expires_at,
            revoked_at: None,
            last_used_at: None,
            scopes: new_key_input.scopes,
            allowed_models: new_key_input.allowed_models,
            ip_allowlist: new_key_input.ip_allowlist,
            rate_limit_rpm: new_key_input.rate_limit_rpm,
            rate_limit_tpm: new_key_input.rate_limit_tpm,
            rotated_from_key_id: Some(old_key_id),
            rotation_grace_until: None,
        })
    }

    async fn get_key_hashes_by_service_account(
        &self,
        service_account_id: Uuid,
    ) -> DbResult<Vec<String>> {
        let hashes: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT key_hash
            FROM api_keys
            WHERE owner_type = 'service_account'
              AND owner_id = $1
              AND revoked_at IS NULL
            "#,
        )
        .bind(service_account_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(hashes)
    }

    async fn get_key_hashes_by_user(&self, user_id: Uuid) -> DbResult<Vec<String>> {
        let hashes: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT key_hash
            FROM api_keys
            WHERE owner_type = 'user'
              AND owner_id = $1
              AND revoked_at IS NULL
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(hashes)
    }
}
