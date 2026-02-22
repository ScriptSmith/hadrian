use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

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

pub struct PostgresUserRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresUserRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
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
            WHERE ROW(created_at, id) {} ROW($1, $2)
            ORDER BY created_at {}, id {}
            LIMIT $3
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| User {
                id: row.get("id"),
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

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
            WHERE om.org_id = $1
            AND ROW(u.created_at, u.id) {} ROW($2, $3)
            ORDER BY u.created_at {}, u.id {}
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
        let mut items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| User {
                id: row.get("id"),
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

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
            WHERE pm.project_id = $1
            AND ROW(u.created_at, u.id) {} ROW($2, $3)
            ORDER BY u.created_at {}, u.id {}
            LIMIT $4
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(project_id)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| User {
                id: row.get("id"),
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

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
impl UserRepo for PostgresUserRepo {
    async fn create(&self, input: CreateUser) -> DbResult<User> {
        let row = sqlx::query(
            r#"
            INSERT INTO users (id, external_id, email, name)
            VALUES ($1, $2, $3, $4)
            RETURNING id, external_id, email, name, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(&input.external_id)
        .bind(&input.email)
        .bind(&input.name)
        .fetch_one(&self.write_pool)
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
            id: row.get("id"),
            external_id: row.get("external_id"),
            email: row.get("email"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<User>> {
        let result = sqlx::query(
            r#"
            SELECT id, external_id, email, name, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| User {
            id: row.get("id"),
            external_id: row.get("external_id"),
            email: row.get("email"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn get_by_external_id(&self, external_id: &str) -> DbResult<Option<User>> {
        let result = sqlx::query(
            r#"
            SELECT id, external_id, email, name, created_at, updated_at
            FROM users
            WHERE external_id = $1
            "#,
        )
        .bind(external_id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| User {
            id: row.get("id"),
            external_id: row.get("external_id"),
            email: row.get("email"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
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
            LIMIT $1
            "#,
        )
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| User {
                id: row.get("id"),
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |user| {
                cursor_from_row(user.created_at, user.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count(&self, _include_deleted: bool) -> DbResult<i64> {
        // Users table doesn't have soft delete
        let row = sqlx::query("SELECT COUNT(*) as count FROM users")
            .fetch_one(&self.read_pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateUser) -> DbResult<User> {
        let mut updates = vec![];
        let mut param_count = 1;

        if input.email.is_some() {
            updates.push(format!("email = ${}", param_count));
            param_count += 1;
        }
        if input.name.is_some() {
            updates.push(format!("name = ${}", param_count));
            param_count += 1;
        }

        if updates.is_empty() {
            return self.get_by_id(id).await?.ok_or(DbError::NotFound);
        }

        let query_str = format!(
            "UPDATE users SET {}, updated_at = NOW() WHERE id = ${} RETURNING id, external_id, email, name, created_at, updated_at",
            updates.join(", "),
            param_count
        );

        let mut query = sqlx::query(&query_str);
        if let Some(ref email) = input.email {
            query = query.bind(email);
        }
        if let Some(ref name) = input.name {
            query = query.bind(name);
        }
        query = query.bind(id);

        let row = query
            .fetch_optional(&self.write_pool)
            .await?
            .ok_or(DbError::NotFound)?;

        Ok(User {
            id: row.get("id"),
            external_id: row.get("external_id"),
            email: row.get("email"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn add_to_org(
        &self,
        user_id: Uuid,
        org_id: Uuid,
        role: &str,
        source: MembershipSource,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO org_memberships (org_id, user_id, role, source)
            VALUES ($1, $2, $3, $4::membership_source)
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .bind(role)
        .bind(source.as_str())
        .execute(&self.write_pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                // Check if this is the single-org constraint (idx_org_memberships_single_org)
                // or the primary key constraint (user already in this org)
                let constraint = db_err.constraint();
                if constraint == Some("idx_org_memberships_single_org") {
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
                DELETE FROM org_memberships
                WHERE user_id = $1 AND source = $2::membership_source AND org_id != ALL($3)
                "#,
            )
            .bind(user_id)
            .bind(source.as_str())
            .bind(except_org_ids)
            .execute(&self.write_pool)
            .await?
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
            SET role = $1
            WHERE org_id = $2 AND user_id = $3
            "#,
        )
        .bind(role)
        .bind(org_id)
        .bind(user_id)
        .execute(&self.write_pool)
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
            WHERE org_id = $1 AND user_id = $2
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .execute(&self.write_pool)
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
            WHERE om.org_id = $1
            ORDER BY u.created_at DESC, u.id DESC
            LIMIT $2
            "#,
        )
        .bind(org_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| User {
                id: row.get("id"),
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |user| {
                cursor_from_row(user.created_at, user.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_org_members(&self, org_id: Uuid, _include_deleted: bool) -> DbResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM org_memberships WHERE org_id = $1")
            .bind(org_id)
            .fetch_one(&self.read_pool)
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
        sqlx::query(
            r#"
            INSERT INTO project_memberships (project_id, user_id, role, source)
            VALUES ($1, $2, $3, $4::membership_source)
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .bind(role)
        .bind(source.as_str())
        .execute(&self.write_pool)
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
                DELETE FROM project_memberships
                WHERE user_id = $1 AND source = $2::membership_source AND project_id != ALL($3)
                "#,
            )
            .bind(user_id)
            .bind(source.as_str())
            .bind(except_project_ids)
            .execute(&self.write_pool)
            .await?
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
            SET role = $1
            WHERE project_id = $2 AND user_id = $3
            "#,
        )
        .bind(role)
        .bind(project_id)
        .bind(user_id)
        .execute(&self.write_pool)
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
            WHERE project_id = $1 AND user_id = $2
            "#,
        )
        .bind(project_id)
        .bind(user_id)
        .execute(&self.write_pool)
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
            WHERE pm.project_id = $1
            ORDER BY u.created_at DESC, u.id DESC
            LIMIT $2
            "#,
        )
        .bind(project_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<User> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| User {
                id: row.get("id"),
                external_id: row.get("external_id"),
                email: row.get("email"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

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
            sqlx::query("SELECT COUNT(*) as count FROM project_memberships WHERE project_id = $1")
                .bind(project_id)
                .fetch_one(&self.read_pool)
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
            WHERE om.user_id = $1
            ORDER BY om.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let source_str: String = row.get("source");
                UserOrgMembership {
                    org_id: row.get("org_id"),
                    org_slug: row.get("org_slug"),
                    org_name: row.get("org_name"),
                    role: row.get("role"),
                    source: MembershipSource::from_str(&source_str).unwrap_or_default(),
                    joined_at: row.get("joined_at"),
                }
            })
            .collect())
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
            WHERE pm.user_id = $1
            ORDER BY pm.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let source_str: String = row.get("source");
                UserProjectMembership {
                    project_id: row.get("project_id"),
                    project_slug: row.get("project_slug"),
                    project_name: row.get("project_name"),
                    org_id: row.get("org_id"),
                    role: row.get("role"),
                    source: MembershipSource::from_str(&source_str).unwrap_or_default(),
                    joined_at: row.get("joined_at"),
                }
            })
            .collect())
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
            WHERE tm.user_id = $1 AND t.deleted_at IS NULL
            ORDER BY tm.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let source_str: String = row.get("source");
                TeamMembership {
                    team_id: row.get("team_id"),
                    team_slug: row.get("team_slug"),
                    team_name: row.get("team_name"),
                    org_id: row.get("org_id"),
                    role: row.get("role"),
                    source: MembershipSource::from_str(&source_str).unwrap_or_default(),
                    joined_at: row.get("joined_at"),
                }
            })
            .collect())
    }

    // ==================== GDPR Deletion Methods ====================

    async fn hard_delete(&self, user_id: Uuid) -> DbResult<UserDeletionResult> {
        let mut result = UserDeletionResult::default();

        // Delete usage records for user's API keys first (they reference api_keys)
        let usage_result = sqlx::query(
            r#"
            DELETE FROM usage_records
            WHERE api_key_id IN (
                SELECT id FROM api_keys
                WHERE owner_type = 'user' AND owner_id = $1::text
            )
            "#,
        )
        .bind(user_id)
        .execute(&self.write_pool)
        .await?;
        result.usage_records_deleted = usage_result.rows_affected();

        // Delete API keys owned by user
        let api_keys_result = sqlx::query(
            r#"
            DELETE FROM api_keys
            WHERE owner_type = 'user' AND owner_id = $1::text
            "#,
        )
        .bind(user_id)
        .execute(&self.write_pool)
        .await?;
        result.api_keys_deleted = api_keys_result.rows_affected();

        // Delete conversations owned by user
        let conversations_result = sqlx::query(
            r#"
            DELETE FROM conversations
            WHERE owner_type = 'user' AND owner_id = $1::text
            "#,
        )
        .bind(user_id)
        .execute(&self.write_pool)
        .await?;
        result.conversations_deleted = conversations_result.rows_affected();

        // Delete dynamic providers owned by user
        let providers_result = sqlx::query(
            r#"
            DELETE FROM dynamic_providers
            WHERE owner_type = 'user' AND owner_id = $1::text
            "#,
        )
        .bind(user_id)
        .execute(&self.write_pool)
        .await?;
        result.dynamic_providers_deleted = providers_result.rows_affected();

        // Delete user (org_memberships and project_memberships cascade automatically)
        let user_result = sqlx::query(
            r#"
            DELETE FROM users WHERE id = $1
            "#,
        )
        .bind(user_id)
        .execute(&self.write_pool)
        .await?;

        if user_result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        result.user_deleted = true;
        Ok(result)
    }
}
