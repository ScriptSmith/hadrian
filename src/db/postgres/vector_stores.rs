use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{Cursor, CursorDirection, ListParams, ListResult, PageCursors, VectorStoresRepo},
    },
    models::{
        AddFileToVectorStore, ChunkingStrategy, CreateVectorStore, ExpiresAfter, FileCounts,
        FileError, OBJECT_TYPE_VECTOR_STORE, OBJECT_TYPE_VECTOR_STORE_FILE, UpdateVectorStore,
        VectorStore, VectorStoreFile, VectorStoreFileStatus, VectorStoreOwnerType,
    },
};

pub struct PostgresVectorStoresRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresVectorStoresRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    fn parse_file_counts(json_value: serde_json::Value) -> DbResult<FileCounts> {
        serde_json::from_value(json_value).map_err(|e| DbError::Internal(e.to_string()))
    }

    fn parse_metadata(
        json_value: Option<serde_json::Value>,
    ) -> DbResult<Option<HashMap<String, serde_json::Value>>> {
        match json_value {
            Some(v) => serde_json::from_value(v).map_err(|e| DbError::Internal(e.to_string())),
            None => Ok(None),
        }
    }

    fn parse_expires_after(
        json_value: Option<serde_json::Value>,
    ) -> DbResult<Option<ExpiresAfter>> {
        match json_value {
            Some(v) => serde_json::from_value(v).map_err(|e| DbError::Internal(e.to_string())),
            None => Ok(None),
        }
    }

    fn parse_chunking_strategy(
        json_value: Option<serde_json::Value>,
    ) -> DbResult<Option<ChunkingStrategy>> {
        match json_value {
            Some(v) => serde_json::from_value(v).map_err(|e| DbError::Internal(e.to_string())),
            None => Ok(None),
        }
    }

    fn parse_file_error(json_value: Option<serde_json::Value>) -> DbResult<Option<FileError>> {
        match json_value {
            Some(v) => serde_json::from_value(v).map_err(|e| DbError::Internal(e.to_string())),
            None => Ok(None),
        }
    }

    fn parse_attributes(
        json_value: Option<serde_json::Value>,
    ) -> DbResult<Option<HashMap<String, serde_json::Value>>> {
        Self::parse_metadata(json_value)
    }

    /// Parse a VectorStore from a database row.
    /// Expects columns: id, owner_type (as TEXT), owner_id, name, description, status (as TEXT),
    /// embedding_model, embedding_dimensions, usage_bytes, file_counts, metadata, expires_after,
    /// expires_at, last_active_at, created_at, updated_at
    fn vector_store_from_row(row: &sqlx::postgres::PgRow) -> DbResult<VectorStore> {
        let owner_type_str: String = row.get("owner_type");
        let status_str: String = row.get("status");

        Ok(VectorStore {
            id: row.get("id"),
            object: OBJECT_TYPE_VECTOR_STORE.to_string(),
            owner_type: owner_type_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            owner_id: row.get("owner_id"),
            name: row.get("name"),
            description: row.get("description"),
            status: status_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            embedding_model: row.get("embedding_model"),
            embedding_dimensions: row.get("embedding_dimensions"),
            usage_bytes: row.get("usage_bytes"),
            file_counts: Self::parse_file_counts(row.get("file_counts"))?,
            metadata: Self::parse_metadata(row.get("metadata"))?,
            expires_after: Self::parse_expires_after(row.get("expires_after"))?,
            expires_at: row.get("expires_at"),
            last_active_at: row.get("last_active_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Parse a VectorStoreFile from a database row.
    /// Expects columns: id, vector_store_id, file_id, status (as TEXT), usage_bytes, last_error,
    /// chunking_strategy, attributes, created_at, updated_at
    fn vector_store_file_from_row(row: &sqlx::postgres::PgRow) -> DbResult<VectorStoreFile> {
        let status_str: String = row.get("status");

        Ok(VectorStoreFile {
            internal_id: row.get("id"),
            file_id: row.get("file_id"),
            object: OBJECT_TYPE_VECTOR_STORE_FILE.to_string(),
            vector_store_id: row.get("vector_store_id"),
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
impl VectorStoresRepo for PostgresVectorStoresRepo {
    // ==================== Vector Stores CRUD ====================

    async fn create_vector_store(&self, input: CreateVectorStore) -> DbResult<VectorStore> {
        let id = Uuid::new_v4();
        let owner_type = input.owner.owner_type();
        let owner_id = input.owner.owner_id();
        // Generate name if not provided (OpenAI-compatible: name is optional)
        let name = input
            .name
            .unwrap_or_else(|| format!("Vector Store {}", &id.to_string()[..8]));
        let metadata_json = input
            .metadata
            .map(|m| serde_json::to_value(&m))
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?;
        let expires_after_json = input
            .expires_after
            .map(|e| serde_json::to_value(&e))
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?;

        let row = sqlx::query(
            r#"
            INSERT INTO vector_stores (id, owner_type, owner_id, name, description, embedding_model, embedding_dimensions, metadata, expires_after)
            VALUES ($1, $2::vector_store_owner_type, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                      usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(owner_type.as_str())
        .bind(owner_id)
        .bind(&name)
        .bind(&input.description)
        .bind(&input.embedding_model)
        .bind(input.embedding_dimensions)
        .bind(&metadata_json)
        .bind(&expires_after_json)
        .fetch_one(&self.write_pool)
        .await?;

        Self::vector_store_from_row(&row)
    }

    async fn get_vector_store(&self, id: Uuid) -> DbResult<Option<VectorStore>> {
        let result = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                   usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
            FROM vector_stores
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::vector_store_from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<VectorStore>> {
        let result = sqlx::query(
            r#"
            SELECT vs.id, vs.owner_type::TEXT, vs.owner_id, vs.name, vs.description, vs.status::TEXT, vs.embedding_model, vs.embedding_dimensions,
                   vs.usage_bytes, vs.file_counts, vs.metadata, vs.expires_after, vs.expires_at, vs.last_active_at, vs.created_at, vs.updated_at
            FROM vector_stores vs
            WHERE vs.id = $1 AND vs.deleted_at IS NULL
            AND (
                (vs.owner_type = 'organization'::vector_store_owner_type AND vs.owner_id = $2)
                OR
                (vs.owner_type = 'team'::vector_store_owner_type AND EXISTS (
                    SELECT 1 FROM teams t WHERE t.id = vs.owner_id AND t.org_id = $3
                ))
                OR
                (vs.owner_type = 'project'::vector_store_owner_type AND EXISTS (
                    SELECT 1 FROM projects pr WHERE pr.id = vs.owner_id AND pr.org_id = $4
                ))
                OR
                (vs.owner_type = 'user'::vector_store_owner_type AND EXISTS (
                    SELECT 1 FROM org_memberships om WHERE om.user_id = vs.owner_id AND om.org_id = $5
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
            SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                   usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
            FROM vector_stores
            WHERE owner_type = $1::vector_store_owner_type AND owner_id = $2 AND name = $3 AND deleted_at IS NULL
            "#,
        )
        .bind(owner_type.as_str())
        .bind(owner_id)
        .bind(name)
        .fetch_optional(&self.read_pool)
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
                SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE owner_type = $1::vector_store_owner_type AND owner_id = $2
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
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
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
                SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE owner_type = $1::vector_store_owner_type AND owner_id = $2
                ORDER BY updated_at {}, id {}
                LIMIT $3
                "#,
                order, order
            )
        } else {
            format!(
                r#"
                SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE owner_type = $1::vector_store_owner_type AND owner_id = $2 AND deleted_at IS NULL
                ORDER BY updated_at {}, id {}
                LIMIT $3
                "#,
                order, order
            )
        };

        let rows = sqlx::query(&query)
            .bind(owner_type.as_str())
            .bind(owner_id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
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
        // PostgreSQL uses $N for positional parameters
        let mut conditions = Vec::new();
        let mut bindings: Vec<(String, Uuid)> = Vec::new();
        let mut param_index = 1;

        if let Some(uid) = user_id {
            conditions.push(format!(
                "(owner_type = ${}::vector_store_owner_type AND owner_id = ${})",
                param_index,
                param_index + 1
            ));
            bindings.push(("user".to_string(), uid));
            param_index += 2;
        }

        for org_id in org_ids {
            conditions.push(format!(
                "(owner_type = ${}::vector_store_owner_type AND owner_id = ${})",
                param_index,
                param_index + 1
            ));
            bindings.push(("organization".to_string(), *org_id));
            param_index += 2;
        }

        for team_id in team_ids {
            conditions.push(format!(
                "(owner_type = ${}::vector_store_owner_type AND owner_id = ${})",
                param_index,
                param_index + 1
            ));
            bindings.push(("team".to_string(), *team_id));
            param_index += 2;
        }

        for project_id in project_ids {
            conditions.push(format!(
                "(owner_type = ${}::vector_store_owner_type AND owner_id = ${})",
                param_index,
                param_index + 1
            ));
            bindings.push(("project".to_string(), *project_id));
            param_index += 2;
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

            let cursor_param = param_index;
            let id_param = param_index + 1;
            let limit_param = param_index + 2;

            let query = format!(
                r#"
                SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE ({})
                AND ROW(updated_at, id) {} ROW(${}, ${})
                {}
                ORDER BY updated_at {}, id {}
                LIMIT ${}
                "#,
                owner_filter,
                comparison,
                cursor_param,
                id_param,
                deleted_filter,
                order_dir,
                order_dir,
                limit_param
            );

            // Build the query dynamically
            let mut query_builder = sqlx::query(&query);
            for (owner_type, owner_id) in &bindings {
                query_builder = query_builder.bind(owner_type).bind(owner_id);
            }
            query_builder = query_builder
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit);

            let rows = query_builder.fetch_all(&self.read_pool).await?;

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
        let limit_param = param_index;
        let query = if params.include_deleted {
            format!(
                r#"
                SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE ({})
                ORDER BY updated_at {}, id {}
                LIMIT ${}
                "#,
                owner_filter, order, order, limit_param
            )
        } else {
            format!(
                r#"
                SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE ({}) AND deleted_at IS NULL
                ORDER BY updated_at {}, id {}
                LIMIT ${}
                "#,
                owner_filter, order, order, limit_param
            )
        };

        // Build the query dynamically
        let mut query_builder = sqlx::query(&query);
        for (owner_type, owner_id) in &bindings {
            query_builder = query_builder.bind(owner_type).bind(owner_id);
        }
        query_builder = query_builder.bind(fetch_limit);

        let rows = query_builder.fetch_all(&self.read_pool).await?;

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
                SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE ROW(updated_at, id) {} ROW($1, $2)
                {}
                ORDER BY updated_at {}, id {}
                LIMIT $3
                "#,
                comparison, deleted_filter, order_dir, order_dir
            );

            let rows = sqlx::query(&query)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
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
                SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                ORDER BY updated_at {}, id {}
                LIMIT $1
                "#,
                order, order
            )
        } else {
            format!(
                r#"
                SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                       usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
                FROM vector_stores
                WHERE deleted_at IS NULL
                ORDER BY updated_at {}, id {}
                LIMIT $1
                "#,
                order, order
            )
        };

        let rows = sqlx::query(&query)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
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
        let mut tx = self.write_pool.begin().await?;

        // Get current values with FOR UPDATE lock
        let current = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                   usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
            FROM vector_stores
            WHERE id = $1 AND deleted_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DbError::NotFound)?;

        let current_name: String = current.get("name");
        let current_description: Option<String> = current.get("description");
        let current_metadata: Option<serde_json::Value> = current.get("metadata");
        let current_expires_after: Option<serde_json::Value> = current.get("expires_after");

        let new_name = input.name.unwrap_or(current_name);
        let new_description = input.description.or(current_description);
        let new_metadata = input
            .metadata
            .map(|m| serde_json::to_value(&m))
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?
            .or(current_metadata);
        let new_expires_after = input
            .expires_after
            .map(|e| serde_json::to_value(&e))
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?
            .or(current_expires_after);

        let row = sqlx::query(
            r#"
            UPDATE vector_stores
            SET name = $1, description = $2, metadata = $3, expires_after = $4, updated_at = NOW()
            WHERE id = $5 AND deleted_at IS NULL
            RETURNING id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                      usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
            "#,
        )
        .bind(&new_name)
        .bind(&new_description)
        .bind(&new_metadata)
        .bind(&new_expires_after)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(DbError::NotFound)?;

        tx.commit().await?;

        let owner_type_str: String = row.get("owner_type");
        let status_str: String = row.get("status");

        Ok(VectorStore {
            id: row.get("id"),
            object: OBJECT_TYPE_VECTOR_STORE.to_string(),
            owner_type: owner_type_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            owner_id: row.get("owner_id"),
            name: row.get("name"),
            description: row.get("description"),
            status: status_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            embedding_model: row.get("embedding_model"),
            embedding_dimensions: row.get("embedding_dimensions"),
            usage_bytes: row.get("usage_bytes"),
            file_counts: Self::parse_file_counts(row.get("file_counts"))?,
            metadata: Self::parse_metadata(row.get("metadata"))?,
            expires_after: Self::parse_expires_after(row.get("expires_after"))?,
            expires_at: row.get("expires_at"),
            last_active_at: row.get("last_active_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn delete_vector_store(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE vector_stores
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

    async fn touch_vector_store(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE vector_stores
            SET last_active_at = NOW(), updated_at = NOW()
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

    async fn hard_delete_vector_store(&self, id: Uuid) -> DbResult<()> {
        // First delete all vector_store_files links
        sqlx::query(
            r#"
            DELETE FROM vector_store_files
            WHERE vector_store_id = $1
            "#,
        )
        .bind(id)
        .execute(&self.write_pool)
        .await?;

        // Then delete the vector store
        let result = sqlx::query(
            r#"
            DELETE FROM vector_stores
            WHERE id = $1
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

    async fn list_deleted_vector_stores(
        &self,
        older_than: DateTime<Utc>,
    ) -> DbResult<Vec<VectorStore>> {
        let rows = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, name, description, status::TEXT, embedding_model, embedding_dimensions,
                   usage_bytes, file_counts, metadata, expires_after, expires_at, last_active_at, created_at, updated_at
            FROM vector_stores
            WHERE deleted_at IS NOT NULL AND deleted_at < $1
            "#,
        )
        .bind(older_than)
        .fetch_all(&self.read_pool)
        .await?;

        rows.into_iter()
            .map(|row| Self::vector_store_from_row(&row))
            .collect()
    }

    // ==================== VectorStore Files CRUD ====================

    async fn add_file_to_vector_store(
        &self,
        input: AddFileToVectorStore,
    ) -> DbResult<VectorStoreFile> {
        let id = Uuid::new_v4();
        let chunking_strategy_json = input
            .chunking_strategy
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?;
        let attributes_json = input
            .attributes
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?;

        let row = sqlx::query(
            r#"
            INSERT INTO vector_store_files (id, vector_store_id, file_id, chunking_strategy, attributes)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, vector_store_id, file_id, status::TEXT, usage_bytes,
                      last_error, chunking_strategy, attributes, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.vector_store_id)
        .bind(input.file_id)
        .bind(&chunking_strategy_json)
        .bind(&attributes_json)
        .fetch_one(&self.write_pool)
        .await?;

        let status_str: String = row.get("status");

        Ok(VectorStoreFile {
            internal_id: row.get("id"),
            file_id: row.get("file_id"),
            object: OBJECT_TYPE_VECTOR_STORE_FILE.to_string(),
            vector_store_id: row.get("vector_store_id"),
            status: status_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            usage_bytes: row.get("usage_bytes"),
            last_error: Self::parse_file_error(row.get("last_error"))?,
            chunking_strategy: input.chunking_strategy,
            attributes: input.attributes,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn get_vector_store_file(&self, id: Uuid) -> DbResult<Option<VectorStoreFile>> {
        let result = sqlx::query(
            r#"
            SELECT id, vector_store_id, file_id, status::TEXT, usage_bytes,
                   last_error, chunking_strategy, attributes, created_at, updated_at
            FROM vector_store_files
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
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
            SELECT id, vector_store_id, file_id, status::TEXT, usage_bytes,
                   last_error, chunking_strategy, attributes, created_at, updated_at
            FROM vector_store_files
            WHERE vector_store_id = $1
              AND file_id = $2
              AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(vector_store_id)
        .bind(file_id)
        .fetch_optional(&self.read_pool)
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
            SELECT cf.id, cf.vector_store_id, cf.file_id, cf.status::TEXT, cf.usage_bytes,
                   cf.last_error, cf.chunking_strategy, cf.attributes, cf.created_at, cf.updated_at
            FROM vector_store_files cf
            JOIN files f ON cf.file_id = f.id
            WHERE cf.vector_store_id = $1
              AND cf.deleted_at IS NULL
              AND f.content_hash = $2
              AND f.owner_type = $3::vector_store_owner_type
              AND f.owner_id = $4
            LIMIT 1
            "#,
        )
        .bind(vector_store_id)
        .bind(content_hash)
        .bind(owner_type.as_str())
        .bind(owner_id)
        .fetch_optional(&self.read_pool)
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
                SELECT id, vector_store_id, file_id, status::TEXT, usage_bytes,
                       last_error, chunking_strategy, attributes, created_at, updated_at
                FROM vector_store_files
                WHERE vector_store_id = $1 AND deleted_at IS NULL
                AND ROW(created_at, id) {} ROW($2, $3)
                ORDER BY created_at {}, id {}
                LIMIT $4
                "#,
                comparison, order, order
            );

            let rows = sqlx::query(&query)
                .bind(vector_store_id)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(fetch_limit)
                .fetch_all(&self.read_pool)
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
            SELECT id, vector_store_id, file_id, status::TEXT, usage_bytes,
                   last_error, chunking_strategy, attributes, created_at, updated_at
            FROM vector_store_files
            WHERE vector_store_id = $1 AND deleted_at IS NULL
            ORDER BY created_at {}, id {}
            LIMIT $2
            "#,
            order, order
        );

        let rows = sqlx::query(&query)
            .bind(vector_store_id)
            .bind(fetch_limit)
            .fetch_all(&self.read_pool)
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
        let error_json = error
            .map(|e| serde_json::to_value(&e))
            .transpose()
            .map_err(|e| DbError::Internal(e.to_string()))?;

        let result = sqlx::query(
            r#"
            UPDATE vector_store_files
            SET status = $1::vector_store_file_status, last_error = $2, updated_at = NOW()
            WHERE id = $3
            "#,
        )
        .bind(status.as_str())
        .bind(&error_json)
        .bind(id)
        .execute(&self.write_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn update_vector_store_file_usage(&self, id: Uuid, usage_bytes: i64) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE vector_store_files
            SET usage_bytes = $1, updated_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(usage_bytes)
        .bind(id)
        .execute(&self.write_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn remove_file_from_vector_store(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE vector_store_files
            SET deleted_at = NOW(), updated_at = NOW()
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

    async fn list_deleted_vector_store_files(
        &self,
        older_than: DateTime<Utc>,
    ) -> DbResult<Vec<VectorStoreFile>> {
        let rows = sqlx::query(
            r#"
            SELECT id, vector_store_id, file_id, status::TEXT, usage_bytes,
                   last_error, chunking_strategy, attributes, created_at, updated_at
            FROM vector_store_files
            WHERE deleted_at IS NOT NULL AND deleted_at < $1
            "#,
        )
        .bind(older_than)
        .fetch_all(&self.read_pool)
        .await?;

        rows.into_iter()
            .map(|row| Self::vector_store_file_from_row(&row))
            .collect()
    }

    async fn hard_delete_vector_store_file(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            DELETE FROM vector_store_files
            WHERE id = $1
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

    async fn hard_delete_soft_deleted_references(&self, file_id: Uuid) -> DbResult<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM vector_store_files
            WHERE file_id = $1 AND deleted_at IS NOT NULL
            "#,
        )
        .bind(file_id)
        .execute(&self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }

    // ==================== Aggregates ====================
    // Note: Chunk operations are handled by the VectorStore trait,
    // as chunks are stored in the vector database (pgvector/Qdrant), not the relational database.

    async fn update_vector_store_stats(&self, vector_store_id: Uuid) -> DbResult<()> {
        // Calculate aggregate stats from files (excluding soft-deleted)
        sqlx::query(
            r#"
            UPDATE vector_stores
            SET
                usage_bytes = COALESCE((
                    SELECT SUM(usage_bytes)
                    FROM vector_store_files
                    WHERE vector_store_id = $1 AND deleted_at IS NULL
                ), 0),
                file_counts = (
                    SELECT jsonb_build_object(
                        'cancelled', COUNT(*) FILTER (WHERE status = 'cancelled'),
                        'completed', COUNT(*) FILTER (WHERE status = 'completed'),
                        'failed', COUNT(*) FILTER (WHERE status = 'failed'),
                        'in_progress', COUNT(*) FILTER (WHERE status = 'in_progress'),
                        'total', COUNT(*)
                    )
                    FROM vector_store_files
                    WHERE vector_store_id = $1 AND deleted_at IS NULL
                ),
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(vector_store_id)
        .execute(&self.write_pool)
        .await?;

        Ok(())
    }
}
