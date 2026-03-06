use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::DbResult,
        repos::{
            AuditLogRepo, Cursor, CursorDirection, ListResult, PageCursors, cursor_from_row,
            truncate_to_millis,
        },
        wasm_sqlite::{WasmSqlitePool, query as wasm_query},
    },
    models::{AuditActorType, AuditLog, AuditLogQuery, CreateAuditLog},
};

pub struct WasmSqliteAuditLogRepo {
    pool: WasmSqlitePool,
}

impl WasmSqliteAuditLogRepo {
    pub fn new(pool: WasmSqlitePool) -> Self {
        Self { pool }
    }

    fn parse_actor_type(s: &str) -> DbResult<AuditActorType> {
        s.parse()
            .map_err(|e: String| crate::db::error::DbError::Internal(e))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl AuditLogRepo for WasmSqliteAuditLogRepo {
    async fn create(&self, input: CreateAuditLog) -> DbResult<AuditLog> {
        let id = Uuid::new_v4();
        // Truncate to milliseconds for cursor pagination compatibility (see cursor.rs)
        let now = truncate_to_millis(chrono::Utc::now());
        let details_json = serde_json::to_string(&input.details)?;

        wasm_query(
            r#"
            INSERT INTO audit_logs (
                id, timestamp, actor_type, actor_id, action,
                resource_type, resource_id, org_id, project_id,
                details, ip_address, user_agent
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(now)
        .bind(input.actor_type.to_string())
        .bind(input.actor_id.map(|id| id.to_string()))
        .bind(&input.action)
        .bind(&input.resource_type)
        .bind(input.resource_id.to_string())
        .bind(input.org_id.map(|id| id.to_string()))
        .bind(input.project_id.map(|id| id.to_string()))
        .bind(&details_json)
        .bind(&input.ip_address)
        .bind(&input.user_agent)
        .execute(&self.pool)
        .await?;

        Ok(AuditLog {
            id,
            timestamp: now,
            actor_type: input.actor_type,
            actor_id: input.actor_id,
            action: input.action,
            resource_type: input.resource_type,
            resource_id: input.resource_id,
            org_id: input.org_id,
            project_id: input.project_id,
            details: input.details,
            ip_address: input.ip_address,
            user_agent: input.user_agent,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<AuditLog>> {
        let result = wasm_query(
            r#"
            SELECT id, timestamp, actor_type, actor_id, action,
                   resource_type, resource_id, org_id, project_id,
                   details, ip_address, user_agent
            FROM audit_logs
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let actor_id: Option<String> = row.get("actor_id");
                let org_id: Option<String> = row.get("org_id");
                let project_id: Option<String> = row.get("project_id");
                let details_str: String = row.get("details");

                Ok(Some(AuditLog {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    timestamp: row.get("timestamp"),
                    actor_type: Self::parse_actor_type(&row.get::<String>("actor_type"))?,
                    actor_id: actor_id.map(|s| parse_uuid(&s)).transpose()?,
                    action: row.get("action"),
                    resource_type: row.get("resource_type"),
                    resource_id: parse_uuid(&row.get::<String>("resource_id"))?,
                    org_id: org_id.map(|s| parse_uuid(&s)).transpose()?,
                    project_id: project_id.map(|s| parse_uuid(&s)).transpose()?,
                    details: serde_json::from_str(&details_str)?,
                    ip_address: row.get("ip_address"),
                    user_agent: row.get("user_agent"),
                }))
            }
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

        // Build dynamic WHERE clause for filters
        let mut conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(actor_type) = &query.actor_type {
            conditions.push("actor_type = ?".to_string());
            params.push(actor_type.to_string());
        }
        if let Some(actor_id) = &query.actor_id {
            conditions.push("actor_id = ?".to_string());
            params.push(actor_id.to_string());
        }
        if let Some(action) = &query.action {
            conditions.push("action = ?".to_string());
            params.push(action.clone());
        }
        if let Some(resource_type) = &query.resource_type {
            conditions.push("resource_type = ?".to_string());
            params.push(resource_type.clone());
        }
        if let Some(resource_id) = &query.resource_id {
            conditions.push("resource_id = ?".to_string());
            params.push(resource_id.to_string());
        }
        if let Some(org_id) = &query.org_id {
            conditions.push("org_id = ?".to_string());
            params.push(org_id.to_string());
        }
        if let Some(project_id) = &query.project_id {
            conditions.push("project_id = ?".to_string());
            params.push(project_id.to_string());
        }
        if let Some(from) = &query.from {
            conditions.push("timestamp >= ?".to_string());
            params.push(from.to_rfc3339());
        }
        if let Some(to) = &query.to {
            conditions.push("timestamp < ?".to_string());
            params.push(to.to_rfc3339());
        }

        // Add cursor condition if present
        // Note: We compare (timestamp, id) for stable ordering since multiple entries may have the same timestamp
        let (order, cursor_condition) = if cursor.is_some() {
            let (comparison, order) = if direction == CursorDirection::Backward {
                (">", "ASC")
            } else {
                ("<", "DESC")
            };
            // Cursor params are bound separately after string params
            (
                order,
                Some(format!("(timestamp, id) {} (?, ?)", comparison)),
            )
        } else {
            ("DESC", None)
        };

        if let Some(cond) = cursor_condition {
            conditions.push(cond);
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = if cursor.is_some() {
            // Cursor-based pagination
            format!(
                r#"
                SELECT id, timestamp, actor_type, actor_id, action,
                       resource_type, resource_id, org_id, project_id,
                       details, ip_address, user_agent
                FROM audit_logs
                {}
                ORDER BY timestamp {}, id {}
                LIMIT ?
                "#,
                where_clause, order, order
            )
        } else {
            // First page (no cursor provided)
            format!(
                r#"
                SELECT id, timestamp, actor_type, actor_id, action,
                       resource_type, resource_id, org_id, project_id,
                       details, ip_address, user_agent
                FROM audit_logs
                {}
                ORDER BY timestamp DESC, id DESC
                LIMIT ?
                "#,
                where_clause
            )
        };

        let mut query_builder = wasm_query(&sql);
        for param in &params {
            query_builder = query_builder.bind(param);
        }
        // Bind cursor params directly as DateTime and UUID (not as strings) for proper comparison
        if let Some(ref c) = cursor {
            query_builder = query_builder.bind(c.created_at).bind(c.id.to_string());
        }
        query_builder = query_builder.bind(fetch_limit);

        let rows = query_builder.fetch_all(&self.pool).await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<AuditLog> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let actor_id: Option<String> = row.get("actor_id");
                let org_id: Option<String> = row.get("org_id");
                let project_id: Option<String> = row.get("project_id");
                let details_str: String = row.get("details");

                Ok(AuditLog {
                    id: parse_uuid(&row.get::<String>("id"))?,
                    timestamp: row.get("timestamp"),
                    actor_type: Self::parse_actor_type(&row.get::<String>("actor_type"))?,
                    actor_id: actor_id.map(|s| parse_uuid(&s)).transpose()?,
                    action: row.get("action"),
                    resource_type: row.get("resource_type"),
                    resource_id: parse_uuid(&row.get::<String>("resource_id"))?,
                    org_id: org_id.map(|s| parse_uuid(&s)).transpose()?,
                    project_id: project_id.map(|s| parse_uuid(&s)).transpose()?,
                    details: serde_json::from_str(&details_str)?,
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
        let mut params: Vec<String> = Vec::new();

        if let Some(actor_type) = &query.actor_type {
            conditions.push("actor_type = ?");
            params.push(actor_type.to_string());
        }
        if let Some(actor_id) = &query.actor_id {
            conditions.push("actor_id = ?");
            params.push(actor_id.to_string());
        }
        if let Some(action) = &query.action {
            conditions.push("action = ?");
            params.push(action.clone());
        }
        if let Some(resource_type) = &query.resource_type {
            conditions.push("resource_type = ?");
            params.push(resource_type.clone());
        }
        if let Some(resource_id) = &query.resource_id {
            conditions.push("resource_id = ?");
            params.push(resource_id.to_string());
        }
        if let Some(org_id) = &query.org_id {
            conditions.push("org_id = ?");
            params.push(org_id.to_string());
        }
        if let Some(project_id) = &query.project_id {
            conditions.push("project_id = ?");
            params.push(project_id.to_string());
        }
        if let Some(from) = &query.from {
            conditions.push("timestamp >= ?");
            params.push(from.to_rfc3339());
        }
        if let Some(to) = &query.to {
            conditions.push("timestamp < ?");
            params.push(to.to_rfc3339());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!("SELECT COUNT(*) as count FROM audit_logs {}", where_clause);

        let mut query_builder = wasm_query(&sql);
        for param in &params {
            query_builder = query_builder.bind(param);
        }

        let row = query_builder.fetch_one(&self.pool).await?;
        Ok(row.get::<i64>("count"))
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

            // Delete a batch using subquery to select IDs
            let result = wasm_query(
                r#"
                DELETE FROM audit_logs
                WHERE id IN (
                    SELECT id FROM audit_logs
                    WHERE timestamp < ?
                    LIMIT ?
                )
                "#,
            )
            .bind(cutoff)
            .bind(limit)
            .execute(&self.pool)
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
