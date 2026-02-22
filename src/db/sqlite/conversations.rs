use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{ConversationRepo, Cursor, CursorDirection, ListParams, ListResult, PageCursors},
    },
    models::{
        AppendMessages, Conversation, ConversationOwnerType, ConversationWithProject,
        CreateConversation, Message, UpdateConversation,
    },
};

pub struct SqliteConversationRepo {
    pool: SqlitePool,
}

impl SqliteConversationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_messages(json_str: &str) -> DbResult<Vec<Message>> {
        serde_json::from_str(json_str).map_err(|e| DbError::Internal(e.to_string()))
    }

    fn parse_models(json_str: &str) -> DbResult<Vec<String>> {
        serde_json::from_str(json_str).map_err(|e| DbError::Internal(e.to_string()))
    }

    /// Create a cursor from a conversation's updated_at and id.
    ///
    /// Note: We use updated_at instead of created_at because conversations
    /// are ordered by updated_at to show recently-used conversations first.
    fn cursor_from_conversation(conv: &Conversation) -> Cursor {
        Cursor::new(conv.updated_at, conv.id)
    }

    /// Helper method for cursor-based pagination.
    ///
    /// Uses keyset pagination with (updated_at, id) tuple for efficient, consistent results.
    async fn list_with_cursor(
        &self,
        owner_type: ConversationOwnerType,
        owner_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<Conversation>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };

        let query = format!(
            r#"
            SELECT id, owner_type, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE owner_type = ? AND owner_id = ?
            AND (updated_at, id) {} (?, ?)
            {}
            ORDER BY updated_at {}, id {}
            LIMIT ?
            "#,
            comparison, deleted_filter, order, order
        );

        let rows = sqlx::query(&query)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .bind(cursor.created_at) // cursor.created_at holds updated_at value
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<Conversation> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let owner_type_str: String = row.get("owner_type");
                let models_json: String = row.get("models");
                let messages_json: String = row.get("messages");

                Ok(Conversation {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: parse_uuid(&row.get::<String, _>("owner_id"))?,
                    title: row.get("title"),
                    models: Self::parse_models(&models_json)?,
                    messages: Self::parse_messages(&messages_json)?,
                    pin_order: row.get("pin_order"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        // Generate cursors
        let cursors = PageCursors::from_items(
            &items,
            has_more,
            params.direction,
            Some(cursor),
            Self::cursor_from_conversation,
        );

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[async_trait]
impl ConversationRepo for SqliteConversationRepo {
    async fn create(&self, input: CreateConversation) -> DbResult<Conversation> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let owner_type = input.owner.owner_type();
        let owner_id = input.owner.owner_id();
        let models_json =
            serde_json::to_string(&input.models).map_err(|e| DbError::Internal(e.to_string()))?;
        let messages_json =
            serde_json::to_string(&input.messages).map_err(|e| DbError::Internal(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO conversations (id, owner_type, owner_id, title, models, messages, pin_order, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(owner_type.as_str())
        .bind(owner_id.to_string())
        .bind(&input.title)
        .bind(&models_json)
        .bind(&messages_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(Conversation {
            id,
            owner_type,
            owner_id,
            title: input.title,
            models: input.models,
            messages: input.messages,
            pin_order: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Conversation>> {
        let result = sqlx::query(
            r#"
            SELECT id, owner_type, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let owner_type_str: String = row.get("owner_type");
                let models_json: String = row.get("models");
                let messages_json: String = row.get("messages");

                Ok(Some(Conversation {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: parse_uuid(&row.get::<String, _>("owner_id"))?,
                    title: row.get("title"),
                    models: Self::parse_models(&models_json)?,
                    messages: Self::parse_messages(&messages_json)?,
                    pin_order: row.get("pin_order"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Conversation>> {
        let result = sqlx::query(
            r#"
            SELECT c.id, c.owner_type, c.owner_id, c.title, c.models, c.messages, c.pin_order, c.created_at, c.updated_at
            FROM conversations c
            WHERE c.id = ? AND c.deleted_at IS NULL
            AND (
                (c.owner_type = 'project' AND EXISTS (
                    SELECT 1 FROM projects p WHERE p.id = c.owner_id AND p.org_id = ?
                ))
                OR
                (c.owner_type = 'user' AND EXISTS (
                    SELECT 1 FROM org_memberships om WHERE om.user_id = c.owner_id AND om.org_id = ?
                ))
            )
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(org_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let owner_type_str: String = row.get("owner_type");
                let models_json: String = row.get("models");
                let messages_json: String = row.get("messages");

                Ok(Some(Conversation {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: parse_uuid(&row.get::<String, _>("owner_id"))?,
                    title: row.get("title"),
                    models: Self::parse_models(&models_json)?,
                    messages: Self::parse_messages(&messages_json)?,
                    pin_order: row.get("pin_order"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_by_owner(
        &self,
        owner_type: ConversationOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Conversation>> {
        let limit = params.limit.unwrap_or(100);
        // Fetch one extra to determine if there are more items
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(owner_type, owner_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let query = if params.include_deleted {
            r#"
            SELECT id, owner_type, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE owner_type = ? AND owner_id = ?
            ORDER BY updated_at DESC, id DESC
            LIMIT ?
            "#
        } else {
            r#"
            SELECT id, owner_type, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE owner_type = ? AND owner_id = ? AND deleted_at IS NULL
            ORDER BY updated_at DESC, id DESC
            LIMIT ?
            "#
        };

        let rows = sqlx::query(query)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<Conversation> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let owner_type_str: String = row.get("owner_type");
                let models_json: String = row.get("models");
                let messages_json: String = row.get("messages");

                Ok(Conversation {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: parse_uuid(&row.get::<String, _>("owner_id"))?,
                    title: row.get("title"),
                    models: Self::parse_models(&models_json)?,
                    messages: Self::parse_messages(&messages_json)?,
                    pin_order: row.get("pin_order"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        // Generate cursors for pagination
        let cursors = PageCursors::from_items(
            &items,
            has_more,
            CursorDirection::Forward,
            None,
            Self::cursor_from_conversation,
        );

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_owner(
        &self,
        owner_type: ConversationOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) as count FROM conversations WHERE owner_type = ? AND owner_id = ?"
        } else {
            "SELECT COUNT(*) as count FROM conversations WHERE owner_type = ? AND owner_id = ? AND deleted_at IS NULL"
        };

        let row = sqlx::query(query)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateConversation) -> DbResult<Conversation> {
        let now = chrono::Utc::now();

        // Use IMMEDIATE transaction mode to acquire write lock before reading.
        // This prevents lost updates from concurrent modifications by blocking
        // other writers until this transaction completes.
        // Note: SQLite doesn't support FOR UPDATE, so we use BEGIN IMMEDIATE instead.
        let mut conn = self.pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

        let result = async {
            // Read current state within transaction (with write lock held)
            let current_row = sqlx::query(
                r#"
                SELECT id, owner_type, owner_id, title, models, messages, pin_order, created_at, updated_at
                FROM conversations
                WHERE id = ? AND deleted_at IS NULL
                "#,
            )
            .bind(id.to_string())
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(DbError::NotFound)?;

            let current_owner_type_str: String = current_row.get("owner_type");
            let current_owner_type: ConversationOwnerType = current_owner_type_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?;
            let current_owner_id = parse_uuid(&current_row.get::<String, _>("owner_id"))?;
            let current_title: String = current_row.get("title");
            let current_models_json: String = current_row.get("models");
            let current_messages_json: String = current_row.get("messages");
            let pin_order: Option<i32> = current_row.get("pin_order");
            let created_at = current_row.get("created_at");

            // Determine new owner (if provided) or keep current
            let (new_owner_type, new_owner_id) = if let Some(ref owner) = input.owner {
                (owner.owner_type(), owner.owner_id())
            } else {
                (current_owner_type, current_owner_id)
            };

            let new_title = input.title.unwrap_or(current_title);
            let new_models = input
                .models
                .unwrap_or_else(|| Self::parse_models(&current_models_json).unwrap_or_default());
            let new_messages = input.messages.unwrap_or_else(|| {
                Self::parse_messages(&current_messages_json).unwrap_or_default()
            });
            let models_json =
                serde_json::to_string(&new_models).map_err(|e| DbError::Internal(e.to_string()))?;
            let messages_json = serde_json::to_string(&new_messages)
                .map_err(|e| DbError::Internal(e.to_string()))?;

            let update_result = sqlx::query(
                r#"
                UPDATE conversations
                SET owner_type = ?, owner_id = ?, title = ?, models = ?, messages = ?, updated_at = ?
                WHERE id = ? AND deleted_at IS NULL
                "#,
            )
            .bind(new_owner_type.as_str())
            .bind(new_owner_id.to_string())
            .bind(&new_title)
            .bind(&models_json)
            .bind(&messages_json)
            .bind(now)
            .bind(id.to_string())
            .execute(&mut *conn)
            .await?;

            if update_result.rows_affected() == 0 {
                return Err(DbError::NotFound);
            }

            Ok(Conversation {
                id,
                owner_type: new_owner_type,
                owner_id: new_owner_id,
                title: new_title,
                models: new_models,
                messages: new_messages,
                pin_order,
                created_at,
                updated_at: now,
            })
        }
        .await;

        // Commit or rollback based on result
        match &result {
            Ok(_) => {
                sqlx::query("COMMIT").execute(&mut *conn).await?;
            }
            Err(_) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            }
        }

        result
    }

    async fn append_messages(&self, id: Uuid, input: AppendMessages) -> DbResult<Vec<Message>> {
        let now = chrono::Utc::now();

        // Use IMMEDIATE transaction mode to acquire write lock before reading.
        // This prevents lost messages from concurrent appends by blocking
        // other writers until this transaction completes.
        // Note: SQLite doesn't support FOR UPDATE, so we use BEGIN IMMEDIATE instead.
        let mut conn = self.pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

        let result = async {
            // Get current messages within transaction (with write lock held)
            let current_row = sqlx::query(
                r#"
                SELECT messages
                FROM conversations
                WHERE id = ? AND deleted_at IS NULL
                "#,
            )
            .bind(id.to_string())
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(DbError::NotFound)?;

            let current_messages_json: String = current_row.get("messages");
            let mut messages = Self::parse_messages(&current_messages_json)?;

            // Append new messages
            messages.extend(input.messages);

            let messages_json =
                serde_json::to_string(&messages).map_err(|e| DbError::Internal(e.to_string()))?;

            let update_result = sqlx::query(
                r#"
                UPDATE conversations
                SET messages = ?, updated_at = ?
                WHERE id = ? AND deleted_at IS NULL
                "#,
            )
            .bind(&messages_json)
            .bind(now)
            .bind(id.to_string())
            .execute(&mut *conn)
            .await?;

            if update_result.rows_affected() == 0 {
                return Err(DbError::NotFound);
            }

            Ok(messages)
        }
        .await;

        // Commit or rollback based on result
        match &result {
            Ok(_) => {
                sqlx::query("COMMIT").execute(&mut *conn).await?;
            }
            Err(_) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            }
        }

        result
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE conversations
            SET deleted_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn list_accessible_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
        include_deleted: bool,
    ) -> DbResult<Vec<ConversationWithProject>> {
        // Query conversations that the user can access:
        // 1. User's own conversations (owner_type='user' AND owner_id=user_id)
        // 2. Project conversations where user is a member
        //
        // Uses a UNION to combine both sources, with LEFT JOIN to projects table
        // for project metadata.
        let deleted_filter = if include_deleted {
            ""
        } else {
            "AND c.deleted_at IS NULL"
        };

        let query = format!(
            r#"
            SELECT
                c.id,
                c.owner_type,
                c.owner_id,
                c.title,
                c.models,
                c.messages,
                c.pin_order,
                c.created_at,
                c.updated_at,
                NULL as project_id,
                NULL as project_name,
                NULL as project_slug
            FROM conversations c
            WHERE c.owner_type = 'user' AND c.owner_id = ?
            {deleted_filter}

            UNION ALL

            SELECT
                c.id,
                c.owner_type,
                c.owner_id,
                c.title,
                c.models,
                c.messages,
                c.pin_order,
                c.created_at,
                c.updated_at,
                p.id as project_id,
                p.name as project_name,
                p.slug as project_slug
            FROM conversations c
            INNER JOIN project_memberships pm ON pm.project_id = c.owner_id AND pm.user_id = ?
            INNER JOIN projects p ON p.id = c.owner_id AND p.deleted_at IS NULL
            WHERE c.owner_type = 'project'
            {deleted_filter}

            ORDER BY updated_at DESC, id DESC
            LIMIT ?
            "#
        );

        let rows = sqlx::query(&query)
            .bind(user_id.to_string())
            .bind(user_id.to_string())
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;

        let items: Vec<ConversationWithProject> = rows
            .into_iter()
            .map(|row| {
                let owner_type_str: String = row.get("owner_type");
                let models_json: String = row.get("models");
                let messages_json: String = row.get("messages");
                let project_id: Option<String> = row.get("project_id");
                let project_name: Option<String> = row.get("project_name");
                let project_slug: Option<String> = row.get("project_slug");

                Ok(ConversationWithProject {
                    conversation: Conversation {
                        id: parse_uuid(&row.get::<String, _>("id"))?,
                        owner_type: owner_type_str
                            .parse()
                            .map_err(|e: String| DbError::Internal(e))?,
                        owner_id: parse_uuid(&row.get::<String, _>("owner_id"))?,
                        title: row.get("title"),
                        models: Self::parse_models(&models_json)?,
                        messages: Self::parse_messages(&messages_json)?,
                        pin_order: row.get("pin_order"),
                        created_at: row.get("created_at"),
                        updated_at: row.get("updated_at"),
                    },
                    project_id: project_id.map(|s| parse_uuid(&s)).transpose()?,
                    project_name,
                    project_slug,
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        Ok(items)
    }

    async fn set_pin_order(&self, id: Uuid, pin_order: Option<i32>) -> DbResult<Conversation> {
        let now = chrono::Utc::now();

        // Use IMMEDIATE transaction mode to acquire write lock
        let mut conn = self.pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

        let result = async {
            // Read current state within transaction (with write lock held)
            let current_row = sqlx::query(
                r#"
                SELECT id, owner_type, owner_id, title, models, messages, pin_order, created_at, updated_at
                FROM conversations
                WHERE id = ? AND deleted_at IS NULL
                "#,
            )
            .bind(id.to_string())
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(DbError::NotFound)?;

            let owner_type_str: String = current_row.get("owner_type");
            let owner_type: ConversationOwnerType = owner_type_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?;
            let owner_id = parse_uuid(&current_row.get::<String, _>("owner_id"))?;
            let title: String = current_row.get("title");
            let models_json: String = current_row.get("models");
            let messages_json: String = current_row.get("messages");
            let created_at = current_row.get("created_at");

            let update_result = sqlx::query(
                r#"
                UPDATE conversations
                SET pin_order = ?, updated_at = ?
                WHERE id = ? AND deleted_at IS NULL
                "#,
            )
            .bind(pin_order)
            .bind(now)
            .bind(id.to_string())
            .execute(&mut *conn)
            .await?;

            if update_result.rows_affected() == 0 {
                return Err(DbError::NotFound);
            }

            Ok(Conversation {
                id,
                owner_type,
                owner_id,
                title,
                models: Self::parse_models(&models_json)?,
                messages: Self::parse_messages(&messages_json)?,
                pin_order,
                created_at,
                updated_at: now,
            })
        }
        .await;

        // Commit or rollback based on result
        match &result {
            Ok(_) => {
                sqlx::query("COMMIT").execute(&mut *conn).await?;
            }
            Err(_) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            }
        }

        result
    }

    // ==================== Retention Operations ====================

    async fn hard_delete_soft_deleted_before(
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

            // Hard delete conversations that were soft-deleted before the cutoff
            let result = sqlx::query(
                r#"
                DELETE FROM conversations
                WHERE id IN (
                    SELECT id FROM conversations
                    WHERE deleted_at IS NOT NULL AND deleted_at < ?
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
    use super::*;
    use crate::{
        db::repos::ConversationRepo,
        models::{ConversationOwner, Message},
    };

    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        sqlx::query(
            r#"
            CREATE TABLE conversations (
                id TEXT PRIMARY KEY NOT NULL,
                owner_type TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                title TEXT NOT NULL,
                models TEXT NOT NULL DEFAULT '[]',
                messages TEXT NOT NULL DEFAULT '[]',
                pin_order INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create conversations table");

        pool
    }

    fn create_message(role: &str, content: &str) -> Message {
        Message {
            role: role.to_string(),
            content: content.to_string(),
        }
    }

    fn create_conversation_input(
        owner: ConversationOwner,
        title: &str,
        models: Vec<&str>,
        messages: Vec<Message>,
    ) -> CreateConversation {
        CreateConversation {
            owner,
            title: title.to_string(),
            models: models.into_iter().map(|m| m.to_string()).collect(),
            messages,
        }
    }

    // ==================== Create Tests ====================

    #[tokio::test]
    async fn test_create_conversation_with_project_owner() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let project_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::Project { project_id },
            "Test Conversation",
            vec!["gpt-4"],
            vec![],
        );

        let conv = repo
            .create(input)
            .await
            .expect("Failed to create conversation");

        assert!(!conv.id.is_nil());
        assert_eq!(conv.owner_type, ConversationOwnerType::Project);
        assert_eq!(conv.owner_id, project_id);
        assert_eq!(conv.title, "Test Conversation");
        assert_eq!(conv.models, vec!["gpt-4"]);
        assert!(conv.messages.is_empty());
    }

    #[tokio::test]
    async fn test_create_conversation_with_user_owner() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "User Chat",
            vec!["claude-3-opus"],
            vec![],
        );

        let conv = repo
            .create(input)
            .await
            .expect("Failed to create conversation");

        assert_eq!(conv.owner_type, ConversationOwnerType::User);
        assert_eq!(conv.owner_id, user_id);
        assert_eq!(conv.title, "User Chat");
    }

    #[tokio::test]
    async fn test_create_conversation_with_messages() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let messages = vec![
            create_message("user", "Hello!"),
            create_message("assistant", "Hi there!"),
        ];
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Chat with history",
            vec!["gpt-4"],
            messages,
        );

        let conv = repo
            .create(input)
            .await
            .expect("Failed to create conversation");

        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[0].role, "user");
        assert_eq!(conv.messages[0].content, "Hello!");
        assert_eq!(conv.messages[1].role, "assistant");
        assert_eq!(conv.messages[1].content, "Hi there!");
    }

    #[tokio::test]
    async fn test_create_conversation_with_multiple_models() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let project_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::Project { project_id },
            "Multi-model chat",
            vec!["gpt-4", "claude-3-opus", "gemini-pro"],
            vec![],
        );

        let conv = repo
            .create(input)
            .await
            .expect("Failed to create conversation");

        assert_eq!(conv.models.len(), 3);
        assert_eq!(conv.models[0], "gpt-4");
        assert_eq!(conv.models[1], "claude-3-opus");
        assert_eq!(conv.models[2], "gemini-pro");
    }

    // ==================== Get by ID Tests ====================

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Fetch Test",
            vec!["gpt-4"],
            vec![create_message("user", "Test message")],
        );

        let created = repo
            .create(input)
            .await
            .expect("Failed to create conversation");
        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get conversation")
            .expect("Conversation should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.title, "Fetch Test");
        assert_eq!(fetched.messages.len(), 1);
        assert_eq!(fetched.messages[0].content, "Test message");
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let result = repo
            .get_by_id(Uuid::new_v4())
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_id_deleted_returns_none() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "To be deleted",
            vec![],
            vec![],
        );

        let created = repo
            .create(input)
            .await
            .expect("Failed to create conversation");
        repo.delete(created.id)
            .await
            .expect("Failed to delete conversation");

        let result = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    // ==================== List by Owner Tests ====================

    #[tokio::test]
    async fn test_list_by_owner_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let result = repo
            .list_by_owner(ConversationOwnerType::User, user_id, ListParams::default())
            .await
            .expect("Failed to list conversations");

        assert!(result.items.is_empty());
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_by_owner_with_records() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        for i in 0..3 {
            let input = create_conversation_input(
                ConversationOwner::User { user_id },
                &format!("Conversation {}", i),
                vec![],
                vec![],
            );
            repo.create(input)
                .await
                .expect("Failed to create conversation");
        }

        let result = repo
            .list_by_owner(ConversationOwnerType::User, user_id, ListParams::default())
            .await
            .expect("Failed to list conversations");

        assert_eq!(result.items.len(), 3);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_by_owner_filters_by_owner_type() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();

        // Create user conversations
        for i in 0..2 {
            let input = create_conversation_input(
                ConversationOwner::User { user_id },
                &format!("User Conv {}", i),
                vec![],
                vec![],
            );
            repo.create(input)
                .await
                .expect("Failed to create user conversation");
        }

        // Create project conversations
        for i in 0..3 {
            let input = create_conversation_input(
                ConversationOwner::Project { project_id },
                &format!("Project Conv {}", i),
                vec![],
                vec![],
            );
            repo.create(input)
                .await
                .expect("Failed to create project conversation");
        }

        let user_result = repo
            .list_by_owner(ConversationOwnerType::User, user_id, ListParams::default())
            .await
            .expect("Failed to list user conversations");

        let project_result = repo
            .list_by_owner(
                ConversationOwnerType::Project,
                project_id,
                ListParams::default(),
            )
            .await
            .expect("Failed to list project conversations");

        assert_eq!(user_result.items.len(), 2);
        assert_eq!(project_result.items.len(), 3);
    }

    #[tokio::test]
    async fn test_list_by_owner_filters_by_owner_id() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();

        let input1 = create_conversation_input(
            ConversationOwner::User { user_id: user1 },
            "User 1 Chat",
            vec![],
            vec![],
        );
        repo.create(input1)
            .await
            .expect("Failed to create conversation");

        let input2 = create_conversation_input(
            ConversationOwner::User { user_id: user2 },
            "User 2 Chat",
            vec![],
            vec![],
        );
        repo.create(input2)
            .await
            .expect("Failed to create conversation");

        let user1_result = repo
            .list_by_owner(ConversationOwnerType::User, user1, ListParams::default())
            .await
            .expect("Failed to list");

        let user2_result = repo
            .list_by_owner(ConversationOwnerType::User, user2, ListParams::default())
            .await
            .expect("Failed to list");

        assert_eq!(user1_result.items.len(), 1);
        assert_eq!(user1_result.items[0].title, "User 1 Chat");
        assert_eq!(user2_result.items.len(), 1);
        assert_eq!(user2_result.items[0].title, "User 2 Chat");
    }

    #[tokio::test]
    async fn test_list_by_owner_pagination() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        for i in 0..5 {
            let input = create_conversation_input(
                ConversationOwner::User { user_id },
                &format!("Conv {}", i),
                vec![],
                vec![],
            );
            repo.create(input)
                .await
                .expect("Failed to create conversation");
        }

        // First page (no cursor)
        let page1 = repo
            .list_by_owner(
                ConversationOwnerType::User,
                user_id,
                ListParams {
                    limit: Some(2),
                    include_deleted: false,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 1");

        // Second page (using cursor from first page)
        let page2 = repo
            .list_by_owner(
                ConversationOwnerType::User,
                user_id,
                ListParams {
                    limit: Some(2),
                    include_deleted: false,
                    cursor: page1.cursors.next.clone(),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 2");

        assert_eq!(page1.items.len(), 2);
        assert_eq!(page2.items.len(), 2);
        assert!(page1.has_more);
        assert!(page2.has_more);
        // Pages should have different conversations
        assert_ne!(page1.items[0].id, page2.items[0].id);
    }

    #[tokio::test]
    async fn test_list_by_owner_excludes_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();

        let input1 = create_conversation_input(
            ConversationOwner::User { user_id },
            "Active Conv",
            vec![],
            vec![],
        );
        repo.create(input1)
            .await
            .expect("Failed to create conversation");

        let input2 = create_conversation_input(
            ConversationOwner::User { user_id },
            "Deleted Conv",
            vec![],
            vec![],
        );
        let to_delete = repo
            .create(input2)
            .await
            .expect("Failed to create conversation");
        repo.delete(to_delete.id).await.expect("Failed to delete");

        let result = repo
            .list_by_owner(ConversationOwnerType::User, user_id, ListParams::default())
            .await
            .expect("Failed to list");

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].title, "Active Conv");
    }

    #[tokio::test]
    async fn test_list_by_owner_include_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();

        let input1 = create_conversation_input(
            ConversationOwner::User { user_id },
            "Active Conv",
            vec![],
            vec![],
        );
        repo.create(input1)
            .await
            .expect("Failed to create conversation");

        let input2 = create_conversation_input(
            ConversationOwner::User { user_id },
            "Deleted Conv",
            vec![],
            vec![],
        );
        let to_delete = repo
            .create(input2)
            .await
            .expect("Failed to create conversation");
        repo.delete(to_delete.id).await.expect("Failed to delete");

        let result = repo
            .list_by_owner(
                ConversationOwnerType::User,
                user_id,
                ListParams {
                    limit: None,
                    include_deleted: true,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list");

        assert_eq!(result.items.len(), 2);
    }

    // ==================== Cursor Pagination Tests ====================

    #[tokio::test]
    async fn test_cursor_pagination_forward() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        // Create 5 conversations
        for i in 0..5 {
            let input = create_conversation_input(
                ConversationOwner::User { user_id },
                &format!("Cursor Conv {}", i),
                vec![],
                vec![],
            );
            repo.create(input)
                .await
                .expect("Failed to create conversation");
        }

        // Get first page
        let page1 = repo
            .list_by_owner(
                ConversationOwnerType::User,
                user_id,
                ListParams {
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 1");

        assert_eq!(page1.items.len(), 2);
        assert!(page1.has_more);
        assert!(page1.cursors.next.is_some());
        assert!(page1.cursors.prev.is_none()); // First page has no prev

        // Get second page using cursor
        let page2 = repo
            .list_by_owner(
                ConversationOwnerType::User,
                user_id,
                ListParams {
                    limit: Some(2),
                    cursor: page1.cursors.next,
                    direction: CursorDirection::Forward,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 2");

        assert_eq!(page2.items.len(), 2);
        assert!(page2.has_more);
        assert!(page2.cursors.next.is_some());
        assert!(page2.cursors.prev.is_some()); // Middle page has prev

        // Verify pages have different conversations
        assert_ne!(page1.items[0].id, page2.items[0].id);
        assert_ne!(page1.items[1].id, page2.items[1].id);

        // Get third/last page
        let page3 = repo
            .list_by_owner(
                ConversationOwnerType::User,
                user_id,
                ListParams {
                    limit: Some(2),
                    cursor: page2.cursors.next,
                    direction: CursorDirection::Forward,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 3");

        assert_eq!(page3.items.len(), 1); // Only 1 remaining
        assert!(!page3.has_more);
        assert!(page3.cursors.next.is_none()); // Last page has no next
        assert!(page3.cursors.prev.is_some());
    }

    #[tokio::test]
    async fn test_cursor_pagination_backward() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        // Create 5 conversations
        for i in 0..5 {
            let input = create_conversation_input(
                ConversationOwner::User { user_id },
                &format!("Back Conv {}", i),
                vec![],
                vec![],
            );
            repo.create(input)
                .await
                .expect("Failed to create conversation");
        }

        // Get all conversations to find middle cursor
        let all = repo
            .list_by_owner(
                ConversationOwnerType::User,
                user_id,
                ListParams {
                    limit: Some(100),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list all");

        assert_eq!(all.items.len(), 5);

        // Get the cursor from the 3rd item (index 2) and go backward
        let middle_cursor = SqliteConversationRepo::cursor_from_conversation(&all.items[2]);

        let backward_page = repo
            .list_by_owner(
                ConversationOwnerType::User,
                user_id,
                ListParams {
                    limit: Some(2),
                    cursor: Some(middle_cursor),
                    direction: CursorDirection::Backward,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list backward");

        // Should get items before the cursor (items at index 0 and 1)
        assert_eq!(backward_page.items.len(), 2);
        // Items should be in descending order (newest first)
        assert!(backward_page.items[0].updated_at >= backward_page.items[1].updated_at);
    }

    #[tokio::test]
    async fn test_offset_pagination_returns_cursors() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        // Create 3 conversations
        for i in 0..3 {
            let input = create_conversation_input(
                ConversationOwner::User { user_id },
                &format!("Offset Cursor Conv {}", i),
                vec![],
                vec![],
            );
            repo.create(input)
                .await
                .expect("Failed to create conversation");
        }

        // Use offset-based pagination
        let result = repo
            .list_by_owner(
                ConversationOwnerType::User,
                user_id,
                ListParams {
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list");

        assert_eq!(result.items.len(), 2);
        assert!(result.has_more);
        // Should still have cursors for hybrid navigation
        assert!(result.cursors.next.is_some());
    }

    // ==================== Count by Owner Tests ====================

    #[tokio::test]
    async fn test_count_by_owner_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let count = repo
            .count_by_owner(ConversationOwnerType::User, user_id, false)
            .await
            .expect("Failed to count");

        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_count_by_owner_with_records() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        for i in 0..4 {
            let input = create_conversation_input(
                ConversationOwner::User { user_id },
                &format!("Conv {}", i),
                vec![],
                vec![],
            );
            repo.create(input)
                .await
                .expect("Failed to create conversation");
        }

        let count = repo
            .count_by_owner(ConversationOwnerType::User, user_id, false)
            .await
            .expect("Failed to count");

        assert_eq!(count, 4);
    }

    #[tokio::test]
    async fn test_count_by_owner_excludes_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();

        let input1 = create_conversation_input(
            ConversationOwner::User { user_id },
            "Active",
            vec![],
            vec![],
        );
        repo.create(input1).await.expect("Failed to create");

        let input2 = create_conversation_input(
            ConversationOwner::User { user_id },
            "To Delete",
            vec![],
            vec![],
        );
        let to_delete = repo.create(input2).await.expect("Failed to create");
        repo.delete(to_delete.id).await.expect("Failed to delete");

        let count = repo
            .count_by_owner(ConversationOwnerType::User, user_id, false)
            .await
            .expect("Failed to count");

        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_count_by_owner_include_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();

        let input1 = create_conversation_input(
            ConversationOwner::User { user_id },
            "Active",
            vec![],
            vec![],
        );
        repo.create(input1).await.expect("Failed to create");

        let input2 = create_conversation_input(
            ConversationOwner::User { user_id },
            "To Delete",
            vec![],
            vec![],
        );
        let to_delete = repo.create(input2).await.expect("Failed to create");
        repo.delete(to_delete.id).await.expect("Failed to delete");

        let count = repo
            .count_by_owner(ConversationOwnerType::User, user_id, true)
            .await
            .expect("Failed to count");

        assert_eq!(count, 2);
    }

    // ==================== Update Tests ====================

    #[tokio::test]
    async fn test_update_title() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Original Title",
            vec!["gpt-4"],
            vec![],
        );
        let created = repo.create(input).await.expect("Failed to create");

        let updated = repo
            .update(
                created.id,
                UpdateConversation {
                    title: Some("New Title".to_string()),
                    models: None,
                    messages: None,
                    owner: None,
                },
            )
            .await
            .expect("Failed to update");

        assert_eq!(updated.title, "New Title");
        assert_eq!(updated.models, vec!["gpt-4"]); // Unchanged
    }

    #[tokio::test]
    async fn test_update_models() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Test",
            vec!["gpt-4"],
            vec![],
        );
        let created = repo.create(input).await.expect("Failed to create");

        let updated = repo
            .update(
                created.id,
                UpdateConversation {
                    title: None,
                    models: Some(vec!["claude-3-opus".to_string(), "gemini-pro".to_string()]),
                    messages: None,
                    owner: None,
                },
            )
            .await
            .expect("Failed to update");

        assert_eq!(updated.title, "Test"); // Unchanged
        assert_eq!(updated.models, vec!["claude-3-opus", "gemini-pro"]);
    }

    #[tokio::test]
    async fn test_update_messages() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Test",
            vec![],
            vec![create_message("user", "Original")],
        );
        let created = repo.create(input).await.expect("Failed to create");

        let new_messages = vec![
            create_message("user", "Replaced"),
            create_message("assistant", "Response"),
        ];
        let updated = repo
            .update(
                created.id,
                UpdateConversation {
                    title: None,
                    models: None,
                    messages: Some(new_messages),
                    owner: None,
                },
            )
            .await
            .expect("Failed to update");

        assert_eq!(updated.messages.len(), 2);
        assert_eq!(updated.messages[0].content, "Replaced");
        assert_eq!(updated.messages[1].content, "Response");
    }

    #[tokio::test]
    async fn test_update_all_fields() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Original",
            vec!["gpt-4"],
            vec![],
        );
        let created = repo.create(input).await.expect("Failed to create");

        let updated = repo
            .update(
                created.id,
                UpdateConversation {
                    title: Some("Updated".to_string()),
                    models: Some(vec!["claude-3".to_string()]),
                    messages: Some(vec![create_message("system", "Hello")]),
                    owner: None,
                },
            )
            .await
            .expect("Failed to update");

        assert_eq!(updated.title, "Updated");
        assert_eq!(updated.models, vec!["claude-3"]);
        assert_eq!(updated.messages.len(), 1);
        assert_eq!(updated.messages[0].role, "system");
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let result = repo
            .update(
                Uuid::new_v4(),
                UpdateConversation {
                    title: Some("Test".to_string()),
                    models: None,
                    messages: None,
                    owner: None,
                },
            )
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_update_deleted_conversation_fails() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input =
            create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
        let created = repo.create(input).await.expect("Failed to create");
        repo.delete(created.id).await.expect("Failed to delete");

        let result = repo
            .update(
                created.id,
                UpdateConversation {
                    title: Some("Updated".to_string()),
                    models: None,
                    messages: None,
                    owner: None,
                },
            )
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_update_preserves_timestamps() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input =
            create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
        let created = repo.create(input).await.expect("Failed to create");

        // Small delay to ensure updated_at differs
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let updated = repo
            .update(
                created.id,
                UpdateConversation {
                    title: Some("Updated".to_string()),
                    models: None,
                    messages: None,
                    owner: None,
                },
            )
            .await
            .expect("Failed to update");

        assert_eq!(updated.created_at, created.created_at);
        assert!(updated.updated_at > created.updated_at);
    }

    // ==================== Append Messages Tests ====================

    #[tokio::test]
    async fn test_append_messages_to_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input =
            create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
        let created = repo.create(input).await.expect("Failed to create");

        let messages = repo
            .append_messages(
                created.id,
                AppendMessages {
                    messages: vec![
                        create_message("user", "Hello"),
                        create_message("assistant", "Hi!"),
                    ],
                },
            )
            .await
            .expect("Failed to append messages");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].content, "Hi!");
    }

    #[tokio::test]
    async fn test_append_messages_to_existing() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Test",
            vec![],
            vec![create_message("user", "First")],
        );
        let created = repo.create(input).await.expect("Failed to create");

        let messages = repo
            .append_messages(
                created.id,
                AppendMessages {
                    messages: vec![
                        create_message("assistant", "Second"),
                        create_message("user", "Third"),
                    ],
                },
            )
            .await
            .expect("Failed to append messages");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].content, "First");
        assert_eq!(messages[1].content, "Second");
        assert_eq!(messages[2].content, "Third");
    }

    #[tokio::test]
    async fn test_append_messages_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let result = repo
            .append_messages(
                Uuid::new_v4(),
                AppendMessages {
                    messages: vec![create_message("user", "Test")],
                },
            )
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_append_messages_to_deleted_fails() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input =
            create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
        let created = repo.create(input).await.expect("Failed to create");
        repo.delete(created.id).await.expect("Failed to delete");

        let result = repo
            .append_messages(
                created.id,
                AppendMessages {
                    messages: vec![create_message("user", "Test")],
                },
            )
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_append_messages_updates_timestamp() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input =
            create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
        let created = repo.create(input).await.expect("Failed to create");

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        repo.append_messages(
            created.id,
            AppendMessages {
                messages: vec![create_message("user", "Test")],
            },
        )
        .await
        .expect("Failed to append messages");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert!(fetched.updated_at > created.updated_at);
    }

    // ==================== Delete Tests ====================

    #[tokio::test]
    async fn test_delete_conversation() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input =
            create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
        let created = repo.create(input).await.expect("Failed to create");

        repo.delete(created.id).await.expect("Failed to delete");

        let result = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let result = repo.delete(Uuid::new_v4()).await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete_already_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input =
            create_conversation_input(ConversationOwner::User { user_id }, "Test", vec![], vec![]);
        let created = repo.create(input).await.expect("Failed to create");
        repo.delete(created.id)
            .await
            .expect("First delete should succeed");

        let result = repo.delete(created.id).await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    // ==================== Edge Case Tests ====================

    #[tokio::test]
    async fn test_messages_with_special_characters() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let special_content = r#"Hello "world"! What's up? <script>alert('xss')</script>"#;
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Test",
            vec![],
            vec![create_message("user", special_content)],
        );
        let created = repo.create(input).await.expect("Failed to create");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.messages[0].content, special_content);
    }

    #[tokio::test]
    async fn test_unicode_content() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let unicode_content = "Hello 你好 مرحبا 🌍 🚀 ñ é ü";
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Test 日本語",
            vec!["model-日本語"],
            vec![create_message("user", unicode_content)],
        );
        let created = repo.create(input).await.expect("Failed to create");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.title, "Test 日本語");
        assert_eq!(fetched.models[0], "model-日本語");
        assert_eq!(fetched.messages[0].content, unicode_content);
    }

    #[tokio::test]
    async fn test_empty_models_vec() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "No Models",
            vec![],
            vec![],
        );
        let created = repo.create(input).await.expect("Failed to create");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert!(fetched.models.is_empty());
    }

    #[tokio::test]
    async fn test_update_to_empty_models() {
        let pool = create_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Test",
            vec!["gpt-4"],
            vec![],
        );
        let created = repo.create(input).await.expect("Failed to create");

        let updated = repo
            .update(
                created.id,
                UpdateConversation {
                    title: None,
                    models: Some(vec![]),
                    messages: None,
                    owner: None,
                },
            )
            .await
            .expect("Failed to update");

        assert!(updated.models.is_empty());
    }

    // ==================== List Accessible For User Tests ====================

    /// Create a test pool with additional tables needed for list_accessible_for_user
    async fn create_test_pool_with_projects() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        sqlx::query(
            r#"
            CREATE TABLE conversations (
                id TEXT PRIMARY KEY NOT NULL,
                owner_type TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                title TEXT NOT NULL,
                models TEXT NOT NULL DEFAULT '[]',
                messages TEXT NOT NULL DEFAULT '[]',
                pin_order INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create conversations table");

        sqlx::query(
            r#"
            CREATE TABLE projects (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL,
                slug TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create projects table");

        sqlx::query(
            r#"
            CREATE TABLE project_memberships (
                project_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'member',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (project_id, user_id)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create project_memberships table");

        pool
    }

    #[tokio::test]
    async fn test_list_accessible_for_user_only_personal() {
        let pool = create_test_pool_with_projects().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();

        // Create personal conversations
        for i in 0..3 {
            let input = create_conversation_input(
                ConversationOwner::User { user_id },
                &format!("Personal Conv {}", i),
                vec!["gpt-4"],
                vec![],
            );
            repo.create(input)
                .await
                .expect("Failed to create conversation");
        }

        let result = repo
            .list_accessible_for_user(user_id, 100, false)
            .await
            .expect("Failed to list");

        assert_eq!(result.len(), 3);
        // Personal conversations should have no project info
        for conv in &result {
            assert!(conv.project_id.is_none());
            assert!(conv.project_name.is_none());
            assert!(conv.project_slug.is_none());
        }
    }

    #[tokio::test]
    async fn test_list_accessible_for_user_with_project_conversations() {
        let pool = create_test_pool_with_projects().await;
        let repo = SqliteConversationRepo::new(pool.clone());

        let user_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        // Create project
        sqlx::query("INSERT INTO projects (id, org_id, slug, name) VALUES (?, ?, ?, ?)")
            .bind(project_id.to_string())
            .bind(org_id.to_string())
            .bind("test-project")
            .bind("Test Project")
            .execute(&pool)
            .await
            .expect("Failed to create project");

        // Add user to project
        sqlx::query("INSERT INTO project_memberships (project_id, user_id) VALUES (?, ?)")
            .bind(project_id.to_string())
            .bind(user_id.to_string())
            .execute(&pool)
            .await
            .expect("Failed to add project membership");

        // Create personal conversation
        let personal_input = create_conversation_input(
            ConversationOwner::User { user_id },
            "Personal Conv",
            vec!["gpt-4"],
            vec![],
        );
        repo.create(personal_input)
            .await
            .expect("Failed to create personal conversation");

        // Create project conversation
        let project_input = create_conversation_input(
            ConversationOwner::Project { project_id },
            "Project Conv",
            vec!["claude-3"],
            vec![],
        );
        repo.create(project_input)
            .await
            .expect("Failed to create project conversation");

        let result = repo
            .list_accessible_for_user(user_id, 100, false)
            .await
            .expect("Failed to list");

        assert_eq!(result.len(), 2);

        // Find the project conversation
        let project_conv = result
            .iter()
            .find(|c| c.conversation.title == "Project Conv")
            .expect("Project conversation not found");

        assert_eq!(project_conv.project_id, Some(project_id));
        assert_eq!(project_conv.project_name, Some("Test Project".to_string()));
        assert_eq!(project_conv.project_slug, Some("test-project".to_string()));

        // Find the personal conversation
        let personal_conv = result
            .iter()
            .find(|c| c.conversation.title == "Personal Conv")
            .expect("Personal conversation not found");

        assert!(personal_conv.project_id.is_none());
    }

    #[tokio::test]
    async fn test_list_accessible_for_user_excludes_non_member_projects() {
        let pool = create_test_pool_with_projects().await;
        let repo = SqliteConversationRepo::new(pool.clone());

        let user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        // Create project
        sqlx::query("INSERT INTO projects (id, org_id, slug, name) VALUES (?, ?, ?, ?)")
            .bind(project_id.to_string())
            .bind(org_id.to_string())
            .bind("test-project")
            .bind("Test Project")
            .execute(&pool)
            .await
            .expect("Failed to create project");

        // Add OTHER user to project (not our test user)
        sqlx::query("INSERT INTO project_memberships (project_id, user_id) VALUES (?, ?)")
            .bind(project_id.to_string())
            .bind(other_user_id.to_string())
            .execute(&pool)
            .await
            .expect("Failed to add project membership");

        // Create project conversation
        let project_input = create_conversation_input(
            ConversationOwner::Project { project_id },
            "Project Conv",
            vec!["claude-3"],
            vec![],
        );
        repo.create(project_input)
            .await
            .expect("Failed to create project conversation");

        // Our user should NOT see this project conversation
        let result = repo
            .list_accessible_for_user(user_id, 100, false)
            .await
            .expect("Failed to list");

        assert_eq!(result.len(), 0);
    }

    #[tokio::test]
    async fn test_list_accessible_for_user_respects_limit() {
        let pool = create_test_pool_with_projects().await;
        let repo = SqliteConversationRepo::new(pool);

        let user_id = Uuid::new_v4();

        // Create 5 personal conversations
        for i in 0..5 {
            let input = create_conversation_input(
                ConversationOwner::User { user_id },
                &format!("Conv {}", i),
                vec![],
                vec![],
            );
            repo.create(input)
                .await
                .expect("Failed to create conversation");
        }

        let result = repo
            .list_accessible_for_user(user_id, 3, false)
            .await
            .expect("Failed to list");

        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_list_accessible_for_user_excludes_deleted_projects() {
        let pool = create_test_pool_with_projects().await;
        let repo = SqliteConversationRepo::new(pool.clone());

        let user_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        // Create deleted project
        sqlx::query(
            "INSERT INTO projects (id, org_id, slug, name, deleted_at) VALUES (?, ?, ?, ?, datetime('now'))",
        )
        .bind(project_id.to_string())
        .bind(org_id.to_string())
        .bind("deleted-project")
        .bind("Deleted Project")
        .execute(&pool)
        .await
        .expect("Failed to create project");

        // Add user to project
        sqlx::query("INSERT INTO project_memberships (project_id, user_id) VALUES (?, ?)")
            .bind(project_id.to_string())
            .bind(user_id.to_string())
            .execute(&pool)
            .await
            .expect("Failed to add project membership");

        // Create project conversation
        let project_input = create_conversation_input(
            ConversationOwner::Project { project_id },
            "Project Conv",
            vec![],
            vec![],
        );
        repo.create(project_input)
            .await
            .expect("Failed to create project conversation");

        // User should NOT see conversations from deleted projects
        let result = repo
            .list_accessible_for_user(user_id, 100, false)
            .await
            .expect("Failed to list");

        assert_eq!(result.len(), 0);
    }

    // ==================== Org-scoped Get Tests ====================

    /// Create a test pool with additional tables needed for org-scoped queries.
    async fn create_org_scoped_test_pool() -> SqlitePool {
        let pool = create_test_pool().await;

        sqlx::query(
            r#"
            CREATE TABLE projects (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL,
                slug TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create projects table");

        sqlx::query(
            r#"
            CREATE TABLE org_memberships (
                id TEXT PRIMARY KEY NOT NULL,
                user_id TEXT NOT NULL,
                org_id TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'member',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create org_memberships table");

        pool
    }

    #[tokio::test]
    async fn test_get_by_id_and_org_project_owned() {
        let pool = create_org_scoped_test_pool().await;
        let repo = SqliteConversationRepo::new(pool.clone());

        let org_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();

        // Insert a project in this org
        sqlx::query("INSERT INTO projects (id, org_id, slug, name) VALUES (?, ?, ?, ?)")
            .bind(project_id.to_string())
            .bind(org_id.to_string())
            .bind("test-project")
            .bind("Test Project")
            .execute(&pool)
            .await
            .expect("Failed to create project");

        // Create a conversation owned by the project
        let input = create_conversation_input(
            ConversationOwner::Project { project_id },
            "Org-scoped conv",
            vec!["gpt-4"],
            vec![],
        );
        let conv = repo
            .create(input)
            .await
            .expect("Failed to create conversation");

        // Should find it with the correct org
        let found = repo
            .get_by_id_and_org(conv.id, org_id)
            .await
            .expect("Query failed");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, conv.id);

        // Should NOT find it with a different org
        let other_org = Uuid::new_v4();
        let not_found = repo
            .get_by_id_and_org(conv.id, other_org)
            .await
            .expect("Query failed");
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_get_by_id_and_org_user_owned() {
        let pool = create_org_scoped_test_pool().await;
        let repo = SqliteConversationRepo::new(pool.clone());

        let org_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Insert an org membership for the user
        sqlx::query("INSERT INTO org_memberships (id, user_id, org_id) VALUES (?, ?, ?)")
            .bind(Uuid::new_v4().to_string())
            .bind(user_id.to_string())
            .bind(org_id.to_string())
            .execute(&pool)
            .await
            .expect("Failed to create org membership");

        // Create a conversation owned by the user
        let input = create_conversation_input(
            ConversationOwner::User { user_id },
            "User org-scoped conv",
            vec!["claude-3"],
            vec![],
        );
        let conv = repo
            .create(input)
            .await
            .expect("Failed to create conversation");

        // Should find it with the correct org
        let found = repo
            .get_by_id_and_org(conv.id, org_id)
            .await
            .expect("Query failed");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, conv.id);

        // Should NOT find it with a different org
        let other_org = Uuid::new_v4();
        let not_found = repo
            .get_by_id_and_org(conv.id, other_org)
            .await
            .expect("Query failed");
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_get_by_id_and_org_nonexistent() {
        let pool = create_org_scoped_test_pool().await;
        let repo = SqliteConversationRepo::new(pool);

        let result = repo
            .get_by_id_and_org(Uuid::new_v4(), Uuid::new_v4())
            .await
            .expect("Query failed");
        assert!(result.is_none());
    }
}
