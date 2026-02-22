use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
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

pub struct SqliteAuditLogRepo {
    pool: SqlitePool,
}

impl SqliteAuditLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_actor_type(s: &str) -> DbResult<AuditActorType> {
        s.parse()
            .map_err(|e: String| crate::db::error::DbError::Internal(e))
    }
}

#[async_trait]
impl AuditLogRepo for SqliteAuditLogRepo {
    async fn create(&self, input: CreateAuditLog) -> DbResult<AuditLog> {
        let id = Uuid::new_v4();
        // Truncate to milliseconds for cursor pagination compatibility (see cursor.rs)
        let now = truncate_to_millis(chrono::Utc::now());
        let details_json = serde_json::to_string(&input.details)?;

        sqlx::query(
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
        let result = sqlx::query(
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
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    timestamp: row.get("timestamp"),
                    actor_type: Self::parse_actor_type(&row.get::<String, _>("actor_type"))?,
                    actor_id: actor_id.map(|s| parse_uuid(&s)).transpose()?,
                    action: row.get("action"),
                    resource_type: row.get("resource_type"),
                    resource_id: parse_uuid(&row.get::<String, _>("resource_id"))?,
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

        let mut query_builder = sqlx::query(&sql);
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
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    timestamp: row.get("timestamp"),
                    actor_type: Self::parse_actor_type(&row.get::<String, _>("actor_type"))?,
                    actor_id: actor_id.map(|s| parse_uuid(&s)).transpose()?,
                    action: row.get("action"),
                    resource_type: row.get("resource_type"),
                    resource_id: parse_uuid(&row.get::<String, _>("resource_id"))?,
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

        let mut query_builder = sqlx::query(&sql);
        for param in &params {
            query_builder = query_builder.bind(param);
        }

        let row = query_builder.fetch_one(&self.pool).await?;
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

            // Delete a batch using subquery to select IDs
            let result = sqlx::query(
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

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use serde_json::json;

    use super::*;
    use crate::db::repos::AuditLogRepo;

    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        sqlx::query(
            r#"
            CREATE TABLE audit_logs (
                id TEXT PRIMARY KEY NOT NULL,
                timestamp TEXT NOT NULL,
                actor_type TEXT NOT NULL,
                actor_id TEXT,
                action TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                org_id TEXT,
                project_id TEXT,
                details TEXT NOT NULL DEFAULT '{}',
                ip_address TEXT,
                user_agent TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create audit_logs table");

        pool
    }

    fn create_audit_log_input(
        actor_type: AuditActorType,
        actor_id: Option<Uuid>,
        action: &str,
        resource_type: &str,
        resource_id: Uuid,
    ) -> CreateAuditLog {
        CreateAuditLog {
            actor_type,
            actor_id,
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id,
            org_id: None,
            project_id: None,
            details: json!({}),
            ip_address: None,
            user_agent: None,
        }
    }

    // ==================== Create Tests ====================

    #[tokio::test]
    async fn test_create_audit_log_basic() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let user_id = Uuid::new_v4();
        let resource_id = Uuid::new_v4();
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(user_id),
            "api_key.create",
            "api_key",
            resource_id,
        );

        let log = repo
            .create(input)
            .await
            .expect("Failed to create audit log");

        assert!(!log.id.is_nil());
        assert_eq!(log.actor_type, AuditActorType::User);
        assert_eq!(log.actor_id, Some(user_id));
        assert_eq!(log.action, "api_key.create");
        assert_eq!(log.resource_type, "api_key");
        assert_eq!(log.resource_id, resource_id);
    }

    #[tokio::test]
    async fn test_create_audit_log_with_all_fields() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let actor_id = Uuid::new_v4();
        let resource_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();

        let input = CreateAuditLog {
            actor_type: AuditActorType::User,
            actor_id: Some(actor_id),
            action: "user.update".to_string(),
            resource_type: "user".to_string(),
            resource_id,
            org_id: Some(org_id),
            project_id: Some(project_id),
            details: json!({"field": "name", "old": "Alice", "new": "Bob"}),
            ip_address: Some("192.168.1.1".to_string()),
            user_agent: Some("Mozilla/5.0".to_string()),
        };

        let log = repo
            .create(input)
            .await
            .expect("Failed to create audit log");

        assert_eq!(log.org_id, Some(org_id));
        assert_eq!(log.project_id, Some(project_id));
        assert_eq!(log.ip_address, Some("192.168.1.1".to_string()));
        assert_eq!(log.user_agent, Some("Mozilla/5.0".to_string()));
        assert_eq!(log.details["field"], "name");
        assert_eq!(log.details["old"], "Alice");
        assert_eq!(log.details["new"], "Bob");
    }

    #[tokio::test]
    async fn test_create_audit_log_with_api_key_actor() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let api_key_id = Uuid::new_v4();
        let resource_id = Uuid::new_v4();
        let input = create_audit_log_input(
            AuditActorType::ApiKey,
            Some(api_key_id),
            "chat.completion",
            "completion",
            resource_id,
        );

        let log = repo
            .create(input)
            .await
            .expect("Failed to create audit log");

        assert_eq!(log.actor_type, AuditActorType::ApiKey);
        assert_eq!(log.actor_id, Some(api_key_id));
    }

    #[tokio::test]
    async fn test_create_audit_log_with_system_actor() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let resource_id = Uuid::new_v4();
        let input = create_audit_log_input(
            AuditActorType::System,
            None,
            "cache.clear",
            "cache",
            resource_id,
        );

        let log = repo
            .create(input)
            .await
            .expect("Failed to create audit log");

        assert_eq!(log.actor_type, AuditActorType::System);
        assert!(log.actor_id.is_none());
    }

