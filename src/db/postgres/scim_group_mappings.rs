//! PostgreSQL implementation of the SCIM group mapping repository.

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, ScimGroupMappingRepo,
        },
    },
    models::{
        CreateScimGroupMapping, ScimGroupMapping, ScimGroupWithTeam, Team, UpdateScimGroupMapping,
    },
    scim::filter_to_sql::{SqlFilter, SqlValue},
};

pub struct PostgresScimGroupMappingRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresScimGroupMappingRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    /// Parse a ScimGroupMapping from a database row.
    fn parse_mapping(row: &sqlx::postgres::PgRow) -> ScimGroupMapping {
        ScimGroupMapping {
            id: row.get("id"),
            org_id: row.get("org_id"),
            scim_group_id: row.get("scim_group_id"),
            team_id: row.get("team_id"),
            display_name: row.get("display_name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    /// Parse a ScimGroupWithTeam from a joined row with aliased columns.
    fn parse_mapping_with_team(row: &sqlx::postgres::PgRow) -> ScimGroupWithTeam {
        ScimGroupWithTeam {
            mapping: ScimGroupMapping {
                id: row.get("m_id"),
                org_id: row.get("m_org_id"),
                scim_group_id: row.get("m_scim_group_id"),
                team_id: row.get("m_team_id"),
                display_name: row.get("m_display_name"),
                created_at: row.get("m_created_at"),
                updated_at: row.get("m_updated_at"),
            },
            team: Team {
                id: row.get("t_id"),
                org_id: row.get("t_org_id"),
                slug: row.get("t_slug"),
                name: row.get("t_name"),
                created_at: row.get("t_created_at"),
                updated_at: row.get("t_updated_at"),
            },
        }
    }
}

#[async_trait]
impl ScimGroupMappingRepo for PostgresScimGroupMappingRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateScimGroupMapping,
    ) -> DbResult<ScimGroupMapping> {
        let row = sqlx::query(
            r#"
            INSERT INTO scim_group_mappings (id, org_id, scim_group_id, team_id, display_name)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, org_id, scim_group_id, team_id, display_name, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(&input.scim_group_id)
        .bind(input.team_id)
        .bind(&input.display_name)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("SCIM group ID already mapped in this organization".into())
            }
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                DbError::Conflict("Referenced team or organization not found".into())
            }
            _ => DbError::from(e),
        })?;

        Ok(Self::parse_mapping(&row))
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ScimGroupMapping>> {
        let row = sqlx::query(
            r#"
            SELECT id, org_id, scim_group_id, team_id, display_name, created_at, updated_at
            FROM scim_group_mappings WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(row.map(|r| Self::parse_mapping(&r)))
    }

    async fn get_by_scim_group_id(
        &self,
        org_id: Uuid,
        scim_group_id: &str,
    ) -> DbResult<Option<ScimGroupMapping>> {
        let row = sqlx::query(
            r#"
            SELECT id, org_id, scim_group_id, team_id, display_name, created_at, updated_at
            FROM scim_group_mappings WHERE org_id = $1 AND scim_group_id = $2
            "#,
        )
        .bind(org_id)
        .bind(scim_group_id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(row.map(|r| Self::parse_mapping(&r)))
    }

    async fn get_by_team_id(
        &self,
        org_id: Uuid,
        team_id: Uuid,
    ) -> DbResult<Option<ScimGroupMapping>> {
        let row = sqlx::query(
            r#"
            SELECT id, org_id, scim_group_id, team_id, display_name, created_at, updated_at
            FROM scim_group_mappings WHERE org_id = $1 AND team_id = $2
            "#,
        )
        .bind(org_id)
        .bind(team_id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(row.map(|r| Self::parse_mapping(&r)))
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ScimGroupMapping>> {
        let limit = params.limit.unwrap_or(100).min(1000);
        let fetch_limit = limit + 1;

        let (comparison_op, order_dir, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let rows = if let Some(cursor) = &params.cursor {
            let query = format!(
                r#"
                SELECT id, org_id, scim_group_id, team_id, display_name, created_at, updated_at
                FROM scim_group_mappings
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
                SELECT id, org_id, scim_group_id, team_id, display_name, created_at, updated_at
                FROM scim_group_mappings
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
        let mut items: Vec<ScimGroupMapping> = rows
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
    ) -> DbResult<(Vec<ScimGroupWithTeam>, i64)> {
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
            "SELECT COUNT(*) as cnt FROM scim_group_mappings m \
             JOIN teams t ON m.team_id = t.id \
             WHERE {}",
            where_clause
        );

        // Data query parameter indices: after org_id ($1) and filter params,
        // limit and offset are the last two
        let limit_idx = 2 + filter_param_count;
        let offset_idx = limit_idx + 1;

        let data_sql = format!(
            r#"SELECT
                m.id as m_id, m.org_id as m_org_id, m.scim_group_id as m_scim_group_id,
                m.team_id as m_team_id, m.display_name as m_display_name,
                m.created_at as m_created_at, m.updated_at as m_updated_at,
                t.id as t_id, t.org_id as t_org_id, t.slug as t_slug,
                t.name as t_name, t.created_at as t_created_at, t.updated_at as t_updated_at
               FROM scim_group_mappings m
               JOIN teams t ON m.team_id = t.id
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

        let items: Vec<ScimGroupWithTeam> =
            rows.iter().map(Self::parse_mapping_with_team).collect();

        Ok((items, total))
    }

    async fn update(&self, id: Uuid, input: UpdateScimGroupMapping) -> DbResult<ScimGroupMapping> {
        let current = self.get_by_id(id).await?.ok_or_else(|| DbError::NotFound)?;

        let team_id = input.team_id.unwrap_or(current.team_id);
        let display_name = match input.display_name {
            Some(v) => v,
            None => current.display_name.clone(),
        };

        let row = sqlx::query(
            r#"
            UPDATE scim_group_mappings SET team_id = $1, display_name = $2, updated_at = NOW()
            WHERE id = $3
            RETURNING id, org_id, scim_group_id, team_id, display_name, created_at, updated_at
            "#,
        )
        .bind(team_id)
        .bind(&display_name)
        .bind(id)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                DbError::Conflict("Referenced team not found".into())
            }
            _ => DbError::from(e),
        })?;

        Ok(Self::parse_mapping(&row))
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM scim_group_mappings WHERE id = $1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn delete_by_team(&self, team_id: Uuid) -> DbResult<u64> {
        let result = sqlx::query("DELETE FROM scim_group_mappings WHERE team_id = $1")
            .bind(team_id)
            .execute(&self.write_pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row =
            sqlx::query("SELECT COUNT(*) as count FROM scim_group_mappings WHERE org_id = $1")
                .bind(org_id)
                .fetch_one(&self.read_pool)
                .await?;

        Ok(row.get::<i64, _>("count"))
    }
}
