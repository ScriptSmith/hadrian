//! SQLite implementation of the SCIM group mapping repository.

use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
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

pub struct SqliteScimGroupMappingRepo {
    pool: SqlitePool,
}

impl SqliteScimGroupMappingRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Parse a ScimGroupMapping from a database row.
    fn parse_mapping(row: &sqlx::sqlite::SqliteRow) -> DbResult<ScimGroupMapping> {
        Ok(ScimGroupMapping {
            id: parse_uuid(&row.get::<String, _>("id"))?,
            org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
            scim_group_id: row.get("scim_group_id"),
            team_id: parse_uuid(&row.get::<String, _>("team_id"))?,
            display_name: row.get("display_name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Parse a ScimGroupWithTeam from a joined row with aliased columns.
    fn parse_mapping_with_team(row: &sqlx::sqlite::SqliteRow) -> DbResult<ScimGroupWithTeam> {
        Ok(ScimGroupWithTeam {
            mapping: ScimGroupMapping {
                id: parse_uuid(&row.get::<String, _>("m_id"))?,
                org_id: parse_uuid(&row.get::<String, _>("m_org_id"))?,
                scim_group_id: row.get("m_scim_group_id"),
                team_id: parse_uuid(&row.get::<String, _>("m_team_id"))?,
                display_name: row.get("m_display_name"),
                created_at: row.get("m_created_at"),
                updated_at: row.get("m_updated_at"),
            },
            team: Team {
                id: parse_uuid(&row.get::<String, _>("t_id"))?,
                org_id: parse_uuid(&row.get::<String, _>("t_org_id"))?,
                slug: row.get("t_slug"),
                name: row.get("t_name"),
                created_at: row.get("t_created_at"),
                updated_at: row.get("t_updated_at"),
            },
        })
    }
}

#[async_trait]
impl ScimGroupMappingRepo for SqliteScimGroupMappingRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateScimGroupMapping,
    ) -> DbResult<ScimGroupMapping> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO scim_group_mappings (
                id, org_id, scim_group_id, team_id, display_name, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(&input.scim_group_id)
        .bind(input.team_id.to_string())
        .bind(&input.display_name)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
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

        Ok(ScimGroupMapping {
            id,
            org_id,
            scim_group_id: input.scim_group_id,
            team_id: input.team_id,
            display_name: input.display_name,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ScimGroupMapping>> {
        let row = sqlx::query("SELECT * FROM scim_group_mappings WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        row.map(|r| Self::parse_mapping(&r)).transpose()
    }

    async fn get_by_scim_group_id(
        &self,
        org_id: Uuid,
        scim_group_id: &str,
    ) -> DbResult<Option<ScimGroupMapping>> {
        let row =
            sqlx::query("SELECT * FROM scim_group_mappings WHERE org_id = ? AND scim_group_id = ?")
                .bind(org_id.to_string())
                .bind(scim_group_id)
                .fetch_optional(&self.pool)
                .await?;

        row.map(|r| Self::parse_mapping(&r)).transpose()
    }

    async fn get_by_team_id(
        &self,
        org_id: Uuid,
        team_id: Uuid,
    ) -> DbResult<Option<ScimGroupMapping>> {
        let row = sqlx::query("SELECT * FROM scim_group_mappings WHERE org_id = ? AND team_id = ?")
            .bind(org_id.to_string())
            .bind(team_id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        row.map(|r| Self::parse_mapping(&r)).transpose()
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ScimGroupMapping>> {
        let limit = params.limit.unwrap_or(100).min(1000);
        let fetch_limit = limit + 1; // Fetch one extra to detect has_more

        let (comparison_op, order_dir, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let rows = if let Some(cursor) = &params.cursor {
            let query = format!(
                r#"
                SELECT * FROM scim_group_mappings
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
                SELECT * FROM scim_group_mappings
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
        let mut items: Vec<ScimGroupMapping> = rows
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
    ) -> DbResult<(Vec<ScimGroupWithTeam>, i64)> {
        // Build WHERE clause: always filter by org_id, optionally add SCIM filter
        let where_clause = if let Some(f) = filter {
            format!("m.org_id = ? AND ({})", f.where_clause)
        } else {
            "m.org_id = ?".to_string()
        };

        // Count query for totalResults
        let count_sql = format!(
            "SELECT COUNT(*) as cnt FROM scim_group_mappings m \
             JOIN teams t ON m.team_id = t.id \
             WHERE {}",
            where_clause
        );

        // Data query with aliased columns to avoid ambiguity
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
            .map(Self::parse_mapping_with_team)
            .collect::<DbResult<Vec<_>>>()?;

        Ok((items, total))
    }

    async fn update(&self, id: Uuid, input: UpdateScimGroupMapping) -> DbResult<ScimGroupMapping> {
        let current = self.get_by_id(id).await?.ok_or_else(|| DbError::NotFound)?;

        let team_id = input.team_id.unwrap_or(current.team_id);
        let display_name = match input.display_name {
            Some(v) => v, // Some(Some(name)) or Some(None)
            None => current.display_name.clone(),
        };
        let now = chrono::Utc::now();

        sqlx::query(
            "UPDATE scim_group_mappings SET team_id = ?, display_name = ?, updated_at = ? WHERE id = ?",
        )
        .bind(team_id.to_string())
        .bind(&display_name)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                DbError::Conflict("Referenced team not found".into())
            }
            _ => DbError::from(e),
        })?;

        Ok(ScimGroupMapping {
            id: current.id,
            org_id: current.org_id,
            scim_group_id: current.scim_group_id,
            team_id,
            display_name,
            created_at: current.created_at,
            updated_at: now,
        })
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM scim_group_mappings WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn delete_by_team(&self, team_id: Uuid) -> DbResult<u64> {
        let result = sqlx::query("DELETE FROM scim_group_mappings WHERE team_id = ?")
            .bind(team_id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM scim_group_mappings WHERE org_id = ?")
            .bind(org_id.to_string())
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get::<i64, _>("count"))
    }
}
