use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, PromptRepo,
            cursor_from_row,
        },
    },
    models::{CreatePrompt, Prompt, PromptOwnerType, UpdatePrompt},
};

pub struct PostgresPromptRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresPromptRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    /// Parse a Prompt from a database row.
    fn parse_prompt(row: &sqlx::postgres::PgRow) -> DbResult<Prompt> {
        let owner_type_str: String = row.get("owner_type");
        let owner_type: PromptOwnerType = owner_type_str
            .parse()
            .map_err(|e: String| DbError::Internal(e))?;

        let metadata: Option<serde_json::Value> = row.get("metadata");
        let metadata: Option<HashMap<String, serde_json::Value>> = metadata
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| DbError::Internal(format!("Failed to parse metadata: {}", e)))?;

        Ok(Prompt {
            id: row.get("id"),
            owner_type,
            owner_id: row.get("owner_id"),
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
            SELECT id, owner_type::TEXT, owner_id, name, description, content, metadata, created_at, updated_at
            FROM prompts
            WHERE owner_type = $1 AND owner_id = $2 AND ROW(created_at, id) {} ROW($3, $4)
            {}
            ORDER BY created_at {}, id {}
            LIMIT $5
            "#,
            comparison, deleted_filter, order, order
        );

        let rows = sqlx::query(&query)
            .bind(owner_type.as_str())
            .bind(owner_id)
            .bind(cursor.created_at)
            .bind(cursor.id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
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

#[async_trait]
impl PromptRepo for PostgresPromptRepo {
    async fn create(&self, input: CreatePrompt) -> DbResult<Prompt> {
        let id = Uuid::new_v4();
        let owner_type = input.owner.owner_type();
        let owner_id = input.owner.owner_id();

        let metadata_json: Option<serde_json::Value> = input
            .metadata
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| DbError::Internal(format!("Failed to serialize metadata: {}", e)))?;

        let row = sqlx::query(
            r#"
            INSERT INTO prompts (id, owner_type, owner_id, name, description, content, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, owner_type::TEXT, owner_id, name, description, content, metadata, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(owner_type.as_str())
        .bind(owner_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.content)
        .bind(&metadata_json)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Prompt with name '{}' already exists for this owner",
                    input.name
                ))
            }
            _ => DbError::from(e),
        })?;

        Self::parse_prompt(&row)
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Prompt>> {
        let result = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, description, content, metadata, created_at, updated_at
            FROM prompts
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_prompt(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Prompt>> {
        let result = sqlx::query(
            r#"
            SELECT p.id, p.owner_type::TEXT, p.owner_id, p.name, p.description, p.content, p.metadata, p.created_at, p.updated_at
            FROM prompts p
            WHERE p.id = $1 AND p.deleted_at IS NULL
            AND (
                (p.owner_type = 'organization' AND p.owner_id = $2)
                OR
                (p.owner_type = 'team' AND EXISTS (
                    SELECT 1 FROM teams t WHERE t.id = p.owner_id AND t.org_id = $3
                ))
                OR
                (p.owner_type = 'project' AND EXISTS (
                    SELECT 1 FROM projects pr WHERE pr.id = p.owner_id AND pr.org_id = $4
                ))
                OR
                (p.owner_type = 'user' AND EXISTS (
                    SELECT 1 FROM org_memberships om WHERE om.user_id = p.owner_id AND om.org_id = $5
                ))
            )
            "#,
        )
        .bind(id)
        .bind(org_id)
        .bind(org_id)
        .bind(org_id)
        .bind(org_id)
        .fetch_optional(&self.read_pool)
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
            SELECT id, owner_type::TEXT, owner_id, name, description, content, metadata, created_at, updated_at
            FROM prompts
            WHERE owner_type = $1 AND owner_id = $2
            ORDER BY created_at DESC, id DESC
            LIMIT $3
            "#
        } else {
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, description, content, metadata, created_at, updated_at
            FROM prompts
            WHERE owner_type = $1 AND owner_id = $2 AND deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
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
            "SELECT COUNT(*) as count FROM prompts WHERE owner_type = $1 AND owner_id = $2"
        } else {
            "SELECT COUNT(*) as count FROM prompts WHERE owner_type = $1 AND owner_id = $2 AND deleted_at IS NULL"
        };

        let row = sqlx::query(query)
            .bind(owner_type.as_str())
            .bind(owner_id)
            .fetch_one(&self.read_pool)
            .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdatePrompt) -> DbResult<Prompt> {
        let has_name = input.name.is_some();
        let has_description = input.description.is_some();
        let has_content = input.content.is_some();
        let has_metadata = input.metadata.is_some();

        if !has_name && !has_description && !has_content && !has_metadata {
            return self.get_by_id(id).await?.ok_or(DbError::NotFound);
        }

        let mut set_clauses: Vec<String> = vec!["updated_at = NOW()".to_string()];
        let mut param_idx = 1;

        if has_name {
            set_clauses.push(format!("name = ${}", param_idx));
            param_idx += 1;
        }
        if has_description {
            set_clauses.push(format!("description = ${}", param_idx));
            param_idx += 1;
        }
        if has_content {
            set_clauses.push(format!("content = ${}", param_idx));
            param_idx += 1;
        }
        if has_metadata {
            set_clauses.push(format!("metadata = ${}", param_idx));
            param_idx += 1;
        }

        let query = format!(
            r#"
            UPDATE prompts
            SET {}
            WHERE id = ${} AND deleted_at IS NULL
            RETURNING id, owner_type::TEXT, owner_id, name, description, content, metadata, created_at, updated_at
            "#,
            set_clauses.join(", "),
            param_idx
        );

        let mut query_builder = sqlx::query(&query);

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
            let metadata_json: serde_json::Value = serde_json::to_value(metadata)
                .map_err(|e| DbError::Internal(format!("Failed to serialize metadata: {}", e)))?;
            query_builder = query_builder.bind(metadata_json);
        }

        let row = query_builder
            .bind(id)
            .fetch_optional(&self.write_pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                    DbError::Conflict("Prompt with this name already exists for this owner".into())
                }
                _ => DbError::from(e),
            })?
            .ok_or(DbError::NotFound)?;

        Self::parse_prompt(&row)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE prompts
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
}
