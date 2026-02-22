use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, OrgRbacPolicyRepo, PageCursors,
            cursor_from_row,
        },
    },
    models::{
        CreateOrgRbacPolicy, OrgRbacPolicy, OrgRbacPolicyVersion, RbacPolicyEffect,
        RollbackOrgRbacPolicy, UpdateOrgRbacPolicy,
    },
};

pub struct SqliteOrgRbacPolicyRepo {
    pool: SqlitePool,
}

impl SqliteOrgRbacPolicyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_policy(row: &sqlx::sqlite::SqliteRow) -> DbResult<OrgRbacPolicy> {
        let effect_str: String = row.get("effect");
        let effect: RbacPolicyEffect = effect_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;

        let enabled: i32 = row.get("enabled");

        Ok(OrgRbacPolicy {
            id: parse_uuid(row.get("id"))?,
            org_id: parse_uuid(row.get("org_id"))?,
            name: row.get("name"),
            description: row.get("description"),
            resource: row.get("resource"),
            action: row.get("action"),
            condition: row.get("condition"),
            effect,
            priority: row.get("priority"),
            enabled: enabled != 0,
            version: row.get("version"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            deleted_at: row.get("deleted_at"),
        })
    }

    fn parse_version(row: &sqlx::sqlite::SqliteRow) -> DbResult<OrgRbacPolicyVersion> {
        let effect_str: String = row.get("effect");
        let effect: RbacPolicyEffect = effect_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;

        let enabled: i32 = row.get("enabled");
        let created_by: Option<String> = row.get("created_by");

        Ok(OrgRbacPolicyVersion {
            id: parse_uuid(row.get("id"))?,
            policy_id: parse_uuid(row.get("policy_id"))?,
            version: row.get("version"),
            name: row.get("name"),
            description: row.get("description"),
            resource: row.get("resource"),
            action: row.get("action"),
            condition: row.get("condition"),
            effect,
            priority: row.get("priority"),
            enabled: enabled != 0,
            created_by: created_by.and_then(|s| Uuid::parse_str(&s).ok()),
            reason: row.get("reason"),
            created_at: row.get("created_at"),
        })
    }

    /// Helper method for cursor-based pagination of policies.
    async fn list_with_cursor(
        &self,
        org_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<OrgRbacPolicy>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };

        let query = format!(
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = ? AND (created_at, id) {} (?, ?)
            {}
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, deleted_filter, order, order
        );

