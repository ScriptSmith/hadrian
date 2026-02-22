use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
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

pub struct SqliteTeamRepo {
    pool: SqlitePool,
}

impl SqliteTeamRepo {
    pub fn new(pool: SqlitePool) -> Self {
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

        let rows = sqlx::query(&query)
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
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
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

        let rows = sqlx::query(&query)
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
                    user_id: parse_uuid(&row.get::<String, _>("user_id"))?,
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

#[async_trait]
impl TeamRepo for SqliteTeamRepo {
    async fn create(&self, org_id: Uuid, input: CreateTeam) -> DbResult<Team> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
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
            id,
            org_id,
            slug: input.slug,
            name: input.name,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Team>> {
        let result = sqlx::query(
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
                id: parse_uuid(&row.get::<String, _>("id"))?,
                org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
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

        let mut query_builder = sqlx::query(&query);
        for id in ids {
            query_builder = query_builder.bind(id.to_string());
        }

        let rows = query_builder.fetch_all(&self.pool).await?;

        rows.into_iter()
            .map(|row| {
                Ok(Team {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect()
    }

    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<Team>> {
        let result = sqlx::query(
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
                id: parse_uuid(&row.get::<String, _>("id"))?,
                org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
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

        let rows = sqlx::query(query)
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
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
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

        let row = sqlx::query(query)
            .bind(org_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateTeam) -> DbResult<Team> {
        if let Some(name) = input.name {
            let now = chrono::Utc::now();

            let result = sqlx::query(
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

        let result = sqlx::query(
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

        sqlx::query(
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
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("User is already a member of this team".to_string())
            }
            _ => DbError::from(e),
        })?;

        // Fetch the user details to return a complete TeamMember
        self.get_member(team_id, input.user_id)
            .await?
            .ok_or(DbError::NotFound)
    }

    async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
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
            sqlx::query(
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
            let mut q = sqlx::query(&query)
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
        let result = sqlx::query(
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

        let rows = sqlx::query(
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
                    user_id: parse_uuid(&row.get::<String, _>("user_id"))?,
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
        let result = sqlx::query(
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
                user_id: parse_uuid(&row.get::<String, _>("user_id"))?,
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
        let row = sqlx::query(
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

        Ok(row.get::<i64, _>("count") > 0)
    }

    async fn count_members(&self, team_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM team_memberships WHERE team_id = ?")
            .bind(team_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::TeamRepo;

    /// Create an in-memory SQLite database with the required tables
    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create the organizations table
        sqlx::query(
            r#"
            CREATE TABLE organizations (
                id TEXT PRIMARY KEY NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create organizations table");

        // Create the teams table
        sqlx::query(
            r#"
            CREATE TABLE teams (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                slug TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(org_id, slug)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create teams table");

        // Create the users table
        sqlx::query(
            r#"
            CREATE TABLE users (
                id TEXT PRIMARY KEY NOT NULL,
                external_id TEXT NOT NULL UNIQUE,
                email TEXT UNIQUE,
                name TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create users table");

        // Create the team_memberships table
        sqlx::query(
            r#"
            CREATE TABLE team_memberships (
                team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
                user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                role TEXT NOT NULL DEFAULT 'member',
                source TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'jit', 'scim')),
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (team_id, user_id)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create team_memberships table");

        pool
    }

    async fn create_test_org(pool: &SqlitePool, slug: &str) -> Uuid {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            r#"
            INSERT INTO organizations (id, slug, name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(slug)
        .bind(format!("Org {}", slug))
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("Failed to create test org");
        id
    }

    async fn create_test_user(pool: &SqlitePool, external_id: &str) -> Uuid {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            r#"
            INSERT INTO users (id, external_id, email, name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(external_id)
        .bind(format!("{}@example.com", external_id))
        .bind(format!("User {}", external_id))
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("Failed to create test user");
        id
    }

    fn create_team_input(slug: &str, name: &str) -> CreateTeam {
        CreateTeam {
            slug: slug.to_string(),
            name: name.to_string(),
        }
    }

    // ==================== Team CRUD Tests ====================

    #[tokio::test]
    async fn test_create_team() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        let input = create_team_input("test-team", "Test Team");
        let team = repo
            .create(org_id, input)
            .await
            .expect("Failed to create team");

        assert_eq!(team.slug, "test-team");
        assert_eq!(team.name, "Test Team");
        assert_eq!(team.org_id, org_id);
        assert!(!team.id.is_nil());
    }

    #[tokio::test]
    async fn test_create_duplicate_slug_same_org_fails() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        let input = create_team_input("duplicate", "First Team");
        repo.create(org_id, input)
            .await
            .expect("Failed to create first team");

        let input2 = create_team_input("duplicate", "Second Team");
        let result = repo.create(org_id, input2).await;

        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_create_same_slug_different_orgs_succeeds() {
        let pool = create_test_pool().await;
        let org1_id = create_test_org(&pool, "org-1").await;
        let org2_id = create_test_org(&pool, "org-2").await;
        let repo = SqliteTeamRepo::new(pool);

        let input1 = create_team_input("same-slug", "Team in Org 1");
        let team1 = repo
            .create(org1_id, input1)
            .await
            .expect("Failed to create team in org 1");

        let input2 = create_team_input("same-slug", "Team in Org 2");
        let team2 = repo
            .create(org2_id, input2)
            .await
            .expect("Failed to create team in org 2");

        assert_eq!(team1.slug, team2.slug);
        assert_ne!(team1.id, team2.id);
        assert_ne!(team1.org_id, team2.org_id);
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        let input = create_team_input("get-test", "Get Test Team");
        let created = repo
            .create(org_id, input)
            .await
            .expect("Failed to create team");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get team")
            .expect("Team should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.org_id, org_id);
        assert_eq!(fetched.slug, "get-test");
        assert_eq!(fetched.name, "Get Test Team");
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteTeamRepo::new(pool);

        let result = repo
            .get_by_id(Uuid::new_v4())
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_slug() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        let input = create_team_input("slug-test", "Slug Test Team");
        let created = repo
            .create(org_id, input)
            .await
            .expect("Failed to create team");

        let fetched = repo
            .get_by_slug(org_id, "slug-test")
            .await
            .expect("Failed to get team")
            .expect("Team should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.slug, "slug-test");
    }

    #[tokio::test]
    async fn test_get_by_slug_wrong_org() {
        let pool = create_test_pool().await;
        let org1_id = create_test_org(&pool, "org-1").await;
        let org2_id = create_test_org(&pool, "org-2").await;
        let repo = SqliteTeamRepo::new(pool);

        let input = create_team_input("team-slug", "Test Team");
        repo.create(org1_id, input)
            .await
            .expect("Failed to create team");

        let result = repo
            .get_by_slug(org2_id, "team-slug")
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_by_org_empty() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list teams");

        assert!(result.items.is_empty());
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_by_org_with_teams() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        for i in 0..3 {
            repo.create(
                org_id,
                create_team_input(&format!("team-{}", i), &format!("Team {}", i)),
            )
            .await
            .expect("Failed to create team");
        }

        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list teams");

        assert_eq!(result.items.len(), 3);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_by_org_filters_by_org() {
        let pool = create_test_pool().await;
        let org1_id = create_test_org(&pool, "org-1").await;
        let org2_id = create_test_org(&pool, "org-2").await;
        let repo = SqliteTeamRepo::new(pool);

        repo.create(org1_id, create_team_input("team-1", "Org1 Team"))
            .await
            .expect("Failed to create team");
        repo.create(org2_id, create_team_input("team-2", "Org2 Team"))
            .await
            .expect("Failed to create team");

        let org1_result = repo
            .list_by_org(org1_id, ListParams::default())
            .await
            .expect("Failed to list");
        let org2_result = repo
            .list_by_org(org2_id, ListParams::default())
            .await
            .expect("Failed to list");

        assert_eq!(org1_result.items.len(), 1);
        assert_eq!(org1_result.items[0].name, "Org1 Team");
        assert_eq!(org2_result.items.len(), 1);
        assert_eq!(org2_result.items[0].name, "Org2 Team");
    }

    #[tokio::test]
    async fn test_list_by_org_with_pagination() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        for i in 0..5 {
            repo.create(
                org_id,
                create_team_input(&format!("team-{}", i), &format!("Team {}", i)),
            )
            .await
            .expect("Failed to create team");
        }

        let page1 = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 1");

        let page2 = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(2),
                    cursor: page1.cursors.next.clone(),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 2");

        assert_eq!(page1.items.len(), 2);
        assert_eq!(page2.items.len(), 2);
        assert!(page1.has_more);
        assert!(page2.has_more);
        assert_ne!(page1.items[0].id, page2.items[0].id);
    }

    #[tokio::test]
    async fn test_count_by_org() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        for i in 0..3 {
            repo.create(
                org_id,
                create_team_input(&format!("team-{}", i), &format!("Team {}", i)),
            )
            .await
            .expect("Failed to create team");
        }

        let count = repo
            .count_by_org(org_id, false)
            .await
            .expect("Failed to count");
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_update_name() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        let created = repo
            .create(org_id, create_team_input("update-test", "Original Name"))
            .await
            .expect("Failed to create team");

        let updated = repo
            .update(
                created.id,
                UpdateTeam {
                    name: Some("Updated Name".to_string()),
                },
            )
            .await
            .expect("Failed to update team");

        assert_eq!(updated.id, created.id);
        assert_eq!(updated.slug, "update-test");
        assert_eq!(updated.name, "Updated Name");
        assert!(updated.updated_at >= created.updated_at);
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteTeamRepo::new(pool);

        let result = repo
            .update(
                Uuid::new_v4(),
                UpdateTeam {
                    name: Some("New Name".to_string()),
                },
            )
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        let created = repo
            .create(org_id, create_team_input("delete-test", "To Delete"))
            .await
            .expect("Failed to create team");

        repo.delete(created.id)
            .await
            .expect("Failed to delete team");

        let result = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed");
        assert!(result.is_none());

        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list");
        assert!(result.items.is_empty());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteTeamRepo::new(pool);

        let result = repo.delete(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_count_excludes_deleted() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        let team1 = repo
            .create(org_id, create_team_input("team-1", "Team 1"))
            .await
            .expect("Failed to create team 1");
        repo.create(org_id, create_team_input("team-2", "Team 2"))
            .await
            .expect("Failed to create team 2");

        repo.delete(team1.id).await.expect("Failed to delete");

        let count = repo
            .count_by_org(org_id, false)
            .await
            .expect("Failed to count");
        assert_eq!(count, 1);

        let count_all = repo
            .count_by_org(org_id, true)
            .await
            .expect("Failed to count all");
        assert_eq!(count_all, 2);
    }

    #[tokio::test]
    async fn test_list_include_deleted() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool);

        let team1 = repo
            .create(org_id, create_team_input("team-1", "Team 1"))
            .await
            .expect("Failed to create team 1");
        repo.create(org_id, create_team_input("team-2", "Team 2"))
            .await
            .expect("Failed to create team 2");

        repo.delete(team1.id).await.expect("Failed to delete");

        let active = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list active");
        assert_eq!(active.items.len(), 1);

        let all = repo
            .list_by_org(
                org_id,
                ListParams {
                    include_deleted: true,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list all");
        assert_eq!(all.items.len(), 2);
    }

    // ==================== Team Membership Tests ====================

    #[tokio::test]
    async fn test_add_member() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let user_id = create_test_user(&pool, "test-user").await;
        let repo = SqliteTeamRepo::new(pool);

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        let member = repo
            .add_member(
                team.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");

        assert_eq!(member.user_id, user_id);
        assert_eq!(member.role, "member");
        assert_eq!(member.external_id, "test-user");
    }

    #[tokio::test]
    async fn test_add_member_with_role() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let user_id = create_test_user(&pool, "admin-user").await;
        let repo = SqliteTeamRepo::new(pool);

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        let member = repo
            .add_member(
                team.id,
                AddTeamMember {
                    user_id,
                    role: "admin".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");

        assert_eq!(member.role, "admin");
    }

    #[tokio::test]
    async fn test_add_member_duplicate_fails() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let user_id = create_test_user(&pool, "test-user").await;
        let repo = SqliteTeamRepo::new(pool);

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        repo.add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

        let result = repo
            .add_member(
                team.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await;

        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_remove_member() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let user_id = create_test_user(&pool, "test-user").await;
        let repo = SqliteTeamRepo::new(pool);

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        repo.add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

        repo.remove_member(team.id, user_id)
            .await
            .expect("Failed to remove member");

        let is_member = repo
            .is_member(team.id, user_id)
            .await
            .expect("Failed to check membership");
        assert!(!is_member);
    }

    #[tokio::test]
    async fn test_remove_member_not_found() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let user_id = create_test_user(&pool, "test-user").await;
        let repo = SqliteTeamRepo::new(pool);

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        let result = repo.remove_member(team.id, user_id).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_update_member_role() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let user_id = create_test_user(&pool, "test-user").await;
        let repo = SqliteTeamRepo::new(pool);

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        repo.add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

        let updated = repo
            .update_member_role(
                team.id,
                user_id,
                UpdateTeamMember {
                    role: "admin".to_string(),
                },
            )
            .await
            .expect("Failed to update role");

        assert_eq!(updated.role, "admin");
    }

    #[tokio::test]
    async fn test_list_members() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool.clone());

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        for i in 0..3 {
            let user_id = create_test_user(&pool, &format!("user-{}", i)).await;
            repo.add_member(
                team.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");
        }

        let result = repo
            .list_members(team.id, ListParams::default())
            .await
            .expect("Failed to list members");

        assert_eq!(result.items.len(), 3);
    }

    #[tokio::test]
    async fn test_list_members_with_pagination() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool.clone());

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        for i in 0..5 {
            let user_id = create_test_user(&pool, &format!("user-{}", i)).await;
            repo.add_member(
                team.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");
        }

        let page1 = repo
            .list_members(
                team.id,
                ListParams {
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 1");

        let page2 = repo
            .list_members(
                team.id,
                ListParams {
                    limit: Some(2),
                    cursor: page1.cursors.next.clone(),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 2");

        assert_eq!(page1.items.len(), 2);
        assert_eq!(page2.items.len(), 2);
        assert!(page1.has_more);
        assert!(page2.has_more);
        assert_ne!(page1.items[0].user_id, page2.items[0].user_id);
    }

    #[tokio::test]
    async fn test_get_member() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let user_id = create_test_user(&pool, "test-user").await;
        let repo = SqliteTeamRepo::new(pool);

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        repo.add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "admin".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

        let member = repo
            .get_member(team.id, user_id)
            .await
            .expect("Failed to get member")
            .expect("Member should exist");

        assert_eq!(member.user_id, user_id);
        assert_eq!(member.role, "admin");
        assert_eq!(member.external_id, "test-user");
    }

    #[tokio::test]
    async fn test_get_member_not_found() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let user_id = create_test_user(&pool, "test-user").await;
        let repo = SqliteTeamRepo::new(pool);

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        let result = repo
            .get_member(team.id, user_id)
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_is_member() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let user_id = create_test_user(&pool, "test-user").await;
        let repo = SqliteTeamRepo::new(pool);

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        assert!(
            !repo
                .is_member(team.id, user_id)
                .await
                .expect("Failed to check membership")
        );

        repo.add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

        assert!(
            repo.is_member(team.id, user_id)
                .await
                .expect("Failed to check membership")
        );
    }

    #[tokio::test]
    async fn test_count_members() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool.clone());

        let team = repo
            .create(org_id, create_team_input("test-team", "Test Team"))
            .await
            .expect("Failed to create team");

        assert_eq!(
            repo.count_members(team.id).await.expect("Failed to count"),
            0
        );

        for i in 0..3 {
            let user_id = create_test_user(&pool, &format!("user-{}", i)).await;
            repo.add_member(
                team.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");
        }

        assert_eq!(
            repo.count_members(team.id).await.expect("Failed to count"),
            3
        );
    }

    #[tokio::test]
    async fn test_user_can_be_in_multiple_teams() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let user_id = create_test_user(&pool, "multi-team-user").await;
        let repo = SqliteTeamRepo::new(pool);

        let team1 = repo
            .create(org_id, create_team_input("team-1", "Team 1"))
            .await
            .expect("Failed to create team 1");
        let team2 = repo
            .create(org_id, create_team_input("team-2", "Team 2"))
            .await
            .expect("Failed to create team 2");

        repo.add_member(
            team1.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add to team 1");
        repo.add_member(
            team2.id,
            AddTeamMember {
                user_id,
                role: "admin".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add to team 2");

        assert!(
            repo.is_member(team1.id, user_id)
                .await
                .expect("Failed to check")
        );
        assert!(
            repo.is_member(team2.id, user_id)
                .await
                .expect("Failed to check")
        );

        let member1 = repo
            .get_member(team1.id, user_id)
            .await
            .expect("Query failed")
            .expect("Should be member");
        let member2 = repo
            .get_member(team2.id, user_id)
            .await
            .expect("Query failed")
            .expect("Should be member");

        assert_eq!(member1.role, "member");
        assert_eq!(member2.role, "admin");
    }

    #[tokio::test]
    async fn test_members_isolated_by_team() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool.clone());

        let team1 = repo
            .create(org_id, create_team_input("team-1", "Team 1"))
            .await
            .expect("Failed to create team 1");
        let team2 = repo
            .create(org_id, create_team_input("team-2", "Team 2"))
            .await
            .expect("Failed to create team 2");

        // Add 2 users to team 1
        for i in 0..2 {
            let user_id = create_test_user(&pool, &format!("t1-user-{}", i)).await;
            repo.add_member(
                team1.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");
        }

        // Add 3 users to team 2
        for i in 0..3 {
            let user_id = create_test_user(&pool, &format!("t2-user-{}", i)).await;
            repo.add_member(
                team2.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");
        }

        assert_eq!(
            repo.count_members(team1.id).await.expect("Failed to count"),
            2
        );
        assert_eq!(
            repo.count_members(team2.id).await.expect("Failed to count"),
            3
        );
    }

    // ==================== Membership Source Sync Tests ====================

    #[tokio::test]
    async fn test_sync_removes_only_jit_memberships() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool.clone());

        let team = repo
            .create(org_id, create_team_input("sync-test", "Sync Test Team"))
            .await
            .expect("Failed to create team");

        // Add members with different sources
        let jit_user = create_test_user(&pool, "jit-user").await;
        let manual_user = create_test_user(&pool, "manual-user").await;
        let scim_user = create_test_user(&pool, "scim-user").await;

        repo.add_member(
            team.id,
            AddTeamMember {
                user_id: jit_user,
                role: "member".to_string(),
                source: MembershipSource::Jit,
            },
        )
        .await
        .expect("Failed to add JIT member");

        repo.add_member(
            team.id,
            AddTeamMember {
                user_id: manual_user,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add manual member");

        repo.add_member(
            team.id,
            AddTeamMember {
                user_id: scim_user,
                role: "member".to_string(),
                source: MembershipSource::Scim,
            },
        )
        .await
        .expect("Failed to add SCIM member");

        // Sync removes JIT memberships not in the except list
        let removed = repo
            .remove_memberships_by_source(jit_user, MembershipSource::Jit, &[])
            .await
            .expect("Failed to remove memberships");

        assert_eq!(removed, 1, "Should remove exactly 1 JIT membership");

        // Manual and SCIM users should still be members
        assert!(
            repo.is_member(team.id, manual_user)
                .await
                .expect("Query failed")
        );
        assert!(
            repo.is_member(team.id, scim_user)
                .await
                .expect("Query failed")
        );

        // JIT user should no longer be a member
        assert!(
            !repo
                .is_member(team.id, jit_user)
                .await
                .expect("Query failed")
        );
    }

    #[tokio::test]
    async fn test_sync_preserves_jit_membership_in_except_list() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteTeamRepo::new(pool.clone());

        let team1 = repo
            .create(org_id, create_team_input("team-1", "Team 1 (should keep)"))
            .await
            .expect("Failed to create team1");

        let team2 = repo
            .create(
                org_id,
                create_team_input("team-2", "Team 2 (should remove)"),
            )
            .await
            .expect("Failed to create team2");

        let user = create_test_user(&pool, "test-user").await;

        // Add user to both teams with JIT source
        repo.add_member(
            team1.id,
            AddTeamMember {
                user_id: user,
                role: "member".to_string(),
                source: MembershipSource::Jit,
            },
        )
        .await
        .expect("Failed to add to team1");

        repo.add_member(
            team2.id,
            AddTeamMember {
                user_id: user,
                role: "member".to_string(),
                source: MembershipSource::Jit,
            },
        )
        .await
        .expect("Failed to add to team2");

        // Sync with team1 in the except list (should preserve team1, remove team2)
        let removed = repo
            .remove_memberships_by_source(user, MembershipSource::Jit, &[team1.id])
            .await
            .expect("Failed to remove memberships");

        assert_eq!(removed, 1, "Should remove exactly 1 JIT membership");

        // Team1 membership should be preserved
        assert!(repo.is_member(team1.id, user).await.expect("Query failed"));

        // Team2 membership should be removed
        assert!(!repo.is_member(team2.id, user).await.expect("Query failed"));
    }
}
