use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, UserDeletionResult,
            UserRepo, cursor_from_row,
        },
    },
    models::{
        CreateUser, MembershipSource, TeamMembership, UpdateUser, User, UserOrgMembership,
        UserProjectMembership,
    },
};

pub struct SqliteUserRepo {
    pool: SqlitePool,
}

impl SqliteUserRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Helper method for cursor-based pagination of users.
    async fn list_with_cursor(
        &self,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<User>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, external_id, email, name, created_at, updated_at
            FROM users
            WHERE (created_at, id) {} (?, ?)
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(User {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    external_id: row.get("external_id"),
                    email: row.get("email"),
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
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |user| {
                cursor_from_row(user.created_at, user.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of org members.
    async fn list_org_members_with_cursor(
        &self,
        org_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<User>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT u.id, u.external_id, u.email, u.name, u.created_at, u.updated_at
            FROM users u
            INNER JOIN org_memberships om ON u.id = om.user_id
            WHERE om.org_id = ?
            AND (u.created_at, u.id) {} (?, ?)
            ORDER BY u.created_at {}, u.id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(org_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(User {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    external_id: row.get("external_id"),
                    email: row.get("email"),
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
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |user| {
                cursor_from_row(user.created_at, user.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    /// Helper method for cursor-based pagination of project members.
    async fn list_project_members_with_cursor(
        &self,
        project_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<User>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT u.id, u.external_id, u.email, u.name, u.created_at, u.updated_at
            FROM users u
            INNER JOIN project_memberships pm ON u.id = pm.user_id
            WHERE pm.project_id = ?
            AND (u.created_at, u.id) {} (?, ?)
            ORDER BY u.created_at {}, u.id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(project_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(User {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    external_id: row.get("external_id"),
                    email: row.get("email"),
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
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |user| {
                cursor_from_row(user.created_at, user.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[async_trait]
impl UserRepo for SqliteUserRepo {
    async fn create(&self, input: CreateUser) -> DbResult<User> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO users (id, external_id, email, name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&input.external_id)
        .bind(&input.email)
        .bind(&input.name)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "User with external_id '{}' already exists",
                    input.external_id
                ))
            }
            _ => DbError::from(e),
        })?;

        Ok(User {
            id,
            external_id: input.external_id,
            email: input.email,
            name: input.name,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<User>> {
        let result = sqlx::query(
            r#"
            SELECT id, external_id, email, name, created_at, updated_at
            FROM users
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(User {
                id: parse_uuid(&row.get::<String, _>("id"))?,
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn get_by_external_id(&self, external_id: &str) -> DbResult<Option<User>> {
        let result = sqlx::query(
            r#"
            SELECT id, external_id, email, name, created_at, updated_at
            FROM users
            WHERE external_id = ?
            "#,
        )
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(User {
                id: parse_uuid(&row.get::<String, _>("id"))?,
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn list(&self, params: ListParams) -> DbResult<ListResult<User>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(&params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT id, external_id, email, name, created_at, updated_at
            FROM users
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(User {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    external_id: row.get("external_id"),
                    email: row.get("email"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |user| {
                cursor_from_row(user.created_at, user.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count(&self, _include_deleted: bool) -> DbResult<i64> {
        // Users table doesn't have soft delete, so include_deleted is ignored
        let row = sqlx::query("SELECT COUNT(*) as count FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateUser) -> DbResult<User> {
        let now = chrono::Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE users
            SET email = COALESCE(?, email),
                name = COALESCE(?, name),
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&input.email)
        .bind(&input.name)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn add_to_org(
        &self,
        user_id: Uuid,
        org_id: Uuid,
        role: &str,
        source: MembershipSource,
    ) -> DbResult<()> {
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO org_memberships (org_id, user_id, role, source, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(org_id.to_string())
        .bind(user_id.to_string())
        .bind(role)
        .bind(source.as_str())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                // SQLite error format: "UNIQUE constraint failed: table.column1, table.column2"
                // Primary key (org_id, user_id) violation includes both columns
                // Single-org unique index (user_id only) violation includes just user_id
                let msg = db_err.message();
                // If message contains user_id but NOT org_id, it's the single-org constraint
                if msg.contains("user_id") && !msg.contains("org_id") {
                    DbError::Conflict(
                        "User already belongs to another organization. \
                         Each user can only belong to one organization."
                            .to_string(),
                    )
                } else {
                    DbError::Conflict("User is already a member of this organization".to_string())
                }
            }
            _ => DbError::from(e),
        })?;

        Ok(())
    }

    async fn remove_org_memberships_by_source(
        &self,
        user_id: Uuid,
        source: MembershipSource,
        except_org_ids: &[Uuid],
    ) -> DbResult<u64> {
        let result = if except_org_ids.is_empty() {
            sqlx::query(
                r#"
                DELETE FROM org_memberships
                WHERE user_id = ? AND source = ?
                "#,
            )
            .bind(user_id.to_string())
            .bind(source.as_str())
            .execute(&self.pool)
            .await?
        } else {
            // SQLite doesn't support ANY/ALL, so we use a subquery with NOT IN
            let placeholders = except_org_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let query = format!(
                r#"
                DELETE FROM org_memberships
                WHERE user_id = ? AND source = ? AND org_id NOT IN ({})
                "#,
                placeholders
            );
            let mut q = sqlx::query(&query)
                .bind(user_id.to_string())
                .bind(source.as_str());
            for id in except_org_ids {
                q = q.bind(id.to_string());
            }
            q.execute(&self.pool).await?
        };

        Ok(result.rows_affected())
    }

    async fn update_org_member_role(
        &self,
        user_id: Uuid,
        org_id: Uuid,
        role: &str,
    ) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE org_memberships
            SET role = ?
            WHERE org_id = ? AND user_id = ?
            "#,
        )
        .bind(role)
        .bind(org_id.to_string())
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn remove_from_org(&self, user_id: Uuid, org_id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            DELETE FROM org_memberships
            WHERE org_id = ? AND user_id = ?
            "#,
        )
        .bind(org_id.to_string())
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn list_org_members(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<User>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_org_members_with_cursor(org_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT u.id, u.external_id, u.email, u.name, u.created_at, u.updated_at
            FROM users u
            INNER JOIN org_memberships om ON u.id = om.user_id
            WHERE om.org_id = ?
            ORDER BY u.created_at DESC, u.id DESC
            LIMIT ?
            "#,
        )
        .bind(org_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(User {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    external_id: row.get("external_id"),
                    email: row.get("email"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |user| {
                cursor_from_row(user.created_at, user.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_org_members(&self, org_id: Uuid, _include_deleted: bool) -> DbResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM org_memberships WHERE org_id = ?")
            .bind(org_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn add_to_project(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        role: &str,
        source: MembershipSource,
    ) -> DbResult<()> {
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO project_memberships (project_id, user_id, role, source, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .bind(role)
        .bind(source.as_str())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("User is already a member of this project".to_string())
            }
            _ => DbError::from(e),
        })?;

        Ok(())
    }

    async fn remove_project_memberships_by_source(
        &self,
        user_id: Uuid,
        source: MembershipSource,
        except_project_ids: &[Uuid],
    ) -> DbResult<u64> {
        let result = if except_project_ids.is_empty() {
            sqlx::query(
                r#"
                DELETE FROM project_memberships
                WHERE user_id = ? AND source = ?
                "#,
            )
            .bind(user_id.to_string())
            .bind(source.as_str())
            .execute(&self.pool)
            .await?
        } else {
            let placeholders = except_project_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let query = format!(
                r#"
                DELETE FROM project_memberships
                WHERE user_id = ? AND source = ? AND project_id NOT IN ({})
                "#,
                placeholders
            );
            let mut q = sqlx::query(&query)
                .bind(user_id.to_string())
                .bind(source.as_str());
            for id in except_project_ids {
                q = q.bind(id.to_string());
            }
            q.execute(&self.pool).await?
        };

        Ok(result.rows_affected())
    }

    async fn update_project_member_role(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        role: &str,
    ) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE project_memberships
            SET role = ?
            WHERE project_id = ? AND user_id = ?
            "#,
        )
        .bind(role)
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn remove_from_project(&self, user_id: Uuid, project_id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            DELETE FROM project_memberships
            WHERE project_id = ? AND user_id = ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(user_id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn list_project_members(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<User>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_project_members_with_cursor(project_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let rows = sqlx::query(
            r#"
            SELECT u.id, u.external_id, u.email, u.name, u.created_at, u.updated_at
            FROM users u
            INNER JOIN project_memberships pm ON u.id = pm.user_id
            WHERE pm.project_id = ?
            ORDER BY u.created_at DESC, u.id DESC
            LIMIT ?
            "#,
        )
        .bind(project_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(User {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    external_id: row.get("external_id"),
                    email: row.get("email"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |user| {
                cursor_from_row(user.created_at, user.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_project_members(
        &self,
        project_id: Uuid,
        _include_deleted: bool,
    ) -> DbResult<i64> {
        let row =
            sqlx::query("SELECT COUNT(*) as count FROM project_memberships WHERE project_id = ?")
                .bind(project_id.to_string())
                .fetch_one(&self.pool)
                .await?;
        Ok(row.get::<i64, _>("count"))
    }

    // ==================== GDPR Export Methods ====================

    async fn get_org_memberships_for_user(
        &self,
        user_id: Uuid,
    ) -> DbResult<Vec<UserOrgMembership>> {
        let rows = sqlx::query(
            r#"
            SELECT
                o.id as org_id,
                o.slug as org_slug,
                o.name as org_name,
                om.role,
                om.source,
                om.created_at as joined_at
            FROM org_memberships om
            INNER JOIN organizations o ON om.org_id = o.id
            WHERE om.user_id = ?
            ORDER BY om.created_at DESC
            "#,
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let source_str: String = row.get("source");
                Ok(UserOrgMembership {
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    org_slug: row.get("org_slug"),
                    org_name: row.get("org_name"),
                    role: row.get("role"),
                    source: MembershipSource::from_str(&source_str).unwrap_or_default(),
                    joined_at: row.get("joined_at"),
                })
            })
            .collect()
    }

    async fn get_project_memberships_for_user(
        &self,
        user_id: Uuid,
    ) -> DbResult<Vec<UserProjectMembership>> {
        let rows = sqlx::query(
            r#"
            SELECT
                p.id as project_id,
                p.slug as project_slug,
                p.name as project_name,
                p.org_id,
                pm.role,
                pm.source,
                pm.created_at as joined_at
            FROM project_memberships pm
            INNER JOIN projects p ON pm.project_id = p.id
            WHERE pm.user_id = ?
            ORDER BY pm.created_at DESC
            "#,
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let source_str: String = row.get("source");
                Ok(UserProjectMembership {
                    project_id: parse_uuid(&row.get::<String, _>("project_id"))?,
                    project_slug: row.get("project_slug"),
                    project_name: row.get("project_name"),
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    role: row.get("role"),
                    source: MembershipSource::from_str(&source_str).unwrap_or_default(),
                    joined_at: row.get("joined_at"),
                })
            })
            .collect()
    }

    async fn get_team_memberships_for_user(&self, user_id: Uuid) -> DbResult<Vec<TeamMembership>> {
        let rows = sqlx::query(
            r#"
            SELECT
                t.id as team_id,
                t.slug as team_slug,
                t.name as team_name,
                t.org_id,
                tm.role,
                tm.source,
                tm.created_at as joined_at
            FROM team_memberships tm
            INNER JOIN teams t ON tm.team_id = t.id
            WHERE tm.user_id = ? AND t.deleted_at IS NULL
            ORDER BY tm.created_at DESC
            "#,
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let source_str: String = row.get("source");
                Ok(TeamMembership {
                    team_id: parse_uuid(&row.get::<String, _>("team_id"))?,
                    team_slug: row.get("team_slug"),
                    team_name: row.get("team_name"),
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    role: row.get("role"),
                    source: MembershipSource::from_str(&source_str).unwrap_or_default(),
                    joined_at: row.get("joined_at"),
                })
            })
            .collect()
    }

    // ==================== GDPR Deletion Methods ====================

    async fn hard_delete(&self, user_id: Uuid) -> DbResult<UserDeletionResult> {
        let user_id_str = user_id.to_string();
        let mut result = UserDeletionResult::default();

        // Delete usage records for user's API keys first (they reference api_keys)
        let usage_result = sqlx::query(
            r#"
            DELETE FROM usage_records
            WHERE api_key_id IN (
                SELECT id FROM api_keys
                WHERE owner_type = 'user' AND owner_id = ?
            )
            "#,
        )
        .bind(&user_id_str)
        .execute(&self.pool)
        .await?;
        result.usage_records_deleted = usage_result.rows_affected();

        // Delete API keys owned by user
        let api_keys_result = sqlx::query(
            r#"
            DELETE FROM api_keys
            WHERE owner_type = 'user' AND owner_id = ?
            "#,
        )
        .bind(&user_id_str)
        .execute(&self.pool)
        .await?;
        result.api_keys_deleted = api_keys_result.rows_affected();

        // Delete conversations owned by user
        let conversations_result = sqlx::query(
            r#"
            DELETE FROM conversations
            WHERE owner_type = 'user' AND owner_id = ?
            "#,
        )
        .bind(&user_id_str)
        .execute(&self.pool)
        .await?;
        result.conversations_deleted = conversations_result.rows_affected();

        // Delete dynamic providers owned by user
        let providers_result = sqlx::query(
            r#"
            DELETE FROM dynamic_providers
            WHERE owner_type = 'user' AND owner_id = ?
            "#,
        )
        .bind(&user_id_str)
        .execute(&self.pool)
        .await?;
        result.dynamic_providers_deleted = providers_result.rows_affected();

        // Delete user (org_memberships and project_memberships cascade automatically)
        let user_result = sqlx::query(
            r#"
            DELETE FROM users WHERE id = ?
            "#,
        )
        .bind(&user_id_str)
        .execute(&self.pool)
        .await?;

        if user_result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        result.user_deleted = true;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::UserRepo;

    /// Create an in-memory SQLite database with all required tables
    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

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

        // Create the organizations table (needed for org_memberships FK)
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

        // Create the projects table (needed for project_memberships FK)
        sqlx::query(
            r#"
            CREATE TABLE projects (
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
        .expect("Failed to create projects table");

        // Create the org_memberships table
        sqlx::query(
            r#"
            CREATE TABLE org_memberships (
                org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                role TEXT NOT NULL DEFAULT 'member',
                source TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'jit', 'scim')),
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (org_id, user_id)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create org_memberships table");

        // Create single-org enforcement index
        sqlx::query(
            "CREATE UNIQUE INDEX idx_org_memberships_single_org ON org_memberships(user_id)",
        )
        .execute(&pool)
        .await
        .expect("Failed to create single-org index");

        // Create the project_memberships table
        sqlx::query(
            r#"
            CREATE TABLE project_memberships (
                project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                role TEXT NOT NULL DEFAULT 'member',
                source TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'jit', 'scim')),
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (project_id, user_id)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create project_memberships table");

        // Create the api_keys table (for hard_delete tests)
        sqlx::query(
            r#"
            CREATE TABLE api_keys (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                key_hash TEXT NOT NULL UNIQUE,
                key_prefix TEXT NOT NULL,
                owner_type TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                budget_amount INTEGER,
                budget_period TEXT,
                revoked_at TEXT,
                expires_at TEXT,
                last_used_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create api_keys table");

        // Create the conversations table (for hard_delete tests)
        sqlx::query(
            r#"
            CREATE TABLE conversations (
                id TEXT PRIMARY KEY NOT NULL,
                owner_type TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                title TEXT NOT NULL,
                models TEXT NOT NULL DEFAULT '[]',
                messages TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create conversations table");

        // Create the dynamic_providers table (for hard_delete tests)
        sqlx::query(
            r#"
            CREATE TABLE dynamic_providers (
                id TEXT PRIMARY KEY NOT NULL,
                owner_type TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                name TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                base_url TEXT NOT NULL,
                api_key_secret_ref TEXT,
                models TEXT NOT NULL DEFAULT '[]',
                is_enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create dynamic_providers table");

        // Create the usage_records table (for hard_delete tests)
        sqlx::query(
            r#"
            CREATE TABLE usage_records (
                id TEXT PRIMARY KEY NOT NULL,
                request_id TEXT NOT NULL UNIQUE,
                api_key_id TEXT NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
                model TEXT NOT NULL,
                provider TEXT NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                cost_microcents INTEGER NOT NULL DEFAULT 0,
                http_referer TEXT,
                recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
                streamed INTEGER NOT NULL DEFAULT 0,
                cached_tokens INTEGER NOT NULL DEFAULT 0,
                reasoning_tokens INTEGER NOT NULL DEFAULT 0,
                finish_reason TEXT,
                latency_ms INTEGER,
                cancelled INTEGER NOT NULL DEFAULT 0,
                status_code INTEGER
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create usage_records table");

        pool
    }

    fn create_user_input(external_id: &str, email: Option<&str>, name: Option<&str>) -> CreateUser {
        CreateUser {
            external_id: external_id.to_string(),
            email: email.map(|e| e.to_string()),
            name: name.map(|n| n.to_string()),
        }
    }

    async fn create_org(pool: &SqlitePool, slug: &str) -> Uuid {
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
        .bind(format!("{} Org", slug))
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("Failed to create org");
        id
    }

    async fn create_project(pool: &SqlitePool, org_id: Uuid, slug: &str) -> Uuid {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            r#"
            INSERT INTO projects (id, org_id, slug, name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(slug)
        .bind(format!("{} Project", slug))
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("Failed to create project");
        id
    }

    // ==================== User CRUD Tests ====================

    #[tokio::test]
    async fn test_create_user() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("user-123", Some("test@example.com"), Some("Test User"));
        let user = repo.create(input).await.expect("Failed to create user");

        assert_eq!(user.external_id, "user-123");
        assert_eq!(user.email, Some("test@example.com".to_string()));
        assert_eq!(user.name, Some("Test User".to_string()));
        assert!(!user.id.is_nil());
    }

    #[tokio::test]
    async fn test_create_user_minimal() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("user-minimal", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        assert_eq!(user.external_id, "user-minimal");
        assert!(user.email.is_none());
        assert!(user.name.is_none());
    }

    #[tokio::test]
    async fn test_create_duplicate_external_id_fails() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let input1 = create_user_input("duplicate-id", Some("first@example.com"), None);
        repo.create(input1)
            .await
            .expect("Failed to create first user");

        let input2 = create_user_input("duplicate-id", Some("second@example.com"), None);
        let result = repo.create(input2).await;

        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("get-test", Some("get@example.com"), Some("Get Test"));
        let created = repo.create(input).await.expect("Failed to create user");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get user")
            .expect("User should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.external_id, "get-test");
        assert_eq!(fetched.email, Some("get@example.com".to_string()));
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let result = repo
            .get_by_id(Uuid::new_v4())
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_external_id() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("ext-id-test", Some("ext@example.com"), None);
        let created = repo.create(input).await.expect("Failed to create user");

        let fetched = repo
            .get_by_external_id("ext-id-test")
            .await
            .expect("Failed to get user")
            .expect("User should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.external_id, "ext-id-test");
    }

    #[tokio::test]
    async fn test_get_by_external_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let result = repo
            .get_by_external_id("nonexistent")
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let result = repo
            .list(ListParams::default())
            .await
            .expect("Failed to list users");

        assert!(result.items.is_empty());
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_with_users() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        for i in 0..3 {
            let input = create_user_input(&format!("user-{}", i), None, None);
            repo.create(input).await.expect("Failed to create user");
        }

        let result = repo
            .list(ListParams::default())
            .await
            .expect("Failed to list users");

        assert_eq!(result.items.len(), 3);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_with_pagination() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        for i in 0..5 {
            let input = create_user_input(&format!("user-{}", i), None, None);
            repo.create(input).await.expect("Failed to create user");
        }

        // First page (no cursor)
        let page1 = repo
            .list(ListParams {
                limit: Some(2),
                include_deleted: false,
                ..Default::default()
            })
            .await
            .expect("Failed to list page 1");

        // Second page (using cursor from first page)
        let page2 = repo
            .list(ListParams {
                limit: Some(2),
                include_deleted: false,
                cursor: page1.cursors.next.clone(),
                ..Default::default()
            })
            .await
            .expect("Failed to list page 2");

        assert_eq!(page1.items.len(), 2);
        assert_eq!(page2.items.len(), 2);
        assert!(page1.has_more);
        assert!(page2.has_more);
        // Pages should have different users
        assert_ne!(page1.items[0].id, page2.items[0].id);
    }

    #[tokio::test]
    async fn test_count_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let count = repo.count(false).await.expect("Failed to count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_count_with_users() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        for i in 0..3 {
            let input = create_user_input(&format!("user-{}", i), None, None);
            repo.create(input).await.expect("Failed to create user");
        }

        let count = repo.count(false).await.expect("Failed to count");
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_update_email() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("update-email", Some("old@example.com"), None);
        let created = repo.create(input).await.expect("Failed to create user");

        let updated = repo
            .update(
                created.id,
                UpdateUser {
                    email: Some("new@example.com".to_string()),
                    name: None,
                },
            )
            .await
            .expect("Failed to update user");

        assert_eq!(updated.email, Some("new@example.com".to_string()));
        assert!(updated.updated_at >= created.updated_at);
    }

    #[tokio::test]
    async fn test_update_name() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("update-name", None, Some("Old Name"));
        let created = repo.create(input).await.expect("Failed to create user");

        let updated = repo
            .update(
                created.id,
                UpdateUser {
                    email: None,
                    name: Some("New Name".to_string()),
                },
            )
            .await
            .expect("Failed to update user");

        assert_eq!(updated.name, Some("New Name".to_string()));
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let result = repo
            .update(
                Uuid::new_v4(),
                UpdateUser {
                    email: Some("new@example.com".to_string()),
                    name: None,
                },
            )
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    // ==================== Organization Membership Tests ====================

    #[tokio::test]
    async fn test_add_to_org() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("org-member", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        repo.add_to_org(user.id, org_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to org");

        let result = repo
            .list_org_members(org_id, ListParams::default())
            .await
            .expect("Failed to list org members");

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].id, user.id);
    }

    #[tokio::test]
    async fn test_add_to_org_duplicate_fails() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("org-member", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        repo.add_to_org(user.id, org_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to org");

        let result = repo
            .add_to_org(user.id, org_id, "member", MembershipSource::Manual)
            .await;
        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_remove_from_org() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("org-member", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        repo.add_to_org(user.id, org_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to org");

        repo.remove_from_org(user.id, org_id)
            .await
            .expect("Failed to remove user from org");

        let result = repo
            .list_org_members(org_id, ListParams::default())
            .await
            .expect("Failed to list org members");

        assert!(result.items.is_empty());
    }

    #[tokio::test]
    async fn test_remove_from_org_not_member() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("not-member", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        let result = repo.remove_from_org(user.id, org_id).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_list_org_members_with_pagination() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let repo = SqliteUserRepo::new(pool);

        for i in 0..5 {
            let input = create_user_input(&format!("member-{}", i), None, None);
            let user = repo.create(input).await.expect("Failed to create user");
            repo.add_to_org(user.id, org_id, "member", MembershipSource::Manual)
                .await
                .expect("Failed to add user to org");
        }

        let page1 = repo
            .list_org_members(
                org_id,
                ListParams {
                    limit: Some(2),
                    include_deleted: false,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 1");

        let page2 = repo
            .list_org_members(
                org_id,
                ListParams {
                    limit: Some(2),
                    include_deleted: false,
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
    async fn test_count_org_members() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let repo = SqliteUserRepo::new(pool);

        for i in 0..3 {
            let input = create_user_input(&format!("member-{}", i), None, None);
            let user = repo.create(input).await.expect("Failed to create user");
            repo.add_to_org(user.id, org_id, "member", MembershipSource::Manual)
                .await
                .expect("Failed to add user to org");
        }

        let count = repo
            .count_org_members(org_id, false)
            .await
            .expect("Failed to count org members");
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_org_members_isolated_by_org() {
        let pool = create_test_pool().await;
        let org_id_1 = create_org(&pool, "org-1").await;
        let org_id_2 = create_org(&pool, "org-2").await;
        let repo = SqliteUserRepo::new(pool);

        // Add 2 users to org 1
        for i in 0..2 {
            let input = create_user_input(&format!("org1-user-{}", i), None, None);
            let user = repo.create(input).await.expect("Failed to create user");
            repo.add_to_org(user.id, org_id_1, "member", MembershipSource::Manual)
                .await
                .expect("Failed to add user to org");
        }

        // Add 3 users to org 2
        for i in 0..3 {
            let input = create_user_input(&format!("org2-user-{}", i), None, None);
            let user = repo.create(input).await.expect("Failed to create user");
            repo.add_to_org(user.id, org_id_2, "member", MembershipSource::Manual)
                .await
                .expect("Failed to add user to org");
        }

        let count1 = repo
            .count_org_members(org_id_1, false)
            .await
            .expect("Failed to count");
        let count2 = repo
            .count_org_members(org_id_2, false)
            .await
            .expect("Failed to count");

        assert_eq!(count1, 2);
        assert_eq!(count2, 3);
    }

    // ==================== Project Membership Tests ====================

    #[tokio::test]
    async fn test_add_to_project() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let project_id = create_project(&pool, org_id, "test-project").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("project-member", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        repo.add_to_project(user.id, project_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to project");

        let result = repo
            .list_project_members(project_id, ListParams::default())
            .await
            .expect("Failed to list project members");

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].id, user.id);
    }

    #[tokio::test]
    async fn test_add_to_project_duplicate_fails() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let project_id = create_project(&pool, org_id, "test-project").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("project-member", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        repo.add_to_project(user.id, project_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to project");

        let result = repo
            .add_to_project(user.id, project_id, "member", MembershipSource::Manual)
            .await;
        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_remove_from_project() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let project_id = create_project(&pool, org_id, "test-project").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("project-member", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        repo.add_to_project(user.id, project_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to project");

        repo.remove_from_project(user.id, project_id)
            .await
            .expect("Failed to remove user from project");

        let result = repo
            .list_project_members(project_id, ListParams::default())
            .await
            .expect("Failed to list project members");

        assert!(result.items.is_empty());
    }

    #[tokio::test]
    async fn test_remove_from_project_not_member() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let project_id = create_project(&pool, org_id, "test-project").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("not-member", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        let result = repo.remove_from_project(user.id, project_id).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_list_project_members_with_pagination() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let project_id = create_project(&pool, org_id, "test-project").await;
        let repo = SqliteUserRepo::new(pool);

        for i in 0..5 {
            let input = create_user_input(&format!("member-{}", i), None, None);
            let user = repo.create(input).await.expect("Failed to create user");
            repo.add_to_project(user.id, project_id, "member", MembershipSource::Manual)
                .await
                .expect("Failed to add user to project");
        }

        let page1 = repo
            .list_project_members(
                project_id,
                ListParams {
                    limit: Some(2),
                    include_deleted: false,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 1");

        let page2 = repo
            .list_project_members(
                project_id,
                ListParams {
                    limit: Some(2),
                    include_deleted: false,
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
    async fn test_count_project_members() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let project_id = create_project(&pool, org_id, "test-project").await;
        let repo = SqliteUserRepo::new(pool);

        for i in 0..3 {
            let input = create_user_input(&format!("member-{}", i), None, None);
            let user = repo.create(input).await.expect("Failed to create user");
            repo.add_to_project(user.id, project_id, "member", MembershipSource::Manual)
                .await
                .expect("Failed to add user to project");
        }

        let count = repo
            .count_project_members(project_id, false)
            .await
            .expect("Failed to count project members");
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_project_members_isolated_by_project() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let project_id_1 = create_project(&pool, org_id, "project-1").await;
        let project_id_2 = create_project(&pool, org_id, "project-2").await;
        let repo = SqliteUserRepo::new(pool);

        // Add 2 users to project 1
        for i in 0..2 {
            let input = create_user_input(&format!("p1-user-{}", i), None, None);
            let user = repo.create(input).await.expect("Failed to create user");
            repo.add_to_project(user.id, project_id_1, "member", MembershipSource::Manual)
                .await
                .expect("Failed to add user to project");
        }

        // Add 3 users to project 2
        for i in 0..3 {
            let input = create_user_input(&format!("p2-user-{}", i), None, None);
            let user = repo.create(input).await.expect("Failed to create user");
            repo.add_to_project(user.id, project_id_2, "member", MembershipSource::Manual)
                .await
                .expect("Failed to add user to project");
        }

        let count1 = repo
            .count_project_members(project_id_1, false)
            .await
            .expect("Failed to count");
        let count2 = repo
            .count_project_members(project_id_2, false)
            .await
            .expect("Failed to count");

        assert_eq!(count1, 2);
        assert_eq!(count2, 3);
    }

    // ==================== Single-Org Enforcement Tests ====================

    #[tokio::test]
    async fn test_add_to_second_org_fails() {
        let pool = create_test_pool().await;
        let org_id_1 = create_org(&pool, "org-1").await;
        let org_id_2 = create_org(&pool, "org-2").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("single-org-user", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        // Adding to first org should succeed
        repo.add_to_org(user.id, org_id_1, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add to org 1");

        // Adding to second org should fail with single-org constraint violation
        let result = repo
            .add_to_org(user.id, org_id_2, "member", MembershipSource::Manual)
            .await;
        match &result {
            Err(DbError::Conflict(msg)) => {
                assert!(
                    msg.contains("another organization"),
                    "Expected 'another organization' in error: {msg}"
                );
            }
            other => panic!("Expected Conflict error, got: {other:?}"),
        }

        // Verify user is only in the first org
        let count1 = repo
            .count_org_members(org_id_1, false)
            .await
            .expect("Failed to count");
        let count2 = repo
            .count_org_members(org_id_2, false)
            .await
            .expect("Failed to count");

        assert_eq!(count1, 1);
        assert_eq!(count2, 0);
    }

    #[tokio::test]
    async fn test_add_to_same_org_twice_fails() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("double-add-user", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        // First add should succeed
        repo.add_to_org(user.id, org_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add to org");

        // Second add to same org should fail with conflict
        // Note: SQLite may report either the primary key or single-org constraint violation
        let result = repo
            .add_to_org(user.id, org_id, "member", MembershipSource::Manual)
            .await;
        assert!(
            matches!(result, Err(DbError::Conflict(_))),
            "Expected Conflict error for duplicate add"
        );
    }

    // ==================== Org Membership Source Sync Tests ====================

    #[tokio::test]
    async fn test_sync_removes_only_jit_org_memberships() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let repo = SqliteUserRepo::new(pool);

        // Create users with different membership sources
        let jit_user = repo
            .create(create_user_input("jit-user", None, None))
            .await
            .expect("Failed to create JIT user");

        let manual_user = repo
            .create(create_user_input("manual-user", None, None))
            .await
            .expect("Failed to create manual user");

        let scim_user = repo
            .create(create_user_input("scim-user", None, None))
            .await
            .expect("Failed to create SCIM user");

        // Add all users to org with different sources
        repo.add_to_org(jit_user.id, org_id, "member", MembershipSource::Jit)
            .await
            .expect("Failed to add JIT user");
        repo.add_to_org(manual_user.id, org_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add manual user");
        repo.add_to_org(scim_user.id, org_id, "member", MembershipSource::Scim)
            .await
            .expect("Failed to add SCIM user");

        // Sync removes JIT membership not in the except list
        let removed = repo
            .remove_org_memberships_by_source(jit_user.id, MembershipSource::Jit, &[])
            .await
            .expect("Failed to remove org memberships");

        assert_eq!(removed, 1, "Should remove exactly 1 JIT org membership");

        // Check remaining memberships
        let jit_memberships = repo
            .get_org_memberships_for_user(jit_user.id)
            .await
            .expect("Query failed");
        assert!(
            jit_memberships.is_empty(),
            "JIT user should have no org memberships"
        );

        let manual_memberships = repo
            .get_org_memberships_for_user(manual_user.id)
            .await
            .expect("Query failed");
        assert_eq!(
            manual_memberships.len(),
            1,
            "Manual user should still have org membership"
        );

        let scim_memberships = repo
            .get_org_memberships_for_user(scim_user.id)
            .await
            .expect("Query failed");
        assert_eq!(
            scim_memberships.len(),
            1,
            "SCIM user should still have org membership"
        );
    }

    #[tokio::test]
    async fn test_sync_preserves_jit_org_membership_in_except_list() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let repo = SqliteUserRepo::new(pool);

        let user = repo
            .create(create_user_input("test-user", None, None))
            .await
            .expect("Failed to create user");

        // Add user to org with JIT source
        repo.add_to_org(user.id, org_id, "member", MembershipSource::Jit)
            .await
            .expect("Failed to add to org");

        // Sync with org in the except list (should preserve membership)
        let removed = repo
            .remove_org_memberships_by_source(user.id, MembershipSource::Jit, &[org_id])
            .await
            .expect("Failed to remove memberships");

        assert_eq!(removed, 0, "Should not remove org in except list");

        // User should still be a member
        let memberships = repo
            .get_org_memberships_for_user(user.id)
            .await
            .expect("Query failed");
        assert_eq!(
            memberships.len(),
            1,
            "User should still have org membership"
        );
    }

    #[tokio::test]
    async fn test_user_can_be_in_multiple_projects() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let project_id_1 = create_project(&pool, org_id, "project-1").await;
        let project_id_2 = create_project(&pool, org_id, "project-2").await;
        let repo = SqliteUserRepo::new(pool);

        let input = create_user_input("multi-project-user", None, None);
        let user = repo.create(input).await.expect("Failed to create user");

        repo.add_to_project(user.id, project_id_1, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add to project 1");
        repo.add_to_project(user.id, project_id_2, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add to project 2");

        let count1 = repo
            .count_project_members(project_id_1, false)
            .await
            .expect("Failed to count");
        let count2 = repo
            .count_project_members(project_id_2, false)
            .await
            .expect("Failed to count");

        assert_eq!(count1, 1);
        assert_eq!(count2, 1);
    }

    // ==================== GDPR Deletion Tests ====================

    #[tokio::test]
    async fn test_hard_delete_user() {
        let pool = create_test_pool().await;
        let org_id = create_org(&pool, "test-org").await;
        let project_id = create_project(&pool, org_id, "test-project").await;
        let repo = SqliteUserRepo::new(pool);

        // Create user
        let input = create_user_input("delete-user", Some("delete@example.com"), Some("Delete Me"));
        let user = repo.create(input).await.expect("Failed to create user");

        // Add to org and project
        repo.add_to_org(user.id, org_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add to org");
        repo.add_to_project(user.id, project_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add to project");

        // Hard delete user
        let result = repo
            .hard_delete(user.id)
            .await
            .expect("Failed to hard delete user");

        assert!(result.user_deleted);

        // Verify user is gone
        let fetched = repo.get_by_id(user.id).await.expect("Query should succeed");
        assert!(fetched.is_none());

        // Verify memberships are gone (cascade delete)
        let org_count = repo
            .count_org_members(org_id, false)
            .await
            .expect("Failed to count");
        assert_eq!(org_count, 0);

        let project_count = repo
            .count_project_members(project_id, false)
            .await
            .expect("Failed to count");
        assert_eq!(project_count, 0);
    }

    #[tokio::test]
    async fn test_hard_delete_user_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteUserRepo::new(pool);

        let result = repo.hard_delete(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }
}
