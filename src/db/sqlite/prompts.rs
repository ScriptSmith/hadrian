use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
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

pub struct SqlitePromptRepo {
    pool: SqlitePool,
}

impl SqlitePromptRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Parse a Prompt from a database row.
    fn parse_prompt(row: &sqlx::sqlite::SqliteRow) -> DbResult<Prompt> {
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
            id: parse_uuid(&row.get::<String, _>("id"))?,
            owner_type,
            owner_id: parse_uuid(&row.get::<String, _>("owner_id"))?,
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

        let rows = sqlx::query(&query)
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

#[async_trait]
impl PromptRepo for SqlitePromptRepo {
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

        sqlx::query(
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
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Prompt with name '{}' already exists for this owner",
                    input.name
                ))
            }
            _ => DbError::from(e),
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
        let result = sqlx::query(
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
        let result = sqlx::query(
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

        let rows = sqlx::query(query)
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

        let row = sqlx::query(query)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .fetch_one(&self.pool)
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

        let mut query_builder = sqlx::query(&query).bind(now);

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
            .map_err(|e| match e {
                sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                    DbError::Conflict("Prompt with this name already exists for this owner".into())
                }
                _ => DbError::from(e),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = sqlx::query(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PromptOwner;

    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        sqlx::query(
            r#"
            CREATE TABLE prompts (
                id TEXT PRIMARY KEY NOT NULL,
                owner_type TEXT NOT NULL CHECK (owner_type IN ('organization', 'team', 'project', 'user')),
                owner_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                content TEXT NOT NULL,
                metadata TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(owner_type, owner_id, name)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create prompts table");

        pool
    }

    fn create_prompt_input(name: &str, content: &str, user_id: Uuid) -> CreatePrompt {
        CreatePrompt {
            owner: PromptOwner::User { user_id },
            name: name.to_string(),
            description: None,
            content: content.to_string(),
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_create_prompt() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        let input = create_prompt_input("test-prompt", "You are a helpful assistant.", user_id);
        let prompt = repo.create(input).await.expect("Failed to create prompt");

        assert_eq!(prompt.name, "test-prompt");
        assert_eq!(prompt.content, "You are a helpful assistant.");
        assert_eq!(prompt.owner_type, PromptOwnerType::User);
        assert_eq!(prompt.owner_id, user_id);
        assert!(!prompt.id.is_nil());
    }

    #[tokio::test]
    async fn test_create_prompt_with_metadata() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        let mut metadata = HashMap::new();
        metadata.insert(
            "temperature".to_string(),
            serde_json::Value::Number(serde_json::Number::from_f64(0.7).unwrap()),
        );

        let input = CreatePrompt {
            owner: PromptOwner::User { user_id },
            name: "test-prompt".to_string(),
            description: Some("A test prompt".to_string()),
            content: "You are a helpful assistant.".to_string(),
            metadata: Some(metadata),
        };

        let prompt = repo.create(input).await.expect("Failed to create prompt");

        assert!(prompt.metadata.is_some());
        assert!(prompt.description.is_some());
        assert_eq!(prompt.description.unwrap(), "A test prompt");
    }

    #[tokio::test]
    async fn test_create_duplicate_name_fails() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        let input = create_prompt_input("duplicate", "First content", user_id);
        repo.create(input).await.expect("Failed to create prompt");

        let input2 = create_prompt_input("duplicate", "Second content", user_id);
        let result = repo.create(input2).await;

        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_same_name_different_owners_succeeds() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user1_id = Uuid::new_v4();
        let user2_id = Uuid::new_v4();

        let input1 = create_prompt_input("same-name", "User 1 content", user1_id);
        let prompt1 = repo.create(input1).await.expect("Failed to create prompt");

        let input2 = create_prompt_input("same-name", "User 2 content", user2_id);
        let prompt2 = repo.create(input2).await.expect("Failed to create prompt");

        assert_eq!(prompt1.name, prompt2.name);
        assert_ne!(prompt1.id, prompt2.id);
        assert_ne!(prompt1.owner_id, prompt2.owner_id);
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        let input = create_prompt_input("get-test", "Test content", user_id);
        let created = repo.create(input).await.expect("Failed to create prompt");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get prompt")
            .expect("Prompt should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "get-test");
        assert_eq!(fetched.content, "Test content");
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);

        let result = repo
            .get_by_id(Uuid::new_v4())
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_by_owner_empty() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        let result = repo
            .list_by_owner(PromptOwnerType::User, user_id, ListParams::default())
            .await
            .expect("Failed to list prompts");

        assert!(result.items.is_empty());
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_by_owner_with_prompts() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        for i in 0..3 {
            let input = create_prompt_input(&format!("prompt-{}", i), "Content", user_id);
            repo.create(input).await.expect("Failed to create prompt");
        }

        let result = repo
            .list_by_owner(PromptOwnerType::User, user_id, ListParams::default())
            .await
            .expect("Failed to list prompts");

        assert_eq!(result.items.len(), 3);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_by_owner_filters_by_owner() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user1_id = Uuid::new_v4();
        let user2_id = Uuid::new_v4();

        repo.create(create_prompt_input("user1-prompt", "Content", user1_id))
            .await
            .expect("Failed to create");
        repo.create(create_prompt_input("user2-prompt", "Content", user2_id))
            .await
            .expect("Failed to create");

        let user1_result = repo
            .list_by_owner(PromptOwnerType::User, user1_id, ListParams::default())
            .await
            .expect("Failed to list");
        let user2_result = repo
            .list_by_owner(PromptOwnerType::User, user2_id, ListParams::default())
            .await
            .expect("Failed to list");

        assert_eq!(user1_result.items.len(), 1);
        assert_eq!(user1_result.items[0].name, "user1-prompt");
        assert_eq!(user2_result.items.len(), 1);
        assert_eq!(user2_result.items[0].name, "user2-prompt");
    }

    #[tokio::test]
    async fn test_count_by_owner() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        for i in 0..3 {
            let input = create_prompt_input(&format!("prompt-{}", i), "Content", user_id);
            repo.create(input).await.expect("Failed to create");
        }

        let count = repo
            .count_by_owner(PromptOwnerType::User, user_id, false)
            .await
            .expect("Failed to count");

        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_update_prompt() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        let input = create_prompt_input("update-test", "Original content", user_id);
        let created = repo.create(input).await.expect("Failed to create");

        let updated = repo
            .update(
                created.id,
                UpdatePrompt {
                    name: Some("updated-name".to_string()),
                    description: Some("New description".to_string()),
                    content: Some("Updated content".to_string()),
                    metadata: None,
                },
            )
            .await
            .expect("Failed to update");

        assert_eq!(updated.id, created.id);
        assert_eq!(updated.name, "updated-name");
        assert_eq!(updated.description.unwrap(), "New description");
        assert_eq!(updated.content, "Updated content");
        assert!(updated.updated_at >= created.updated_at);
    }

    #[tokio::test]
    async fn test_update_no_changes() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        let input = create_prompt_input("no-change", "Original", user_id);
        let created = repo.create(input).await.expect("Failed to create");

        let result = repo
            .update(
                created.id,
                UpdatePrompt {
                    name: None,
                    description: None,
                    content: None,
                    metadata: None,
                },
            )
            .await
            .expect("Failed to update");

        assert_eq!(result.name, "no-change");
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);

        let result = repo
            .update(
                Uuid::new_v4(),
                UpdatePrompt {
                    name: Some("New Name".to_string()),
                    description: None,
                    content: None,
                    metadata: None,
                },
            )
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete_prompt() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        let input = create_prompt_input("delete-test", "Content", user_id);
        let created = repo.create(input).await.expect("Failed to create");

        repo.delete(created.id)
            .await
            .expect("Failed to delete prompt");

        let result = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed");
        assert!(result.is_none());

        let list = repo
            .list_by_owner(PromptOwnerType::User, user_id, ListParams::default())
            .await
            .expect("Failed to list");
        assert!(list.items.is_empty());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);

        let result = repo.delete(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_count_excludes_deleted() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);
        let user_id = Uuid::new_v4();

        let prompt1 = repo
            .create(create_prompt_input("prompt-1", "Content", user_id))
            .await
            .expect("Failed to create");
        repo.create(create_prompt_input("prompt-2", "Content", user_id))
            .await
            .expect("Failed to create");

        repo.delete(prompt1.id).await.expect("Failed to delete");

        let count = repo
            .count_by_owner(PromptOwnerType::User, user_id, false)
            .await
            .expect("Failed to count");
        assert_eq!(count, 1);

        let count_all = repo
            .count_by_owner(PromptOwnerType::User, user_id, true)
            .await
            .expect("Failed to count all");
        assert_eq!(count_all, 2);
    }

    #[tokio::test]
    async fn test_different_owner_types() {
        let pool = create_test_pool().await;
        let repo = SqlitePromptRepo::new(pool);

        let org_id = Uuid::new_v4();
        let team_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // Create prompts for different owner types
        repo.create(CreatePrompt {
            owner: PromptOwner::Organization {
                organization_id: org_id,
            },
            name: "org-prompt".to_string(),
            description: None,
            content: "Org content".to_string(),
            metadata: None,
        })
        .await
        .expect("Failed to create org prompt");

        repo.create(CreatePrompt {
            owner: PromptOwner::Team { team_id },
            name: "team-prompt".to_string(),
            description: None,
            content: "Team content".to_string(),
            metadata: None,
        })
        .await
        .expect("Failed to create team prompt");

        repo.create(CreatePrompt {
            owner: PromptOwner::Project { project_id },
            name: "project-prompt".to_string(),
            description: None,
            content: "Project content".to_string(),
            metadata: None,
        })
        .await
        .expect("Failed to create project prompt");

        repo.create(CreatePrompt {
            owner: PromptOwner::User { user_id },
            name: "user-prompt".to_string(),
            description: None,
            content: "User content".to_string(),
            metadata: None,
        })
        .await
        .expect("Failed to create user prompt");

        // Verify each owner type only sees their prompts
        let org_prompts = repo
            .list_by_owner(PromptOwnerType::Organization, org_id, ListParams::default())
            .await
            .expect("Failed to list");
        assert_eq!(org_prompts.items.len(), 1);
        assert_eq!(org_prompts.items[0].name, "org-prompt");

        let team_prompts = repo
            .list_by_owner(PromptOwnerType::Team, team_id, ListParams::default())
            .await
            .expect("Failed to list");
        assert_eq!(team_prompts.items.len(), 1);
        assert_eq!(team_prompts.items[0].name, "team-prompt");

        let project_prompts = repo
            .list_by_owner(PromptOwnerType::Project, project_id, ListParams::default())
            .await
            .expect("Failed to list");
        assert_eq!(project_prompts.items.len(), 1);
        assert_eq!(project_prompts.items[0].name, "project-prompt");

        let user_prompts = repo
            .list_by_owner(PromptOwnerType::User, user_id, ListParams::default())
            .await
            .expect("Failed to list");
        assert_eq!(user_prompts.items.len(), 1);
        assert_eq!(user_prompts.items[0].name, "user-prompt");
    }
}
