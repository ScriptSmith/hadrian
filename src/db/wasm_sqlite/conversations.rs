use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{ConversationRepo, Cursor, CursorDirection, ListParams, ListResult, PageCursors},
        wasm_sqlite::{WasmSqlitePool, query as wasm_query},
    },
    models::{
        AppendMessages, Conversation, ConversationOwnerType, ConversationWithProject,
        CreateConversation, Message, UpdateConversation,
    },
};

pub struct WasmSqliteConversationRepo {
    pool: WasmSqlitePool,
}

impl WasmSqliteConversationRepo {
    pub fn new(pool: WasmSqlitePool) -> Self {
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

        let rows = wasm_query(&query)
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
                    id: parse_uuid(&row.get::<String>("id"))?,
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: parse_uuid(&row.get::<String>("owner_id"))?,
                    title: row.get("title"),
                    models: Self::parse_models(&models_json)?,
                    messages: Self::parse_messages(&messages_json)?,
                    pin_order: row.get::<Option<i64>>("pin_order").map(|v| v as i32),
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

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ConversationRepo for WasmSqliteConversationRepo {
    async fn create(&self, input: CreateConversation) -> DbResult<Conversation> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let owner_type = input.owner.owner_type();
        let owner_id = input.owner.owner_id();
        let models_json =
            serde_json::to_string(&input.models).map_err(|e| DbError::Internal(e.to_string()))?;
        let messages_json =
            serde_json::to_string(&input.messages).map_err(|e| DbError::Internal(e.to_string()))?;

        wasm_query(
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
        let result = wasm_query(
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
                    id: parse_uuid(&row.get::<String>("id"))?,
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: parse_uuid(&row.get::<String>("owner_id"))?,
                    title: row.get("title"),
                    models: Self::parse_models(&models_json)?,
                    messages: Self::parse_messages(&messages_json)?,
                    pin_order: row.get::<Option<i64>>("pin_order").map(|v| v as i32),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Conversation>> {
        let result = wasm_query(
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
                    id: parse_uuid(&row.get::<String>("id"))?,
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: parse_uuid(&row.get::<String>("owner_id"))?,
                    title: row.get("title"),
                    models: Self::parse_models(&models_json)?,
                    messages: Self::parse_messages(&messages_json)?,
                    pin_order: row.get::<Option<i64>>("pin_order").map(|v| v as i32),
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

        let rows = wasm_query(query)
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
                    id: parse_uuid(&row.get::<String>("id"))?,
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: parse_uuid(&row.get::<String>("owner_id"))?,
                    title: row.get("title"),
                    models: Self::parse_models(&models_json)?,
                    messages: Self::parse_messages(&messages_json)?,
                    pin_order: row.get::<Option<i64>>("pin_order").map(|v| v as i32),
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

        let row = wasm_query(query)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateConversation) -> DbResult<Conversation> {
        let now = chrono::Utc::now();

        // Read current state
        let current_row = wasm_query(
            r#"
            SELECT id, owner_type, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;

        let current_owner_type_str: String = current_row.get("owner_type");
        let current_owner_type: ConversationOwnerType = current_owner_type_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;
        let current_owner_id = parse_uuid(&current_row.get::<String>("owner_id"))?;
        let current_title: String = current_row.get("title");
        let current_models_json: String = current_row.get("models");
        let current_messages_json: String = current_row.get("messages");
        let pin_order: Option<i32> = current_row
            .get::<Option<i64>>("pin_order")
            .map(|v| v as i32);
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
        let new_messages = input
            .messages
            .unwrap_or_else(|| Self::parse_messages(&current_messages_json).unwrap_or_default());
        let models_json =
            serde_json::to_string(&new_models).map_err(|e| DbError::Internal(e.to_string()))?;
        let messages_json =
            serde_json::to_string(&new_messages).map_err(|e| DbError::Internal(e.to_string()))?;

        let update_result = wasm_query(
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
        .execute(&self.pool)
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

    async fn append_messages(&self, id: Uuid, input: AppendMessages) -> DbResult<Vec<Message>> {
        let now = chrono::Utc::now();

        // Get current messages
        let current_row = wasm_query(
            r#"
            SELECT messages
            FROM conversations
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;

        let current_messages_json: String = current_row.get("messages");
        let mut messages = Self::parse_messages(&current_messages_json)?;

        // Append new messages
        messages.extend(input.messages);

        let messages_json =
            serde_json::to_string(&messages).map_err(|e| DbError::Internal(e.to_string()))?;

        let update_result = wasm_query(
            r#"
            UPDATE conversations
            SET messages = ?, updated_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&messages_json)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if update_result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(messages)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = wasm_query(
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

        let rows = wasm_query(&query)
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
                        id: parse_uuid(&row.get::<String>("id"))?,
                        owner_type: owner_type_str
                            .parse()
                            .map_err(|e: String| DbError::Internal(e))?,
                        owner_id: parse_uuid(&row.get::<String>("owner_id"))?,
                        title: row.get("title"),
                        models: Self::parse_models(&models_json)?,
                        messages: Self::parse_messages(&messages_json)?,
                        pin_order: row.get::<Option<i64>>("pin_order").map(|v| v as i32),
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

        // Read current state
        let current_row = wasm_query(
            r#"
            SELECT id, owner_type, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;

        let owner_type_str: String = current_row.get("owner_type");
        let owner_type: ConversationOwnerType = owner_type_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;
        let owner_id = parse_uuid(&current_row.get::<String>("owner_id"))?;
        let title: String = current_row.get("title");
        let models_json: String = current_row.get("models");
        let messages_json: String = current_row.get("messages");
        let created_at = current_row.get("created_at");

        let update_result = wasm_query(
            r#"
            UPDATE conversations
            SET pin_order = ?, updated_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(pin_order)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
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
            let result = wasm_query(
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
