use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{Cursor, CursorDirection, ListParams, ListResult, PageCursors, VectorStoresRepo},
    },
    models::{
        AddFileToVectorStore, ChunkingStrategy, CreateVectorStore, ExpiresAfter, FileCounts,
        FileError, OBJECT_TYPE_VECTOR_STORE, OBJECT_TYPE_VECTOR_STORE_FILE, UpdateVectorStore,
        VectorStore, VectorStoreFile, VectorStoreFileStatus, VectorStoreOwnerType,
        VectorStoreStatus,
    },
};

pub struct SqliteVectorStoresRepo {
    pool: SqlitePool,
}

impl SqliteVectorStoresRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_file_counts(json_str: &str) -> DbResult<FileCounts> {
        serde_json::from_str(json_str).map_err(|e| DbError::Internal(e.to_string()))
    }

    fn parse_metadata(
        json_str: Option<String>,
    ) -> DbResult<Option<HashMap<String, serde_json::Value>>> {
        match json_str {
            Some(s) => serde_json::from_str(&s).map_err(|e| DbError::Internal(e.to_string())),
            None => Ok(None),
        }
    }

    fn parse_expires_after(json_str: Option<String>) -> DbResult<Option<ExpiresAfter>> {
        match json_str {
            Some(s) => serde_json::from_str(&s).map_err(|e| DbError::Internal(e.to_string())),
            None => Ok(None),
        }
    }

    fn parse_chunking_strategy(json_str: Option<String>) -> DbResult<Option<ChunkingStrategy>> {
        match json_str {
            Some(s) => serde_json::from_str(&s).map_err(|e| DbError::Internal(e.to_string())),
            None => Ok(None),
        }
    }

    fn parse_file_error(json_str: Option<String>) -> DbResult<Option<FileError>> {
        match json_str {
            Some(s) => serde_json::from_str(&s).map_err(|e| DbError::Internal(e.to_string())),
            None => Ok(None),
        }
    }

    fn parse_attributes(
        json_str: Option<String>,
    ) -> DbResult<Option<HashMap<String, serde_json::Value>>> {
        Self::parse_metadata(json_str)
    }

    /// Parse a VectorStore from a database row.
    /// Expects columns: id, owner_type, owner_id, name, description, status, embedding_model,
    /// embedding_dimensions, usage_bytes, file_counts, metadata, expires_after, expires_at,
    /// last_active_at, created_at, updated_at
    fn vector_store_from_row(row: &sqlx::sqlite::SqliteRow) -> DbResult<VectorStore> {
        let owner_type_str: String = row.get("owner_type");
        let status_str: String = row.get("status");
        let file_counts_str: String = row.get("file_counts");

        Ok(VectorStore {
            id: parse_uuid(&row.get::<String, _>("id"))?,
            object: OBJECT_TYPE_VECTOR_STORE.to_string(),
            owner_type: owner_type_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            owner_id: parse_uuid(&row.get::<String, _>("owner_id"))?,
            name: row.get("name"),
            description: row.get("description"),
            status: status_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            embedding_model: row.get("embedding_model"),
            embedding_dimensions: row.get("embedding_dimensions"),
            usage_bytes: row.get("usage_bytes"),
            file_counts: Self::parse_file_counts(&file_counts_str)?,
            metadata: Self::parse_metadata(row.get("metadata"))?,
            expires_after: Self::parse_expires_after(row.get("expires_after"))?,
            expires_at: row.get("expires_at"),
            last_active_at: row.get("last_active_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Parse a VectorStoreFile from a database row.
    /// Expects columns: id, vector_store_id, file_id, status, usage_bytes, last_error,
    /// chunking_strategy, attributes, created_at, updated_at
    fn vector_store_file_from_row(row: &sqlx::sqlite::SqliteRow) -> DbResult<VectorStoreFile> {
        let status_str: String = row.get("status");

        Ok(VectorStoreFile {
            internal_id: parse_uuid(&row.get::<String, _>("id"))?,
            file_id: parse_uuid(&row.get::<String, _>("file_id"))?,
            object: OBJECT_TYPE_VECTOR_STORE_FILE.to_string(),
            vector_store_id: parse_uuid(&row.get::<String, _>("vector_store_id"))?,
            status: status_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            usage_bytes: row.get("usage_bytes"),
            last_error: Self::parse_file_error(row.get("last_error"))?,
            chunking_strategy: Self::parse_chunking_strategy(row.get("chunking_strategy"))?,
            attributes: Self::parse_attributes(row.get("attributes"))?,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    fn cursor_from_vector_store(vector_store: &VectorStore) -> Cursor {
        Cursor::new(vector_store.updated_at, vector_store.id)
    }

    fn cursor_from_file(file: &VectorStoreFile) -> Cursor {
        Cursor::new(file.created_at, file.internal_id)
    }
}

#[async_trait]
impl VectorStoresRepo for SqliteVectorStoresRepo {
    // ==================== Vector Stores CRUD ====================

    async fn create_vector_store(&self, input: CreateVectorStore) -> DbResult<VectorStore> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let owner_type = input.owner.owner_type();
        let owner_id = input.owner.owner_id();
        // Generate name if not provided (OpenAI-compatible: name is optional)
        let name = input
            .name
            .unwrap_or_else(|| format!("Vector Store {}", &id.to_string()[..8]));
        let metadata_json = input
            .metadata
            .map(|m| serde_json::to_string(&m))
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?;
        let expires_after_json = input
            .expires_after
            .as_ref()
            .map(|e| serde_json::to_string(&e))
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?;

        let default_file_counts =
            r#"{"cancelled":0,"completed":0,"failed":0,"in_progress":0,"total":0}"#;

        sqlx::query(
            r#"
            INSERT INTO vector_stores (id, owner_type, owner_id, name, description, embedding_model, embedding_dimensions, metadata, expires_after, file_counts, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(owner_type.as_str())
        .bind(owner_id.to_string())
        .bind(&name)
        .bind(&input.description)
        .bind(&input.embedding_model)
        .bind(input.embedding_dimensions)
        .bind(&metadata_json)
        .bind(&expires_after_json)
        .bind(default_file_counts)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(VectorStore {
            id,
            object: OBJECT_TYPE_VECTOR_STORE.to_string(),
            owner_type,
            owner_id,
            name,
            description: input.description,
            status: VectorStoreStatus::Completed,
            embedding_model: input.embedding_model,
            embedding_dimensions: input.embedding_dimensions,
            usage_bytes: 0,
            file_counts: FileCounts::default(),
            metadata: Self::parse_metadata(metadata_json)?,
            expires_after: input.expires_after,
            expires_at: None,
            last_active_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_vector_store(&self, id: Uuid) -> DbResult<Option<VectorStore>> {
        let result = sqlx::query(
            r#"
            SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                   usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
            FROM vector_stores
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::vector_store_from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<VectorStore>> {
        let result = sqlx::query(
            r#"
            SELECT vs.id, vs.owner_type, vs.owner_id, vs.name, vs.description, vs.status, vs.embedding_model, vs.embedding_dimensions,
                   vs.usage_bytes, vs.file_counts, vs.metadata, vs.expires_after, vs.expires_at, vs.last_active_at, vs.created_at, vs.updated_at
            FROM vector_stores vs
            WHERE vs.id = ? AND vs.deleted_at IS NULL
            AND (
                (vs.owner_type = 'organization' AND vs.owner_id = ?)
                OR
                (vs.owner_type = 'team' AND EXISTS (
                    SELECT 1 FROM teams t WHERE t.id = vs.owner_id AND t.org_id = ?
                ))
                OR
                (vs.owner_type = 'project' AND EXISTS (
                    SELECT 1 FROM projects pr WHERE pr.id = vs.owner_id AND pr.org_id = ?
                ))
                OR
                (vs.owner_type = 'user' AND EXISTS (
                    SELECT 1 FROM org_memberships om WHERE om.user_id = vs.owner_id AND om.org_id = ?
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
            Some(row) => Ok(Some(Self::vector_store_from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_vector_store_by_name(
        &self,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
        name: &str,
    ) -> DbResult<Option<VectorStore>> {
        let result = sqlx::query(
            r#"
            SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                   usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
            FROM vector_stores
            WHERE owner_type = ? AND owner_id = ? AND name = ? AND deleted_at IS NULL
            "#,
        )
        .bind(owner_type.as_str())
        .bind(owner_id.to_string())
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::vector_store_from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_vector_stores(
        &self,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<VectorStore>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Handle cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            let (comparison, order, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let deleted_filter = if params.include_deleted {
                ""
            } else {
                "AND deleted_at IS NULL"
            };

            let query = format!(
                r#"
                SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
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
                .bind(cursor.created_at)
                .bind(cursor.id.to_string())
                .bind(fetch_limit)
                .fetch_all(&self.pool)
                .await?;

            let has_more = rows.len() as i64 > limit;
            let mut items: Vec<VectorStore> = rows
                .into_iter()
                .take(limit as usize)
                .map(|row| Self::vector_store_from_row(&row))
                .collect::<DbResult<Vec<_>>>()?;

            if should_reverse {
                items.reverse();
            }

            let cursors = PageCursors::from_items(
                &items,
                has_more,
                params.direction,
                Some(cursor),
                Self::cursor_from_vector_store,
            );

            return Ok(ListResult::new(items, has_more, cursors));
        }

        // First page (no cursor)
        let order = params.sort_order.as_sql();
        let query = if params.include_deleted {
            format!(
                r#"
                SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE owner_type = ? AND owner_id = ?
                ORDER BY updated_at {}, id {}
                LIMIT ?
                "#,
                order, order
            )
        } else {
            format!(
                r#"
                SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE owner_type = ? AND owner_id = ? AND deleted_at IS NULL
                ORDER BY updated_at {}, id {}
                LIMIT ?
                "#,
                order, order
            )
        };

        let rows = sqlx::query(&query)
            .bind(owner_type.as_str())
            .bind(owner_id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<VectorStore> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Self::vector_store_from_row(&row))
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(
            &items,
            has_more,
            CursorDirection::Forward,
            None,
            Self::cursor_from_vector_store,
        );

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn list_accessible_vector_stores(
        &self,
        user_id: Option<Uuid>,
        org_ids: &[Uuid],
        team_ids: &[Uuid],
        project_ids: &[Uuid],
        params: ListParams,
    ) -> DbResult<ListResult<VectorStore>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Build the owner filter dynamically based on provided IDs
        let mut conditions = Vec::new();
        let mut bindings: Vec<String> = Vec::new();

        if let Some(uid) = user_id {
            conditions.push("(owner_type = ? AND owner_id = ?)".to_string());
            bindings.push("user".to_string());
            bindings.push(uid.to_string());
        }

        for org_id in org_ids {
            conditions.push("(owner_type = ? AND owner_id = ?)".to_string());
            bindings.push("organization".to_string());
            bindings.push(org_id.to_string());
        }

        for team_id in team_ids {
            conditions.push("(owner_type = ? AND owner_id = ?)".to_string());
            bindings.push("team".to_string());
            bindings.push(team_id.to_string());
        }

        for project_id in project_ids {
            conditions.push("(owner_type = ? AND owner_id = ?)".to_string());
            bindings.push("project".to_string());
            bindings.push(project_id.to_string());
        }

        // If no conditions, return empty result
        if conditions.is_empty() {
            return Ok(ListResult::new(vec![], false, PageCursors::default()));
        }

        let owner_filter = conditions.join(" OR ");
        let order = params.sort_order.as_sql();

        // Handle cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            let (comparison, order_dir, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let deleted_filter = if params.include_deleted {
                ""
            } else {
                "AND deleted_at IS NULL"
            };

            let query = format!(
                r#"
                SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE ({})
                AND (updated_at, id) {} (?, ?)
                {}
                ORDER BY updated_at {}, id {}
                LIMIT ?
                "#,
                owner_filter, comparison, deleted_filter, order_dir, order_dir
            );

            // Build the query dynamically
            let mut query_builder = sqlx::query(&query);
            for binding in &bindings {
                query_builder = query_builder.bind(binding);
            }
            query_builder = query_builder
                .bind(cursor.created_at)
                .bind(cursor.id.to_string())
                .bind(fetch_limit);

            let rows = query_builder.fetch_all(&self.pool).await?;

            let has_more = rows.len() as i64 > limit;
            let mut items: Vec<VectorStore> = rows
                .into_iter()
                .take(limit as usize)
                .map(|row| Self::vector_store_from_row(&row))
                .collect::<DbResult<Vec<_>>>()?;

            if should_reverse {
                items.reverse();
            }

            let cursors = PageCursors::from_items(
                &items,
                has_more,
                params.direction,
                Some(cursor),
                Self::cursor_from_vector_store,
            );

            return Ok(ListResult::new(items, has_more, cursors));
        }

        // First page (no cursor)
        let query = if params.include_deleted {
            format!(
                r#"
                SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE ({})
                ORDER BY updated_at {}, id {}
                LIMIT ?
                "#,
                owner_filter, order, order
            )
        } else {
            format!(
                r#"
                SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE ({}) AND deleted_at IS NULL
                ORDER BY updated_at {}, id {}
                LIMIT ?
                "#,
                owner_filter, order, order
            )
        };

        // Build the query dynamically
        let mut query_builder = sqlx::query(&query);
        for binding in &bindings {
            query_builder = query_builder.bind(binding);
        }
        query_builder = query_builder.bind(fetch_limit);

        let rows = query_builder.fetch_all(&self.pool).await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<VectorStore> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Self::vector_store_from_row(&row))
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(
            &items,
            has_more,
            CursorDirection::Forward,
            None,
            Self::cursor_from_vector_store,
        );

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn list_all_vector_stores(
        &self,
        params: ListParams,
    ) -> DbResult<ListResult<VectorStore>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;
        let order = params.sort_order.as_sql();

        // Handle cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            let (comparison, order_dir, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let deleted_filter = if params.include_deleted {
                ""
            } else {
                "AND deleted_at IS NULL"
            };

            let query = format!(
                r#"
                SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE (updated_at, id) {} (?, ?)
                {}
                ORDER BY updated_at {}, id {}
                LIMIT ?
                "#,
                comparison, deleted_filter, order_dir, order_dir
            );

            let rows = sqlx::query(&query)
                .bind(cursor.created_at)
                .bind(cursor.id.to_string())
                .bind(fetch_limit)
                .fetch_all(&self.pool)
                .await?;

            let has_more = rows.len() as i64 > limit;
            let mut items: Vec<VectorStore> = rows
                .into_iter()
                .take(limit as usize)
                .map(|row| Self::vector_store_from_row(&row))
                .collect::<DbResult<Vec<_>>>()?;

            if should_reverse {
                items.reverse();
            }

            let cursors = PageCursors::from_items(
                &items,
                has_more,
                params.direction,
                Some(cursor),
                Self::cursor_from_vector_store,
            );

            return Ok(ListResult::new(items, has_more, cursors));
        }

        // First page (no cursor)
        let query = if params.include_deleted {
            format!(
                r#"
                SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                ORDER BY updated_at {}, id {}
                LIMIT ?
                "#,
                order, order
            )
        } else {
            format!(
                r#"
                SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE deleted_at IS NULL
                ORDER BY updated_at {}, id {}
                LIMIT ?
                "#,
                order, order
            )
        };

        let rows = sqlx::query(&query)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<VectorStore> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Self::vector_store_from_row(&row))
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(
            &items,
            has_more,
            CursorDirection::Forward,
            None,
            Self::cursor_from_vector_store,
        );

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn update_vector_store(
        &self,
        id: Uuid,
        input: UpdateVectorStore,
    ) -> DbResult<VectorStore> {
        let now = chrono::Utc::now();

        // Use IMMEDIATE transaction mode to acquire write lock before reading
        let mut conn = self.pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

        let result = async {
            // Read current state within transaction
            let current = sqlx::query(
                r#"
                SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE id = ? AND deleted_at IS NULL
                "#,
            )
            .bind(id.to_string())
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(DbError::NotFound)?;

            let owner_type_str: String = current.get("owner_type");
            let owner_type: VectorStoreOwnerType = owner_type_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?;
            let owner_id = parse_uuid(&current.get::<String, _>("owner_id"))?;
            let status_str: String = current.get("status");
            let file_counts_str: String = current.get("file_counts");
            let embedding_model: String = current.get("embedding_model");
            let embedding_dimensions: i32 = current.get("embedding_dimensions");

            let current_name: String = current.get("name");
            let current_description: Option<String> = current.get("description");
            let current_metadata: Option<String> = current.get("metadata");
            let current_expires_after: Option<String> = current.get("expires_after");

            let new_name = input.name.unwrap_or(current_name);
            let new_description = input.description.or(current_description);
            let new_metadata = input
                .metadata
                .map(|m| serde_json::to_string(&m))
                .transpose()
                .map_err(|e| DbError::Internal(e.to_string()))?
                .or(current_metadata);
            let new_expires_after = input
                .expires_after
                .map(|e| serde_json::to_string(&e))
                .transpose()
                .map_err(|e| DbError::Internal(e.to_string()))?
                .or(current_expires_after);

            let update_result = sqlx::query(
                r#"
                UPDATE vector_stores
                SET name = ?, description = ?, metadata = ?, expires_after = ?, updated_at = ?
                WHERE id = ? AND deleted_at IS NULL
                "#,
            )
            .bind(&new_name)
            .bind(&new_description)
            .bind(&new_metadata)
            .bind(&new_expires_after)
            .bind(now)
            .bind(id.to_string())
            .execute(&mut *conn)
            .await?;

            if update_result.rows_affected() == 0 {
                return Err(DbError::NotFound);
            }

            Ok(VectorStore {
                id,
                object: OBJECT_TYPE_VECTOR_STORE.to_string(),
                owner_type,
                owner_id,
                name: new_name,
                description: new_description,
                status: status_str
                    .parse()
                    .map_err(|e: String| DbError::Internal(e))?,
                embedding_model,
                embedding_dimensions,
                usage_bytes: current.get("usage_bytes"),
                file_counts: Self::parse_file_counts(&file_counts_str)?,
                metadata: Self::parse_metadata(new_metadata)?,
                expires_after: Self::parse_expires_after(new_expires_after)?,
                expires_at: current.get("expires_at"),
                last_active_at: current.get("last_active_at"),
                created_at: current.get("created_at"),
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

    async fn delete_vector_store(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE vector_stores
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

    async fn hard_delete_vector_store(&self, id: Uuid) -> DbResult<()> {
        // First delete all vector_store_files links
        sqlx::query(
            r#"
            DELETE FROM vector_store_files
            WHERE vector_store_id = ?
            "#,
        )
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        // Then delete the vector store
        let result = sqlx::query(
            r#"
            DELETE FROM vector_stores
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn list_deleted_vector_stores(
        &self,
        older_than: DateTime<Utc>,
    ) -> DbResult<Vec<VectorStore>> {
        let rows = sqlx::query(
            r#"
            SELECT id, owner_type, owner_id, name, description, status, embedding_model, embedding_dimensions,
                   usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
            FROM vector_stores
            WHERE deleted_at IS NOT NULL AND deleted_at < ?
            "#,
        )
        .bind(older_than)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| Self::vector_store_from_row(&row))
            .collect()
    }

    async fn touch_vector_store(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE vector_stores
            SET last_active_at = ?, updated_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(now)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    // ==================== VectorStore Files CRUD ====================

    async fn add_file_to_vector_store(
        &self,
        input: AddFileToVectorStore,
    ) -> DbResult<VectorStoreFile> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let chunking_strategy_json = input
            .chunking_strategy
            .as_ref()
            .map(|c| serde_json::to_string(&c))
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?;
        let attributes_json = input
            .attributes
            .as_ref()
            .map(|a| serde_json::to_string(&a))
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO vector_store_files (id, vector_store_id, file_id, chunking_strategy, attributes, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(input.vector_store_id.to_string())
        .bind(input.file_id.to_string())
        .bind(&chunking_strategy_json)
        .bind(&attributes_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(VectorStoreFile {
            internal_id: id,
            file_id: input.file_id,
            object: OBJECT_TYPE_VECTOR_STORE_FILE.to_string(),
            vector_store_id: input.vector_store_id,
            status: VectorStoreFileStatus::InProgress,
            usage_bytes: 0,
            last_error: None,
            chunking_strategy: input.chunking_strategy,
            attributes: input.attributes,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_vector_store_file(&self, id: Uuid) -> DbResult<Option<VectorStoreFile>> {
        let result = sqlx::query(
            r#"
            SELECT id, vector_store_id, file_id, status, usage_bytes,
                   last_error, chunking_strategy, attributes, created_at, updated_at
            FROM vector_store_files
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::vector_store_file_from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn find_vector_store_file_by_file_id(
        &self,
        vector_store_id: Uuid,
        file_id: Uuid,
    ) -> DbResult<Option<VectorStoreFile>> {
        let result = sqlx::query(
            r#"
            SELECT id, vector_store_id, file_id, status, usage_bytes,
                   last_error, chunking_strategy, attributes, created_at, updated_at
            FROM vector_store_files
            WHERE vector_store_id = ?
              AND file_id = ?
              AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(vector_store_id.to_string())
        .bind(file_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::vector_store_file_from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn find_vector_store_file_by_content_hash_and_owner(
        &self,
        vector_store_id: Uuid,
        content_hash: &str,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
    ) -> DbResult<Option<VectorStoreFile>> {
        let result = sqlx::query(
            r#"
            SELECT cf.id, cf.vector_store_id, cf.file_id, cf.status, cf.usage_bytes,
                   cf.last_error, cf.chunking_strategy, cf.attributes, cf.created_at, cf.updated_at
            FROM vector_store_files cf
            JOIN files f ON cf.file_id = f.id
            WHERE cf.vector_store_id = ?
              AND cf.deleted_at IS NULL
              AND f.content_hash = ?
              AND f.owner_type = ?
              AND f.owner_id = ?
            LIMIT 1
            "#,
        )
        .bind(vector_store_id.to_string())
        .bind(content_hash)
        .bind(owner_type.as_str())
        .bind(owner_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::vector_store_file_from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_vector_store_files(
        &self,
        vector_store_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<VectorStoreFile>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Handle cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            let (comparison, order, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let query = format!(
                r#"
                SELECT id, vector_store_id, file_id, status, usage_bytes,
                       last_error, chunking_strategy, attributes, created_at, updated_at
                FROM vector_store_files
                WHERE vector_store_id = ? AND deleted_at IS NULL
                AND (created_at, id) {} (?, ?)
                ORDER BY created_at {}, id {}
                LIMIT ?
                "#,
                comparison, order, order
            );

            let rows = sqlx::query(&query)
                .bind(vector_store_id.to_string())
                .bind(cursor.created_at)
                .bind(cursor.id.to_string())
                .bind(fetch_limit)
                .fetch_all(&self.pool)
                .await?;

            let has_more = rows.len() as i64 > limit;
            let mut items: Vec<VectorStoreFile> = rows
                .into_iter()
                .take(limit as usize)
                .map(|row| Self::vector_store_file_from_row(&row))
                .collect::<DbResult<Vec<_>>>()?;

            if should_reverse {
                items.reverse();
            }

            let cursors = PageCursors::from_items(
                &items,
                has_more,
                params.direction,
                Some(cursor),
                Self::cursor_from_file,
            );

            return Ok(ListResult::new(items, has_more, cursors));
        }

        // First page (no cursor)
        let order = params.sort_order.as_sql();
        let query = format!(
            r#"
            SELECT id, vector_store_id, file_id, status, usage_bytes,
                   last_error, chunking_strategy, attributes, created_at, updated_at
            FROM vector_store_files
            WHERE vector_store_id = ? AND deleted_at IS NULL
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            order, order
        );

        let rows = sqlx::query(&query)
            .bind(vector_store_id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<VectorStoreFile> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| Self::vector_store_file_from_row(&row))
            .collect::<DbResult<Vec<_>>>()?;

        let cursors = PageCursors::from_items(
            &items,
            has_more,
            CursorDirection::Forward,
            None,
            Self::cursor_from_file,
        );

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn update_vector_store_file_status(
        &self,
        id: Uuid,
        status: VectorStoreFileStatus,
        error: Option<FileError>,
    ) -> DbResult<()> {
        let now = chrono::Utc::now();
        let error_json = error
            .map(|e| serde_json::to_string(&e))
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?;

        let result = sqlx::query(
            r#"
            UPDATE vector_store_files
            SET status = ?, last_error = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(&error_json)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn update_vector_store_file_usage(&self, id: Uuid, usage_bytes: i64) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE vector_store_files
            SET usage_bytes = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(usage_bytes)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn remove_file_from_vector_store(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE vector_store_files
            SET deleted_at = ?, updated_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(now)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn list_deleted_vector_store_files(
        &self,
        older_than: DateTime<Utc>,
    ) -> DbResult<Vec<VectorStoreFile>> {
        let rows = sqlx::query(
            r#"
            SELECT id, vector_store_id, file_id, status, usage_bytes,
                   last_error, chunking_strategy, attributes, created_at, updated_at
            FROM vector_store_files
            WHERE deleted_at IS NOT NULL AND deleted_at < ?
            "#,
        )
        .bind(older_than)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| Self::vector_store_file_from_row(&row))
            .collect()
    }

    async fn hard_delete_vector_store_file(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            DELETE FROM vector_store_files
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn hard_delete_soft_deleted_references(&self, file_id: Uuid) -> DbResult<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM vector_store_files
            WHERE file_id = ? AND deleted_at IS NOT NULL
            "#,
        )
        .bind(file_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ==================== Aggregates ====================
    // Note: Chunk operations are handled by the VectorStore trait,
    // as chunks are stored in the vector database (pgvector/Qdrant), not the relational database.

    async fn update_vector_store_stats(&self, vector_store_id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        // Calculate aggregate stats from files (excluding soft-deleted)
        // SQLite doesn't have jsonb_build_object, so we build the JSON string manually
        let stats = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(usage_bytes), 0) as total_usage,
                COUNT(*) FILTER (WHERE status = 'cancelled') as cancelled,
                COUNT(*) FILTER (WHERE status = 'completed') as completed,
                COUNT(*) FILTER (WHERE status = 'failed') as failed,
                COUNT(*) FILTER (WHERE status = 'in_progress') as in_progress,
                COUNT(*) as total
            FROM vector_store_files
            WHERE vector_store_id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(vector_store_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        let total_usage: i64 = stats.get("total_usage");
        let cancelled: i32 = stats.get("cancelled");
        let completed: i32 = stats.get("completed");
        let failed: i32 = stats.get("failed");
        let in_progress: i32 = stats.get("in_progress");
        let total: i32 = stats.get("total");

        let file_counts_json = serde_json::json!({
            "cancelled": cancelled,
            "completed": completed,
            "failed": failed,
            "in_progress": in_progress,
            "total": total
        })
        .to_string();

        sqlx::query(
            r#"
            UPDATE vector_stores
            SET usage_bytes = ?, file_counts = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(total_usage)
        .bind(&file_counts_json)
        .bind(now)
        .bind(vector_store_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::VectorStoreOwner;

    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create vector_stores table with embedding fields
        sqlx::query(
            r#"
            CREATE TABLE vector_stores (
                id TEXT PRIMARY KEY NOT NULL,
                owner_type TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                status TEXT NOT NULL DEFAULT 'completed',
                embedding_model TEXT NOT NULL DEFAULT 'text-embedding-3-small',
                embedding_dimensions INTEGER NOT NULL DEFAULT 1536,
                usage_bytes INTEGER NOT NULL DEFAULT 0,
                file_counts TEXT NOT NULL DEFAULT '{"cancelled":0,"completed":0,"failed":0,"in_progress":0,"total":0}',
                metadata TEXT,
                expires_after TEXT,
                expires_at TEXT,
                last_active_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(owner_type, owner_id, name)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create vector stores table");

        // Create files table (OpenAI Files API)
        sqlx::query(
            r#"
            CREATE TABLE files (
                id TEXT PRIMARY KEY NOT NULL,
                owner_type TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                purpose TEXT NOT NULL DEFAULT 'assistants',
                content_type TEXT,
                size_bytes INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'uploaded',
                status_details TEXT,
                content_hash TEXT,
                storage_backend TEXT NOT NULL DEFAULT 'database',
                file_data BLOB,
                storage_path TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create files table");

        // Create vector_store_files table (links files to vector_stores)
        sqlx::query(
            r#"
            CREATE TABLE vector_store_files (
                id TEXT PRIMARY KEY NOT NULL,
                vector_store_id TEXT NOT NULL REFERENCES vector_stores(id) ON DELETE CASCADE,
                file_id TEXT NOT NULL REFERENCES files(id),
                status TEXT NOT NULL DEFAULT 'in_progress',
                usage_bytes INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                chunking_strategy TEXT,
                attributes TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(vector_store_id, file_id)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create vector_store_files table");

        // Note: Chunks are stored in the vector database (pgvector/Qdrant), not here

        pool
    }

    /// Helper to create a test file in the files table
    async fn create_test_file(pool: &SqlitePool, owner_id: Uuid) -> Uuid {
        let file_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO files (id, owner_type, owner_id, filename, purpose, size_bytes, created_at)
            VALUES (?, 'user', ?, 'test.txt', 'assistants', 100, ?)
            "#,
        )
        .bind(file_id.to_string())
        .bind(owner_id.to_string())
        .bind(now)
        .execute(pool)
        .await
        .expect("Failed to create test file");

        file_id
    }

    #[tokio::test]
    async fn test_create_vector_store() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("Test VectorStore".to_string()),
            description: Some("A test vector store".to_string()),
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let vector_store = repo
            .create_vector_store(input)
            .await
            .expect("Failed to create vector store");

        assert!(!vector_store.id.is_nil());
        assert_eq!(vector_store.name, "Test VectorStore");
        assert_eq!(
            vector_store.description,
            Some("A test vector store".to_string())
        );
        assert_eq!(vector_store.owner_type, VectorStoreOwnerType::User);
        assert_eq!(vector_store.owner_id, user_id);
        assert_eq!(vector_store.status, VectorStoreStatus::Completed);
        assert_eq!(vector_store.embedding_model, "text-embedding-3-small");
        assert_eq!(vector_store.embedding_dimensions, 1536);
    }

    #[tokio::test]
    async fn test_get_vector_store() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("Get Test".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let created = repo
            .create_vector_store(input)
            .await
            .expect("Failed to create vector store");

        let fetched = repo
            .get_vector_store(created.id)
            .await
            .expect("Failed to get vector store")
            .expect("VectorStore should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Get Test");
    }

    #[tokio::test]
    async fn test_get_vector_store_by_name() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("Named VectorStore".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let created = repo
            .create_vector_store(input)
            .await
            .expect("Failed to create vector store");

        let fetched = repo
            .get_vector_store_by_name(VectorStoreOwnerType::User, user_id, "Named VectorStore")
            .await
            .expect("Failed to get vector store by name")
            .expect("VectorStore should exist");

        assert_eq!(fetched.id, created.id);
    }

    #[tokio::test]
    async fn test_list_vector_stores() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool);

        let user_id = Uuid::new_v4();

        for i in 0..3 {
            let input = CreateVectorStore {
                owner: VectorStoreOwner::User { user_id },
                file_ids: vec![],
                name: Some(format!("VectorStore {}", i)),
                description: None,
                embedding_model: "text-embedding-3-small".to_string(),
                embedding_dimensions: 1536,
                metadata: None,
                expires_after: None,
                chunking_strategy: None,
            };
            repo.create_vector_store(input)
                .await
                .expect("Failed to create vector store");
        }

        let result = repo
            .list_vector_stores(VectorStoreOwnerType::User, user_id, ListParams::default())
            .await
            .expect("Failed to list collections");

        assert_eq!(result.items.len(), 3);
    }

    #[tokio::test]
    async fn test_update_vector_store() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("Original".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let created = repo
            .create_vector_store(input)
            .await
            .expect("Failed to create vector store");

        let updated = repo
            .update_vector_store(
                created.id,
                UpdateVectorStore {
                    name: Some("Updated".to_string()),
                    description: Some("New description".to_string()),
                    metadata: None,
                    expires_after: None,
                },
            )
            .await
            .expect("Failed to update vector store");

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.description, Some("New description".to_string()));
    }

    #[tokio::test]
    async fn test_delete_vector_store() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool);

        let user_id = Uuid::new_v4();
        let input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("To Delete".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let created = repo
            .create_vector_store(input)
            .await
            .expect("Failed to create vector store");

        repo.delete_vector_store(created.id)
            .await
            .expect("Failed to delete vector store");

        let result = repo
            .get_vector_store(created.id)
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_add_and_get_vector_store_file() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool.clone());

        let user_id = Uuid::new_v4();
        let vector_store_input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("File Test".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let vector_store = repo
            .create_vector_store(vector_store_input)
            .await
            .expect("Failed to create vector store");

        // First create a file in the files table
        let file_id = create_test_file(&pool, user_id).await;

        // Then link it to the vector store
        let add_file_input = AddFileToVectorStore {
            vector_store_id: vector_store.id,
            file_id,
            chunking_strategy: None,
            attributes: None,
        };

        let vector_store_file = repo
            .add_file_to_vector_store(add_file_input)
            .await
            .expect("Failed to add file to vector store");

        assert_eq!(vector_store_file.vector_store_id, vector_store.id);
        assert_eq!(vector_store_file.file_id, file_id);
        assert_eq!(vector_store_file.status, VectorStoreFileStatus::InProgress);

        let fetched = repo
            .get_vector_store_file(vector_store_file.internal_id)
            .await
            .expect("Failed to get vector store file")
            .expect("VectorStore file should exist");

        assert_eq!(fetched.internal_id, vector_store_file.internal_id);
        assert_eq!(fetched.file_id, file_id);
    }

    // Note: Chunk tests have been removed as chunks are now stored in the vector database,
    // not the relational database. See VectorStore tests for chunk operations.

    #[tokio::test]
    async fn test_update_vector_store_stats() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool.clone());

        let user_id = Uuid::new_v4();
        let vector_store_input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("Stats Test".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let vector_store = repo
            .create_vector_store(vector_store_input)
            .await
            .expect("Failed to create vector store");

        // First create a file in the files table
        let file_id = create_test_file(&pool, user_id).await;

        // Add file to vector store
        let add_file_input = AddFileToVectorStore {
            vector_store_id: vector_store.id,
            file_id,
            chunking_strategy: None,
            attributes: None,
        };

        let vector_store_file = repo
            .add_file_to_vector_store(add_file_input)
            .await
            .expect("Failed to add file to vector store");

        repo.update_vector_store_file_usage(vector_store_file.internal_id, 50)
            .await
            .expect("Failed to update file usage");

        repo.update_vector_store_file_status(
            vector_store_file.internal_id,
            VectorStoreFileStatus::Completed,
            None,
        )
        .await
        .expect("Failed to update file status");

        repo.update_vector_store_stats(vector_store.id)
            .await
            .expect("Failed to update stats");

        let updated = repo
            .get_vector_store(vector_store.id)
            .await
            .expect("Failed to get vector store")
            .expect("VectorStore should exist");

        assert_eq!(updated.usage_bytes, 50);
        assert_eq!(updated.file_counts.completed, 1);
        assert_eq!(updated.file_counts.total, 1);
    }

    #[tokio::test]
    async fn test_find_vector_store_file_by_file_id() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool.clone());

        let user_id = Uuid::new_v4();
        let vector_store_input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("Idempotency Test".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let vector_store = repo
            .create_vector_store(vector_store_input)
            .await
            .expect("Failed to create vector store");

        // Create a test file
        let file_id = create_test_file(&pool, user_id).await;

        // Initially, file should not be in the vector store
        let result = repo
            .find_vector_store_file_by_file_id(vector_store.id, file_id)
            .await
            .expect("Query should succeed");
        assert!(result.is_none(), "Should not find file before adding");

        // Add the file to the vector store
        let add_file_input = AddFileToVectorStore {
            vector_store_id: vector_store.id,
            file_id,
            chunking_strategy: None,
            attributes: None,
        };

        let vector_store_file = repo
            .add_file_to_vector_store(add_file_input)
            .await
            .expect("Failed to add file to vector store");

        // Now should find the file by file_id
        let result = repo
            .find_vector_store_file_by_file_id(vector_store.id, file_id)
            .await
            .expect("Query should succeed");
        assert!(result.is_some(), "Should find file after adding");
        let found = result.unwrap();
        assert_eq!(found.internal_id, vector_store_file.internal_id);
        assert_eq!(found.file_id, file_id);

        // Different file_id should not find anything
        let different_file_id = create_test_file(&pool, user_id).await;
        let result = repo
            .find_vector_store_file_by_file_id(vector_store.id, different_file_id)
            .await
            .expect("Query should succeed");
        assert!(
            result.is_none(),
            "Should not find different file in vector store"
        );

        // Different vector store should not find the file
        let other_vector_store_input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("Other VectorStore".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };
        let other_vector_store = repo
            .create_vector_store(other_vector_store_input)
            .await
            .expect("Failed to create other vector store");

        let result = repo
            .find_vector_store_file_by_file_id(other_vector_store.id, file_id)
            .await
            .expect("Query should succeed");
        assert!(
            result.is_none(),
            "Should not find file in different vector store"
        );
    }

    #[tokio::test]
    async fn test_find_vector_store_file_by_file_id_excludes_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool.clone());

        let user_id = Uuid::new_v4();
        let vector_store_input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("Idempotency Deleted Test".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let vector_store = repo
            .create_vector_store(vector_store_input)
            .await
            .expect("Failed to create vector store");

        // Create and add a file
        let file_id = create_test_file(&pool, user_id).await;

        let add_file_input = AddFileToVectorStore {
            vector_store_id: vector_store.id,
            file_id,
            chunking_strategy: None,
            attributes: None,
        };

        let vector_store_file = repo
            .add_file_to_vector_store(add_file_input)
            .await
            .expect("Failed to add file to vector store");

        // Should find it
        let result = repo
            .find_vector_store_file_by_file_id(vector_store.id, file_id)
            .await
            .expect("Query should succeed");
        assert!(result.is_some());

        // Soft-delete the file
        repo.remove_file_from_vector_store(vector_store_file.internal_id)
            .await
            .expect("Failed to remove file");

        // Should NOT find it anymore (soft-deleted)
        let result = repo
            .find_vector_store_file_by_file_id(vector_store.id, file_id)
            .await
            .expect("Query should succeed");
        assert!(
            result.is_none(),
            "Should not find soft-deleted file by file_id"
        );
    }

    /// Helper to create a test file with a specific content hash and owner
    async fn create_test_file_with_hash_and_owner(
        pool: &SqlitePool,
        owner_id: Uuid,
        content_hash: &str,
    ) -> Uuid {
        let file_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO files (id, owner_type, owner_id, filename, purpose, size_bytes, content_hash, created_at)
            VALUES (?, 'user', ?, 'test.txt', 'assistants', 100, ?, ?)
            "#,
        )
        .bind(file_id.to_string())
        .bind(owner_id.to_string())
        .bind(content_hash)
        .bind(now)
        .execute(pool)
        .await
        .expect("Failed to create test file with hash");

        file_id
    }

    #[tokio::test]
    async fn test_find_vector_store_file_by_content_hash_and_owner() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool.clone());

        let user_id = Uuid::new_v4();
        let vector_store_input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("Same-Owner Dedup Test".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let vector_store = repo
            .create_vector_store(vector_store_input)
            .await
            .expect("Failed to create vector store");

        // Create a file with a known content hash
        let content_hash = "abc123def456abc123def456abc123def456abc123def456abc123def456abcd";
        let file_id = create_test_file_with_hash_and_owner(&pool, user_id, content_hash).await;

        // Initially, no file with this hash should be in the vector store
        let result = repo
            .find_vector_store_file_by_content_hash_and_owner(
                vector_store.id,
                content_hash,
                VectorStoreOwnerType::User,
                user_id,
            )
            .await
            .expect("Query should succeed");
        assert!(result.is_none(), "Should not find file before adding");

        // Add the file to the vector store
        let add_file_input = AddFileToVectorStore {
            vector_store_id: vector_store.id,
            file_id,
            chunking_strategy: None,
            attributes: None,
        };

        let vector_store_file = repo
            .add_file_to_vector_store(add_file_input)
            .await
            .expect("Failed to add file to vector store");

        // Now should find the file by content hash and owner
        let result = repo
            .find_vector_store_file_by_content_hash_and_owner(
                vector_store.id,
                content_hash,
                VectorStoreOwnerType::User,
                user_id,
            )
            .await
            .expect("Query should succeed");
        assert!(result.is_some(), "Should find file after adding");
        let found = result.unwrap();
        assert_eq!(found.internal_id, vector_store_file.internal_id);
        assert_eq!(found.file_id, file_id);

        // Different owner should NOT find the file (cross-user protection)
        let other_user_id = Uuid::new_v4();
        let result = repo
            .find_vector_store_file_by_content_hash_and_owner(
                vector_store.id,
                content_hash,
                VectorStoreOwnerType::User,
                other_user_id,
            )
            .await
            .expect("Query should succeed");
        assert!(
            result.is_none(),
            "Should not find file with different owner"
        );

        // Different hash should not find anything
        let different_hash = "zzz999zzz999zzz999zzz999zzz999zzz999zzz999zzz999zzz999zzz999zzzz";
        let result = repo
            .find_vector_store_file_by_content_hash_and_owner(
                vector_store.id,
                different_hash,
                VectorStoreOwnerType::User,
                user_id,
            )
            .await
            .expect("Query should succeed");
        assert!(result.is_none(), "Should not find file with different hash");
    }

    #[tokio::test]
    async fn test_find_vector_store_file_by_content_hash_and_owner_excludes_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteVectorStoresRepo::new(pool.clone());

        let user_id = Uuid::new_v4();
        let vector_store_input = CreateVectorStore {
            owner: VectorStoreOwner::User { user_id },
            file_ids: vec![],
            name: Some("Same-Owner Dedup Deleted Test".to_string()),
            description: None,
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimensions: 1536,
            metadata: None,
            expires_after: None,
            chunking_strategy: None,
        };

        let vector_store = repo
            .create_vector_store(vector_store_input)
            .await
            .expect("Failed to create vector store");

        // Create and add a file
        let content_hash = "deleted123deleted123deleted123deleted123deleted123deleted123dele";
        let file_id = create_test_file_with_hash_and_owner(&pool, user_id, content_hash).await;

        let add_file_input = AddFileToVectorStore {
            vector_store_id: vector_store.id,
            file_id,
            chunking_strategy: None,
            attributes: None,
        };

        let vector_store_file = repo
            .add_file_to_vector_store(add_file_input)
            .await
            .expect("Failed to add file to vector store");

        // Should find it
        let result = repo
            .find_vector_store_file_by_content_hash_and_owner(
                vector_store.id,
                content_hash,
                VectorStoreOwnerType::User,
                user_id,
            )
            .await
            .expect("Query should succeed");
        assert!(result.is_some());

        // Soft-delete the file
        repo.remove_file_from_vector_store(vector_store_file.internal_id)
            .await
            .expect("Failed to remove file");

        // Should NOT find it anymore (soft-deleted)
        let result = repo
            .find_vector_store_file_by_content_hash_and_owner(
                vector_store.id,
                content_hash,
                VectorStoreOwnerType::User,
                user_id,
            )
            .await
            .expect("Query should succeed");
        assert!(
            result.is_none(),
            "Should not find soft-deleted file by content hash"
        );
    }
}
