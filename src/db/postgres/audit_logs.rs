use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::DbResult,
        repos::{
            AuditLogRepo, Cursor, CursorDirection, ListResult, PageCursors, cursor_from_row,
            truncate_to_millis,
        },
    },
    models::{AuditActorType, AuditLog, AuditLogQuery, CreateAuditLog},
};

pub struct PostgresAuditLogRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresAuditLogRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    fn parse_actor_type(s: &str) -> DbResult<AuditActorType> {
        s.parse()
            .map_err(|e: String| crate::db::error::DbError::Internal(e))
    }
}

#[async_trait]
impl AuditLogRepo for PostgresAuditLogRepo {
    async fn create(&self, input: CreateAuditLog) -> DbResult<AuditLog> {
        let id = Uuid::new_v4();
        // Truncate to milliseconds for cursor pagination compatibility (see cursor.rs)
        let now = truncate_to_millis(chrono::Utc::now());

        let row = sqlx::query(
            r#"
            INSERT INTO audit_logs (
                id, timestamp, actor_type, actor_id, action,
                resource_type, resource_id, org_id, project_id,
                details, ip_address, user_agent
            )
            VALUES ($1, $2, $3::audit_actor_type, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id, timestamp, actor_type::text, actor_id, action,
                      resource_type, resource_id, org_id, project_id,
                      details, ip_address, user_agent
            "#,
        )
        .bind(id)
        .bind(now)
        .bind(input.actor_type.to_string())
        .bind(input.actor_id)
        .bind(&input.action)
        .bind(&input.resource_type)
        .bind(input.resource_id)
        .bind(input.org_id)
        .bind(input.project_id)
        .bind(&input.details)
        .bind(&input.ip_address)
        .bind(&input.user_agent)
        .fetch_one(&self.write_pool)
        .await?;

        Ok(AuditLog {
            id: row.get("id"),
            timestamp: row.get("timestamp"),
            actor_type: Self::parse_actor_type(&row.get::<String, _>("actor_type"))?,
            actor_id: row.get("actor_id"),
            action: row.get("action"),
            resource_type: row.get("resource_type"),
            resource_id: row.get("resource_id"),
            org_id: row.get("org_id"),
            project_id: row.get("project_id"),
            details: row.get("details"),
            ip_address: row.get("ip_address"),
            user_agent: row.get("user_agent"),
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<AuditLog>> {
        let result = sqlx::query(
            r#"
            SELECT id, timestamp, actor_type::text, actor_id, action,
                   resource_type, resource_id, org_id, project_id,
                   details, ip_address, user_agent
            FROM audit_logs
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        match result {
            Some(row) => Ok(Some(AuditLog {
                id: row.get("id"),
                timestamp: row.get("timestamp"),
                actor_type: Self::parse_actor_type(&row.get::<String, _>("actor_type"))?,
                actor_id: row.get("actor_id"),
                action: row.get("action"),
                resource_type: row.get("resource_type"),
                resource_id: row.get("resource_id"),
                org_id: row.get("org_id"),
                project_id: row.get("project_id"),
                details: row.get("details"),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
            })),
            None => Ok(None),
        }
    }

    async fn list(&self, query: AuditLogQuery) -> DbResult<ListResult<AuditLog>> {
        let limit = query.limit.unwrap_or(100);
        let fetch_limit = limit + 1; // Fetch one extra to determine if there are more items

        // Parse cursor if provided
        let cursor = match &query.cursor {
            Some(c) => Some(Cursor::decode(c).map_err(|e| {
                crate::db::error::DbError::Internal(format!("Invalid cursor: {}", e))
            })?),
            None => None,
        };

        let direction = match query.direction.as_deref() {
            Some("backward") => CursorDirection::Backward,
            _ => CursorDirection::Forward,
        };

        // Build dynamic WHERE clause
        let mut conditions = Vec::new();
        let mut param_idx = 1u32;

        if query.actor_type.is_some() {
            conditions.push(format!("actor_type = ${}::audit_actor_type", param_idx));
            param_idx += 1;
        }
        if query.actor_id.is_some() {
            conditions.push(format!("actor_id = ${}", param_idx));
            param_idx += 1;
        }
        if query.action.is_some() {
            conditions.push(format!("action = ${}", param_idx));
            param_idx += 1;
        }
        if query.resource_type.is_some() {
            conditions.push(format!("resource_type = ${}", param_idx));
            param_idx += 1;
        }
        if query.resource_id.is_some() {
            conditions.push(format!("resource_id = ${}", param_idx));
            param_idx += 1;
        }
        if query.org_id.is_some() {
            conditions.push(format!("org_id = ${}", param_idx));
            param_idx += 1;
        }
        if query.project_id.is_some() {
            conditions.push(format!("project_id = ${}", param_idx));
            param_idx += 1;
        }
        if query.from.is_some() {
            conditions.push(format!("timestamp >= ${}", param_idx));
            param_idx += 1;
        }
        if query.to.is_some() {
            conditions.push(format!("timestamp < ${}", param_idx));
            param_idx += 1;
        }

        // Add cursor condition if present
        // Note: PostgreSQL uses ROW comparison for tuple ordering
        let order = if let Some(ref _c) = cursor {
            let (comparison, order) = if direction == CursorDirection::Backward {
                (">", "ASC")
            } else {
                ("<", "DESC")
            };
            conditions.push(format!(
                "ROW(timestamp, id) {} ROW(${}, ${})",
                comparison,
                param_idx,
                param_idx + 1
            ));
            param_idx += 2;
            order
        } else {
            "DESC"
        };

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            r#"
            SELECT id, timestamp, actor_type::text, actor_id, action,
                   resource_type, resource_id, org_id, project_id,
                   details, ip_address, user_agent
            FROM audit_logs
            {}
            ORDER BY timestamp {}, id {}
            LIMIT ${}
            "#,
            where_clause, order, order, param_idx
        );

        let mut query_builder = sqlx::query(&sql);

        if let Some(actor_type) = &query.actor_type {
            query_builder = query_builder.bind(actor_type.to_string());
        }
        if let Some(actor_id) = &query.actor_id {
            query_builder = query_builder.bind(actor_id);
        }
        if let Some(action) = &query.action {
            query_builder = query_builder.bind(action);
        }
        if let Some(resource_type) = &query.resource_type {
            query_builder = query_builder.bind(resource_type);
        }
        if let Some(resource_id) = &query.resource_id {
            query_builder = query_builder.bind(resource_id);
        }
        if let Some(org_id) = &query.org_id {
            query_builder = query_builder.bind(org_id);
        }
        if let Some(project_id) = &query.project_id {
            query_builder = query_builder.bind(project_id);
        }
        if let Some(from) = &query.from {
            query_builder = query_builder.bind(from);
        }
        if let Some(to) = &query.to {
            query_builder = query_builder.bind(to);
        }

        // Bind cursor parameters if present
        if let Some(ref c) = cursor {
            query_builder = query_builder.bind(c.created_at).bind(c.id);
        }

        // Bind limit parameter
        query_builder = query_builder.bind(fetch_limit);

        let rows = query_builder.fetch_all(&self.read_pool).await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<AuditLog> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(AuditLog {
                    id: row.get("id"),
                    timestamp: row.get("timestamp"),
                    actor_type: Self::parse_actor_type(&row.get::<String, _>("actor_type"))?,
                    actor_id: row.get("actor_id"),
                    action: row.get("action"),
                    resource_type: row.get("resource_type"),
                    resource_id: row.get("resource_id"),
                    org_id: row.get("org_id"),
                    project_id: row.get("project_id"),
                    details: row.get("details"),
                    ip_address: row.get("ip_address"),
                    user_agent: row.get("user_agent"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        // For backward pagination, reverse results to maintain descending order
        if direction == CursorDirection::Backward {
            items.reverse();
        }

        // Generate cursors
        let cursors =
            PageCursors::from_items(&items, has_more, direction, cursor.as_ref(), |log| {
                cursor_from_row(log.timestamp, log.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count(&self, query: AuditLogQuery) -> DbResult<i64> {
        // Build dynamic WHERE clause
        let mut conditions = Vec::new();
        let mut param_idx = 1u32;

        if query.actor_type.is_some() {
            conditions.push(format!("actor_type = ${}::audit_actor_type", param_idx));
            param_idx += 1;
        }
        if query.actor_id.is_some() {
            conditions.push(format!("actor_id = ${}", param_idx));
            param_idx += 1;
        }
        if query.action.is_some() {
            conditions.push(format!("action = ${}", param_idx));
            param_idx += 1;
        }
        if query.resource_type.is_some() {
            conditions.push(format!("resource_type = ${}", param_idx));
            param_idx += 1;
        }
        if query.resource_id.is_some() {
            conditions.push(format!("resource_id = ${}", param_idx));
            param_idx += 1;
        }
        if query.org_id.is_some() {
            conditions.push(format!("org_id = ${}", param_idx));
            param_idx += 1;
        }
        if query.project_id.is_some() {
            conditions.push(format!("project_id = ${}", param_idx));
            param_idx += 1;
        }
        if query.from.is_some() {
            conditions.push(format!("timestamp >= ${}", param_idx));
            param_idx += 1;
        }
        if query.to.is_some() {
            conditions.push(format!("timestamp < ${}", param_idx));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!("SELECT COUNT(*) as count FROM audit_logs {}", where_clause);

        let mut query_builder = sqlx::query(&sql);

        if let Some(actor_type) = &query.actor_type {
            query_builder = query_builder.bind(actor_type.to_string());
        }
        if let Some(actor_id) = &query.actor_id {
            query_builder = query_builder.bind(actor_id);
        }
        if let Some(action) = &query.action {
            query_builder = query_builder.bind(action);
        }
        if let Some(resource_type) = &query.resource_type {
            query_builder = query_builder.bind(resource_type);
        }
        if let Some(resource_id) = &query.resource_id {
            query_builder = query_builder.bind(resource_id);
        }
        if let Some(org_id) = &query.org_id {
            query_builder = query_builder.bind(org_id);
        }
        if let Some(project_id) = &query.project_id {
            query_builder = query_builder.bind(project_id);
        }
        if let Some(from) = &query.from {
            query_builder = query_builder.bind(from);
        }
        if let Some(to) = &query.to {
            query_builder = query_builder.bind(to);
        }

        let row = query_builder.fetch_one(&self.read_pool).await?;
        Ok(row.get::<i64, _>("count"))
    }

    // ==================== Retention Operations ====================

    async fn delete_before(
        &self,
        cutoff: DateTime<Utc>,
        batch_size: u32,
        max_deletes: u64,
    ) -> DbResult<u64> {
        let mut total_deleted: u64 = 0;

        loop {
            if total_deleted >= max_deletes {
                break;
            }

            let remaining = max_deletes - total_deleted;
            let limit = std::cmp::min(batch_size as u64, remaining) as i64;

            // PostgreSQL efficient batched deletion using ctid
            let result = sqlx::query(
                r#"
                DELETE FROM audit_logs
                WHERE ctid IN (
                    SELECT ctid FROM audit_logs
                    WHERE timestamp < $1
                    LIMIT $2
                )
                "#,
            )
            .bind(cutoff)
            .bind(limit)
            .execute(&self.write_pool)
            .await?;

            let rows_deleted = result.rows_affected();
            total_deleted += rows_deleted;

            if rows_deleted < limit as u64 {
                break;
            }
        }

        Ok(total_deleted)
    }
}
