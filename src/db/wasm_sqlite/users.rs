use async_trait::async_trait;
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, UserDeletionResult,
            UserRepo, cursor_from_row,
        },
        wasm_sqlite::{WasmSqlitePool, query as wasm_query},
    },
    models::{
        CreateUser, MembershipSource, TeamMembership, UpdateUser, User, UserOrgMembership,
        UserProjectMembership,
    },
};

pub struct WasmSqliteUserRepo {
    pool: WasmSqlitePool,
}

impl WasmSqliteUserRepo {
    pub fn new(pool: WasmSqlitePool) -> Self {
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

        let rows = wasm_query(&query)
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
                    id: parse_uuid(&row.get::<String>("id"))?,
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

        let rows = wasm_query(&query)
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
                    id: parse_uuid(&row.get::<String>("id"))?,
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

        let rows = wasm_query(&query)
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
                    id: parse_uuid(&row.get::<String>("id"))?,
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

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl UserRepo for WasmSqliteUserRepo {
    async fn create(&self, input: CreateUser) -> DbResult<User> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        wasm_query(
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
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict(format!(
                    "User with external_id '{}' already exists",
                    input.external_id
                ))
            } else {
                DbError::from(e)
            }
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
        let result = wasm_query(
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
                id: parse_uuid(&row.get::<String>("id"))?,
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
        let result = wasm_query(
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
                id: parse_uuid(&row.get::<String>("id"))?,
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
        let rows = wasm_query(
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
                    id: parse_uuid(&row.get::<String>("id"))?,
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
        let row = wasm_query("SELECT COUNT(*) as count FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateUser) -> DbResult<User> {
        let now = chrono::Utc::now();

        let result = wasm_query(
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

        wasm_query(
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
        .map_err(|e| {
            if e.is_unique_violation() {
                // WasmDbError message format: "UNIQUE constraint failed: table.column1, table.column2"
                // Primary key (org_id, user_id) violation includes both columns
                // Single-org unique index (user_id only) violation includes just user_id
                let msg = e.to_string();
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
            } else {
                DbError::from(e)
            }
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
            wasm_query(
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
            let query_str = format!(
                r#"
                DELETE FROM org_memberships
                WHERE user_id = ? AND source = ? AND org_id NOT IN ({})
                "#,
                placeholders
            );
            let mut q = wasm_query(&query_str)
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
        let result = wasm_query(
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
        let result = wasm_query(
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
        let rows = wasm_query(
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
                    id: parse_uuid(&row.get::<String>("id"))?,
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
        let row = wasm_query("SELECT COUNT(*) as count FROM org_memberships WHERE org_id = ?")
            .bind(org_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64>("count"))
    }

    async fn add_to_project(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        role: &str,
        source: MembershipSource,
    ) -> DbResult<()> {
        let now = chrono::Utc::now();

        wasm_query(
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
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict("User is already a member of this project".to_string())
            } else {
                DbError::from(e)
            }
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
            wasm_query(
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
            let query_str = format!(
                r#"
                DELETE FROM project_memberships
                WHERE user_id = ? AND source = ? AND project_id NOT IN ({})
                "#,
                placeholders
            );
            let mut q = wasm_query(&query_str)
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
        let result = wasm_query(
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
        let result = wasm_query(
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
        let rows = wasm_query(
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
                    id: parse_uuid(&row.get::<String>("id"))?,
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
            wasm_query("SELECT COUNT(*) as count FROM project_memberships WHERE project_id = ?")
                .bind(project_id.to_string())
                .fetch_one(&self.pool)
                .await?;
        Ok(row.get::<i64>("count"))
    }

    // ==================== GDPR Export Methods ====================

    async fn get_org_memberships_for_user(
        &self,
        user_id: Uuid,
    ) -> DbResult<Vec<UserOrgMembership>> {
        let rows = wasm_query(
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
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
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
        let rows = wasm_query(
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
                    project_id: parse_uuid(&row.get::<String>("project_id"))?,
                    project_slug: row.get("project_slug"),
                    project_name: row.get("project_name"),
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
                    role: row.get("role"),
                    source: MembershipSource::from_str(&source_str).unwrap_or_default(),
                    joined_at: row.get("joined_at"),
                })
            })
            .collect()
    }

    async fn get_team_memberships_for_user(&self, user_id: Uuid) -> DbResult<Vec<TeamMembership>> {
        let rows = wasm_query(
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
                    team_id: parse_uuid(&row.get::<String>("team_id"))?,
                    team_slug: row.get("team_slug"),
                    team_name: row.get("team_name"),
                    org_id: parse_uuid(&row.get::<String>("org_id"))?,
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
        let usage_result = wasm_query(
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
        let api_keys_result = wasm_query(
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
        let conversations_result = wasm_query(
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
        let providers_result = wasm_query(
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
        let user_result = wasm_query(
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