    #[tokio::test]
    async fn test_create_audit_log_with_complex_details() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let resource_id = Uuid::new_v4();
        let details = json!({
            "changes": [
                {"field": "name", "before": "old", "after": "new"},
                {"field": "email", "before": "old@example.com", "after": "new@example.com"}
            ],
            "metadata": {
                "reason": "user request",
                "approved_by": "admin"
            }
        });

        let input = CreateAuditLog {
            actor_type: AuditActorType::User,
            actor_id: Some(Uuid::new_v4()),
            action: "user.update".to_string(),
            resource_type: "user".to_string(),
            resource_id,
            org_id: None,
            project_id: None,
            details: details.clone(),
            ip_address: None,
            user_agent: None,
        };

        let log = repo
            .create(input)
            .await
            .expect("Failed to create audit log");

        assert_eq!(log.details, details);
    }

    // ==================== Get by ID Tests ====================

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let resource_id = Uuid::new_v4();
        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "api_key.create",
            "api_key",
            resource_id,
        );

        let created = repo
            .create(input)
            .await
            .expect("Failed to create audit log");
        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get audit log")
            .expect("Audit log should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.action, "api_key.create");
        assert_eq!(fetched.resource_id, resource_id);
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let result = repo
            .get_by_id(Uuid::new_v4())
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    // ==================== List Tests ====================

    #[tokio::test]
    async fn test_list_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let logs = repo
            .list(AuditLogQuery::default())
            .await
            .expect("Failed to list audit logs");

        assert!(logs.items.is_empty());
    }

    #[tokio::test]
    async fn test_list_with_records() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        for i in 0..3 {
            let input = create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                &format!("action.{}", i),
                "resource",
                Uuid::new_v4(),
            );
            repo.create(input)
                .await
                .expect("Failed to create audit log");
        }

        let logs = repo
            .list(AuditLogQuery::default())
            .await
            .expect("Failed to list audit logs");

        assert_eq!(logs.items.len(), 3);
    }

    #[tokio::test]
    async fn test_list_filter_by_actor_type() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        // Create logs with different actor types
        let user_input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "user.action",
            "resource",
            Uuid::new_v4(),
        );
        repo.create(user_input).await.expect("Failed to create");

        let api_key_input = create_audit_log_input(
            AuditActorType::ApiKey,
            Some(Uuid::new_v4()),
            "api_key.action",
            "resource",
            Uuid::new_v4(),
        );
        repo.create(api_key_input).await.expect("Failed to create");

        let system_input = create_audit_log_input(
            AuditActorType::System,
            None,
            "system.action",
            "resource",
            Uuid::new_v4(),
        );
        repo.create(system_input).await.expect("Failed to create");

        let user_logs = repo
            .list(AuditLogQuery {
                actor_type: Some(AuditActorType::User),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(user_logs.items.len(), 1);
        assert_eq!(user_logs.items[0].actor_type, AuditActorType::User);
    }

    #[tokio::test]
    async fn test_list_filter_by_actor_id() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let actor1 = Uuid::new_v4();
        let actor2 = Uuid::new_v4();

        for _ in 0..2 {
            let input = create_audit_log_input(
                AuditActorType::User,
                Some(actor1),
                "action",
                "resource",
                Uuid::new_v4(),
            );
            repo.create(input).await.expect("Failed to create");
        }

        let input = create_audit_log_input(
            AuditActorType::User,
            Some(actor2),
            "action",
            "resource",
            Uuid::new_v4(),
        );
        repo.create(input).await.expect("Failed to create");

        let logs = repo
            .list(AuditLogQuery {
                actor_id: Some(actor1),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(logs.items.len(), 2);
        assert!(logs.items.iter().all(|l| l.actor_id == Some(actor1)));
    }

    #[tokio::test]
    async fn test_list_filter_by_action() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let input1 = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "api_key.create",
            "api_key",
            Uuid::new_v4(),
        );
        repo.create(input1).await.expect("Failed to create");

        let input2 = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "api_key.delete",
            "api_key",
            Uuid::new_v4(),
        );
        repo.create(input2).await.expect("Failed to create");

        let logs = repo
            .list(AuditLogQuery {
                action: Some("api_key.create".to_string()),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(logs.items.len(), 1);
        assert_eq!(logs.items[0].action, "api_key.create");
    }

    #[tokio::test]
    async fn test_list_filter_by_resource_type() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let input1 = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "create",
            "api_key",
            Uuid::new_v4(),
        );
        repo.create(input1).await.expect("Failed to create");

        let input2 = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "create",
            "user",
            Uuid::new_v4(),
        );
        repo.create(input2).await.expect("Failed to create");

        let logs = repo
            .list(AuditLogQuery {
                resource_type: Some("api_key".to_string()),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(logs.items.len(), 1);
        assert_eq!(logs.items[0].resource_type, "api_key");
    }

    #[tokio::test]
    async fn test_list_filter_by_resource_id() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let resource1 = Uuid::new_v4();
        let resource2 = Uuid::new_v4();

        let input1 = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            resource1,
        );
        repo.create(input1).await.expect("Failed to create");

        let input2 = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action",
            "resource",
            resource2,
        );
        repo.create(input2).await.expect("Failed to create");

        let logs = repo
            .list(AuditLogQuery {
                resource_id: Some(resource1),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(logs.items.len(), 1);
        assert_eq!(logs.items[0].resource_id, resource1);
    }

    #[tokio::test]
    async fn test_list_filter_by_org_id() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let org1 = Uuid::new_v4();
        let org2 = Uuid::new_v4();

        let input1 = CreateAuditLog {
            org_id: Some(org1),
            ..create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "action",
                "resource",
                Uuid::new_v4(),
            )
        };
        repo.create(input1).await.expect("Failed to create");

        let input2 = CreateAuditLog {
            org_id: Some(org2),
            ..create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "action",
                "resource",
                Uuid::new_v4(),
            )
        };
        repo.create(input2).await.expect("Failed to create");

        let logs = repo
            .list(AuditLogQuery {
                org_id: Some(org1),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(logs.items.len(), 1);
        assert_eq!(logs.items[0].org_id, Some(org1));
    }

    #[tokio::test]
    async fn test_list_filter_by_project_id() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let project1 = Uuid::new_v4();
        let project2 = Uuid::new_v4();

        let input1 = CreateAuditLog {
            project_id: Some(project1),
            ..create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "action",
                "resource",
                Uuid::new_v4(),
            )
        };
        repo.create(input1).await.expect("Failed to create");

        let input2 = CreateAuditLog {
            project_id: Some(project2),
            ..create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "action",
                "resource",
                Uuid::new_v4(),
            )
        };
        repo.create(input2).await.expect("Failed to create");

        let logs = repo
            .list(AuditLogQuery {
                project_id: Some(project1),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(logs.items.len(), 1);
        assert_eq!(logs.items[0].project_id, Some(project1));
    }

    #[tokio::test]
    async fn test_list_filter_by_date_range() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        // Create logs at different times
        for _ in 0..3 {
            let input = create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "action",
                "resource",
                Uuid::new_v4(),
            );
            repo.create(input).await.expect("Failed to create");
        }

        let now = chrono::Utc::now();
        let future = now + Duration::hours(1);

        // Query with from in the future should return nothing
        let logs = repo
            .list(AuditLogQuery {
                from: Some(future),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert!(logs.items.is_empty());

        // Query with to in the past should return nothing
        let past = now - Duration::hours(1);
        let logs = repo
            .list(AuditLogQuery {
                to: Some(past),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert!(logs.items.is_empty());

        // Query with range including now should return all
        let logs = repo
            .list(AuditLogQuery {
                from: Some(past),
                to: Some(future),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(logs.items.len(), 3);
    }

    #[tokio::test]
    async fn test_list_pagination() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        // Add small delays between creates to ensure distinct timestamps for stable ordering
        for i in 0..5 {
            let input = create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                &format!("action.{}", i),
                "resource",
                Uuid::new_v4(),
            );
            repo.create(input).await.expect("Failed to create");
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // First page (no cursor)
        let page1 = repo
            .list(AuditLogQuery {
                limit: Some(2),
                ..Default::default()
            })
            .await
            .expect("Failed to list page 1");

        // Second page (using cursor from first page)
        let page2 = repo
            .list(AuditLogQuery {
                limit: Some(2),
                cursor: page1.cursors.next.as_ref().map(|c| c.encode()),
                ..Default::default()
            })
            .await
            .expect("Failed to list page 2");

        assert_eq!(page1.items.len(), 2);
        assert_eq!(page2.items.len(), 2);
        // Pages should have different logs
        assert_ne!(page1.items[0].id, page2.items[0].id);
    }

    #[tokio::test]
    async fn test_list_combined_filters() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let org_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();

        // Create matching log
        let matching_input = CreateAuditLog {
            org_id: Some(org_id),
            ..create_audit_log_input(
                AuditActorType::User,
                Some(actor_id),
                "api_key.create",
                "api_key",
                Uuid::new_v4(),
            )
        };
        repo.create(matching_input).await.expect("Failed to create");

        // Create non-matching logs
        let input2 = CreateAuditLog {
            org_id: Some(Uuid::new_v4()), // Different org
            ..create_audit_log_input(
                AuditActorType::User,
                Some(actor_id),
                "api_key.create",
                "api_key",
                Uuid::new_v4(),
            )
        };
        repo.create(input2).await.expect("Failed to create");

        let input3 = CreateAuditLog {
            org_id: Some(org_id),
            ..create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()), // Different actor
                "api_key.create",
                "api_key",
                Uuid::new_v4(),
            )
        };
        repo.create(input3).await.expect("Failed to create");

        let logs = repo
            .list(AuditLogQuery {
                org_id: Some(org_id),
                actor_id: Some(actor_id),
                action: Some("api_key.create".to_string()),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(logs.items.len(), 1);
        assert_eq!(logs.items[0].org_id, Some(org_id));
        assert_eq!(logs.items[0].actor_id, Some(actor_id));
    }

    #[tokio::test]
    async fn test_list_ordered_by_timestamp_desc() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        // Create logs with small delays
        for i in 0..3 {
            let input = create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                &format!("action.{}", i),
                "resource",
                Uuid::new_v4(),
            );
            repo.create(input).await.expect("Failed to create");
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        let logs = repo
            .list(AuditLogQuery::default())
            .await
            .expect("Failed to list");

        // Most recent should be first
        assert!(logs.items[0].timestamp >= logs.items[1].timestamp);
        assert!(logs.items[1].timestamp >= logs.items[2].timestamp);
    }

    // ==================== Count Tests ====================

    #[tokio::test]
    async fn test_count_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let count = repo
            .count(AuditLogQuery::default())
            .await
            .expect("Failed to count");

        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_count_with_records() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        for _ in 0..5 {
            let input = create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "action",
                "resource",
                Uuid::new_v4(),
            );
            repo.create(input).await.expect("Failed to create");
        }

        let count = repo
            .count(AuditLogQuery::default())
            .await
            .expect("Failed to count");

        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn test_count_with_filters() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        // Create logs with different actions
        for _ in 0..3 {
            let input = create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "api_key.create",
                "api_key",
                Uuid::new_v4(),
            );
            repo.create(input).await.expect("Failed to create");
        }

        for _ in 0..2 {
            let input = create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "api_key.delete",
                "api_key",
                Uuid::new_v4(),
            );
            repo.create(input).await.expect("Failed to create");
        }

        let create_count = repo
            .count(AuditLogQuery {
                action: Some("api_key.create".to_string()),
                ..Default::default()
            })
            .await
            .expect("Failed to count");

        let delete_count = repo
            .count(AuditLogQuery {
                action: Some("api_key.delete".to_string()),
                ..Default::default()
            })
            .await
            .expect("Failed to count");

        assert_eq!(create_count, 3);
        assert_eq!(delete_count, 2);
    }

    #[tokio::test]
    async fn test_count_with_combined_filters() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let org_id = Uuid::new_v4();

        // Create matching logs
        for _ in 0..2 {
            let input = CreateAuditLog {
                org_id: Some(org_id),
                ..create_audit_log_input(
                    AuditActorType::User,
                    Some(Uuid::new_v4()),
                    "action",
                    "resource",
                    Uuid::new_v4(),
                )
            };
            repo.create(input).await.expect("Failed to create");
        }

        // Create non-matching log (different org)
        let input = CreateAuditLog {
            org_id: Some(Uuid::new_v4()),
            ..create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "action",
                "resource",
                Uuid::new_v4(),
            )
        };
        repo.create(input).await.expect("Failed to create");

        let count = repo
            .count(AuditLogQuery {
                org_id: Some(org_id),
                ..Default::default()
            })
            .await
            .expect("Failed to count");

        assert_eq!(count, 2);
    }

    // ==================== Edge Cases ====================

    #[tokio::test]
    async fn test_special_characters_in_action() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let input = create_audit_log_input(
            AuditActorType::User,
            Some(Uuid::new_v4()),
            "action:with/special.chars_and-more",
            "resource",
            Uuid::new_v4(),
        );

        let log = repo.create(input).await.expect("Failed to create");

        let fetched = repo
            .get_by_id(log.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.action, "action:with/special.chars_and-more");
    }

    #[tokio::test]
    async fn test_unicode_in_details() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let details = json!({
            "message": "Hello 你好 مرحبا 🌍",
            "user_name": "Jürgen Müller"
        });

        let input = CreateAuditLog {
            details: details.clone(),
            ..create_audit_log_input(
                AuditActorType::User,
                Some(Uuid::new_v4()),
                "action",
                "resource",
                Uuid::new_v4(),
            )
        };

        let log = repo.create(input).await.expect("Failed to create");

        let fetched = repo
            .get_by_id(log.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.details, details);
    }

    #[tokio::test]
    async fn test_null_optional_fields() {
        let pool = create_test_pool().await;
        let repo = SqliteAuditLogRepo::new(pool);

        let input = CreateAuditLog {
            actor_type: AuditActorType::System,
            actor_id: None,
            action: "system.startup".to_string(),
            resource_type: "system".to_string(),
            resource_id: Uuid::new_v4(),
            org_id: None,
            project_id: None,
            details: json!({}),
            ip_address: None,
            user_agent: None,
        };

        let log = repo.create(input).await.expect("Failed to create");

        let fetched = repo
            .get_by_id(log.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert!(fetched.actor_id.is_none());
        assert!(fetched.org_id.is_none());
        assert!(fetched.project_id.is_none());
        assert!(fetched.ip_address.is_none());
        assert!(fetched.user_agent.is_none());
    }
}
