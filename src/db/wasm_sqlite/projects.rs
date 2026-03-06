use async_trait::async_trait;
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, ProjectRepo,
            cursor_from_row,
        },
        wasm_sqlite::{WasmSqlitePool, query as wasm_query},
    },
    models::{CreateProject, Project, UpdateProject},
};

pub struct WasmSqliteProjectRepo {
    pool: WasmSqlitePool,
}

impl WasmSqliteProjectRepo {
    pub fn new(pool: WasmSqlitePool) -> Self {
        Self { pool }
    }

    /// Helper method for cursor-based pagination.
    ///
    /// Uses keyset pagination with (created_at, id) tuple for efficient, consistent results.
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
        let mut items: Vec<Project> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let team_id: Option<String> = row.get("team_id");
                Ok(Project {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
                    team_id: team_id.as_deref().map(parse_uuid).transpose()?,
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
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |proj| {
                cursor_from_row(proj.created_at, proj.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ProjectRepo for WasmSqliteProjectRepo {
    async fn create(&self, org_id: Uuid, input: CreateProject) -> DbResult<Project> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        wasm_query(
            r#"
            INSERT INTO projects (id, org_id, team_id, slug, name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(input.team_id.map(|id| id.to_string()))
        .bind(&input.slug)
        .bind(&input.name)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict(format!(
                    "Project with slug '{}' already exists in this organization",
                    input.slug
                ))
            } else {
                DbError::from(e)
            }
        })?;

        Ok(Project {
            id,
            org_id,
            team_id: input.team_id,
            slug: input.slug,
            name: input.name,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Project>> {
        let result = wasm_query(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let team_id: Option<String> = row.get("team_id");
                Ok(Some(Project {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
                    team_id: team_id.as_deref().map(parse_uuid).transpose()?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Project>> {
        let result = wasm_query(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE id = ? AND org_id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let team_id: Option<String> = row.get("team_id");
                Ok(Some(Project {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
                    team_id: team_id.as_deref().map(parse_uuid).transpose()?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<Project>> {
        let result = wasm_query(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE org_id = ? AND slug = ? AND deleted_at IS NULL
            "#,
        )
        .bind(org_id.to_string())
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let team_id: Option<String> = row.get("team_id");
                Ok(Some(Project {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
                    team_id: team_id.as_deref().map(parse_uuid).transpose()?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
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
            WHERE org_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        } else {
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
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
        let items: Vec<Project> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let team_id: Option<String> = row.get("team_id");
                Ok(Project {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
                    team_id: team_id.as_deref().map(parse_uuid).transpose()?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        // Generate cursors for pagination
        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |proj| {
                cursor_from_row(proj.created_at, proj.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_org(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) as count FROM projects WHERE org_id = ?"
        } else {
            "SELECT COUNT(*) as count FROM projects WHERE org_id = ? AND deleted_at IS NULL"
        };

        let row = wasm_query(query)
            .bind(org_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateProject) -> DbResult<Project> {
        let has_name_update = input.name.is_some();
        let has_team_update = input.team_id.is_some();

        if !has_name_update && !has_team_update {
            return self.get_by_id(id).await?.ok_or(DbError::NotFound);
        }

        let now = chrono::Utc::now();

        // Build dynamic update query
        let mut set_clauses = vec!["updated_at = ?"];
        if has_name_update {
            set_clauses.push("name = ?");
        }
        if has_team_update {
            set_clauses.push("team_id = ?");
        }

        let query = format!(
            "UPDATE projects SET {} WHERE id = ? AND deleted_at IS NULL",
            set_clauses.join(", ")
        );

        let mut query_builder = wasm_query(&query).bind(now);

        if let Some(ref name) = input.name {
            query_builder = query_builder.bind(name);
        }
        if let Some(ref team_id_opt) = input.team_id {
            query_builder = query_builder.bind(team_id_opt.map(|id| id.to_string()));
        }

        let result = query_builder
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = wasm_query(
            r#"
            UPDATE projects
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
