use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
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

pub struct SqliteApiKeyRepo {
    pool: SqlitePool,
}

impl SqliteApiKeyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_owner(owner_type: &str, owner_id: &str) -> DbResult<ApiKeyOwner> {
        let owner_uuid = Uuid::parse_str(owner_id).map_err(|e| DbError::Internal(e.to_string()))?;
        match owner_type {
            "organization" => Ok(ApiKeyOwner::Organization { org_id: owner_uuid }),
            "team" => Ok(ApiKeyOwner::Team {
                team_id: owner_uuid,
            }),
            "project" => Ok(ApiKeyOwner::Project {
                project_id: owner_uuid,
            }),
            "user" => Ok(ApiKeyOwner::User {
                user_id: owner_uuid,
            }),
            "service_account" => Ok(ApiKeyOwner::ServiceAccount {
                service_account_id: owner_uuid,
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

    fn parse_api_key(row: &sqlx::sqlite::SqliteRow) -> DbResult<ApiKey> {
        let owner = Self::parse_owner(row.get("owner_type"), row.get("owner_id"))?;
        let budget_period: Option<String> = row.get("budget_period");

        // Parse JSON columns
        let scopes: Option<String> = row.get("scopes");
        let allowed_models: Option<String> = row.get("allowed_models");
        let ip_allowlist: Option<String> = row.get("ip_allowlist");

        Ok(ApiKey {
            id: Uuid::parse_str(row.get("id")).map_err(|e| DbError::Internal(e.to_string()))?,
            key_prefix: row.get("key_prefix"),
            name: row.get("name"),
            owner,
            budget_limit_cents: row.get("budget_amount"),
            budget_period: budget_period.and_then(|p| match p.as_str() {
                "daily" => Some(BudgetPeriod::Daily),
                "monthly" => Some(BudgetPeriod::Monthly),
                _ => None,
            }),
            created_at: row.get("created_at"),
            expires_at: row.get("expires_at"),
            revoked_at: row.get("revoked_at"),
            last_used_at: row.get("last_used_at"),
            scopes: scopes.and_then(|s| serde_json::from_str(&s).ok()),
            allowed_models: allowed_models.and_then(|s| serde_json::from_str(&s).ok()),
            ip_allowlist: ip_allowlist.and_then(|s| serde_json::from_str(&s).ok()),
            rate_limit_rpm: row.get("rate_limit_rpm"),
            rate_limit_tpm: row.get("rate_limit_tpm"),
            rotated_from_key_id: row
                .get::<Option<String>, _>("rotated_from_key_id")
                .and_then(|s| Uuid::parse_str(&s).ok()),
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
            SELECT id, key_prefix, name, owner_type, owner_id, budget_amount, budget_period,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
            SELECT id, key_prefix, name, owner_type, owner_id, budget_amount, budget_period,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
            SELECT id, key_prefix, name, owner_type, owner_id, budget_amount, budget_period,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
            SELECT id, key_prefix, name, owner_type, owner_id, budget_amount, budget_period,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
            SELECT id, key_prefix, name, owner_type, owner_id, budget_amount, budget_period,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
            WHERE owner_type = 'service_account' AND owner_id = ?
            AND (created_at, id) {} (?, ?)
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(service_account_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
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
impl ApiKeyRepo for SqliteApiKeyRepo {
    async fn create(&self, input: CreateApiKey, key_hash: &str) -> DbResult<ApiKey> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        // Extract first 8 characters of hash as prefix (gw_live_xxx...)
        let key_prefix = if key_hash.len() >= 8 {
            &key_hash[..8]
        } else {
            key_hash
        };

        let (owner_type, owner_id) = Self::owner_to_parts(&input.owner);

        sqlx::query(
            r#"
            INSERT INTO api_keys (
                id, name, key_hash, key_prefix, owner_type, owner_id,
                budget_amount, budget_period, expires_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&input.name)
        .bind(key_hash)
        .bind(key_prefix)
        .bind(owner_type)
        .bind(owner_id.to_string())
        .bind(input.budget_limit_cents)
        .bind(input.budget_period.map(|p| p.as_str()))
        .bind(input.expires_at)
        .bind(
            input
                .scopes
                .as_ref()
                .and_then(|s| serde_json::to_string(s).ok()),
        )
        .bind(
            input
                .allowed_models
                .as_ref()
                .and_then(|s| serde_json::to_string(s).ok()),
        )
        .bind(
            input
                .ip_allowlist
                .as_ref()
                .and_then(|s| serde_json::to_string(s).ok()),
        )
        .bind(input.rate_limit_rpm)
        .bind(input.rate_limit_tpm)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
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
            created_at: now,
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
            SELECT id, key_prefix, name, owner_type, owner_id, budget_amount, budget_period,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        Ok(Some(Self::parse_api_key(&row)?))
    }

    async fn get_by_hash(&self, key_hash: &str) -> DbResult<Option<ApiKeyWithOwner>> {
        let now = Utc::now();
        let row = sqlx::query(
            r#"
            SELECT
                k.id, k.key_prefix, k.name, k.owner_type, k.owner_id,
                k.budget_amount, k.budget_period, k.expires_at, k.last_used_at, k.created_at,
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
            WHERE k.key_hash = ? AND k.revoked_at IS NULL
              AND (k.rotation_grace_until IS NULL OR k.rotation_grace_until > ?)
            "#,
        )
        .bind(key_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let key = Self::parse_api_key(&row)?;

        let org_id: Option<String> = row.get("org_id");
        let team_id: Option<String> = row.get("team_id");
        let project_id: Option<String> = row.get("project_id");
        let user_id: Option<String> = row.get("user_id");
        let service_account_id: Option<String> = row.get("service_account_id");

        // Parse service account roles from JSON TEXT
        let service_account_roles: Option<Vec<String>> = row
            .get::<Option<String>, _>("service_account_roles")
            .and_then(|s| serde_json::from_str(&s).ok());

        Ok(Some(ApiKeyWithOwner {
            key,
            org_id: org_id.and_then(|s| Uuid::parse_str(&s).ok()),
            team_id: team_id.and_then(|s| Uuid::parse_str(&s).ok()),
            project_id: project_id.and_then(|s| Uuid::parse_str(&s).ok()),
            user_id: user_id.and_then(|s| Uuid::parse_str(&s).ok()),
            service_account_id: service_account_id.and_then(|s| Uuid::parse_str(&s).ok()),
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
            SELECT id, key_prefix, name, owner_type, owner_id, budget_amount, budget_period,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'organization' AND owner_id = ?",
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
            SELECT id, key_prefix, name, owner_type, owner_id, budget_amount, budget_period,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'team' AND owner_id = ?",
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
            SELECT id, key_prefix, name, owner_type, owner_id, budget_amount, budget_period,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'project' AND owner_id = ?",
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
            SELECT id, key_prefix, name, owner_type, owner_id, budget_amount, budget_period,
                   expires_at, last_used_at, created_at, revoked_at,
                   scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                   rotated_from_key_id, rotation_grace_until
            FROM api_keys
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
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'user' AND owner_id = ?",
        )
        .bind(user_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn revoke(&self, id: Uuid) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE api_keys
            SET revoked_at = datetime('now'), updated_at = datetime('now')
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_last_used(&self, id: Uuid) -> DbResult<()> {
        sqlx::query(
            r#"
            UPDATE api_keys
            SET last_used_at = datetime('now')
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn revoke_by_user(&self, user_id: Uuid) -> DbResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE api_keys
            SET revoked_at = datetime('now'), updated_at = datetime('now')
            WHERE owner_type = 'user' AND owner_id = ? AND revoked_at IS NULL
            "#,
        )
        .bind(user_id.to_string())
        .execute(&self.pool)
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
                id, key_prefix, name, owner_type, owner_id,
                budget_amount, budget_period, expires_at, last_used_at, created_at, revoked_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                rotated_from_key_id, rotation_grace_until
            FROM api_keys
            WHERE owner_type = 'service_account' AND owner_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(service_account_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
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
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'service_account' AND owner_id = ?"
        } else {
            "SELECT COUNT(*) as count FROM api_keys WHERE owner_type = 'service_account' AND owner_id = ? AND revoked_at IS NULL"
        };
        let row = sqlx::query(query)
            .bind(service_account_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn revoke_by_service_account(&self, service_account_id: Uuid) -> DbResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE api_keys
            SET revoked_at = datetime('now'), updated_at = datetime('now')
            WHERE owner_type = 'service_account' AND owner_id = ? AND revoked_at IS NULL
            "#,
        )
        .bind(service_account_id.to_string())
        .execute(&self.pool)
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
        let now = Utc::now();

        // Extract first 8 characters of hash as prefix
        let key_prefix = if new_key_hash.len() >= 8 {
            &new_key_hash[..8]
        } else {
            new_key_hash
        };

        let (owner_type, owner_id) = Self::owner_to_parts(&new_key_input.owner);

        // Use a transaction to ensure both operations succeed or fail together
        let mut tx = self.pool.begin().await?;

        // 1. Update old key with grace period
        sqlx::query(
            r#"
            UPDATE api_keys
            SET rotation_grace_until = ?, updated_at = datetime('now')
            WHERE id = ?
            "#,
        )
        .bind(grace_until)
        .bind(old_key_id.to_string())
        .execute(&mut *tx)
        .await?;

        // 2. Insert new key with rotated_from_key_id
        sqlx::query(
            r#"
            INSERT INTO api_keys (
                id, name, key_hash, key_prefix, owner_type, owner_id,
                budget_amount, budget_period, expires_at,
                scopes, allowed_models, ip_allowlist, rate_limit_rpm, rate_limit_tpm,
                rotated_from_key_id,
                created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(new_id.to_string())
        .bind(&new_key_input.name)
        .bind(new_key_hash)
        .bind(key_prefix)
        .bind(owner_type)
        .bind(owner_id.to_string())
        .bind(new_key_input.budget_limit_cents)
        .bind(new_key_input.budget_period.map(|p| p.as_str()))
        .bind(new_key_input.expires_at)
        .bind(
            new_key_input
                .scopes
                .as_ref()
                .and_then(|s| serde_json::to_string(s).ok()),
        )
        .bind(
            new_key_input
                .allowed_models
                .as_ref()
                .and_then(|s| serde_json::to_string(s).ok()),
        )
        .bind(
            new_key_input
                .ip_allowlist
                .as_ref()
                .and_then(|s| serde_json::to_string(s).ok()),
        )
        .bind(new_key_input.rate_limit_rpm)
        .bind(new_key_input.rate_limit_tpm)
        .bind(old_key_id.to_string())
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
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
            created_at: now,
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
              AND owner_id = ?
              AND revoked_at IS NULL
            "#,
        )
        .bind(service_account_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(hashes)
    }

    async fn get_key_hashes_by_user(&self, user_id: Uuid) -> DbResult<Vec<String>> {
        let hashes: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT key_hash
            FROM api_keys
            WHERE owner_type = 'user'
              AND owner_id = ?
              AND revoked_at IS NULL
            "#,
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(hashes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::ApiKeyRepo;

    /// Create an in-memory SQLite database with the required tables
    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create organizations table (needed for project FK)
        sqlx::query(
            r#"
            CREATE TABLE organizations (
                id TEXT PRIMARY KEY NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create organizations table");

        // Create projects table (needed for get_by_hash with project owner)
        sqlx::query(
            r#"
            CREATE TABLE projects (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                slug TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(org_id, slug)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create projects table");

        // Create teams table (needed for get_by_hash with team owner)
        sqlx::query(
            r#"
            CREATE TABLE teams (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                slug TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(org_id, slug)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create teams table");

        // Create service_accounts table (needed for get_by_hash with service_account owner)
        sqlx::query(
            r#"
            CREATE TABLE service_accounts (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                slug TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                roles TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(org_id, slug)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create service_accounts table");

        // Create api_keys table
        sqlx::query(
            r#"
            CREATE TABLE api_keys (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                key_hash TEXT NOT NULL UNIQUE,
                key_prefix TEXT NOT NULL,
                owner_type TEXT NOT NULL CHECK (owner_type IN ('organization', 'team', 'project', 'user', 'service_account')),
                owner_id TEXT NOT NULL,
                budget_amount INTEGER,
                budget_period TEXT CHECK (budget_period IN ('daily', 'monthly')),
                revoked_at TEXT,
                expires_at TEXT,
                last_used_at TEXT,
                scopes TEXT,
                allowed_models TEXT,
                ip_allowlist TEXT,
                rate_limit_rpm INTEGER,
                rate_limit_tpm INTEGER,
                rotated_from_key_id TEXT REFERENCES api_keys(id) ON DELETE SET NULL,
                rotation_grace_until TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create api_keys table");

        pool
    }

    fn create_org_api_key(name: &str, org_id: Uuid) -> CreateApiKey {
        CreateApiKey {
            name: name.to_string(),
            owner: ApiKeyOwner::Organization { org_id },
            budget_limit_cents: None,
            budget_period: None,
            expires_at: None,
            scopes: None,
            allowed_models: None,
            ip_allowlist: None,
            rate_limit_rpm: None,
            rate_limit_tpm: None,
        }
    }

    fn create_project_api_key(name: &str, project_id: Uuid) -> CreateApiKey {
        CreateApiKey {
            name: name.to_string(),
            owner: ApiKeyOwner::Project { project_id },
            budget_limit_cents: None,
            budget_period: None,
            expires_at: None,
            scopes: None,
            allowed_models: None,
            ip_allowlist: None,
            rate_limit_rpm: None,
            rate_limit_tpm: None,
        }
    }

    fn create_user_api_key(name: &str, user_id: Uuid) -> CreateApiKey {
        CreateApiKey {
            name: name.to_string(),
            owner: ApiKeyOwner::User { user_id },
            budget_limit_cents: None,
            budget_period: None,
            expires_at: None,
            scopes: None,
            allowed_models: None,
            ip_allowlist: None,
            rate_limit_rpm: None,
            rate_limit_tpm: None,
        }
    }

    #[tokio::test]
    async fn test_create_org_api_key() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let input = create_org_api_key("Test Key", org_id);
        let key_hash = "abcdef123456789";

        let key = repo
            .create(input, key_hash)
            .await
            .expect("Failed to create API key");

        assert_eq!(key.name, "Test Key");
        assert_eq!(key.key_prefix, "abcdef12"); // First 8 chars of hash
        assert!(matches!(key.owner, ApiKeyOwner::Organization { org_id: id } if id == org_id));
        assert!(key.revoked_at.is_none());
        assert!(key.last_used_at.is_none());
    }

    #[tokio::test]
    async fn test_create_project_api_key() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let project_id = Uuid::new_v4();
        let input = create_project_api_key("Project Key", project_id);
        let key_hash = "projecthash12345";

        let key = repo
            .create(input, key_hash)
            .await
            .expect("Failed to create API key");

        assert_eq!(key.name, "Project Key");
        assert!(matches!(key.owner, ApiKeyOwner::Project { project_id: id } if id == project_id));
    }

    #[tokio::test]
    async fn test_create_user_api_key() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_user_api_key("User Key", user_id);
        let key_hash = "userhash12345678";

        let key = repo
            .create(input, key_hash)
            .await
            .expect("Failed to create API key");

        assert_eq!(key.name, "User Key");
        assert!(matches!(key.owner, ApiKeyOwner::User { user_id: id } if id == user_id));
    }

    #[tokio::test]
    async fn test_create_api_key_with_budget() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let input = CreateApiKey {
            name: "Budget Key".to_string(),
            owner: ApiKeyOwner::Organization { org_id },
            budget_limit_cents: Some(10000), // $100
            budget_period: Some(BudgetPeriod::Monthly),
            expires_at: None,
            scopes: None,
            allowed_models: None,
            ip_allowlist: None,
            rate_limit_rpm: None,
            rate_limit_tpm: None,
        };

        let key = repo
            .create(input, "budgethash123456")
            .await
            .expect("Failed to create API key");

        assert_eq!(key.budget_limit_cents, Some(10000));
        assert_eq!(key.budget_period, Some(BudgetPeriod::Monthly));
    }

    #[tokio::test]
    async fn test_create_api_key_with_scopes() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let input = CreateApiKey {
            name: "Scoped Key".to_string(),
            owner: ApiKeyOwner::Organization { org_id },
            budget_limit_cents: None,
            budget_period: None,
            expires_at: None,
            scopes: Some(vec!["chat".to_string(), "embeddings".to_string()]),
            allowed_models: Some(vec!["gpt-4*".to_string()]),
            ip_allowlist: Some(vec!["10.0.0.0/8".to_string()]),
            rate_limit_rpm: Some(100),
            rate_limit_tpm: Some(50000),
        };

        let key = repo
            .create(input, "scopedhash12345")
            .await
            .expect("Failed to create API key");

        assert_eq!(
            key.scopes,
            Some(vec!["chat".to_string(), "embeddings".to_string()])
        );
        assert_eq!(key.allowed_models, Some(vec!["gpt-4*".to_string()]));
        assert_eq!(key.ip_allowlist, Some(vec!["10.0.0.0/8".to_string()]));
        assert_eq!(key.rate_limit_rpm, Some(100));
        assert_eq!(key.rate_limit_tpm, Some(50000));
        assert!(key.rotated_from_key_id.is_none());
        assert!(key.rotation_grace_until.is_none());

        // Also verify it can be fetched correctly
        let fetched = repo
            .get_by_id(key.id)
            .await
            .expect("Query should succeed")
            .expect("Key should exist");

        assert_eq!(
            fetched.scopes,
            Some(vec!["chat".to_string(), "embeddings".to_string()])
        );
        assert_eq!(fetched.allowed_models, Some(vec!["gpt-4*".to_string()]));
        assert_eq!(fetched.ip_allowlist, Some(vec!["10.0.0.0/8".to_string()]));
        assert_eq!(fetched.rate_limit_rpm, Some(100));
        assert_eq!(fetched.rate_limit_tpm, Some(50000));
    }

    #[tokio::test]
    async fn test_create_duplicate_hash_fails() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let key_hash = "duplicatehash123";

        repo.create(create_org_api_key("First Key", org_id), key_hash)
            .await
            .expect("First key should succeed");

        let result = repo
            .create(create_org_api_key("Second Key", org_id), key_hash)
            .await;

        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let created = repo
            .create(create_org_api_key("Get Test", org_id), "gettesthash12345")
            .await
            .expect("Failed to create key");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed")
            .expect("Key should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Get Test");
        assert!(matches!(fetched.owner, ApiKeyOwner::Organization { .. }));
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let result = repo
            .get_by_id(Uuid::new_v4())
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_id_returns_revoked_key() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let created = repo
            .create(create_org_api_key("Revoke Test", org_id), "revoketesthash")
            .await
            .expect("Failed to create key");

        repo.revoke(created.id).await.expect("Failed to revoke key");

        // get_by_id should still return revoked keys
        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed")
            .expect("Key should exist");

        assert_eq!(fetched.id, created.id);
        assert!(fetched.revoked_at.is_some());
    }

    #[tokio::test]
    async fn test_get_by_hash() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let key_hash = "hashfortesting12";
        let created = repo
            .create(create_org_api_key("Hash Test", org_id), key_hash)
            .await
            .expect("Failed to create key");

        let fetched = repo
            .get_by_hash(key_hash)
            .await
            .expect("Query should succeed")
            .expect("Key should exist");

        assert_eq!(fetched.key.id, created.id);
        assert_eq!(fetched.key.name, "Hash Test");
        assert_eq!(fetched.org_id, Some(org_id));
        assert!(fetched.project_id.is_none());
        assert!(fetched.user_id.is_none());
    }

    #[tokio::test]
    async fn test_get_by_hash_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let result = repo
            .get_by_hash("nonexistenthash1")
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_hash_excludes_revoked() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let key_hash = "revokedhash12345";
        let created = repo
            .create(create_org_api_key("Revoked Key", org_id), key_hash)
            .await
            .expect("Failed to create key");

        repo.revoke(created.id).await.expect("Failed to revoke key");

        // get_by_hash should NOT return revoked keys
        let result = repo
            .get_by_hash(key_hash)
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_hash_project_key_includes_org_id() {
        let pool = create_test_pool().await;

        // Create org and project first
        let org_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO organizations (id, slug, name) VALUES (?, 'test-org', 'Test Org')",
        )
        .bind(org_id.to_string())
        .execute(&pool)
        .await
        .expect("Failed to create org");

        sqlx::query(
            "INSERT INTO projects (id, org_id, slug, name) VALUES (?, ?, 'test-project', 'Test Project')",
        )
        .bind(project_id.to_string())
        .bind(org_id.to_string())
        .execute(&pool)
        .await
        .expect("Failed to create project");

        let repo = SqliteApiKeyRepo::new(pool);
        let key_hash = "projectkeyhash12";

        repo.create(create_project_api_key("Project Key", project_id), key_hash)
            .await
            .expect("Failed to create key");

        let fetched = repo
            .get_by_hash(key_hash)
            .await
            .expect("Query should succeed")
            .expect("Key should exist");

        // For project keys, org_id should be looked up from the project
        assert_eq!(fetched.org_id, Some(org_id));
        assert_eq!(fetched.project_id, Some(project_id));
        assert!(fetched.user_id.is_none());
    }

    #[tokio::test]
    async fn test_get_by_hash_user_key() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let user_id = Uuid::new_v4();
        let key_hash = "userkeyhashhash1";

        repo.create(create_user_api_key("User Key", user_id), key_hash)
            .await
            .expect("Failed to create key");

        let fetched = repo
            .get_by_hash(key_hash)
            .await
            .expect("Query should succeed")
            .expect("Key should exist");

        // For user keys, org_id and project_id should be None
        assert!(fetched.org_id.is_none());
        assert!(fetched.project_id.is_none());
        assert_eq!(fetched.user_id, Some(user_id));
    }

    #[tokio::test]
    async fn test_list_by_org_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let result = repo
            .list_by_org(Uuid::new_v4(), ListParams::default())
            .await
            .expect("Failed to list keys");

        assert!(result.items.is_empty());
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_by_org() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();

        for i in 0..3 {
            repo.create(
                create_org_api_key(&format!("Key {}", i), org_id),
                &format!("hash{:016}", i),
            )
            .await
            .expect("Failed to create key");
        }

        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list keys");

        assert_eq!(result.items.len(), 3);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_by_org_only_returns_org_keys() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let other_org_id = Uuid::new_v4();

        repo.create(create_org_api_key("Our Key", org_id), "ourkeyhash123456")
            .await
            .expect("Failed to create key");

        repo.create(
            create_org_api_key("Other Key", other_org_id),
            "otherkeyhash1234",
        )
        .await
        .expect("Failed to create key");

        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list keys");

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "Our Key");
    }

    #[tokio::test]
    async fn test_list_by_org_pagination() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();

        for i in 0..5 {
            repo.create(
                create_org_api_key(&format!("Key {}", i), org_id),
                &format!("paginationhash{:02}", i),
            )
            .await
            .expect("Failed to create key");
        }

        // First page (no cursor)
        let page1 = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(2),
                    include_deleted: false,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 1");

        // Second page (using cursor from first page)
        let page2 = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(2),
                    include_deleted: false,
                    cursor: page1.cursors.next.clone(),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 2");

        assert_eq!(page1.items.len(), 2);
        assert_eq!(page2.items.len(), 2);
        assert!(page1.has_more);
        assert!(page2.has_more);
        assert_ne!(page1.items[0].id, page2.items[0].id);
    }

    #[tokio::test]
    async fn test_count_by_org() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();

        for i in 0..3 {
            repo.create(
                create_org_api_key(&format!("Key {}", i), org_id),
                &format!("countorg{:08}", i),
            )
            .await
            .expect("Failed to create key");
        }

        let count = repo
            .count_by_org(org_id, false)
            .await
            .expect("Failed to count keys");

        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_list_by_project() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let project_id = Uuid::new_v4();

        for i in 0..3 {
            repo.create(
                create_project_api_key(&format!("Project Key {}", i), project_id),
                &format!("projecthash{:05}", i),
            )
            .await
            .expect("Failed to create key");
        }

        let result = repo
            .list_by_project(project_id, ListParams::default())
            .await
            .expect("Failed to list keys");

        assert_eq!(result.items.len(), 3);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_count_by_project() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let project_id = Uuid::new_v4();

        for i in 0..2 {
            repo.create(
                create_project_api_key(&format!("Key {}", i), project_id),
                &format!("projcount{:06}", i),
            )
            .await
            .expect("Failed to create key");
        }

        let count = repo
            .count_by_project(project_id, false)
            .await
            .expect("Failed to count keys");

        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_list_by_user() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let user_id = Uuid::new_v4();

        for i in 0..3 {
            repo.create(
                create_user_api_key(&format!("User Key {}", i), user_id),
                &format!("userhash{:07}", i),
            )
            .await
            .expect("Failed to create key");
        }

        let result = repo
            .list_by_user(user_id, ListParams::default())
            .await
            .expect("Failed to list keys");

        assert_eq!(result.items.len(), 3);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_count_by_user() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let user_id = Uuid::new_v4();

        for i in 0..4 {
            repo.create(
                create_user_api_key(&format!("Key {}", i), user_id),
                &format!("usercount{:05}", i),
            )
            .await
            .expect("Failed to create key");
        }

        let count = repo
            .count_by_user(user_id, false)
            .await
            .expect("Failed to count keys");

        assert_eq!(count, 4);
    }

    #[tokio::test]
    async fn test_revoke() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let created = repo
            .create(create_org_api_key("To Revoke", org_id), "revokekeyhash123")
            .await
            .expect("Failed to create key");

        assert!(created.revoked_at.is_none());

        repo.revoke(created.id).await.expect("Failed to revoke key");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed")
            .expect("Key should exist");

        assert!(fetched.revoked_at.is_some());
    }

    #[tokio::test]
    async fn test_revoke_nonexistent_key_succeeds() {
        // revoke doesn't check if the key exists, it just sets is_active=0
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        // This should not error even though key doesn't exist
        repo.revoke(Uuid::new_v4())
            .await
            .expect("Revoke should succeed even for non-existent key");
    }

    #[tokio::test]
    async fn test_update_last_used() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let created = repo
            .create(
                create_org_api_key("Last Used Test", org_id),
                "lastusedkey1234",
            )
            .await
            .expect("Failed to create key");

        assert!(created.last_used_at.is_none());

        repo.update_last_used(created.id)
            .await
            .expect("Failed to update last_used");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed")
            .expect("Key should exist");

        assert!(fetched.last_used_at.is_some());
    }

    #[tokio::test]
    async fn test_list_includes_revoked_keys() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();

        let key1 = repo
            .create(create_org_api_key("Active Key", org_id), "activekey1234567")
            .await
            .expect("Failed to create key");

        let key2 = repo
            .create(
                create_org_api_key("Revoked Key", org_id),
                "revokedkey123456",
            )
            .await
            .expect("Failed to create key");

        repo.revoke(key2.id).await.expect("Failed to revoke key");

        // list_by_org includes all keys (active and revoked)
        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list keys");

        assert_eq!(result.items.len(), 2);

        let active_key = result.items.iter().find(|k| k.id == key1.id).unwrap();
        let revoked_key = result.items.iter().find(|k| k.id == key2.id).unwrap();

        assert!(active_key.revoked_at.is_none());
        assert!(revoked_key.revoked_at.is_some());
    }

    #[tokio::test]
    async fn test_count_includes_revoked_keys() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();

        let key1 = repo
            .create(create_org_api_key("Key 1", org_id), "countrevoked1234")
            .await
            .expect("Failed to create key");

        repo.create(create_org_api_key("Key 2", org_id), "countrevoked5678")
            .await
            .expect("Failed to create key");

        repo.revoke(key1.id).await.expect("Failed to revoke key");

        // count_by_org includes all keys (active and revoked)
        let count = repo
            .count_by_org(org_id, false)
            .await
            .expect("Failed to count keys");

        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_budget_period_daily() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let input = CreateApiKey {
            name: "Daily Budget".to_string(),
            owner: ApiKeyOwner::Organization { org_id },
            budget_limit_cents: Some(5000),
            budget_period: Some(BudgetPeriod::Daily),
            expires_at: None,
            scopes: None,
            allowed_models: None,
            ip_allowlist: None,
            rate_limit_rpm: None,
            rate_limit_tpm: None,
        };

        let created = repo
            .create(input, "dailybudgethash1")
            .await
            .expect("Failed to create key");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed")
            .expect("Key should exist");

        assert_eq!(fetched.budget_period, Some(BudgetPeriod::Daily));
    }

    #[tokio::test]
    async fn test_owner_parsing() {
        // Test the internal parse_owner function via create/get cycle
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let org_key = repo
            .create(create_org_api_key("Org", org_id), "parseorg12345678")
            .await
            .expect("Failed to create org key");

        let proj_key = repo
            .create(
                create_project_api_key("Project", project_id),
                "parseproj1234567",
            )
            .await
            .expect("Failed to create project key");

        let user_key = repo
            .create(create_user_api_key("User", user_id), "parseuser1234567")
            .await
            .expect("Failed to create user key");

        let fetched_org = repo.get_by_id(org_key.id).await.unwrap().unwrap();
        let fetched_proj = repo.get_by_id(proj_key.id).await.unwrap().unwrap();
        let fetched_user = repo.get_by_id(user_key.id).await.unwrap().unwrap();

        assert!(matches!(
            fetched_org.owner,
            ApiKeyOwner::Organization { org_id: id } if id == org_id
        ));
        assert!(matches!(
            fetched_proj.owner,
            ApiKeyOwner::Project { project_id: id } if id == project_id
        ));
        assert!(matches!(
            fetched_user.owner,
            ApiKeyOwner::User { user_id: id } if id == user_id
        ));
    }

    #[tokio::test]
    async fn test_rotate_creates_new_key_with_rotated_from_key_id() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let old_key = repo
            .create(
                create_org_api_key("Original Key", org_id),
                "originalhash1234",
            )
            .await
            .expect("Failed to create old key");

        let grace_until = Utc::now() + chrono::Duration::hours(24);
        let new_key_input = create_org_api_key("Original Key (rotated)", org_id);

        let new_key = repo
            .rotate(old_key.id, new_key_input, "newkeyhash123456", grace_until)
            .await
            .expect("Failed to rotate key");

        assert_eq!(new_key.name, "Original Key (rotated)");
        assert_eq!(new_key.rotated_from_key_id, Some(old_key.id));
        assert!(new_key.rotation_grace_until.is_none()); // New key doesn't have grace period
        assert!(matches!(new_key.owner, ApiKeyOwner::Organization { org_id: id } if id == org_id));
    }

    #[tokio::test]
    async fn test_rotate_sets_grace_until_on_old_key() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let old_key = repo
            .create(
                create_org_api_key("Key to Rotate", org_id),
                "keytorotate12345",
            )
            .await
            .expect("Failed to create old key");

        assert!(old_key.rotation_grace_until.is_none());

        let grace_until = Utc::now() + chrono::Duration::hours(1);
        let new_key_input = create_org_api_key("Key to Rotate (rotated)", org_id);

        repo.rotate(old_key.id, new_key_input, "rotatedkey123456", grace_until)
            .await
            .expect("Failed to rotate key");

        // Fetch the old key and verify grace_until is set
        let updated_old_key = repo
            .get_by_id(old_key.id)
            .await
            .expect("Query should succeed")
            .expect("Old key should still exist");

        assert!(updated_old_key.rotation_grace_until.is_some());
    }

    #[tokio::test]
    async fn test_old_key_works_during_grace_period() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let old_key = repo
            .create(create_org_api_key("Grace Test", org_id), "gracetesthash123")
            .await
            .expect("Failed to create old key");

        // Set grace period to 1 hour from now
        let grace_until = Utc::now() + chrono::Duration::hours(1);
        let new_key_input = create_org_api_key("Grace Test (rotated)", org_id);

        repo.rotate(old_key.id, new_key_input, "newgracetesthash", grace_until)
            .await
            .expect("Failed to rotate key");

        // Old key should still be retrievable by hash during grace period
        let result = repo
            .get_by_hash("gracetesthash123")
            .await
            .expect("Query should succeed");

        assert!(
            result.is_some(),
            "Old key should still work during grace period"
        );
    }

    #[tokio::test]
    async fn test_old_key_fails_after_grace_period() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let old_key = repo
            .create(
                create_org_api_key("Expired Grace", org_id),
                "expiredgracehash",
            )
            .await
            .expect("Failed to create old key");

        // Set grace period to 1 second ago (already expired)
        let grace_until = Utc::now() - chrono::Duration::seconds(1);
        let new_key_input = create_org_api_key("Expired Grace (rotated)", org_id);

        repo.rotate(old_key.id, new_key_input, "newexpiredhash12", grace_until)
            .await
            .expect("Failed to rotate key");

        // Old key should NOT be retrievable by hash after grace period expired
        let result = repo
            .get_by_hash("expiredgracehash")
            .await
            .expect("Query should succeed");

        assert!(
            result.is_none(),
            "Old key should not work after grace period expired"
        );
    }

    #[tokio::test]
    async fn test_new_key_works_after_rotation() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let old_key = repo
            .create(
                create_org_api_key("New Key Test", org_id),
                "newkeytesthash12",
            )
            .await
            .expect("Failed to create old key");

        let grace_until = Utc::now() + chrono::Duration::hours(1);
        let new_key_input = create_org_api_key("New Key Test (rotated)", org_id);
        let new_key_hash = "rotatednewkey123";

        let new_key = repo
            .rotate(old_key.id, new_key_input, new_key_hash, grace_until)
            .await
            .expect("Failed to rotate key");

        // New key should be retrievable by its hash
        let result = repo
            .get_by_hash(new_key_hash)
            .await
            .expect("Query should succeed");

        assert!(result.is_some());
        assert_eq!(result.unwrap().key.id, new_key.id);
    }

    #[tokio::test]
    async fn test_rotate_copies_key_settings() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let org_id = Uuid::new_v4();
        let input = CreateApiKey {
            name: "Settings Test".to_string(),
            owner: ApiKeyOwner::Organization { org_id },
            budget_limit_cents: Some(10000),
            budget_period: Some(BudgetPeriod::Monthly),
            expires_at: None,
            scopes: Some(vec!["chat".to_string(), "embeddings".to_string()]),
            allowed_models: Some(vec!["gpt-4*".to_string()]),
            ip_allowlist: Some(vec!["10.0.0.0/8".to_string()]),
            rate_limit_rpm: Some(100),
            rate_limit_tpm: Some(50000),
        };

        let old_key = repo
            .create(input, "settingstesthash")
            .await
            .expect("Failed to create old key");

        let grace_until = Utc::now() + chrono::Duration::hours(1);
        let new_key_input = CreateApiKey {
            name: "Settings Test (rotated)".to_string(),
            owner: ApiKeyOwner::Organization { org_id },
            budget_limit_cents: Some(10000),
            budget_period: Some(BudgetPeriod::Monthly),
            expires_at: None,
            scopes: Some(vec!["chat".to_string(), "embeddings".to_string()]),
            allowed_models: Some(vec!["gpt-4*".to_string()]),
            ip_allowlist: Some(vec!["10.0.0.0/8".to_string()]),
            rate_limit_rpm: Some(100),
            rate_limit_tpm: Some(50000),
        };

        let new_key = repo
            .rotate(old_key.id, new_key_input, "newsettingshash1", grace_until)
            .await
            .expect("Failed to rotate key");

        // Verify settings were copied to new key
        assert_eq!(new_key.budget_limit_cents, Some(10000));
        assert_eq!(new_key.budget_period, Some(BudgetPeriod::Monthly));
        assert_eq!(
            new_key.scopes,
            Some(vec!["chat".to_string(), "embeddings".to_string()])
        );
        assert_eq!(new_key.allowed_models, Some(vec!["gpt-4*".to_string()]));
        assert_eq!(new_key.ip_allowlist, Some(vec!["10.0.0.0/8".to_string()]));
        assert_eq!(new_key.rate_limit_rpm, Some(100));
        assert_eq!(new_key.rate_limit_tpm, Some(50000));
    }

    #[tokio::test]
    async fn test_get_key_hashes_by_user() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();

        // Create two active keys for the user
        let key1 = repo
            .create(create_user_api_key("User Key 1", user_id), "userhash_1_abc")
            .await
            .expect("Failed to create key");
        let _key2 = repo
            .create(create_user_api_key("User Key 2", user_id), "userhash_2_def")
            .await
            .expect("Failed to create key");

        // Create a key for another user (should not be returned)
        let _other = repo
            .create(
                create_user_api_key("Other Key", other_user_id),
                "otherhash_abc",
            )
            .await
            .expect("Failed to create key");

        // Create an org key (should not be returned — different owner_type)
        let org_id = Uuid::new_v4();
        let _org_key = repo
            .create(create_org_api_key("Org Key", org_id), "orghash_abc1234")
            .await
            .expect("Failed to create key");

        let hashes = repo
            .get_key_hashes_by_user(user_id)
            .await
            .expect("Failed to get key hashes");

        assert_eq!(hashes.len(), 2);
        assert!(hashes.contains(&"userhash_1_abc".to_string()));
        assert!(hashes.contains(&"userhash_2_def".to_string()));

        // Revoke one key — should be excluded
        repo.revoke(key1.id).await.expect("Failed to revoke");
        let hashes_after = repo
            .get_key_hashes_by_user(user_id)
            .await
            .expect("Failed to get key hashes");
        assert_eq!(hashes_after.len(), 1);
        assert!(hashes_after.contains(&"userhash_2_def".to_string()));
    }

    #[tokio::test]
    async fn test_get_key_hashes_by_user_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteApiKeyRepo::new(pool);

        let hashes = repo
            .get_key_hashes_by_user(Uuid::new_v4())
            .await
            .expect("Failed to get key hashes");
        assert!(hashes.is_empty());
    }
}
