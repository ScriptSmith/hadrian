use async_trait::async_trait;
use chrono::SubsecRound;
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, ServiceAccountRepo,
            cursor_from_row,
        },
        wasm_sqlite::{WasmSqlitePool, query as wasm_query},
    },
    models::{CreateServiceAccount, ServiceAccount, UpdateServiceAccount},
};

pub struct WasmSqliteServiceAccountRepo {
    pool: WasmSqlitePool,
}

impl WasmSqliteServiceAccountRepo {
    pub fn new(pool: WasmSqlitePool) -> Self {
        Self { pool }
    }

    /// Parse roles from JSON string to Vec<String>
    fn parse_roles(roles_json: &str) -> Vec<String> {
        serde_json::from_str(roles_json).unwrap_or_default()
    }

    /// Serialize roles to JSON string
    fn serialize_roles(roles: &[String]) -> String {
        serde_json::to_string(roles).unwrap_or_else(|_| "[]".to_string())
    }

    /// Helper method for cursor-based pagination.
    async fn list_with_cursor(
        &self,
        org_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<ServiceAccount>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE org_id = ? AND deleted_at IS NULL AND (created_at, id) {} (?, ?)
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = wasm_query(&query)
            .bind(org_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<ServiceAccount> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(ServiceAccount {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    description: row.get("description"),
                    roles: Self::parse_roles(&row.get::<String>("roles")),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |sa| {
                cursor_from_row(sa.created_at, sa.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ServiceAccountRepo for WasmSqliteServiceAccountRepo {
    async fn create(&self, org_id: Uuid, input: CreateServiceAccount) -> DbResult<ServiceAccount> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().trunc_subsecs(3);
        let roles_json = Self::serialize_roles(&input.roles);

        wasm_query(
            r#"
            INSERT INTO service_accounts (id, org_id, slug, name, description, roles, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(&input.slug)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&roles_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict(format!(
                    "Service account with slug '{}' already exists in this organization",
                    input.slug
                ))
            } else {
                DbError::from(e)
            }
        })?;

        Ok(ServiceAccount {
            id,
            org_id,
            slug: input.slug,
            name: input.name,
            description: input.description,
            roles: input.roles,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ServiceAccount>> {
        let result = wasm_query(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(ServiceAccount {
                id: parse_uuid(&row.get::<String>("id"))?,
                org_id: parse_uuid(&row.get::<String>("org_id"))?,
                slug: row.get("slug"),
                name: row.get("name"),
                description: row.get("description"),
                roles: Self::parse_roles(&row.get::<String>("roles")),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<ServiceAccount>> {
        let result = wasm_query(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE org_id = ? AND slug = ? AND deleted_at IS NULL
            "#,
        )
        .bind(org_id.to_string())
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(ServiceAccount {
                id: parse_uuid(&row.get::<String>("id"))?,
                org_id: parse_uuid(&row.get::<String>("org_id"))?,
                slug: row.get("slug"),
                name: row.get("name"),
                description: row.get("description"),
                roles: Self::parse_roles(&row.get::<String>("roles")),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ServiceAccount>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(org_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        let rows = wasm_query(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE org_id = ? AND deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(org_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<ServiceAccount> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(ServiceAccount {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    description: row.get("description"),
                    roles: Self::parse_roles(&row.get::<String>("roles")),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |sa| {
                cursor_from_row(sa.created_at, sa.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = wasm_query(
            "SELECT COUNT(*) as count FROM service_accounts WHERE org_id = ? AND deleted_at IS NULL",
        )
        .bind(org_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get::<i64>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateServiceAccount) -> DbResult<ServiceAccount> {
        // First check if the service account exists
        let existing = self.get_by_id(id).await?.ok_or(DbError::NotFound)?;

        let now = chrono::Utc::now().trunc_subsecs(3);
        let new_name = input.name.unwrap_or(existing.name);
        let new_description = input.description.or(existing.description);
        let new_roles = input.roles.unwrap_or(existing.roles);
        let roles_json = Self::serialize_roles(&new_roles);

        let result = wasm_query(
            r#"
            UPDATE service_accounts
            SET name = ?, description = ?, roles = ?, updated_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&new_name)
        .bind(&new_description)
        .bind(&roles_json)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        // Record was deleted between get_by_id and UPDATE
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(ServiceAccount {
            id,
            org_id: existing.org_id,
            slug: existing.slug,
            name: new_name,
            description: new_description,
            roles: new_roles,
            created_at: existing.created_at,
            updated_at: now,
        })
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now().trunc_subsecs(3);

        let result = wasm_query(
            r#"
            UPDATE service_accounts
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

    async fn delete_with_api_key_revocation(&self, id: Uuid) -> DbResult<Vec<Uuid>> {
        let now = chrono::Utc::now().trunc_subsecs(3);

        // No transaction support in WASM — execute statements sequentially.
        // Single-threaded WASM environment ensures no concurrency issues.

        // 1. Check if the service account exists
        let exists = wasm_query(
            r#"
            SELECT id FROM service_accounts
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        if exists.is_none() {
            return Err(DbError::NotFound);
        }

        // 2. Get API key IDs that will be revoked
        let revoked_rows = wasm_query(
            r#"
            SELECT id FROM api_keys
            WHERE owner_type = 'service_account' AND owner_id = ? AND revoked_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let revoked_ids: Vec<String> = revoked_rows
            .iter()
            .map(|row| row.get::<String>("id"))
            .collect();

        // 3. Revoke the API keys
        wasm_query(
            r#"
            UPDATE api_keys
            SET revoked_at = ?, updated_at = ?
            WHERE owner_type = 'service_account' AND owner_id = ? AND revoked_at IS NULL
            "#,
        )
        .bind(now)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        // 4. Soft-delete the service account
        wasm_query(
            r#"
            UPDATE service_accounts
            SET deleted_at = ?
            WHERE id = ?
            "#,
        )
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        // Convert string IDs to UUIDs
        let revoked_uuids: Vec<Uuid> = revoked_ids
            .into_iter()
            .filter_map(|s| parse_uuid(&s).ok())
            .collect();

        Ok(revoked_uuids)
    }
}
