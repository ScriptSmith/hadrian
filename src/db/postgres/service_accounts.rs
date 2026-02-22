use async_trait::async_trait;
use chrono::SubsecRound;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, ServiceAccountRepo,
            cursor_from_row,
        },
    },
    models::{CreateServiceAccount, ServiceAccount, UpdateServiceAccount},
};

pub struct PostgresServiceAccountRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresServiceAccountRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
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
            WHERE org_id = $1 AND deleted_at IS NULL AND ROW(created_at, id) {} ROW($2, $3)
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
        let mut items: Vec<ServiceAccount> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let roles: serde_json::Value = row.get("roles");
                ServiceAccount {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    slug: row.get("slug"),
                    name: row.get("name"),
                    description: row.get("description"),
                    roles: serde_json::from_value(roles).unwrap_or_default(),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();

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

#[async_trait]
impl ServiceAccountRepo for PostgresServiceAccountRepo {
    async fn create(&self, org_id: Uuid, input: CreateServiceAccount) -> DbResult<ServiceAccount> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().trunc_subsecs(3);
        let roles_json = serde_json::to_value(&input.roles).unwrap_or(serde_json::json!([]));

        sqlx::query(
            r#"
            INSERT INTO service_accounts (id, org_id, slug, name, description, roles, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(org_id)
        .bind(&input.slug)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&roles_json)
        .bind(now)
        .bind(now)
        .execute(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Service account with slug '{}' already exists in this organization",
                    input.slug
                ))
            }
            _ => DbError::from(e),
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
        let result = sqlx::query(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| {
            let roles: serde_json::Value = row.get("roles");
            ServiceAccount {
                id: row.get("id"),
                org_id: row.get("org_id"),
                slug: row.get("slug"),
                name: row.get("name"),
                description: row.get("description"),
                roles: serde_json::from_value(roles).unwrap_or_default(),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }
        }))
    }

    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<ServiceAccount>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE org_id = $1 AND slug = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(org_id)
        .bind(slug)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| {
            let roles: serde_json::Value = row.get("roles");
            ServiceAccount {
                id: row.get("id"),
                org_id: row.get("org_id"),
                slug: row.get("slug"),
                name: row.get("name"),
                description: row.get("description"),
                roles: serde_json::from_value(roles).unwrap_or_default(),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }
        }))
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

        let rows = sqlx::query(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE org_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(org_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<ServiceAccount> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let roles: serde_json::Value = row.get("roles");
                ServiceAccount {
                    id: row.get("id"),
                    org_id: row.get("org_id"),
                    slug: row.get("slug"),
                    name: row.get("name"),
                    description: row.get("description"),
                    roles: serde_json::from_value(roles).unwrap_or_default(),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }
            })
            .collect();

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |sa| {
                cursor_from_row(sa.created_at, sa.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM service_accounts WHERE org_id = $1 AND deleted_at IS NULL",
        )
        .bind(org_id)
        .fetch_one(&self.read_pool)
        .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateServiceAccount) -> DbResult<ServiceAccount> {
        // First check if the service account exists
        let existing = self.get_by_id(id).await?.ok_or(DbError::NotFound)?;

        let now = chrono::Utc::now().trunc_subsecs(3);
        let new_name = input.name.unwrap_or(existing.name);
        let new_description = input.description.or(existing.description);
        let new_roles = input.roles.unwrap_or(existing.roles);
        let roles_json = serde_json::to_value(&new_roles).unwrap_or(serde_json::json!([]));

        let result = sqlx::query(
            r#"
            UPDATE service_accounts
            SET name = $1, description = $2, roles = $3, updated_at = $4
            WHERE id = $5 AND deleted_at IS NULL
            "#,
        )
        .bind(&new_name)
        .bind(&new_description)
        .bind(&roles_json)
        .bind(now)
        .bind(id)
        .execute(&self.write_pool)
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

        let result = sqlx::query(
            r#"
            UPDATE service_accounts
            SET deleted_at = $1
            WHERE id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(now)
        .bind(id)
        .execute(&self.write_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn delete_with_api_key_revocation(&self, id: Uuid) -> DbResult<Vec<Uuid>> {
        let mut tx = self.write_pool.begin().await?;
        let now = chrono::Utc::now().trunc_subsecs(3);

        // 1. Lock the service account row to prevent race conditions where new API keys
        // could be created between deleting the SA and revoking its keys
        let exists = sqlx::query(
            r#"
            SELECT id FROM service_accounts
            WHERE id = $1 AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;

        if exists.is_none() {
            return Err(DbError::NotFound);
        }

        // 2. Soft-delete the service account
        sqlx::query(
            r#"
            UPDATE service_accounts
            SET deleted_at = $1
            WHERE id = $2
            "#,
        )
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        // 3. Revoke all API keys owned by this service account and return their IDs
        let revoked_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            UPDATE api_keys
            SET revoked_at = $1
            WHERE owner_type = 'service_account' AND owner_id = $2 AND revoked_at IS NULL
            RETURNING id
            "#,
        )
        .bind(now)
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(revoked_ids)
    }
}
