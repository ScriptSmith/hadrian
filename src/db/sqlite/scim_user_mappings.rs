//! SQLite implementation of the SCIM user mapping repository.

use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, ScimUserMappingRepo,
        },
    },
    models::{
        CreateScimUserMapping, ScimUserMapping, ScimUserWithMapping, UpdateScimUserMapping, User,
    },
    scim::filter_to_sql::{SqlFilter, SqlValue},
};

pub struct SqliteScimUserMappingRepo {
    pool: SqlitePool,
}

impl SqliteScimUserMappingRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Parse a ScimUserMapping from a database row.
    fn parse_mapping(row: &sqlx::sqlite::SqliteRow) -> DbResult<ScimUserMapping> {
        Ok(ScimUserMapping {
            id: parse_uuid(&row.get::<String, _>("id"))?,
            org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
            scim_external_id: row.get("scim_external_id"),
            user_id: parse_uuid(&row.get::<String, _>("user_id"))?,
            active: row.get::<i32, _>("active") != 0,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Parse a ScimUserWithMapping from a joined row with aliased columns.
    fn parse_mapping_with_user(row: &sqlx::sqlite::SqliteRow) -> DbResult<ScimUserWithMapping> {
        Ok(ScimUserWithMapping {
            mapping: ScimUserMapping {
                id: parse_uuid(&row.get::<String, _>("m_id"))?,
                org_id: parse_uuid(&row.get::<String, _>("m_org_id"))?,
                scim_external_id: row.get("m_scim_external_id"),
                user_id: parse_uuid(&row.get::<String, _>("m_user_id"))?,
                active: row.get::<i32, _>("m_active") != 0,
                created_at: row.get("m_created_at"),
                updated_at: row.get("m_updated_at"),
            },
            user: User {
                id: parse_uuid(&row.get::<String, _>("u_id"))?,
                external_id: row.get("u_external_id"),
                email: row.get("u_email"),
                name: row.get("u_name"),
                created_at: row.get("u_created_at"),
                updated_at: row.get("u_updated_at"),
            },
        })
    }
}

#[async_trait]
impl ScimUserMappingRepo for SqliteScimUserMappingRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateScimUserMapping,
    ) -> DbResult<ScimUserMapping> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO scim_user_mappings (
                id, org_id, scim_external_id, user_id, active, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(&input.scim_external_id)
        .bind(input.user_id.to_string())
        .bind(input.active as i32)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("SCIM external ID already mapped in this organization".into())
            }
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                DbError::Conflict("Referenced user or organization not found".into())
            }
            _ => DbError::from(e),
        })?;

        Ok(ScimUserMapping {
            id,
            org_id,
            scim_external_id: input.scim_external_id,
            user_id: input.user_id,
            active: input.active,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ScimUserMapping>> {
        let row = sqlx::query("SELECT * FROM scim_user_mappings WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        row.map(|r| Self::parse_mapping(&r)).transpose()
    }

    async fn get_by_scim_external_id(
        &self,
        org_id: Uuid,
        scim_external_id: &str,
    ) -> DbResult<Option<ScimUserMapping>> {
        let row = sqlx::query(
            "SELECT * FROM scim_user_mappings WHERE org_id = ? AND scim_external_id = ?",
        )
        .bind(org_id.to_string())
        .bind(scim_external_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| Self::parse_mapping(&r)).transpose()
    }

    async fn get_by_user_id(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> DbResult<Option<ScimUserMapping>> {
        let row = sqlx::query("SELECT * FROM scim_user_mappings WHERE org_id = ? AND user_id = ?")
            .bind(org_id.to_string())
            .bind(user_id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        row.map(|r| Self::parse_mapping(&r)).transpose()
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ScimUserMapping>> {
        let limit = params.limit.unwrap_or(100).min(1000);
        let fetch_limit = limit + 1; // Fetch one extra to detect has_more

        let (comparison_op, order_dir, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let rows = if let Some(cursor) = &params.cursor {
            let query = format!(
                r#"
                SELECT * FROM scim_user_mappings
                WHERE org_id = ?
                AND (created_at, id) {} (?, ?)
                ORDER BY created_at {}, id {}
                LIMIT ?
                "#,
                comparison_op, order_dir, order_dir
            );
            sqlx::query(&query)
                .bind(org_id.to_string())
                .bind(cursor.created_at)
                .bind(cursor.id.to_string())
                .bind(fetch_limit)
                .fetch_all(&self.pool)
                .await?
        } else {
            let query = format!(
                r#"
                SELECT * FROM scim_user_mappings
                WHERE org_id = ?
                ORDER BY created_at {}, id {}
                LIMIT ?
                "#,
                order_dir, order_dir
            );
            sqlx::query(&query)
                .bind(org_id.to_string())
                .bind(fetch_limit)
                .fetch_all(&self.pool)
                .await?
        };

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<ScimUserMapping> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_mapping)
            .collect::<DbResult<_>>()?;

        if should_reverse {
            items.reverse();
        }

        // Build cursors
        let cursors = if items.is_empty() {
            PageCursors {
                next: None,
                prev: None,
            }
        } else {
            let first = items.first().unwrap();
            let last = items.last().unwrap();
            PageCursors {
                next: if has_more || params.direction == CursorDirection::Backward {
                    Some(Cursor {
                        created_at: last.created_at,
                        id: last.id,
                    })
                } else {
                    None
                },
                prev: if params.cursor.is_some() || params.direction == CursorDirection::Backward {
                    Some(Cursor {
                        created_at: first.created_at,
                        id: first.id,
                    })
                } else {
                    None
                },
            }
        };

        Ok(ListResult {
            items,
            has_more,
            cursors,
        })
    }

    async fn list_by_org_filtered(
        &self,
        org_id: Uuid,
        filter: Option<&SqlFilter>,
        limit: i64,
        offset: i64,
    ) -> DbResult<(Vec<ScimUserWithMapping>, i64)> {
        // Build WHERE clause: always filter by org_id, optionally add SCIM filter
        let where_clause = if let Some(f) = filter {
            format!("m.org_id = ? AND ({})", f.where_clause)
        } else {
            "m.org_id = ?".to_string()
        };

        // Count query for totalResults
        let count_sql = format!(
            "SELECT COUNT(*) as cnt FROM scim_user_mappings m \
             JOIN users u ON m.user_id = u.id \
             WHERE {}",
            where_clause
        );

        // Data query with aliased columns to avoid ambiguity
        let data_sql = format!(
            r#"SELECT
                m.id as m_id, m.org_id as m_org_id, m.scim_external_id as m_scim_external_id,
                m.user_id as m_user_id, m.active as m_active, m.created_at as m_created_at,
                m.updated_at as m_updated_at,
                u.id as u_id, u.external_id as u_external_id, u.email as u_email,
                u.name as u_name, u.created_at as u_created_at, u.updated_at as u_updated_at
               FROM scim_user_mappings m
               JOIN users u ON m.user_id = u.id
               WHERE {}
               ORDER BY m.created_at ASC, m.id ASC
               LIMIT ? OFFSET ?"#,
            where_clause
        );

        // Execute count query
        let mut count_query = sqlx::query(&count_sql).bind(org_id.to_string());
        if let Some(f) = filter {
            for val in &f.bindings {
                count_query = match val {
                    SqlValue::String(s) => count_query.bind(s.clone()),
                    SqlValue::Bool(b) => count_query.bind(*b as i32),
                    SqlValue::Float(n) => count_query.bind(*n),
                };
            }
        }
        let count_row = count_query.fetch_one(&self.pool).await?;
        let total: i64 = count_row.get("cnt");

        // Execute data query
        let mut data_query = sqlx::query(&data_sql).bind(org_id.to_string());
        if let Some(f) = filter {
            for val in &f.bindings {
                data_query = match val {
                    SqlValue::String(s) => data_query.bind(s.clone()),
                    SqlValue::Bool(b) => data_query.bind(*b as i32),
                    SqlValue::Float(n) => data_query.bind(*n),
                };
            }
        }
        data_query = data_query.bind(limit).bind(offset);
        let rows = data_query.fetch_all(&self.pool).await?;

        let items = rows
            .iter()
            .map(Self::parse_mapping_with_user)
            .collect::<DbResult<Vec<_>>>()?;

        Ok((items, total))
    }

    async fn list_by_user(&self, user_id: Uuid) -> DbResult<Vec<ScimUserMapping>> {
        let rows = sqlx::query("SELECT * FROM scim_user_mappings WHERE user_id = ?")
            .bind(user_id.to_string())
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(Self::parse_mapping).collect()
    }

    async fn update(&self, id: Uuid, input: UpdateScimUserMapping) -> DbResult<ScimUserMapping> {
        let current = self.get_by_id(id).await?.ok_or_else(|| DbError::NotFound)?;

        let active = input.active.unwrap_or(current.active);
        let now = chrono::Utc::now();

        sqlx::query("UPDATE scim_user_mappings SET active = ?, updated_at = ? WHERE id = ?")
            .bind(active as i32)
            .bind(now)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(ScimUserMapping {
            id: current.id,
            org_id: current.org_id,
            scim_external_id: current.scim_external_id,
            user_id: current.user_id,
            active,
            created_at: current.created_at,
            updated_at: now,
        })
    }

    async fn set_active(&self, id: Uuid, active: bool) -> DbResult<ScimUserMapping> {
        self.update(
            id,
            UpdateScimUserMapping {
                active: Some(active),
            },
        )
        .await
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM scim_user_mappings WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn delete_by_user(&self, user_id: Uuid) -> DbResult<u64> {
        let result = sqlx::query("DELETE FROM scim_user_mappings WHERE user_id = ?")
            .bind(user_id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM scim_user_mappings WHERE org_id = ?")
            .bind(org_id.to_string())
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get::<i64, _>("count"))
    }
}
