use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{common::parse_uuid, types::WasmRow};
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, OrgRbacPolicyRepo, PageCursors,
            cursor_from_row,
        },
        wasm_sqlite::{WasmSqlitePool, query as wasm_query},
    },
    models::{
        CreateOrgRbacPolicy, OrgRbacPolicy, OrgRbacPolicyVersion, RbacPolicyEffect,
        RollbackOrgRbacPolicy, UpdateOrgRbacPolicy,
    },
};

pub struct WasmSqliteOrgRbacPolicyRepo {
    pool: WasmSqlitePool,
}

impl WasmSqliteOrgRbacPolicyRepo {
    pub fn new(pool: WasmSqlitePool) -> Self {
        Self { pool }
    }

    fn parse_policy(row: &WasmRow) -> DbResult<OrgRbacPolicy> {
        let effect_str: String = row.get("effect");
        let effect: RbacPolicyEffect = effect_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;

        let enabled: i32 = row.get("enabled");

        Ok(OrgRbacPolicy {
            id: parse_uuid(&row.get::<String>("id"))?,
            org_id: parse_uuid(&row.get::<String>("org_id"))?,
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

    fn parse_version(row: &WasmRow) -> DbResult<OrgRbacPolicyVersion> {
        let effect_str: String = row.get("effect");
        let effect: RbacPolicyEffect = effect_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;

        let enabled: i32 = row.get("enabled");
        let created_by: Option<String> = row.get("created_by");

        Ok(OrgRbacPolicyVersion {
            id: parse_uuid(&row.get::<String>("id"))?,
            policy_id: parse_uuid(&row.get::<String>("policy_id"))?,
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

        let rows = wasm_query(&query)
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

        let rows = wasm_query(&query)
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

    /// Create a version record from a policy snapshot.
    ///
    /// In the WASM environment there are no transactions, so this executes
    /// directly against the pool. The single-threaded WASM runtime ensures
    /// no concurrency issues.
    async fn create_version_record(
        &self,
        policy: &OrgRbacPolicy,
        created_by: Option<Uuid>,
        reason: Option<String>,
    ) -> DbResult<()> {
        let version_id = Uuid::new_v4();

        wasm_query(
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
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl OrgRbacPolicyRepo for WasmSqliteOrgRbacPolicyRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgRbacPolicy,
        created_by: Option<Uuid>,
    ) -> DbResult<OrgRbacPolicy> {
        let id = Uuid::new_v4();
        let now: DateTime<Utc> = Utc::now();

        // No transaction support in WASM — execute statements sequentially.
        // Single-threaded WASM environment ensures no concurrency issues.

        // Insert the policy
        wasm_query(
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
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict(format!(
                    "Policy with name '{}' already exists in this organization",
                    input.name
                ))
            } else {
                DbError::from(e)
            }
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
        self.create_version_record(&policy, created_by, input.reason)
            .await?;

        Ok(policy)
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgRbacPolicy>> {
        let row = wasm_query(
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
        let row = wasm_query(
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
        let rows = wasm_query(
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

        let rows = wasm_query(query)
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
        let rows = wasm_query(
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
        let rows = wasm_query(
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
        // No transaction support in WASM — execute statements sequentially.
        // Single-threaded WASM environment ensures no concurrency issues.

        // Fetch current policy (excluding soft-deleted)
        let row = wasm_query(
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
        let result = wasm_query(
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
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict(format!(
                    "Policy with name '{}' already exists in this organization",
                    policy.name
                ))
            } else {
                DbError::from(e)
            }
        })?;

        // Check for concurrent modification (optimistic locking)
        if result.rows_affected() == 0 {
            return Err(DbError::Conflict(
                "Policy was modified concurrently. Please refresh and try again.".to_string(),
            ));
        }

        // Create version record
        self.create_version_record(&policy, updated_by, input.reason)
            .await?;

        Ok(policy)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = Utc::now();

        // Soft-delete by setting deleted_at timestamp
        let result = wasm_query(
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
        // No transaction support in WASM — execute statements sequentially.
        // Single-threaded WASM environment ensures no concurrency issues.

        // Fetch the target version
        let version_row = wasm_query(
            r#"
            SELECT id, policy_id, version, name, description, resource, action,
                   condition, effect, priority, enabled, created_by, reason, created_at
            FROM org_rbac_policy_versions
            WHERE policy_id = ? AND version = ?
            "#,
        )
        .bind(id.to_string())
        .bind(input.target_version)
        .fetch_optional(&self.pool)
        .await?;

        let Some(version_row) = version_row else {
            return Err(DbError::NotFound);
        };

        let target_version = Self::parse_version(&version_row)?;

        // Fetch current policy to get current version number and timestamps (excluding soft-deleted)
        let policy_row = wasm_query(
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
        let result = wasm_query(
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
        .execute(&self.pool)
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
        self.create_version_record(&policy, rolled_back_by, Some(reason))
            .await?;

        Ok(policy)
    }

    async fn get_version(
        &self,
        policy_id: Uuid,
        version: i32,
    ) -> DbResult<Option<OrgRbacPolicyVersion>> {
        let row = wasm_query(
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
        let rows = wasm_query(
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
        let rows = wasm_query(
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
        .bind(limit as i64)
        .bind(offset as i64)
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
        let rows = wasm_query(
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
        let row = wasm_query(
            "SELECT COUNT(*) as count FROM org_rbac_policy_versions WHERE policy_id = ?",
        )
        .bind(policy_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64>("count"))
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = wasm_query(
            "SELECT COUNT(*) as count FROM org_rbac_policies WHERE org_id = ? AND deleted_at IS NULL",
        )
        .bind(org_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64>("count"))
    }

    async fn count_all(&self) -> DbResult<i64> {
        let row =
            wasm_query("SELECT COUNT(*) as count FROM org_rbac_policies WHERE deleted_at IS NULL")
                .fetch_one(&self.pool)
                .await?;

        Ok(row.get::<i64>("count"))
    }
}
