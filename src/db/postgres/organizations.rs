use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, OrganizationRepo, PageCursors,
            cursor_from_row,
        },
    },
    models::{CreateOrganization, Organization, UpdateOrganization},
};

pub struct PostgresOrganizationRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresOrganizationRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    /// Helper method for cursor-based pagination.
    ///
    /// Uses keyset pagination with ROW(created_at, id) tuple for efficient, consistent results.
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
            WHERE ROW(created_at, id) {} ROW($1, $2)
            {}
            ORDER BY created_at {}, id {}
            LIMIT $3
            "#,
            comparison, deleted_filter, order, order
        );

        let rows = sqlx::query(&query)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<Organization> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Organization {
                id: row.get("id"),
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

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

#[async_trait]
impl OrganizationRepo for PostgresOrganizationRepo {
    async fn create(&self, input: CreateOrganization) -> DbResult<Organization> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            INSERT INTO organizations (id, slug, name)
            VALUES ($1, $2, $3)
            RETURNING id, slug, name, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&input.slug)
        .bind(&input.name)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => DbError::Conflict(
                format!("Organization with slug '{}' already exists", input.slug),
            ),
            _ => DbError::from(e),
        })?;

        Ok(Organization {
            id: row.get("id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Organization>> {
        let result = sqlx::query(
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Organization {
            id: row.get("id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn get_by_slug(&self, slug: &str) -> DbResult<Option<Organization>> {
        let result = sqlx::query(
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE slug = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Organization {
            id: row.get("id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
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
            LIMIT $1
            "#
        } else {
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            LIMIT $1
            "#
        };

        let rows = sqlx::query(query)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<Organization> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Organization {
                id: row.get("id"),
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

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

        let row = sqlx::query(query).fetch_one(&self.read_pool).await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateOrganization) -> DbResult<Organization> {
        if let Some(name) = input.name {
            let row = sqlx::query(
                r#"
                UPDATE organizations
                SET name = $1, updated_at = NOW()
                WHERE id = $2 AND deleted_at IS NULL
                RETURNING id, slug, name, created_at, updated_at
                "#,
            )
            .bind(&name)
            .bind(id)
            .fetch_optional(&self.write_pool)
            .await?
            .ok_or(DbError::NotFound)?;

            Ok(Organization {
                id: row.get("id"),
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
        } else {
            self.get_by_id(id).await?.ok_or(DbError::NotFound)
        }
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE organizations
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
}
