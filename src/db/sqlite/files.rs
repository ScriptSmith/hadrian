use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{Cursor, CursorDirection, FilesRepo, ListParams, ListResult, PageCursors},
    },
    models::{CreateFile, File, FilePurpose, FileStatus, OBJECT_TYPE_FILE, VectorStoreOwnerType},
};

pub struct SqliteFilesRepo {
    pool: SqlitePool,
}

impl SqliteFilesRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn cursor_from_file(file: &File) -> Cursor {
        Cursor::new(file.created_at, file.id)
    }
}

#[async_trait]
impl FilesRepo for SqliteFilesRepo {
    async fn create_file(&self, input: CreateFile) -> DbResult<File> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO files (id, owner_type, owner_id, filename, purpose, content_type, size_bytes, status, content_hash, storage_backend, file_data, storage_path, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(input.owner_type.as_str())
        .bind(input.owner_id.to_string())
        .bind(&input.filename)
        .bind(input.purpose.as_str())
        .bind(&input.content_type)
        .bind(input.size_bytes)
        .bind(FileStatus::Uploaded.as_str())
        .bind(&input.content_hash)
        .bind(input.storage_backend.as_str())
        .bind(&input.file_data)
        .bind(&input.storage_path)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(File {
            id,
            object: OBJECT_TYPE_FILE.to_string(),
            owner_type: input.owner_type,
            owner_id: input.owner_id,
            filename: input.filename,
            purpose: input.purpose,
            content_type: input.content_type,
            size_bytes: input.size_bytes,
            status: FileStatus::Uploaded,
            status_details: None,
            content_hash: input.content_hash,
            storage_backend: input.storage_backend,
            storage_path: input.storage_path,
            created_at: now,
            expires_at: None,
        })
    }

