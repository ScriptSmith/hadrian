use async_trait::async_trait;
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, OrganizationRepo, PageCursors,
            cursor_from_row,
        },
        wasm_sqlite::{WasmSqlitePool, query as wasm_query},
    },
    models::{CreateOrganization, Organization, UpdateOrganization},
};

pub struct WasmSqliteOrganizationRepo {
    pool: WasmSqlitePool,
}

impl WasmSqliteOrganizationRepo {
    pub fn new(pool: WasmSqlitePool) -> Self {
        Self { pool }
    }

    /// Helper method for cursor-based pagination.
    ///
    /// Uses keyset pagination with (created_at, id) tuple for efficient, consistent results.
    async fn list_with_cursor(
        &self,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<Organization>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };

        let query = format!(
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE (created_at, id) {} (?, ?)
            {}
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, deleted_filter, order, order
        );

        let rows = wasm_query(&query)
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<Organization> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(Organization {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        // Generate cursors
        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |org| {
                cursor_from_row(org.created_at, org.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl OrganizationRepo for WasmSqliteOrganizationRepo {
    async fn create(&self, input: CreateOrganization) -> DbResult<Organization> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        wasm_query(
            r#"
            INSERT INTO organizations (id, slug, name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&input.slug)
        .bind(&input.name)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict(format!(
                    "Organization with slug '{}' already exists",
                    input.slug
                ))
            } else {
                DbError::from(e)
            }
        })?;

        Ok(Organization {
            id,
            slug: input.slug,
            name: input.name,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Organization>> {
        let result = wasm_query(
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Organization {
                id: parse_uuid(&row.get::<String>("id"))?,
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn get_by_slug(&self, slug: &str) -> DbResult<Option<Organization>> {
        let result = wasm_query(
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE slug = ? AND deleted_at IS NULL
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Organization {
                id: parse_uuid(&row.get::<String>("id"))?,
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn list(&self, params: ListParams) -> DbResult<ListResult<Organization>> {
        let limit = params.limit.unwrap_or(100);
        // Fetch one extra to determine if there are more items
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(&params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let query = if params.include_deleted {
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        } else {
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        };

        let rows = wasm_query(query)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<Organization> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(Organization {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        // Generate cursors for pagination
        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |org| {
                cursor_from_row(org.created_at, org.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count(&self, include_deleted: bool) -> DbResult<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) as count FROM organizations"
        } else {
            "SELECT COUNT(*) as count FROM organizations WHERE deleted_at IS NULL"
        };

        let row = wasm_query(query).fetch_one(&self.pool).await?;
        Ok(row.get::<i64>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateOrganization) -> DbResult<Organization> {
        if let Some(name) = input.name {
            let now = chrono::Utc::now();

            let result = wasm_query(
                r#"
                UPDATE organizations
                SET name = ?, updated_at = ?
                WHERE id = ? AND deleted_at IS NULL
                "#,
            )
            .bind(&name)
            .bind(now)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

            if result.rows_affected() == 0 {
                return Err(DbError::NotFound);
            }

            // Return the updated org
            self.get_by_id(id).await?.ok_or(DbError::NotFound)
        } else {
            self.get_by_id(id).await?.ok_or(DbError::NotFound)
        }
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = wasm_query(
            r#"
            UPDATE organizations
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
}
