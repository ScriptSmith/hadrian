use async_trait::async_trait;
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, TeamRepo, cursor_from_row,
        },
        wasm_sqlite::{WasmSqlitePool, query as wasm_query},
    },
    models::{
        AddTeamMember, CreateTeam, MembershipSource, Team, TeamMember, UpdateTeam, UpdateTeamMember,
    },
};

pub struct WasmSqliteTeamRepo {
    pool: WasmSqlitePool,
}

impl WasmSqliteTeamRepo {
    pub fn new(pool: WasmSqlitePool) -> Self {
        Self { pool }
    }

    /// Helper method for cursor-based pagination of teams.
    async fn list_with_cursor(
        &self,
        org_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<Team>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };

        let query = format!(
            r#"
            SELECT id, org_id, slug, name, created_at, updated_at
            FROM teams
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
        let mut items: Vec<Team> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(Team {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
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

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |team| {
                cursor_from_row(team.created_at, team.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of team members.
    async fn list_members_with_cursor(
        &self,
        team_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<TeamMember>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT u.id as user_id, u.external_id, u.email, u.name,
                   tm.role, tm.created_at as joined_at
            FROM users u
            INNER JOIN team_memberships tm ON u.id = tm.user_id
            WHERE tm.team_id = ?
            AND (tm.created_at, u.id) {} (?, ?)
            ORDER BY tm.created_at {}, u.id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = wasm_query(&query)
            .bind(team_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<TeamMember> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(TeamMember {
                    user_id: parse_uuid(&row.get::<String>("user_id"))?,
                    external_id: row.get("external_id"),
                    email: row.get("email"),
                    name: row.get("name"),
                    role: row.get("role"),
                    joined_at: row.get("joined_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |member| {
                cursor_from_row(member.joined_at, member.user_id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl TeamRepo for WasmSqliteTeamRepo {
    async fn create(&self, org_id: Uuid, input: CreateTeam) -> DbResult<Team> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        wasm_query(
            r#"
            INSERT INTO teams (id, org_id, slug, name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(&input.slug)
        .bind(&input.name)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict(format!(
                    "Team with slug '{}' already exists in this organization",
                    input.slug
                ))
            } else {
                DbError::from(e)
            }
        })?;

        Ok(Team {
            id,
            org_id,
            slug: input.slug,
            name: input.name,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Team>> {
        let result = wasm_query(
            r#"
            SELECT id, org_id, slug, name, created_at, updated_at
            FROM teams
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Team {
                id: parse_uuid(&row.get::<String>("id"))?,
                org_id: parse_uuid(&row.get::<String>("org_id"))?,
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn get_by_ids(&self, ids: &[Uuid]) -> DbResult<Vec<Team>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // Build query with placeholders for each ID
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            r#"
            SELECT id, org_id, slug, name, created_at, updated_at
            FROM teams
            WHERE id IN ({}) AND deleted_at IS NULL
            "#,
            placeholders
        );

        let mut query_builder = wasm_query(&query);
        for id in ids {
            query_builder = query_builder.bind(id.to_string());
        }

        let rows = query_builder.fetch_all(&self.pool).await?;

        rows.into_iter()
            .map(|row| {
                Ok(Team {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect()
    }

    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<Team>> {
        let result = wasm_query(
            r#"
            SELECT id, org_id, slug, name, created_at, updated_at
            FROM teams
            WHERE org_id = ? AND slug = ? AND deleted_at IS NULL
            "#,
        )
        .bind(org_id.to_string())
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Team {
                id: parse_uuid(&row.get::<String>("id"))?,
                org_id: parse_uuid(&row.get::<String>("org_id"))?,
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn list_by_org(&self, org_id: Uuid, params: ListParams) -> DbResult<ListResult<Team>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(org_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        let query = if params.include_deleted {
            r#"
            SELECT id, org_id, slug, name, created_at, updated_at
            FROM teams
            WHERE org_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        } else {
            r#"
            SELECT id, org_id, slug, name, created_at, updated_at
            FROM teams
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
        let items: Vec<Team> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(Team {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |team| {
                cursor_from_row(team.created_at, team.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_org(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) as count FROM teams WHERE org_id = ?"
        } else {
            "SELECT COUNT(*) as count FROM teams WHERE org_id = ? AND deleted_at IS NULL"
        };

        let row = wasm_query(query)
            .bind(org_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateTeam) -> DbResult<Team> {
        if let Some(name) = input.name {
            let now = chrono::Utc::now();

            let result = wasm_query(
                r#"
                UPDATE teams
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

            self.get_by_id(id).await?.ok_or(DbError::NotFound)
        } else {
            self.get_by_id(id).await?.ok_or(DbError::NotFound)
        }
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = wasm_query(
            r#"
            UPDATE teams
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

    // ========================================================================
    // Team membership operations
    // ========================================================================

    async fn add_member(&self, team_id: Uuid, input: AddTeamMember) -> DbResult<TeamMember> {
        let now = chrono::Utc::now();

        wasm_query(
            r#"
            INSERT INTO team_memberships (team_id, user_id, role, source, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(team_id.to_string())
        .bind(input.user_id.to_string())
        .bind(&input.role)
        .bind(input.source.as_str())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict("User is already a member of this team".to_string())
            } else {
                DbError::from(e)
            }
        })?;

        // Fetch the user details to return a complete TeamMember
        self.get_member(team_id, input.user_id)
            .await?
            .ok_or(DbError::NotFound)
    }

    async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<()> {
        let result = wasm_query(
            r#"
            DELETE FROM team_memberships
            WHERE team_id = ? AND user_id = ?
            "#,
        )
        .bind(team_id.to_string())
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn remove_memberships_by_source(
        &self,
        user_id: Uuid,
        source: MembershipSource,
        except_team_ids: &[Uuid],
    ) -> DbResult<u64> {
        let result = if except_team_ids.is_empty() {
            wasm_query(
                r#"
                DELETE FROM team_memberships
                WHERE user_id = ? AND source = ?
                "#,
            )
            .bind(user_id.to_string())
            .bind(source.as_str())
            .execute(&self.pool)
            .await?
        } else {
            let placeholders = except_team_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let query = format!(
                r#"
                DELETE FROM team_memberships
                WHERE user_id = ? AND source = ? AND team_id NOT IN ({})
                "#,
                placeholders
            );
            let mut q = wasm_query(&query)
                .bind(user_id.to_string())
                .bind(source.as_str());
            for id in except_team_ids {
                q = q.bind(id.to_string());
            }
            q.execute(&self.pool).await?
        };

        Ok(result.rows_affected())
    }

    async fn update_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        input: UpdateTeamMember,
    ) -> DbResult<TeamMember> {
        let result = wasm_query(
            r#"
            UPDATE team_memberships
            SET role = ?
            WHERE team_id = ? AND user_id = ?
            "#,
        )
        .bind(&input.role)
        .bind(team_id.to_string())
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        self.get_member(team_id, user_id)
            .await?
            .ok_or(DbError::NotFound)
    }

    async fn list_members(
        &self,
        team_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<TeamMember>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_members_with_cursor(team_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        let rows = wasm_query(
            r#"
            SELECT u.id as user_id, u.external_id, u.email, u.name,
                   tm.role, tm.created_at as joined_at
            FROM users u
            INNER JOIN team_memberships tm ON u.id = tm.user_id
            WHERE tm.team_id = ?
            ORDER BY tm.created_at DESC, u.id DESC
            LIMIT ?
            "#,
        )
        .bind(team_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<TeamMember> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(TeamMember {
                    user_id: parse_uuid(&row.get::<String>("user_id"))?,
                    external_id: row.get("external_id"),
                    email: row.get("email"),
                    name: row.get("name"),
                    role: row.get("role"),
                    joined_at: row.get("joined_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |member| {
                cursor_from_row(member.joined_at, member.user_id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn get_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<Option<TeamMember>> {
        let result = wasm_query(
            r#"
            SELECT u.id as user_id, u.external_id, u.email, u.name,
                   tm.role, tm.created_at as joined_at
            FROM users u
            INNER JOIN team_memberships tm ON u.id = tm.user_id
            WHERE tm.team_id = ? AND tm.user_id = ?
            "#,
        )
        .bind(team_id.to_string())
        .bind(user_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(TeamMember {
                user_id: parse_uuid(&row.get::<String>("user_id"))?,
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                role: row.get("role"),
                joined_at: row.get("joined_at"),
            })),
            None => Ok(None),
        }
    }

    async fn is_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<bool> {
        let row = wasm_query(
            r#"
            SELECT COUNT(*) as count
            FROM team_memberships
            WHERE team_id = ? AND user_id = ?
            "#,
        )
        .bind(team_id.to_string())
        .bind(user_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64>("count") > 0)
    }

    async fn count_members(&self, team_id: Uuid) -> DbResult<i64> {
        let row = wasm_query("SELECT COUNT(*) as count FROM team_memberships WHERE team_id = ?")
            .bind(team_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64>("count"))
    }
}