    async fn get_file(&self, id: Uuid) -> DbResult<Option<File>> {
        let result = sqlx::query(
            r#"
            SELECT id, owner_type, owner_id, filename, purpose, content_type, size_bytes, status,
                   status_details, content_hash, storage_backend, storage_path, created_at, expires_at
            FROM files
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let owner_type_str: String = row.get("owner_type");
                let purpose_str: String = row.get("purpose");
                let status_str: String = row.get("status");
                let storage_backend_str: String = row.get("storage_backend");

                Ok(Some(File {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    object: OBJECT_TYPE_FILE.to_string(),
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: parse_uuid(&row.get::<String, _>("owner_id"))?,
                    filename: row.get("filename"),
                    purpose: purpose_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    content_type: row.get("content_type"),
                    size_bytes: row.get("size_bytes"),
                    status: status_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    status_details: row.get("status_details"),
                    content_hash: row.get("content_hash"),
                    storage_backend: storage_backend_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    storage_path: row.get("storage_path"),
                    created_at: row.get("created_at"),
                    expires_at: row.get("expires_at"),
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_file_data(&self, id: Uuid) -> DbResult<Option<Vec<u8>>> {
        let result = sqlx::query(
            r#"
            SELECT file_data
            FROM files
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.and_then(|row| row.get::<Option<Vec<u8>>, _>("file_data")))
    }

    async fn list_files(
        &self,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
        purpose: Option<FilePurpose>,
        params: ListParams,
    ) -> DbResult<ListResult<File>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        // Handle cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            let (comparison, order, should_reverse) =
                params.sort_order.cursor_query_params(params.direction);

            let (query, bind_purpose) = match &purpose {
                Some(p) => (
                    format!(
                        r#"
                        SELECT id, owner_type, owner_id, filename, purpose, content_type, size_bytes, status,
                               status_details, content_hash, storage_backend, storage_path, created_at, expires_at
                        FROM files
                        WHERE owner_type = ? AND owner_id = ? AND purpose = ?
                        AND (created_at, id) {} (?, ?)
                        ORDER BY created_at {}, id {}
                        LIMIT ?
                        "#,
                        comparison, order, order
                    ),
                    Some(p.as_str().to_string()),
                ),
                None => (
                    format!(
                        r#"
                        SELECT id, owner_type, owner_id, filename, purpose, content_type, size_bytes, status,
                               status_details, content_hash, storage_backend, storage_path, created_at, expires_at
                        FROM files
                        WHERE owner_type = ? AND owner_id = ?
                        AND (created_at, id) {} (?, ?)
                        ORDER BY created_at {}, id {}
                        LIMIT ?
                        "#,
                        comparison, order, order
                    ),
                    None,
                ),
            };

            let rows = if let Some(purpose_str) = bind_purpose {
                sqlx::query(&query)
                    .bind(owner_type.as_str())
                    .bind(owner_id.to_string())
                    .bind(purpose_str)
                    .bind(cursor.created_at)
                    .bind(cursor.id.to_string())
                    .bind(fetch_limit)
                    .fetch_all(&self.pool)
                    .await?
            } else {
                sqlx::query(&query)
                    .bind(owner_type.as_str())
                    .bind(owner_id.to_string())
                    .bind(cursor.created_at)
                    .bind(cursor.id.to_string())
                    .bind(fetch_limit)
                    .fetch_all(&self.pool)
                    .await?
            };

            let has_more = rows.len() as i64 > limit;
            let mut items: Vec<File> = rows
                .into_iter()
                .take(limit as usize)
                .map(|row| {
                    let owner_type_str: String = row.get("owner_type");
                    let purpose_str: String = row.get("purpose");
                    let status_str: String = row.get("status");
                    let storage_backend_str: String = row.get("storage_backend");

                    Ok(File {
                        id: parse_uuid(&row.get::<String, _>("id"))?,
                        object: OBJECT_TYPE_FILE.to_string(),
                        owner_type: owner_type_str
                            .parse()
                            .map_err(|e: String| DbError::Internal(e))?,
                        owner_id: parse_uuid(&row.get::<String, _>("owner_id"))?,
                        filename: row.get("filename"),
                        purpose: purpose_str
                            .parse()
                            .map_err(|e: String| DbError::Internal(e))?,
                        content_type: row.get("content_type"),
                        size_bytes: row.get("size_bytes"),
                        status: status_str
                            .parse()
                            .map_err(|e: String| DbError::Internal(e))?,
                        status_details: row.get("status_details"),
                        content_hash: row.get("content_hash"),
                        storage_backend: storage_backend_str
                            .parse()
                            .map_err(|e: String| DbError::Internal(e))?,
                        storage_path: row.get("storage_path"),
                        created_at: row.get("created_at"),
                        expires_at: row.get("expires_at"),
                    })
                })
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
        let (query, bind_purpose) = match &purpose {
            Some(p) => (
                format!(
                    r#"
                    SELECT id, owner_type, owner_id, filename, purpose, content_type, size_bytes, status,
                           status_details, content_hash, storage_backend, storage_path, created_at, expires_at
                    FROM files
                    WHERE owner_type = ? AND owner_id = ? AND purpose = ?
                    ORDER BY created_at {}, id {}
                    LIMIT ?
                    "#,
                    order, order
                ),
                Some(p.as_str().to_string()),
            ),
            None => (
                format!(
                    r#"
                    SELECT id, owner_type, owner_id, filename, purpose, content_type, size_bytes, status,
                           status_details, content_hash, storage_backend, storage_path, created_at, expires_at
                    FROM files
                    WHERE owner_type = ? AND owner_id = ?
                    ORDER BY created_at {}, id {}
                    LIMIT ?
                    "#,
                    order, order
                ),
                None,
            ),
        };

        let rows = if let Some(purpose_str) = bind_purpose {
            sqlx::query(&query)
                .bind(owner_type.as_str())
                .bind(owner_id.to_string())
                .bind(purpose_str)
                .bind(fetch_limit)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query(&query)
                .bind(owner_type.as_str())
                .bind(owner_id.to_string())
                .bind(fetch_limit)
                .fetch_all(&self.pool)
                .await?
        };

        let has_more = rows.len() as i64 > limit;
        let items: Vec<File> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let owner_type_str: String = row.get("owner_type");
                let purpose_str: String = row.get("purpose");
                let status_str: String = row.get("status");
                let storage_backend_str: String = row.get("storage_backend");

                Ok(File {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    object: OBJECT_TYPE_FILE.to_string(),
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: parse_uuid(&row.get::<String, _>("owner_id"))?,
                    filename: row.get("filename"),
                    purpose: purpose_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    content_type: row.get("content_type"),
                    size_bytes: row.get("size_bytes"),
                    status: status_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    status_details: row.get("status_details"),
                    content_hash: row.get("content_hash"),
                    storage_backend: storage_backend_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    storage_path: row.get("storage_path"),
                    created_at: row.get("created_at"),
                    expires_at: row.get("expires_at"),
                })
            })
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

    async fn delete_file(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            DELETE FROM files
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

    async fn update_file_status(
        &self,
        id: Uuid,
        status: FileStatus,
        status_details: Option<String>,
    ) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE files
            SET status = ?, status_details = ?
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(&status_details)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn count_file_references(&self, file_id: Uuid) -> DbResult<i64> {
        let result = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM vector_store_files
            WHERE file_id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(file_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(result.get("count"))
    }
}
