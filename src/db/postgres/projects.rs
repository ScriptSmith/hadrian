use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, ProjectRepo,
            cursor_from_row,
        },
    },
    models::{CreateProject, Project, UpdateProject},
};

pub struct PostgresProjectRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresProjectRepo {
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
        org_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<Project>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };

        let query = format!(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
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
        let mut items: Vec<Project> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Project {
                id: row.get("id"),
                org_id: row.get("org_id"),
                team_id: row.get("team_id"),
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
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |proj| {
                cursor_from_row(proj.created_at, proj.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[async_trait]
impl ProjectRepo for PostgresProjectRepo {
    async fn create(&self, org_id: Uuid, input: CreateProject) -> DbResult<Project> {
        let row = sqlx::query(
            r#"
            INSERT INTO projects (id, org_id, team_id, slug, name)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, org_id, team_id, slug, name, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(input.team_id)
        .bind(&input.slug)
        .bind(&input.name)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Project with slug '{}' already exists in this organization",
                    input.slug
                ))
            }
            _ => DbError::from(e),
        })?;

        Ok(Project {
            id: row.get("id"),
            org_id: row.get("org_id"),
            team_id: row.get("team_id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Project>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Project {
            id: row.get("id"),
            org_id: row.get("org_id"),
            team_id: row.get("team_id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Project>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE id = $1 AND org_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .bind(org_id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Project {
            id: row.get("id"),
            org_id: row.get("org_id"),
            team_id: row.get("team_id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<Project>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE org_id = $1 AND slug = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(org_id)
        .bind(slug)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Project {
            id: row.get("id"),
            org_id: row.get("org_id"),
            team_id: row.get("team_id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn list_by_org(&self, org_id: Uuid, params: ListParams) -> DbResult<ListResult<Project>> {
        let limit = params.limit.unwrap_or(100);
        // Fetch one extra to determine if there are more items
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(org_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let query = if params.include_deleted {
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE org_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#
        } else {
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
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
        let items: Vec<Project> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Project {
                id: row.get("id"),
                org_id: row.get("org_id"),
                team_id: row.get("team_id"),
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        // Generate cursors for pagination
        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |proj| {
                cursor_from_row(proj.created_at, proj.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_org(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) as count FROM projects WHERE org_id = $1"
        } else {
            "SELECT COUNT(*) as count FROM projects WHERE org_id = $1 AND deleted_at IS NULL"
        };

        let row = sqlx::query(query)
            .bind(org_id)
            .fetch_one(&self.read_pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateProject) -> DbResult<Project> {
        let has_name_update = input.name.is_some();
        let has_team_update = input.team_id.is_some();

        if !has_name_update && !has_team_update {
            return self.get_by_id(id).await?.ok_or(DbError::NotFound);
        }

        // Build dynamic update query with parameterized SET clauses
        let mut set_clauses: Vec<String> = vec!["updated_at = NOW()".to_string()];
        let mut param_idx = 1;
        if has_name_update {
            set_clauses.push(format!("name = ${}", param_idx));
            param_idx += 1;
        }
        if has_team_update {
            set_clauses.push(format!("team_id = ${}", param_idx));
            param_idx += 1;
        }

        let query = format!(
            r#"
            UPDATE projects
            SET {}
            WHERE id = ${} AND deleted_at IS NULL
            RETURNING id, org_id, team_id, slug, name, created_at, updated_at
            "#,
            set_clauses.join(", "),
            param_idx
        );

        let mut query_builder = sqlx::query(&query);

        if let Some(ref name) = input.name {
            query_builder = query_builder.bind(name);
        }
        if let Some(ref team_id_opt) = input.team_id {
            query_builder = query_builder.bind(*team_id_opt);
        }

        let row = query_builder
            .bind(id)
            .fetch_optional(&self.write_pool)
            .await?
            .ok_or(DbError::NotFound)?;

        Ok(Project {
            id: row.get("id"),
            org_id: row.get("org_id"),
            team_id: row.get("team_id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE projects
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
