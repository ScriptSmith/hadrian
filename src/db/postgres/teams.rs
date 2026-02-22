use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, TeamRepo, cursor_from_row,
        },
    },
    models::{
        AddTeamMember, CreateTeam, MembershipSource, Team, TeamMember, UpdateTeam, UpdateTeamMember,
    },
};

pub struct PostgresTeamRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresTeamRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
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
        let mut items: Vec<Team> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Team {
                id: row.get("id"),
                org_id: row.get("org_id"),
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

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
            WHERE tm.team_id = $1
            AND ROW(tm.created_at, u.id) {} ROW($2, $3)
            ORDER BY tm.created_at {}, u.id {}
            LIMIT $4
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(team_id)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<TeamMember> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| TeamMember {
                user_id: row.get("user_id"),
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                role: row.get("role"),
                joined_at: row.get("joined_at"),
            })
            .collect();

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

#[async_trait]
impl TeamRepo for PostgresTeamRepo {
    async fn create(&self, org_id: Uuid, input: CreateTeam) -> DbResult<Team> {
        let row = sqlx::query(
            r#"
            INSERT INTO teams (id, org_id, slug, name)
            VALUES ($1, $2, $3, $4)
            RETURNING id, org_id, slug, name, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(&input.slug)
        .bind(&input.name)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Team with slug '{}' already exists in this organization",
                    input.slug
                ))
            }
            _ => DbError::from(e),
        })?;

        Ok(Team {
            id: row.get("id"),
            org_id: row.get("org_id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Team>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, slug, name, created_at, updated_at
            FROM teams
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Team {
            id: row.get("id"),
            org_id: row.get("org_id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn get_by_ids(&self, ids: &[Uuid]) -> DbResult<Vec<Team>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT id, org_id, slug, name, created_at, updated_at
            FROM teams
            WHERE id = ANY($1) AND deleted_at IS NULL
            "#,
        )
        .bind(ids)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| Team {
                id: row.get("id"),
                org_id: row.get("org_id"),
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<Team>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, slug, name, created_at, updated_at
            FROM teams
            WHERE org_id = $1 AND slug = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(org_id)
        .bind(slug)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Team {
            id: row.get("id"),
            org_id: row.get("org_id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
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
            WHERE org_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#
        } else {
            r#"
            SELECT id, org_id, slug, name, created_at, updated_at
            FROM teams
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
        let items: Vec<Team> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Team {
                id: row.get("id"),
                org_id: row.get("org_id"),
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |team| {
                cursor_from_row(team.created_at, team.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_org(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) as count FROM teams WHERE org_id = $1"
        } else {
            "SELECT COUNT(*) as count FROM teams WHERE org_id = $1 AND deleted_at IS NULL"
        };

        let row = sqlx::query(query)
            .bind(org_id)
            .fetch_one(&self.read_pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateTeam) -> DbResult<Team> {
        if let Some(name) = input.name {
            let row = sqlx::query(
                r#"
                UPDATE teams
                SET name = $1, updated_at = NOW()
                WHERE id = $2 AND deleted_at IS NULL
                RETURNING id, org_id, slug, name, created_at, updated_at
                "#,
            )
            .bind(&name)
            .bind(id)
            .fetch_optional(&self.write_pool)
            .await?
            .ok_or(DbError::NotFound)?;

            Ok(Team {
                id: row.get("id"),
                org_id: row.get("org_id"),
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
            UPDATE teams
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

    // ========================================================================
    // Team membership operations
    // ========================================================================

    async fn add_member(&self, team_id: Uuid, input: AddTeamMember) -> DbResult<TeamMember> {
        sqlx::query(
            r#"
            INSERT INTO team_memberships (team_id, user_id, role, source)
            VALUES ($1, $2, $3, $4::membership_source)
            "#,
        )
        .bind(team_id)
        .bind(input.user_id)
        .bind(&input.role)
        .bind(input.source.as_str())
        .execute(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("User is already a member of this team".to_string())
            }
            _ => DbError::from(e),
        })?;

        self.get_member(team_id, input.user_id)
            .await?
            .ok_or(DbError::NotFound)
    }

    async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            DELETE FROM team_memberships
            WHERE team_id = $1 AND user_id = $2
            "#,
        )
        .bind(team_id)
        .bind(user_id)
        .execute(&self.write_pool)
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
            sqlx::query(
                r#"
                DELETE FROM team_memberships
                WHERE user_id = $1 AND source = $2::membership_source
                "#,
            )
            .bind(user_id)
            .bind(source.as_str())
            .execute(&self.write_pool)
            .await?
        } else {
            sqlx::query(
                r#"
                DELETE FROM team_memberships
                WHERE user_id = $1 AND source = $2::membership_source AND team_id != ALL($3)
                "#,
            )
            .bind(user_id)
            .bind(source.as_str())
            .bind(except_team_ids)
            .execute(&self.write_pool)
            .await?
        };

        Ok(result.rows_affected())
    }

    async fn update_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        input: UpdateTeamMember,
    ) -> DbResult<TeamMember> {
        // Use UPDATE ... RETURNING with JOIN to get the updated member in one query
        // This ensures we read from the primary (write_pool) to avoid replication lag
        let result = sqlx::query(
            r#"
            UPDATE team_memberships tm
            SET role = $1
            FROM users u
            WHERE tm.team_id = $2 AND tm.user_id = $3 AND u.id = tm.user_id
            RETURNING u.id as user_id, u.external_id, u.email, u.name,
                      tm.role, tm.created_at as joined_at
            "#,
        )
        .bind(&input.role)
        .bind(team_id)
        .bind(user_id)
        .fetch_optional(&self.write_pool)
        .await?;

        result
            .map(|row| TeamMember {
                user_id: row.get("user_id"),
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                role: row.get("role"),
                joined_at: row.get("joined_at"),
            })
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

        let rows = sqlx::query(
            r#"
            SELECT u.id as user_id, u.external_id, u.email, u.name,
                   tm.role, tm.created_at as joined_at
            FROM users u
            INNER JOIN team_memberships tm ON u.id = tm.user_id
            WHERE tm.team_id = $1
            ORDER BY tm.created_at DESC, u.id DESC
            LIMIT $2
            "#,
        )
        .bind(team_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<TeamMember> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| TeamMember {
                user_id: row.get("user_id"),
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                role: row.get("role"),
                joined_at: row.get("joined_at"),
            })
            .collect();

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |member| {
                cursor_from_row(member.joined_at, member.user_id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn get_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<Option<TeamMember>> {
        let result = sqlx::query(
            r#"
            SELECT u.id as user_id, u.external_id, u.email, u.name,
                   tm.role, tm.created_at as joined_at
            FROM users u
            INNER JOIN team_memberships tm ON u.id = tm.user_id
            WHERE tm.team_id = $1 AND tm.user_id = $2
            "#,
        )
        .bind(team_id)
        .bind(user_id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| TeamMember {
            user_id: row.get("user_id"),
            external_id: row.get("external_id"),
            email: row.get("email"),
            name: row.get("name"),
            role: row.get("role"),
            joined_at: row.get("joined_at"),
        }))
    }

    async fn is_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<bool> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM team_memberships
            WHERE team_id = $1 AND user_id = $2
            "#,
        )
        .bind(team_id)
        .bind(user_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(row.get::<i64, _>("count") > 0)
    }

    async fn count_members(&self, team_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM team_memberships WHERE team_id = $1")
            .bind(team_id)
            .fetch_one(&self.read_pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }
}
