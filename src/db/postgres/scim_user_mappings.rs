//! PostgreSQL implementation of the SCIM user mapping repository.

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

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

pub struct PostgresScimUserMappingRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresScimUserMappingRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    /// Parse a ScimUserMapping from a database row.
    fn parse_mapping(row: &sqlx::postgres::PgRow) -> ScimUserMapping {
        ScimUserMapping {
            id: row.get("id"),
            org_id: row.get("org_id"),
            scim_external_id: row.get("scim_external_id"),
            user_id: row.get("user_id"),
            active: row.get("active"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    /// Parse a ScimUserWithMapping from a joined row with aliased columns.
    fn parse_mapping_with_user(row: &sqlx::postgres::PgRow) -> ScimUserWithMapping {
        ScimUserWithMapping {
            mapping: ScimUserMapping {
                id: row.get("m_id"),
                org_id: row.get("m_org_id"),
                scim_external_id: row.get("m_scim_external_id"),
                user_id: row.get("m_user_id"),
                active: row.get("m_active"),
                created_at: row.get("m_created_at"),
                updated_at: row.get("m_updated_at"),
            },
            user: User {
                id: row.get("u_id"),
                external_id: row.get("u_external_id"),
                email: row.get("u_email"),
                name: row.get("u_name"),
                created_at: row.get("u_created_at"),
                updated_at: row.get("u_updated_at"),
            },
        }
    }
}

#[async_trait]
impl ScimUserMappingRepo for PostgresScimUserMappingRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateScimUserMapping,
    ) -> DbResult<ScimUserMapping> {
        let row = sqlx::query(
            r#"
            INSERT INTO scim_user_mappings (id, org_id, scim_external_id, user_id, active)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, org_id, scim_external_id, user_id, active, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(&input.scim_external_id)
        .bind(input.user_id)
        .bind(input.active)
        .fetch_one(&self.write_pool)
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

        Ok(Self::parse_mapping(&row))
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ScimUserMapping>> {
        let row = sqlx::query(
            r#"
            SELECT id, org_id, scim_external_id, user_id, active, created_at, updated_at
            FROM scim_user_mappings WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(row.map(|r| Self::parse_mapping(&r)))
    }

    async fn get_by_scim_external_id(
        &self,
        org_id: Uuid,
        scim_external_id: &str,
    ) -> DbResult<Option<ScimUserMapping>> {
        let row = sqlx::query(
            r#"
            SELECT id, org_id, scim_external_id, user_id, active, created_at, updated_at
            FROM scim_user_mappings WHERE org_id = $1 AND scim_external_id = $2
            "#,
        )
        .bind(org_id)
        .bind(scim_external_id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(row.map(|r| Self::parse_mapping(&r)))
    }

    async fn get_by_user_id(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> DbResult<Option<ScimUserMapping>> {
        let row = sqlx::query(
            r#"
            SELECT id, org_id, scim_external_id, user_id, active, created_at, updated_at
            FROM scim_user_mappings WHERE org_id = $1 AND user_id = $2
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(row.map(|r| Self::parse_mapping(&r)))
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ScimUserMapping>> {
        let limit = params.limit.unwrap_or(100).min(1000);
        let fetch_limit = limit + 1;

        let (comparison_op, order_dir, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let rows = if let Some(cursor) = &params.cursor {
            let query = format!(
                r#"
                SELECT id, org_id, scim_external_id, user_id, active, created_at, updated_at
                FROM scim_user_mappings
                WHERE org_id = $1 AND (created_at, id) {} ($2, $3)
                ORDER BY created_at {}, id {}
                LIMIT $4
                "#,
                comparison_op, order_dir, order_dir
            );
            sqlx::query(&query)
                .bind(org_id)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
                .await?
        } else {
            let query = format!(
                r#"
                SELECT id, org_id, scim_external_id, user_id, active, created_at, updated_at
                FROM scim_user_mappings
                WHERE org_id = $1
                ORDER BY created_at {}, id {}
                LIMIT $2
                "#,
                order_dir, order_dir
            );
            sqlx::query(&query)
                .bind(org_id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
                .await?
        };

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<ScimUserMapping> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_mapping)
            .collect();

        if should_reverse {
            items.reverse();
        }

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
        // Build WHERE clause with PostgreSQL-style numbered parameters
        // org_id is always $1, then filter params start at $2
        let (where_clause, filter_param_count) = if let Some(f) = filter {
            // Convert ?-style placeholders to $N-style for PostgreSQL
            let mut pg_clause = String::new();
            let mut param_idx = 2; // $1 is org_id
            for ch in f.where_clause.chars() {
                if ch == '?' {
                    pg_clause.push_str(&format!("${}", param_idx));
                    param_idx += 1;
                } else {
                    pg_clause.push(ch);
                }
            }
            (format!("m.org_id = $1 AND ({})", pg_clause), param_idx - 2)
        } else {
            ("m.org_id = $1".to_string(), 0)
        };

        // Count query for totalResults
        let count_sql = format!(
            "SELECT COUNT(*) as cnt FROM scim_user_mappings m \
             JOIN users u ON m.user_id = u.id \
             WHERE {}",
            where_clause
        );

        // Data query parameter indices: after org_id ($1) and filter params,
        // limit and offset are the last two
        let limit_idx = 2 + filter_param_count;
        let offset_idx = limit_idx + 1;

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
               LIMIT ${} OFFSET ${}"#,
            where_clause, limit_idx, offset_idx
        );

        // Execute count query
        let mut count_query = sqlx::query(&count_sql).bind(org_id);
        if let Some(f) = filter {
            for val in &f.bindings {
                count_query = match val {
                    SqlValue::String(s) => count_query.bind(s.clone()),
                    SqlValue::Bool(b) => count_query.bind(*b),
                    SqlValue::Float(n) => count_query.bind(*n),
                };
            }
        }
        let count_row = count_query.fetch_one(&self.read_pool).await?;
        let total: i64 = count_row.get("cnt");

        // Execute data query
        let mut data_query = sqlx::query(&data_sql).bind(org_id);
        if let Some(f) = filter {
            for val in &f.bindings {
                data_query = match val {
                    SqlValue::String(s) => data_query.bind(s.clone()),
                    SqlValue::Bool(b) => data_query.bind(*b),
                    SqlValue::Float(n) => data_query.bind(*n),
                };
            }
        }
        data_query = data_query.bind(limit).bind(offset);
        let rows = data_query.fetch_all(&self.read_pool).await?;

        let items: Vec<ScimUserWithMapping> =
            rows.iter().map(Self::parse_mapping_with_user).collect();

        Ok((items, total))
    }

    async fn list_by_user(&self, user_id: Uuid) -> DbResult<Vec<ScimUserMapping>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_id, scim_external_id, user_id, active, created_at, updated_at
            FROM scim_user_mappings WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows.iter().map(Self::parse_mapping).collect())
    }

    async fn update(&self, id: Uuid, input: UpdateScimUserMapping) -> DbResult<ScimUserMapping> {
        let current = self.get_by_id(id).await?.ok_or_else(|| DbError::NotFound)?;

        let active = input.active.unwrap_or(current.active);

        let row = sqlx::query(
            r#"
            UPDATE scim_user_mappings SET active = $1, updated_at = NOW()
            WHERE id = $2
            RETURNING id, org_id, scim_external_id, user_id, active, created_at, updated_at
            "#,
        )
        .bind(active)
        .bind(id)
        .fetch_one(&self.write_pool)
        .await?;

        Ok(Self::parse_mapping(&row))
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
        let result = sqlx::query("DELETE FROM scim_user_mappings WHERE id = $1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn delete_by_user(&self, user_id: Uuid) -> DbResult<u64> {
        let result = sqlx::query("DELETE FROM scim_user_mappings WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.write_pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM scim_user_mappings WHERE org_id = $1")
            .bind(org_id)
            .fetch_one(&self.read_pool)
            .await?;

        Ok(row.get::<i64, _>("count"))
    }
}
