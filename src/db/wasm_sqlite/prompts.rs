use std::collections::HashMap;

use async_trait::async_trait;
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, PromptRepo,
            cursor_from_row,
        },
        wasm_sqlite::{WasmRow, WasmSqlitePool, query as wasm_query},
    },
    models::{CreatePrompt, Prompt, PromptOwnerType, UpdatePrompt},
};

pub struct WasmSqlitePromptRepo {
    pool: WasmSqlitePool,
}

impl WasmSqlitePromptRepo {
    pub fn new(pool: WasmSqlitePool) -> Self {
        Self { pool }
    }

    /// Parse a Prompt from a database row.
    fn parse_prompt(row: &WasmRow) -> DbResult<Prompt> {
        let owner_type_str: String = row.get("owner_type");
        let owner_type: PromptOwnerType = owner_type_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;

        let metadata: Option<String> = row.get("metadata");
        let metadata: Option<HashMap<String, serde_json::Value>> = metadata
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| DbError::Internal(format!("Failed to parse metadata: {}", e)))?;

        Ok(Prompt {
            id: parse_uuid(&row.get::<String>("id"))?,
            owner_type,
            owner_id: parse_uuid(&row.get::<String>("owner_id"))?,
            name: row.get("name"),
            description: row.get("description"),
            content: row.get("content"),
            metadata,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Helper method for cursor-based pagination.
    async fn list_with_cursor(
        &self,
        owner_type: PromptOwnerType,
        owner_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<Prompt>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };

        let query = format!(
            r#"
            SELECT id, owner_type, owner_id, name, description, content, metadata, created_at, updated_at
            FROM prompts
            WHERE owner_type = ? AND owner_id = ? AND (created_at, id) {} (?, ?)
            {}
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, deleted_filter, order, order
        );

        let rows = wasm_query(&query)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<Prompt> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_prompt)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |p| {
                cursor_from_row(p.created_at, p.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl PromptRepo for WasmSqlitePromptRepo {
    async fn create(&self, input: CreatePrompt) -> DbResult<Prompt> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let owner_type = input.owner.owner_type();
        let owner_id = input.owner.owner_id();

        let metadata_json = input
            .metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| DbError::Internal(format!("Failed to serialize metadata: {}", e)))?;

        wasm_query(
            r#"
            INSERT INTO prompts (id, owner_type, owner_id, name, description, content, metadata, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(owner_type.as_str())
        .bind(owner_id.to_string())
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.content)
        .bind(&metadata_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if e.is_unique_violation() {
                DbError::Conflict(format!(
                    "Prompt with name '{}' already exists for this owner",
                    input.name
                ))
            } else {
                DbError::from(e)
            }
        })?;

        Ok(Prompt {
            id,
            owner_type,
            owner_id,
            name: input.name,
            description: input.description,
            content: input.content,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Prompt>> {
        let result = wasm_query(
            r#"
            SELECT id, owner_type, owner_id, name, description, content, metadata, created_at, updated_at
            FROM prompts
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_prompt(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Prompt>> {
        let result = wasm_query(
            r#"
            SELECT p.id, p.owner_type, p.owner_id, p.name, p.description, p.content, p.metadata, p.created_at, p.updated_at
            FROM prompts p
            WHERE p.id = ? AND p.deleted_at IS NULL
            AND (
                (p.owner_type = 'organization' AND p.owner_id = ?)
                OR
                (p.owner_type = 'team' AND EXISTS (
                    SELECT 1 FROM teams t WHERE t.id = p.owner_id AND t.org_id = ?
                ))
                OR
                (p.owner_type = 'project' AND EXISTS (
                    SELECT 1 FROM projects pr WHERE pr.id = p.owner_id AND pr.org_id = ?
                ))
                OR
                (p.owner_type = 'user' AND EXISTS (
                    SELECT 1 FROM org_memberships om WHERE om.user_id = p.owner_id AND om.org_id = ?
                ))
            )
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(org_id.to_string())
        .bind(org_id.to_string())
        .bind(org_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_prompt(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_by_owner(
        &self,
        owner_type: PromptOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Prompt>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(owner_type, owner_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        let query = if params.include_deleted {
            r#"
            SELECT id, owner_type, owner_id, name, description, content, metadata, created_at, updated_at
            FROM prompts
            WHERE owner_type = ? AND owner_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        } else {
            r#"
            SELECT id, owner_type, owner_id, name, description, content, metadata, created_at, updated_at
            FROM prompts
            WHERE owner_type = ? AND owner_id = ? AND deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
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
        let items: Vec<Prompt> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_prompt)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |p| {
                cursor_from_row(p.created_at, p.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_owner(
        &self,
        owner_type: PromptOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) as count FROM prompts WHERE owner_type = ? AND owner_id = ?"
        } else {
            "SELECT COUNT(*) as count FROM prompts WHERE owner_type = ? AND owner_id = ? AND deleted_at IS NULL"
        };

        let row = wasm_query(query)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get::<i64>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdatePrompt) -> DbResult<Prompt> {
        let has_name = input.name.is_some();
        let has_description = input.description.is_some();
        let has_content = input.content.is_some();
        let has_metadata = input.metadata.is_some();

        if !has_name && !has_description && !has_content && !has_metadata {
            return self.get_by_id(id).await?.ok_or(DbError::NotFound);
        }

        let now = chrono::Utc::now();

        let mut set_clauses = vec!["updated_at = ?"];
        if has_name {
            set_clauses.push("name = ?");
        }
        if has_description {
            set_clauses.push("description = ?");
        }
        if has_content {
            set_clauses.push("content = ?");
        }
        if has_metadata {
            set_clauses.push("metadata = ?");
        }

        let query = format!(
            "UPDATE prompts SET {} WHERE id = ? AND deleted_at IS NULL",
            set_clauses.join(", ")
        );

        let mut query_builder = wasm_query(&query).bind(now);

        if let Some(ref name) = input.name {
            query_builder = query_builder.bind(name);
        }
        if let Some(ref description) = input.description {
            query_builder = query_builder.bind(description);
        }
        if let Some(ref content) = input.content {
            query_builder = query_builder.bind(content);
        }
        if let Some(ref metadata) = input.metadata {
            let metadata_json = serde_json::to_string(metadata)
                .map_err(|e| DbError::Internal(format!("Failed to serialize metadata: {}", e)))?;
            query_builder = query_builder.bind(metadata_json);
        }

        let result = query_builder
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| {
                if e.is_unique_violation() {
                    DbError::Conflict("Prompt with this name already exists for this owner".into())
                } else {
                    DbError::from(e)
                }
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = wasm_query(
            r#"
            UPDATE prompts
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
}