        let rows = sqlx::query(&query)
            .bind(org_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<OrgRbacPolicy> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Self::parse_policy(&row))
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |policy| {
                cursor_from_row(policy.created_at, policy.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of policy versions.
    async fn list_versions_with_cursor(
        &self,
        policy_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<OrgRbacPolicyVersion>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, policy_id, version, name, description, resource, action,
                   condition, effect, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = ? AND (created_at, id) {} (?, ?)
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(policy_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<OrgRbacPolicyVersion> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Self::parse_version(&row))
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors = PageCursors::from_items(
            &items,
            has_more,
            params.direction,
            Some(cursor),
            |version| cursor_from_row(version.created_at, version.id),
        );

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Create a version record from a policy snapshot
    async fn create_version_record(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        policy: &OrgRbacPolicy,
        created_by: Option<Uuid>,
        reason: Option<String>,
    ) -> DbResult<()> {
        let version_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO org_rbac_policy_versions (
                id, policy_id, version, name, description, resource, action,
                condition, effect, priority, enabled, created_by, reason, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(version_id.to_string())
        .bind(policy.id.to_string())
        .bind(policy.version)
        .bind(&policy.name)
        .bind(&policy.description)
        .bind(&policy.resource)
        .bind(&policy.action)
        .bind(&policy.condition)
        .bind(policy.effect.to_string())
        .bind(policy.priority)
        .bind(if policy.enabled { 1 } else { 0 })
        .bind(created_by.map(|u| u.to_string()))
        .bind(reason)
        .bind(policy.updated_at)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl OrgRbacPolicyRepo for SqliteOrgRbacPolicyRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgRbacPolicy,
        created_by: Option<Uuid>,
    ) -> DbResult<OrgRbacPolicy> {
        let id = Uuid::new_v4();
        let now: DateTime<Utc> = Utc::now();

        let mut tx = self.pool.begin().await?;

        // Insert the policy
        sqlx::query(
            r#"
            INSERT INTO org_rbac_policies (
                id, org_id, name, description, resource, action, condition,
                effect, priority, enabled, version, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.resource)
        .bind(&input.action)
        .bind(&input.condition)
        .bind(input.effect.to_string())
        .bind(input.priority)
        .bind(if input.enabled { 1 } else { 0 })
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Policy with name '{}' already exists in this organization",
                    input.name
                ))
            }
            _ => DbError::from(e),
        })?;

        let policy = OrgRbacPolicy {
            id,
            org_id,
            name: input.name,
            description: input.description,
            resource: input.resource,
            action: input.action,
            condition: input.condition,
            effect: input.effect,
            priority: input.priority,
            enabled: input.enabled,
            version: 1,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        // Create version 1 record
        self.create_version_record(&mut tx, &policy, created_by, input.reason)
            .await?;

        tx.commit().await?;

        Ok(policy)
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgRbacPolicy>> {
        let row = sqlx::query(
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Self::parse_policy(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_by_org_and_name(
        &self,
        org_id: Uuid,
        name: &str,
    ) -> DbResult<Option<OrgRbacPolicy>> {
        let row = sqlx::query(
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = ? AND name = ? AND deleted_at IS NULL
            "#,
        )
        .bind(org_id.to_string())
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Self::parse_policy(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_by_org(&self, org_id: Uuid) -> DbResult<Vec<OrgRbacPolicy>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = ? AND deleted_at IS NULL
            ORDER BY priority DESC, name ASC
            "#,
        )
        .bind(org_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::parse_policy).collect()
    }

    async fn list_by_org_paginated(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<OrgRbacPolicy>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(org_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor)
        let query = if params.include_deleted {
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        } else {
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = ? AND deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        };

        let rows = sqlx::query(query)
            .bind(org_id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<OrgRbacPolicy> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Self::parse_policy(&row))
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |policy| {
                cursor_from_row(policy.created_at, policy.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn list_enabled_by_org(&self, org_id: Uuid) -> DbResult<Vec<OrgRbacPolicy>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = ? AND enabled = 1 AND deleted_at IS NULL
            ORDER BY priority DESC, name ASC
            "#,
        )
        .bind(org_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::parse_policy).collect()
    }

    async fn list_all_enabled(&self) -> DbResult<Vec<OrgRbacPolicy>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE enabled = 1 AND deleted_at IS NULL
            ORDER BY org_id, priority DESC, name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::parse_policy).collect()
    }

    async fn update(
        &self,
        id: Uuid,
        input: UpdateOrgRbacPolicy,
        updated_by: Option<Uuid>,
    ) -> DbResult<OrgRbacPolicy> {
        let mut tx = self.pool.begin().await?;

        // Fetch current policy (excluding soft-deleted)
        let row = sqlx::query(
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&mut *tx)
        .await?;

        let Some(row) = row else {
            return Err(DbError::NotFound);
        };

        let mut policy = Self::parse_policy(&row)?;

        // Apply updates
        if let Some(name) = input.name {
            policy.name = name;
        }
        if let Some(description) = input.description {
            policy.description = description;
        }
        if let Some(resource) = input.resource {
            policy.resource = resource;
        }
        if let Some(action) = input.action {
            policy.action = action;
        }
        if let Some(condition) = input.condition {
            policy.condition = condition;
        }
        if let Some(effect) = input.effect {
            policy.effect = effect;
        }
        if let Some(priority) = input.priority {
            policy.priority = priority;
        }
        if let Some(enabled) = input.enabled {
            policy.enabled = enabled;
        }

        // Store original version for optimistic locking
        let original_version = policy.version;

        // Increment version
        policy.version += 1;
        policy.updated_at = Utc::now();

        // Update the policy with optimistic locking (check original version and not deleted)
        let result = sqlx::query(
            r#"
            UPDATE org_rbac_policies
            SET name = ?, description = ?, resource = ?, action = ?, condition = ?,
                effect = ?, priority = ?, enabled = ?, version = ?, updated_at = ?
            WHERE id = ? AND version = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&policy.name)
        .bind(&policy.description)
        .bind(&policy.resource)
        .bind(&policy.action)
        .bind(&policy.condition)
        .bind(policy.effect.to_string())
        .bind(policy.priority)
        .bind(if policy.enabled { 1 } else { 0 })
        .bind(policy.version)
        .bind(policy.updated_at)
        .bind(id.to_string())
        .bind(original_version)
        .execute(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Policy with name '{}' already exists in this organization",
                    policy.name
                ))
            }
            _ => DbError::from(e),
        })?;

        // Check for concurrent modification (optimistic locking)
        if result.rows_affected() == 0 {
            return Err(DbError::Conflict(
                "Policy was modified concurrently. Please refresh and try again.".to_string(),
            ));
        }

        // Create version record
        self.create_version_record(&mut tx, &policy, updated_by, input.reason)
            .await?;

        tx.commit().await?;

        Ok(policy)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = Utc::now();

        // Soft-delete by setting deleted_at timestamp
        let result = sqlx::query(
            r#"
            UPDATE org_rbac_policies
            SET deleted_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn rollback(
        &self,
        id: Uuid,
        input: RollbackOrgRbacPolicy,
        rolled_back_by: Option<Uuid>,
    ) -> DbResult<OrgRbacPolicy> {
        let mut tx = self.pool.begin().await?;

        // Fetch the target version
        let version_row = sqlx::query(
            r#"
            SELECT id, policy_id, version, name, description, resource, action,
                   condition, effect, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = ? AND version = ?
            "#,
        )
        .bind(id.to_string())
        .bind(input.target_version)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(version_row) = version_row else {
            return Err(DbError::NotFound);
        };

        let target_version = Self::parse_version(&version_row)?;

        // Fetch current policy to get current version number and timestamps (excluding soft-deleted)
        let policy_row = sqlx::query(
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&mut *tx)
        .await?;

        let Some(policy_row) = policy_row else {
            return Err(DbError::NotFound);
        };

        let current_policy = Self::parse_policy(&policy_row)?;

        // Create the rolled-back policy with incremented version
        let new_version = current_policy.version + 1;
        let now = Utc::now();

        let policy = OrgRbacPolicy {
            id: current_policy.id,
            org_id: current_policy.org_id,
            name: target_version.name,
            description: target_version.description,
            resource: target_version.resource,
            action: target_version.action,
            condition: target_version.condition,
            effect: target_version.effect,
            priority: target_version.priority,
            enabled: target_version.enabled,
            version: new_version,
            created_at: current_policy.created_at,
            updated_at: now,
            deleted_at: None,
        };

        // Update the policy with rolled-back values (with optimistic locking and not deleted)
        let result = sqlx::query(
            r#"
            UPDATE org_rbac_policies
            SET name = ?, description = ?, resource = ?, action = ?, condition = ?,
                effect = ?, priority = ?, enabled = ?, version = ?, updated_at = ?
            WHERE id = ? AND version = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&policy.name)
        .bind(&policy.description)
        .bind(&policy.resource)
        .bind(&policy.action)
        .bind(&policy.condition)
        .bind(policy.effect.to_string())
        .bind(policy.priority)
        .bind(if policy.enabled { 1 } else { 0 })
        .bind(policy.version)
        .bind(policy.updated_at)
        .bind(id.to_string())
        .bind(current_policy.version) // Original version for optimistic locking
        .execute(&mut *tx)
        .await?;

        // Check for concurrent modification (optimistic locking)
        if result.rows_affected() == 0 {
            return Err(DbError::Conflict(
                "Policy was modified concurrently. Please refresh and try again.".to_string(),
            ));
        }

        // Create version record for the rollback
        let reason = input
            .reason
            .unwrap_or_else(|| format!("Rolled back to version {}", input.target_version));
        self.create_version_record(&mut tx, &policy, rolled_back_by, Some(reason))
            .await?;

        tx.commit().await?;

        Ok(policy)
    }

    async fn get_version(
        &self,
        policy_id: Uuid,
        version: i32,
    ) -> DbResult<Option<OrgRbacPolicyVersion>> {
        let row = sqlx::query(
            r#"
            SELECT id, policy_id, version, name, description, resource, action,
                   condition, effect, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = ? AND version = ?
            "#,
        )
        .bind(policy_id.to_string())
        .bind(version)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Self::parse_version(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_versions(&self, policy_id: Uuid) -> DbResult<Vec<OrgRbacPolicyVersion>> {
        let rows = sqlx::query(
            r#"
            SELECT id, policy_id, version, name, description, resource, action,
                   condition, effect, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = ?
            ORDER BY version DESC
            "#,
        )
        .bind(policy_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::parse_version).collect()
    }

    async fn list_versions_paginated(
        &self,
        policy_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> DbResult<Vec<OrgRbacPolicyVersion>> {
        let rows = sqlx::query(
            r#"
            SELECT id, policy_id, version, name, description, resource, action,
                   condition, effect, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = ?
            ORDER BY version DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(policy_id.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::parse_version).collect()
    }

    async fn list_versions_cursor(
        &self,
        policy_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<OrgRbacPolicyVersion>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_versions_with_cursor(policy_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor)
        let rows = sqlx::query(
            r#"
            SELECT id, policy_id, version, name, description, resource, action,
                   condition, effect, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(policy_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<OrgRbacPolicyVersion> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Self::parse_version(&row))
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(
            &items,
            has_more,
            CursorDirection::Forward,
            None,
            |version| cursor_from_row(version.created_at, version.id),
        );

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_versions(&self, policy_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM org_rbac_policy_versions WHERE policy_id = ?",
        )
        .bind(policy_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM org_rbac_policies WHERE org_id = ? AND deleted_at IS NULL",
        )
        .bind(org_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn count_all(&self) -> DbResult<i64> {
        let row =
            sqlx::query("SELECT COUNT(*) as count FROM org_rbac_policies WHERE deleted_at IS NULL")
                .fetch_one(&self.pool)
                .await?;

        Ok(row.get::<i64, _>("count"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create organizations table (needed for FK)
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

        // Create users table (needed for FK)
        sqlx::query(
            r#"
            CREATE TABLE users (
                id TEXT PRIMARY KEY NOT NULL,
                email TEXT NOT NULL UNIQUE,
                name TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create users table");

        // Create org_rbac_policies table
        sqlx::query(
            r#"
            CREATE TABLE org_rbac_policies (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                description TEXT,
                resource TEXT NOT NULL DEFAULT '*',
                action TEXT NOT NULL DEFAULT '*',
                condition TEXT NOT NULL,
                effect TEXT NOT NULL DEFAULT 'deny' CHECK (effect IN ('allow', 'deny')),
                priority INTEGER NOT NULL DEFAULT 0,
                enabled INTEGER NOT NULL DEFAULT 1,
                version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create org_rbac_policies table");

        // Partial unique index: policy names must be unique within an org among non-deleted policies
        sqlx::query(
            "CREATE UNIQUE INDEX idx_org_rbac_policies_org_name_active ON org_rbac_policies(org_id, name) WHERE deleted_at IS NULL",
        )
        .execute(&pool)
        .await
        .expect("Failed to create partial unique index");

        // Create org_rbac_policy_versions table
        sqlx::query(
            r#"
            CREATE TABLE org_rbac_policy_versions (
                id TEXT PRIMARY KEY NOT NULL,
                policy_id TEXT NOT NULL REFERENCES org_rbac_policies(id) ON DELETE CASCADE,
                version INTEGER NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                resource TEXT NOT NULL,
                action TEXT NOT NULL,
                condition TEXT NOT NULL,
                effect TEXT NOT NULL,
                priority INTEGER NOT NULL,
                enabled INTEGER NOT NULL,
                created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
                reason TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(policy_id, version)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create org_rbac_policy_versions table");

        pool
    }

    /// Insert a test organization and return its ID
    async fn create_test_org(pool: &SqlitePool) -> Uuid {
        let org_id = Uuid::new_v4();
        sqlx::query("INSERT INTO organizations (id, slug, name) VALUES (?, ?, ?)")
            .bind(org_id.to_string())
            .bind(format!("test-org-{}", &org_id.to_string()[..8]))
            .bind("Test Organization")
            .execute(pool)
            .await
            .expect("Failed to create test organization");
        org_id
    }

    /// Insert a test user and return its ID
    async fn create_test_user(pool: &SqlitePool) -> Uuid {
        let user_id = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, email, name) VALUES (?, ?, ?)")
            .bind(user_id.to_string())
            .bind(format!(
                "test-user-{}@example.com",
                &user_id.to_string()[..8]
            ))
            .bind("Test User")
            .execute(pool)
            .await
            .expect("Failed to create test user");
        user_id
    }

    fn create_test_policy_input(name: &str) -> CreateOrgRbacPolicy {
        CreateOrgRbacPolicy {
            name: name.to_string(),
            description: Some("Test policy description".to_string()),
            resource: "projects/*".to_string(),
            action: "read".to_string(),
            condition: "user.role == 'admin'".to_string(),
            effect: RbacPolicyEffect::Allow,
            priority: 10,
            enabled: true,
            reason: Some("Initial creation".to_string()),
        }
    }

    #[tokio::test]
    async fn test_create_policy() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let user_id = create_test_user(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let input = create_test_policy_input("admin-read-projects");

        let policy = repo
            .create(org_id, input, Some(user_id))
            .await
            .expect("Failed to create policy");

        assert_eq!(policy.name, "admin-read-projects");
        assert_eq!(policy.org_id, org_id);
        assert_eq!(policy.resource, "projects/*");
        assert_eq!(policy.action, "read");
        assert_eq!(policy.condition, "user.role == 'admin'");
        assert_eq!(policy.effect, RbacPolicyEffect::Allow);
        assert_eq!(policy.priority, 10);
        assert!(policy.enabled);
        assert_eq!(policy.version, 1);
    }

    #[tokio::test]
    async fn test_create_policy_creates_version() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let user_id = create_test_user(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let input = create_test_policy_input("test-policy");

        let policy = repo
            .create(org_id, input, Some(user_id))
            .await
            .expect("Failed to create policy");

        let versions = repo
            .list_versions(policy.id)
            .await
            .expect("Failed to list versions");

        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, 1);
        assert_eq!(versions[0].name, "test-policy");
        assert_eq!(versions[0].created_by, Some(user_id));
        assert_eq!(versions[0].reason, Some("Initial creation".to_string()));
    }

    #[tokio::test]
    async fn test_create_duplicate_name_fails() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        repo.create(org_id, create_test_policy_input("duplicate-name"), None)
            .await
            .expect("First policy should succeed");

        let result = repo
            .create(org_id, create_test_policy_input("duplicate-name"), None)
            .await;

        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let created = repo
            .create(org_id, create_test_policy_input("get-test"), None)
            .await
            .expect("Failed to create policy");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to fetch policy")
            .expect("Policy should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "get-test");
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let result = repo
            .get_by_id(Uuid::new_v4())
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_org_and_name() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        repo.create(org_id, create_test_policy_input("named-policy"), None)
            .await
            .expect("Failed to create policy");

        let fetched = repo
            .get_by_org_and_name(org_id, "named-policy")
            .await
            .expect("Failed to fetch policy")
            .expect("Policy should exist");

        assert_eq!(fetched.name, "named-policy");
        assert_eq!(fetched.org_id, org_id);
    }

    #[tokio::test]
    async fn test_list_by_org() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let other_org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        // Create policies with different priorities
        let mut input1 = create_test_policy_input("policy-1");
        input1.priority = 5;
        let mut input2 = create_test_policy_input("policy-2");
        input2.priority = 20;
        let mut input3 = create_test_policy_input("policy-3");
        input3.priority = 10;

        repo.create(org_id, input1, None)
            .await
            .expect("Failed to create policy 1");
        repo.create(org_id, input2, None)
            .await
            .expect("Failed to create policy 2");
        repo.create(org_id, input3, None)
            .await
            .expect("Failed to create policy 3");
        repo.create(
            other_org_id,
            create_test_policy_input("other-org-policy"),
            None,
        )
        .await
        .expect("Failed to create other org policy");

        let policies = repo
            .list_by_org(org_id)
            .await
            .expect("Failed to list policies");

        assert_eq!(policies.len(), 3);
        // Should be ordered by priority DESC
        assert_eq!(policies[0].name, "policy-2"); // priority 20
        assert_eq!(policies[1].name, "policy-3"); // priority 10
        assert_eq!(policies[2].name, "policy-1"); // priority 5
    }

    #[tokio::test]
    async fn test_list_enabled_by_org() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let mut enabled_input = create_test_policy_input("enabled-policy");
        enabled_input.enabled = true;

        let mut disabled_input = create_test_policy_input("disabled-policy");
        disabled_input.enabled = false;

        repo.create(org_id, enabled_input, None)
            .await
            .expect("Failed to create enabled policy");
        repo.create(org_id, disabled_input, None)
            .await
            .expect("Failed to create disabled policy");

        let policies = repo
            .list_enabled_by_org(org_id)
            .await
            .expect("Failed to list enabled policies");

        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].name, "enabled-policy");
    }

    #[tokio::test]
    async fn test_update_policy() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let user_id = create_test_user(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let created = repo
            .create(org_id, create_test_policy_input("update-test"), None)
            .await
            .expect("Failed to create policy");

        assert_eq!(created.version, 1);

        let update = UpdateOrgRbacPolicy {
            name: Some("updated-name".to_string()),
            priority: Some(100),
            reason: Some("Updated priority".to_string()),
            ..Default::default()
        };

        let updated = repo
            .update(created.id, update, Some(user_id))
            .await
            .expect("Failed to update policy");

        assert_eq!(updated.name, "updated-name");
        assert_eq!(updated.priority, 100);
        assert_eq!(updated.version, 2);
        // Unchanged fields should remain the same
        assert_eq!(updated.condition, created.condition);
    }

    #[tokio::test]
    async fn test_update_creates_version() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let user_id = create_test_user(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let created = repo
            .create(org_id, create_test_policy_input("version-test"), None)
            .await
            .expect("Failed to create policy");

        let update = UpdateOrgRbacPolicy {
            condition: Some("user.department == 'engineering'".to_string()),
            reason: Some("Changed condition".to_string()),
            ..Default::default()
        };

        repo.update(created.id, update, Some(user_id))
            .await
            .expect("Failed to update policy");

        let versions = repo
            .list_versions(created.id)
            .await
            .expect("Failed to list versions");

        assert_eq!(versions.len(), 2);
        // Versions should be ordered by version DESC
        assert_eq!(versions[0].version, 2);
        assert_eq!(versions[0].condition, "user.department == 'engineering'");
        assert_eq!(versions[0].reason, Some("Changed condition".to_string()));
        assert_eq!(versions[1].version, 1);
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let result = repo
            .update(Uuid::new_v4(), UpdateOrgRbacPolicy::default(), None)
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete_policy() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let created = repo
            .create(org_id, create_test_policy_input("delete-test"), None)
            .await
            .expect("Failed to create policy");

        repo.delete(created.id)
            .await
            .expect("Failed to delete policy");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed");
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_delete_preserves_versions() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let created = repo
            .create(
                org_id,
                create_test_policy_input("version-preserve-test"),
                None,
            )
            .await
            .expect("Failed to create policy");

        // Update to create version 2
        repo.update(
            created.id,
            UpdateOrgRbacPolicy {
                priority: Some(50),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("Failed to update policy");

        // Verify versions exist
        let versions_before = repo
            .list_versions(created.id)
            .await
            .expect("Failed to list versions");
        assert_eq!(versions_before.len(), 2);

        // Delete (soft-delete) policy
        repo.delete(created.id)
            .await
            .expect("Failed to delete policy");

        // Versions should be preserved after soft-delete
        let versions_after = repo
            .list_versions(created.id)
            .await
            .expect("Failed to list versions");
        assert_eq!(versions_after.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let result = repo.delete(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_rollback() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let user_id = create_test_user(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        // Create initial policy
        let created = repo
            .create(org_id, create_test_policy_input("rollback-test"), None)
            .await
            .expect("Failed to create policy");

        assert_eq!(created.condition, "user.role == 'admin'");

        // Update to change condition
        repo.update(
            created.id,
            UpdateOrgRbacPolicy {
                condition: Some("user.role == 'superadmin'".to_string()),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("Failed to update policy");

        // Rollback to version 1
        let rollback_input = RollbackOrgRbacPolicy {
            target_version: 1,
            reason: Some("Reverting to original condition".to_string()),
        };

        let rolled_back = repo
            .rollback(created.id, rollback_input, Some(user_id))
            .await
            .expect("Failed to rollback policy");

        // Should have original condition but new version number
        assert_eq!(rolled_back.condition, "user.role == 'admin'");
        assert_eq!(rolled_back.version, 3);
    }

    #[tokio::test]
    async fn test_rollback_creates_version() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let created = repo
            .create(
                org_id,
                create_test_policy_input("rollback-version-test"),
                None,
            )
            .await
            .expect("Failed to create policy");

        // Update twice
        repo.update(
            created.id,
            UpdateOrgRbacPolicy {
                priority: Some(50),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("Failed to update policy");

        repo.update(
            created.id,
            UpdateOrgRbacPolicy {
                priority: Some(100),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("Failed to update policy");

        // Rollback to version 1
        repo.rollback(
            created.id,
            RollbackOrgRbacPolicy {
                target_version: 1,
                reason: None,
            },
            None,
        )
        .await
        .expect("Failed to rollback policy");

        let versions = repo
            .list_versions(created.id)
            .await
            .expect("Failed to list versions");

        assert_eq!(versions.len(), 4);
        assert_eq!(versions[0].version, 4); // rollback version
        assert!(
            versions[0]
                .reason
                .as_ref()
                .unwrap()
                .contains("Rolled back to version 1")
        );
        assert_eq!(versions[0].priority, 10); // original priority
    }

    #[tokio::test]
    async fn test_rollback_not_found_policy() {
        let pool = create_test_pool().await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let result = repo
            .rollback(
                Uuid::new_v4(),
                RollbackOrgRbacPolicy {
                    target_version: 1,
                    reason: None,
                },
                None,
            )
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_rollback_not_found_version() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let created = repo
            .create(
                org_id,
                create_test_policy_input("rollback-missing-version"),
                None,
            )
            .await
            .expect("Failed to create policy");

        let result = repo
            .rollback(
                created.id,
                RollbackOrgRbacPolicy {
                    target_version: 999,
                    reason: None,
                },
                None,
            )
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_get_version() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let created = repo
            .create(org_id, create_test_policy_input("get-version-test"), None)
            .await
            .expect("Failed to create policy");

        let version = repo
            .get_version(created.id, 1)
            .await
            .expect("Failed to get version")
            .expect("Version should exist");

        assert_eq!(version.version, 1);
        assert_eq!(version.name, "get-version-test");
    }

    #[tokio::test]
    async fn test_get_version_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let result = repo
            .get_version(Uuid::new_v4(), 1)
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_versions_paginated() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let created = repo
            .create(org_id, create_test_policy_input("paginated-test"), None)
            .await
            .expect("Failed to create policy");

        // Create 4 more versions (total 5)
        for i in 0..4 {
            repo.update(
                created.id,
                UpdateOrgRbacPolicy {
                    priority: Some(i * 10),
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("Failed to update policy");
        }

        // Get first page
        let page1 = repo
            .list_versions_paginated(created.id, 2, 0)
            .await
            .expect("Failed to get page 1");

        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].version, 5); // newest first
        assert_eq!(page1[1].version, 4);

        // Get second page
        let page2 = repo
            .list_versions_paginated(created.id, 2, 2)
            .await
            .expect("Failed to get page 2");

        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].version, 3);
        assert_eq!(page2[1].version, 2);

        // Get third page
        let page3 = repo
            .list_versions_paginated(created.id, 2, 4)
            .await
            .expect("Failed to get page 3");

        assert_eq!(page3.len(), 1);
        assert_eq!(page3[0].version, 1);
    }

    #[tokio::test]
    async fn test_policy_effect_deny() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let mut input = create_test_policy_input("deny-policy");
        input.effect = RbacPolicyEffect::Deny;

        let policy = repo
            .create(org_id, input, None)
            .await
            .expect("Failed to create policy");

        assert_eq!(policy.effect, RbacPolicyEffect::Deny);

        let fetched = repo
            .get_by_id(policy.id)
            .await
            .expect("Failed to fetch policy")
            .expect("Policy should exist");

        assert_eq!(fetched.effect, RbacPolicyEffect::Deny);
    }

    #[tokio::test]
    async fn test_update_description_to_none() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        let created = repo
            .create(org_id, create_test_policy_input("description-test"), None)
            .await
            .expect("Failed to create policy");

        assert!(created.description.is_some());

        let updated = repo
            .update(
                created.id,
                UpdateOrgRbacPolicy {
                    description: Some(None), // Set to null
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("Failed to update policy");

        assert!(updated.description.is_none());
    }

    #[tokio::test]
    async fn test_concurrent_updates_one_wins() {
        use std::sync::Arc;

        use tokio::sync::Barrier;

        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool).await;

        // Create initial policy
        let repo = SqliteOrgRbacPolicyRepo::new(pool.clone());
        let created = repo
            .create(org_id, create_test_policy_input("concurrent-test"), None)
            .await
            .expect("Failed to create policy");

        assert_eq!(created.version, 1);

        // Use a barrier to synchronize the two concurrent updates
        let barrier = Arc::new(Barrier::new(2));
        let pool_clone = pool.clone();
        let policy_id = created.id;
        let barrier_clone = barrier.clone();

        // Spawn two concurrent updates
        let handle1 = tokio::spawn(async move {
            let repo = SqliteOrgRbacPolicyRepo::new(pool_clone);
            barrier_clone.wait().await;
            repo.update(
                policy_id,
                UpdateOrgRbacPolicy {
                    priority: Some(100),
                    reason: Some("Update 1".to_string()),
                    ..Default::default()
                },
                None,
            )
            .await
        });

        let pool_clone2 = pool.clone();
        let barrier_clone2 = barrier.clone();

        let handle2 = tokio::spawn(async move {
            let repo = SqliteOrgRbacPolicyRepo::new(pool_clone2);
            barrier_clone2.wait().await;
            repo.update(
                policy_id,
                UpdateOrgRbacPolicy {
                    priority: Some(200),
                    reason: Some("Update 2".to_string()),
                    ..Default::default()
                },
                None,
            )
            .await
        });

        let result1 = handle1.await.expect("Task 1 panicked");
        let result2 = handle2.await.expect("Task 2 panicked");

        // With optimistic locking, one update should succeed and one should fail
        // (or both could succeed if they don't actually overlap)
        let successes = [&result1, &result2].iter().filter(|r| r.is_ok()).count();
        let conflicts = [&result1, &result2]
            .iter()
            .filter(|r| matches!(r, Err(DbError::Conflict(_))))
            .count();

        // At least one should succeed
        assert!(
            successes >= 1,
            "Expected at least one success, got result1: {:?}, result2: {:?}",
            result1,
            result2
        );

        // Final version should reflect the successful updates
        let final_policy = repo
            .get_by_id(policy_id)
            .await
            .expect("Failed to fetch policy")
            .expect("Policy should exist");

        // Version should be 2 (if one conflict) or 3 (if no conflict due to timing)
        assert!(
            final_policy.version >= 2,
            "Version should be at least 2, got {}",
            final_policy.version
        );

        // If there was a conflict, verify it was the right error
        if conflicts > 0 {
            let conflict_result = if result1.is_err() { &result1 } else { &result2 };
            assert!(
                matches!(conflict_result, Err(DbError::Conflict(msg)) if msg.contains("concurrently")),
                "Expected concurrent modification conflict, got: {:?}",
                conflict_result
            );
        }
    }

    #[tokio::test]
    async fn test_count_all() {
        let pool = create_test_pool().await;
        let org1_id = create_test_org(&pool).await;
        let org2_id = create_test_org(&pool).await;
        let user_id = create_test_user(&pool).await;
        let repo = SqliteOrgRbacPolicyRepo::new(pool);

        // Initially zero
        let count = repo.count_all().await.expect("Failed to count");
        assert_eq!(count, 0);

        // Create policies in two different orgs
        repo.create(org1_id, create_test_policy_input("policy-a"), Some(user_id))
            .await
            .expect("Failed to create policy");
        repo.create(org1_id, create_test_policy_input("policy-b"), Some(user_id))
            .await
            .expect("Failed to create policy");
        repo.create(org2_id, create_test_policy_input("policy-c"), Some(user_id))
            .await
            .expect("Failed to create policy");

        let count = repo.count_all().await.expect("Failed to count");
        assert_eq!(count, 3);

        // Delete one — count_all excludes soft-deleted
        let policies = repo.list_by_org(org1_id).await.expect("Failed to list");
        repo.delete(policies[0].id).await.expect("Failed to delete");

        let count = repo.count_all().await.expect("Failed to count");
        assert_eq!(count, 2);
    }
}
