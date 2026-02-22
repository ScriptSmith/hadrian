use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, SsoGroupMappingRepo,
            cursor_from_row,
        },
    },
    models::{CreateSsoGroupMapping, SsoGroupMapping, UpdateSsoGroupMapping},
};

pub struct PostgresSsoGroupMappingRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresSsoGroupMappingRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    /// Helper method for cursor-based pagination.
    async fn list_with_cursor(
        &self,
        org_id: Uuid,
        sso_connection_name: Option<&str>,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<SsoGroupMapping>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let (query, needs_connection_bind) = if sso_connection_name.is_some() {
            (
                format!(
                    r#"
                    SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
                    FROM sso_group_mappings
                    WHERE org_id = $1 AND sso_connection_name = $2 AND ROW(created_at, id) {} ROW($3, $4)
                    ORDER BY created_at {}, id {}
                    LIMIT $5
                    "#,
                    comparison, order, order
                ),
                true,
            )
        } else {
            (
                format!(
                    r#"
                    SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
                    FROM sso_group_mappings
                    WHERE org_id = $1 AND ROW(created_at, id) {} ROW($2, $3)
                    ORDER BY created_at {}, id {}
                    LIMIT $4
                    "#,
                    comparison, order, order
                ),
                false,
            )
        };

        let rows = if needs_connection_bind {
            sqlx::query(&query)
                .bind(org_id)
                .bind(sso_connection_name.unwrap())
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
                .await?
        } else {
            sqlx::query(&query)
                .bind(org_id)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
                .await?
        };

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<SsoGroupMapping> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| SsoGroupMapping {
                id: row.get("id"),
                sso_connection_name: row.get("sso_connection_name"),
                idp_group: row.get("idp_group"),
                org_id: row.get("org_id"),
                team_id: row.get("team_id"),
                role: row.get("role"),
                priority: row.get("priority"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |m| {
                cursor_from_row(m.created_at, m.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[async_trait]
impl SsoGroupMappingRepo for PostgresSsoGroupMappingRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateSsoGroupMapping,
    ) -> DbResult<SsoGroupMapping> {
        let row = sqlx::query(
            r#"
            INSERT INTO sso_group_mappings (id, sso_connection_name, idp_group, org_id, team_id, role, priority)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(&input.sso_connection_name)
        .bind(&input.idp_group)
        .bind(org_id)
        .bind(input.team_id)
        .bind(&input.role)
        .bind(input.priority)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Mapping for IdP group '{}' already exists for this connection/org/team combination",
                    input.idp_group
                ))
            }
            _ => DbError::from(e),
        })?;

        Ok(SsoGroupMapping {
            id: row.get("id"),
            sso_connection_name: row.get("sso_connection_name"),
            idp_group: row.get("idp_group"),
            org_id: row.get("org_id"),
            team_id: row.get("team_id"),
            role: row.get("role"),
            priority: row.get("priority"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<SsoGroupMapping>> {
        let result = sqlx::query(
            r#"
            SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            FROM sso_group_mappings
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| SsoGroupMapping {
            id: row.get("id"),
            sso_connection_name: row.get("sso_connection_name"),
            idp_group: row.get("idp_group"),
            org_id: row.get("org_id"),
            team_id: row.get("team_id"),
            role: row.get("role"),
            priority: row.get("priority"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<SsoGroupMapping>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(org_id, None, &params, cursor, fetch_limit, limit)
                .await;
        }

        let rows = sqlx::query(
            r#"
            SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            FROM sso_group_mappings
            WHERE org_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(org_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<SsoGroupMapping> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| SsoGroupMapping {
                id: row.get("id"),
                sso_connection_name: row.get("sso_connection_name"),
                idp_group: row.get("idp_group"),
                org_id: row.get("org_id"),
                team_id: row.get("team_id"),
                role: row.get("role"),
                priority: row.get("priority"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |m| {
                cursor_from_row(m.created_at, m.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn list_by_connection(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<SsoGroupMapping>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(
                    org_id,
                    Some(sso_connection_name),
                    &params,
                    cursor,
                    fetch_limit,
                    limit,
                )
                .await;
        }

        let rows = sqlx::query(
            r#"
            SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            FROM sso_group_mappings
            WHERE sso_connection_name = $1 AND org_id = $2
            ORDER BY created_at DESC, id DESC
            LIMIT $3
            "#,
        )
        .bind(sso_connection_name)
        .bind(org_id)
        .bind(fetch_limit)
        .fetch_all(&self.read_pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<SsoGroupMapping> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| SsoGroupMapping {
                id: row.get("id"),
                sso_connection_name: row.get("sso_connection_name"),
                idp_group: row.get("idp_group"),
                org_id: row.get("org_id"),
                team_id: row.get("team_id"),
                role: row.get("role"),
                priority: row.get("priority"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |m| {
                cursor_from_row(m.created_at, m.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn find_mappings_for_groups(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        idp_groups: &[String],
    ) -> DbResult<Vec<SsoGroupMapping>> {
        if idp_groups.is_empty() {
            return Ok(Vec::new());
        }

        // Use ANY($3) for array-based IN clause in PostgreSQL
        let rows = sqlx::query(
            r#"
            SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            FROM sso_group_mappings
            WHERE sso_connection_name = $1 AND org_id = $2 AND idp_group = ANY($3)
            ORDER BY priority DESC, idp_group, created_at
            "#,
        )
        .bind(sso_connection_name)
        .bind(org_id)
        .bind(idp_groups)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| SsoGroupMapping {
                id: row.get("id"),
                sso_connection_name: row.get("sso_connection_name"),
                idp_group: row.get("idp_group"),
                org_id: row.get("org_id"),
                team_id: row.get("team_id"),
                role: row.get("role"),
                priority: row.get("priority"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM sso_group_mappings WHERE org_id = $1")
            .bind(org_id)
            .fetch_one(&self.read_pool)
            .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateSsoGroupMapping) -> DbResult<SsoGroupMapping> {
        let has_idp_group = input.idp_group.is_some();
        let has_team_id = input.team_id.is_some();
        let has_role = input.role.is_some();
        let has_priority = input.priority.is_some();

        if !has_idp_group && !has_team_id && !has_role && !has_priority {
            return self.get_by_id(id).await?.ok_or(DbError::NotFound);
        }

        // Build dynamic UPDATE query based on which fields are provided
        let mut set_clauses: Vec<String> = vec!["updated_at = NOW()".to_string()];
        let mut param_index = 1;

        if has_idp_group {
            set_clauses.push(format!("idp_group = ${}", param_index));
            param_index += 1;
        }
        if has_team_id {
            set_clauses.push(format!("team_id = ${}", param_index));
            param_index += 1;
        }
        if has_role {
            set_clauses.push(format!("role = ${}", param_index));
            param_index += 1;
        }
        if has_priority {
            set_clauses.push(format!("priority = ${}", param_index));
            param_index += 1;
        }

        let query = format!(
            r#"
            UPDATE sso_group_mappings
            SET {}
            WHERE id = ${}
            RETURNING id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            "#,
            set_clauses.join(", "),
            param_index
        );

        // Use sqlx::query_scalar approach with dynamic binding
        // We build the query dynamically and bind values in order
        let mut query_builder = sqlx::query(&query);

        if let Some(ref idp_group) = input.idp_group {
            query_builder = query_builder.bind(idp_group);
        }
        if let Some(team_id) = input.team_id {
            query_builder = query_builder.bind(team_id);
        }
        if let Some(ref role) = input.role {
            query_builder = query_builder.bind(role);
        }
        if let Some(priority) = input.priority {
            query_builder = query_builder.bind(priority);
        }
        query_builder = query_builder.bind(id);

        let row = query_builder
            .fetch_optional(&self.write_pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                    DbError::Conflict("Mapping with this combination already exists".into())
                }
                _ => DbError::from(e),
            })?
            .ok_or(DbError::NotFound)?;

        Ok(SsoGroupMapping {
            id: row.get("id"),
            sso_connection_name: row.get("sso_connection_name"),
            idp_group: row.get("idp_group"),
            org_id: row.get("org_id"),
            team_id: row.get("team_id"),
            role: row.get("role"),
            priority: row.get("priority"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM sso_group_mappings WHERE id = $1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn delete_by_idp_group(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        idp_group: &str,
    ) -> DbResult<u64> {
        let result = sqlx::query(
            "DELETE FROM sso_group_mappings WHERE sso_connection_name = $1 AND org_id = $2 AND idp_group = $3",
        )
        .bind(sso_connection_name)
        .bind(org_id)
        .bind(idp_group)
        .execute(&self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }
}
