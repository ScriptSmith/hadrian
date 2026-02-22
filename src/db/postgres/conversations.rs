use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

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

pub struct PostgresConversationRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresConversationRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    fn parse_messages(json_value: serde_json::Value) -> DbResult<Vec<Message>> {
        serde_json::from_value(json_value).map_err(|e| DbError::Internal(e.to_string()))
    }

    fn parse_models(json_value: serde_json::Value) -> DbResult<Vec<String>> {
        serde_json::from_value(json_value).map_err(|e| DbError::Internal(e.to_string()))
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
            SELECT id, owner_type::TEXT, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE owner_type = $1::conversation_owner_type AND owner_id = $2
            AND ROW(updated_at, id) {} ROW($3, $4)
            {}
            ORDER BY updated_at {}, id {}
            LIMIT $5
            "#,
            comparison, deleted_filter, order, order
        );

        let rows = sqlx::query(&query)
            .bind(owner_type.as_str())
            .bind(owner_id)
            .bind(cursor.created_at) // cursor.created_at holds updated_at value
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<Conversation> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let owner_type_str: String = row.get("owner_type");

                Ok(Conversation {
                    id: row.get("id"),
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: row.get("owner_id"),
                    title: row.get("title"),
                    models: Self::parse_models(row.get("models"))?,
                    messages: Self::parse_messages(row.get("messages"))?,
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
impl ConversationRepo for PostgresConversationRepo {
    async fn create(&self, input: CreateConversation) -> DbResult<Conversation> {
        let owner_type = input.owner.owner_type();
        let owner_id = input.owner.owner_id();
        let models_json =
            serde_json::to_value(&input.models).map_err(|e| DbError::Internal(e.to_string()))?;
        let messages_json =
            serde_json::to_value(&input.messages).map_err(|e| DbError::Internal(e.to_string()))?;

        let row = sqlx::query(
            r#"
            INSERT INTO conversations (id, owner_type, owner_id, title, models, messages, pin_order)
            VALUES ($1, $2::conversation_owner_type, $3, $4, $5, $6, NULL)
            RETURNING id, owner_type::TEXT, owner_id, title, models, messages, pin_order, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(owner_type.as_str())
        .bind(owner_id)
        .bind(&input.title)
        .bind(&models_json)
        .bind(&messages_json)
        .fetch_one(&self.write_pool)
        .await?;

        let owner_type_str: String = row.get("owner_type");

        Ok(Conversation {
            id: row.get("id"),
            owner_type: owner_type_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            owner_id: row.get("owner_id"),
            title: row.get("title"),
            models: Self::parse_models(row.get("models"))?,
            messages: Self::parse_messages(row.get("messages"))?,
            pin_order: row.get("pin_order"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Conversation>> {
        let result = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        match result {
            Some(row) => {
                let owner_type_str: String = row.get("owner_type");

                Ok(Some(Conversation {
                    id: row.get("id"),
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: row.get("owner_id"),
                    title: row.get("title"),
                    models: Self::parse_models(row.get("models"))?,
                    messages: Self::parse_messages(row.get("messages"))?,
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
            SELECT c.id, c.owner_type::TEXT, c.owner_id, c.title, c.models, c.messages, c.pin_order, c.created_at, c.updated_at
            FROM conversations c
            WHERE c.id = $1 AND c.deleted_at IS NULL
            AND (
                (c.owner_type = 'project'::conversation_owner_type AND EXISTS (
                    SELECT 1 FROM projects p WHERE p.id = c.owner_id AND p.org_id = $2
                ))
                OR
                (c.owner_type = 'user'::conversation_owner_type AND EXISTS (
                    SELECT 1 FROM org_memberships om WHERE om.user_id = c.owner_id AND om.org_id = $3
                ))
            )
            "#,
        )
        .bind(id)
        .bind(org_id)
        .bind(org_id)
        .fetch_optional(&self.read_pool)
        .await?;

        match result {
            Some(row) => {
                let owner_type_str: String = row.get("owner_type");

                Ok(Some(Conversation {
                    id: row.get("id"),
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: row.get("owner_id"),
                    title: row.get("title"),
                    models: Self::parse_models(row.get("models"))?,
                    messages: Self::parse_messages(row.get("messages"))?,
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
            SELECT id, owner_type::TEXT, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE owner_type = $1::conversation_owner_type AND owner_id = $2
            ORDER BY updated_at DESC, id DESC
            LIMIT $3
            "#
        } else {
            r#"
            SELECT id, owner_type::TEXT, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE owner_type = $1::conversation_owner_type AND owner_id = $2 AND deleted_at IS NULL
            ORDER BY updated_at DESC, id DESC
            LIMIT $3
            "#
        };

        let rows = sqlx::query(query)
            .bind(owner_type.as_str())
            .bind(owner_id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<Conversation> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let owner_type_str: String = row.get("owner_type");

                Ok(Conversation {
                    id: row.get("id"),
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: row.get("owner_id"),
                    title: row.get("title"),
                    models: Self::parse_models(row.get("models"))?,
                    messages: Self::parse_messages(row.get("messages"))?,
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
            "SELECT COUNT(*) as count FROM conversations WHERE owner_type = $1::conversation_owner_type AND owner_id = $2"
        } else {
            "SELECT COUNT(*) as count FROM conversations WHERE owner_type = $1::conversation_owner_type AND owner_id = $2 AND deleted_at IS NULL"
        };

        let row = sqlx::query(query)
            .bind(owner_type.as_str())
            .bind(owner_id)
            .fetch_one(&self.read_pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateConversation) -> DbResult<Conversation> {
        // Use a transaction with FOR UPDATE to prevent lost updates from concurrent modifications
        let mut tx = self.write_pool.begin().await?;

        // Lock the row for update to prevent concurrent modifications
        let current_row = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, title, models, messages, pin_order, created_at, updated_at
            FROM conversations
            WHERE id = $1 AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DbError::NotFound)?;

        let current_owner_type_str: String = current_row.get("owner_type");
        let current_owner_type: ConversationOwnerType = current_owner_type_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;
        let current_owner_id: Uuid = current_row.get("owner_id");
        let current_title: String = current_row.get("title");
        let current_models: serde_json::Value = current_row.get("models");
        let current_messages: serde_json::Value = current_row.get("messages");
        let pin_order: Option<i32> = current_row.get("pin_order");

        // Determine new owner (if provided) or keep current
        let (new_owner_type, new_owner_id) = if let Some(ref owner) = input.owner {
            (owner.owner_type(), owner.owner_id())
        } else {
            (current_owner_type, current_owner_id)
        };

        let new_title = input.title.unwrap_or(current_title);
        let new_models = input
            .models
            .map(|m| serde_json::to_value(&m).map_err(|e| DbError::Internal(e.to_string())))
            .transpose()?
            .unwrap_or(current_models);
        let new_messages = input
            .messages
            .map(|m| serde_json::to_value(&m).map_err(|e| DbError::Internal(e.to_string())))
            .transpose()?
            .unwrap_or(current_messages);

        let row = sqlx::query(
            r#"
            UPDATE conversations
            SET owner_type = $1::conversation_owner_type, owner_id = $2, title = $3, models = $4, messages = $5, updated_at = NOW()
            WHERE id = $6 AND deleted_at IS NULL
            RETURNING id, owner_type::TEXT, owner_id, title, models, messages, pin_order, created_at, updated_at
            "#,
        )
        .bind(new_owner_type.as_str())
        .bind(new_owner_id)
        .bind(&new_title)
        .bind(&new_models)
        .bind(&new_messages)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DbError::NotFound)?;

        tx.commit().await?;

        Ok(Conversation {
            id: row.get("id"),
            owner_type: new_owner_type,
            owner_id: new_owner_id,
            title: row.get("title"),
            models: Self::parse_models(row.get("models"))?,
            messages: Self::parse_messages(row.get("messages"))?,
            pin_order,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn append_messages(&self, id: Uuid, input: AppendMessages) -> DbResult<Vec<Message>> {
        // Use atomic JSONB concatenation to append messages without read-modify-write
        let new_messages_json =
            serde_json::to_value(&input.messages).map_err(|e| DbError::Internal(e.to_string()))?;

        let row = sqlx::query(
            r#"
            UPDATE conversations
            SET messages = messages || $1::jsonb, updated_at = NOW()
            WHERE id = $2 AND deleted_at IS NULL
            RETURNING messages
            "#,
        )
        .bind(&new_messages_json)
        .bind(id)
        .fetch_optional(&self.write_pool)
        .await?
        .ok_or(DbError::NotFound)?;

        Self::parse_messages(row.get("messages"))
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE conversations
            SET deleted_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .execute(&self.write_pool)
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
                c.owner_type::TEXT,
                c.owner_id,
                c.title,
                c.models,
                c.messages,
                c.pin_order,
                c.created_at,
                c.updated_at,
                NULL::UUID as project_id,
                NULL::VARCHAR as project_name,
                NULL::VARCHAR as project_slug
            FROM conversations c
            WHERE c.owner_type = 'user'::conversation_owner_type AND c.owner_id = $1
            {deleted_filter}

            UNION ALL

            SELECT
                c.id,
                c.owner_type::TEXT,
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
            INNER JOIN project_memberships pm ON pm.project_id = c.owner_id AND pm.user_id = $2
            INNER JOIN projects p ON p.id = c.owner_id AND p.deleted_at IS NULL
            WHERE c.owner_type = 'project'::conversation_owner_type
            {deleted_filter}

            ORDER BY updated_at DESC, id DESC
            LIMIT $3
            "#
        );

        let rows = sqlx::query(&query)
            .bind(user_id)
            .bind(user_id)
            .bind(limit)
            .fetch_all(&self.read_pool)
            .await?;

        let items: Vec<ConversationWithProject> = rows
            .into_iter()
            .map(|row| {
                let owner_type_str: String = row.get("owner_type");
                let project_id: Option<Uuid> = row.get("project_id");
                let project_name: Option<String> = row.get("project_name");
                let project_slug: Option<String> = row.get("project_slug");

                Ok(ConversationWithProject {
                    conversation: Conversation {
                        id: row.get("id"),
                        owner_type: owner_type_str
                            .parse()
                            .map_err(|e: String| DbError::Internal(e))?,
                        owner_id: row.get("owner_id"),
                        title: row.get("title"),
                        models: Self::parse_models(row.get("models"))?,
                        messages: Self::parse_messages(row.get("messages"))?,
                        pin_order: row.get("pin_order"),
                        created_at: row.get("created_at"),
                        updated_at: row.get("updated_at"),
                    },
                    project_id,
                    project_name,
                    project_slug,
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        Ok(items)
    }

    async fn set_pin_order(&self, id: Uuid, pin_order: Option<i32>) -> DbResult<Conversation> {
        let row = sqlx::query(
            r#"
            UPDATE conversations
            SET pin_order = $1, updated_at = NOW()
            WHERE id = $2 AND deleted_at IS NULL
            RETURNING id, owner_type::TEXT, owner_id, title, models, messages, pin_order, created_at, updated_at
            "#,
        )
        .bind(pin_order)
        .bind(id)
        .fetch_optional(&self.write_pool)
        .await?
        .ok_or(DbError::NotFound)?;

        let owner_type_str: String = row.get("owner_type");

        Ok(Conversation {
            id: row.get("id"),
            owner_type: owner_type_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            owner_id: row.get("owner_id"),
            title: row.get("title"),
            models: Self::parse_models(row.get("models"))?,
            messages: Self::parse_messages(row.get("messages"))?,
            pin_order: row.get("pin_order"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
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

            // PostgreSQL efficient batched deletion using ctid
            // Hard delete conversations that were soft-deleted before the cutoff
            let result = sqlx::query(
                r#"
                DELETE FROM conversations
                WHERE ctid IN (
                    SELECT ctid FROM conversations
                    WHERE deleted_at IS NOT NULL AND deleted_at < $1
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
