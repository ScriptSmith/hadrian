use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

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

pub struct PostgresOrgRbacPolicyRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresOrgRbacPolicyRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    fn parse_policy(row: &sqlx::postgres::PgRow) -> DbResult<OrgRbacPolicy> {
        let effect_str: String = row.get("effect");
        let effect: RbacPolicyEffect = effect_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;

        Ok(OrgRbacPolicy {
            id: row.get("id"),
            org_id: row.get("org_id"),
            name: row.get("name"),
            description: row.get("description"),
            resource: row.get("resource"),
            action: row.get("action"),
            condition: row.get("condition"),
            effect,
            priority: row.get("priority"),
            enabled: row.get("enabled"),
            version: row.get("version"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            deleted_at: row.get("deleted_at"),
        })
    }

    fn parse_version(row: &sqlx::postgres::PgRow) -> DbResult<OrgRbacPolicyVersion> {
        let effect_str: String = row.get("effect");
        let effect: RbacPolicyEffect = effect_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;

        Ok(OrgRbacPolicyVersion {
            id: row.get("id"),
            policy_id: row.get("policy_id"),
            version: row.get("version"),
            name: row.get("name"),
            description: row.get("description"),
            resource: row.get("resource"),
            action: row.get("action"),
            condition: row.get("condition"),
            effect,
            priority: row.get("priority"),
            enabled: row.get("enabled"),
            created_by: row.get("created_by"),
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
                   effect::text, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = $1 AND ROW(created_at, id) {} ROW($2, $3)
            {}
            ORDER BY created_at {}, id {}
            LIMIT $4
            "#,
            comparison, deleted_filter, order, order
        );

        let rows = sqlx::query(&query)
            .bind(org_id)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
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
                   condition, effect::text, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = $1 AND ROW(created_at, id) {} ROW($2, $3)
            ORDER BY created_at {}, id {}
            LIMIT $4
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(policy_id)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
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
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        policy: &OrgRbacPolicy,
        created_by: Option<Uuid>,
        reason: Option<String>,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO org_rbac_policy_versions (
                id, policy_id, version, name, description, resource, action,
                condition, effect, priority, enabled, created_by, reason, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::rbac_policy_effect, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(policy.id)
        .bind(policy.version)
        .bind(&policy.name)
        .bind(&policy.description)
        .bind(&policy.resource)
        .bind(&policy.action)
        .bind(&policy.condition)
        .bind(policy.effect.to_string())
        .bind(policy.priority)
        .bind(policy.enabled)
        .bind(created_by)
        .bind(reason)
        .bind(policy.updated_at)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }
}

#[async_trait]
impl OrgRbacPolicyRepo for PostgresOrgRbacPolicyRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgRbacPolicy,
        created_by: Option<Uuid>,
    ) -> DbResult<OrgRbacPolicy> {
        let id = Uuid::new_v4();
        let now: DateTime<Utc> = Utc::now();

        let mut tx = self.write_pool.begin().await?;

        // Insert the policy
        sqlx::query(
            r#"
            INSERT INTO org_rbac_policies (
                id, org_id, name, description, resource, action, condition,
                effect, priority, enabled, version, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8::rbac_policy_effect, $9, $10, 1, $11, $12)
            "#,
        )
        .bind(id)
        .bind(org_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.resource)
        .bind(&input.action)
        .bind(&input.condition)
        .bind(input.effect.to_string())
        .bind(input.priority)
        .bind(input.enabled)
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
                   effect::text, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
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
                   effect::text, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = $1 AND name = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(org_id)
        .bind(name)
        .fetch_optional(&self.read_pool)
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
                   effect::text, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = $1 AND deleted_at IS NULL
            ORDER BY priority DESC, name ASC
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.read_pool)
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
                   effect::text, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#
        } else {
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect::text, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#
        };

        let rows = sqlx::query(query)
            .bind(org_id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
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
                   effect::text, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE org_id = $1 AND enabled = TRUE AND deleted_at IS NULL
            ORDER BY priority DESC, name ASC
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.read_pool)
        .await?;

        rows.iter().map(Self::parse_policy).collect()
    }

    async fn list_all_enabled(&self) -> DbResult<Vec<OrgRbacPolicy>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect::text, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE enabled = TRUE AND deleted_at IS NULL
            ORDER BY org_id, priority DESC, name ASC
            "#,
        )
        .fetch_all(&self.read_pool)
        .await?;

        rows.iter().map(Self::parse_policy).collect()
    }

    async fn update(
        &self,
        id: Uuid,
        input: UpdateOrgRbacPolicy,
        updated_by: Option<Uuid>,
    ) -> DbResult<OrgRbacPolicy> {
        let mut tx = self.write_pool.begin().await?;

        // Fetch current policy (excluding soft-deleted)
        let row = sqlx::query(
            r#"
            SELECT id, org_id, name, description, resource, action, condition,
                   effect::text, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
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
            SET name = $1, description = $2, resource = $3, action = $4, condition = $5,
                effect = $6::rbac_policy_effect, priority = $7, enabled = $8, version = $9, updated_at = $10
            WHERE id = $11 AND version = $12 AND deleted_at IS NULL
            "#,
        )
        .bind(&policy.name)
        .bind(&policy.description)
        .bind(&policy.resource)
        .bind(&policy.action)
        .bind(&policy.condition)
        .bind(policy.effect.to_string())
        .bind(policy.priority)
        .bind(policy.enabled)
        .bind(policy.version)
        .bind(policy.updated_at)
        .bind(id)
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
        // Soft-delete by setting deleted_at timestamp
        let result = sqlx::query(
            r#"
            UPDATE org_rbac_policies
            SET deleted_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .execute(&self.write_pool)
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
        let mut tx = self.write_pool.begin().await?;

        // Fetch the target version
        let version_row = sqlx::query(
            r#"
            SELECT id, policy_id, version, name, description, resource, action,
                   condition, effect::text, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = $1 AND version = $2
            "#,
        )
        .bind(id)
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
                   effect::text, priority, enabled, version, created_at, updated_at, deleted_at
            FROM org_rbac_policies
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
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
            SET name = $1, description = $2, resource = $3, action = $4, condition = $5,
                effect = $6::rbac_policy_effect, priority = $7, enabled = $8, version = $9, updated_at = $10
            WHERE id = $11 AND version = $12 AND deleted_at IS NULL
            "#,
        )
        .bind(&policy.name)
        .bind(&policy.description)
        .bind(&policy.resource)
        .bind(&policy.action)
        .bind(&policy.condition)
        .bind(policy.effect.to_string())
        .bind(policy.priority)
        .bind(policy.enabled)
        .bind(policy.version)
        .bind(policy.updated_at)
        .bind(id)
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
                   condition, effect::text, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = $1 AND version = $2
            "#,
        )
        .bind(policy_id)
        .bind(version)
        .fetch_optional(&self.read_pool)
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
                   condition, effect::text, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = $1
            ORDER BY version DESC
            "#,
        )
        .bind(policy_id)
        .fetch_all(&self.read_pool)
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
                   condition, effect::text, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = $1
            ORDER BY version DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(policy_id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.read_pool)
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
                   condition, effect::text, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(policy_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
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
            "SELECT COUNT(*) as count FROM org_rbac_policy_versions WHERE policy_id = $1",
        )
        .bind(policy_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM org_rbac_policies WHERE org_id = $1 AND deleted_at IS NULL",
        )
        .bind(org_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn count_all(&self) -> DbResult<i64> {
        let row =
            sqlx::query("SELECT COUNT(*) as count FROM org_rbac_policies WHERE deleted_at IS NULL")
                .fetch_one(&self.read_pool)
                .await?;

        Ok(row.get::<i64, _>("count"))
    }
}
