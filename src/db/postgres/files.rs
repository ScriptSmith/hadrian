use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{Cursor, CursorDirection, FilesRepo, ListParams, ListResult, PageCursors},
    },
    models::{CreateFile, File, FilePurpose, FileStatus, OBJECT_TYPE_FILE, VectorStoreOwnerType},
};

pub struct PostgresFilesRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresFilesRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        Self {
            read_pool: read_pool.clone().unwrap_or_else(|| write_pool.clone()),
            write_pool,
        }
    }

    fn cursor_from_file(file: &File) -> Cursor {
        Cursor::new(file.created_at, file.id)
    }
}

#[async_trait]
impl FilesRepo for PostgresFilesRepo {
    async fn create_file(&self, input: CreateFile) -> DbResult<File> {
        let id = Uuid::new_v4();

        let row = sqlx::query(
            r#"
            INSERT INTO files (id, owner_type, owner_id, filename, purpose, content_type, size_bytes, content_hash, storage_backend, file_data, storage_path)
            VALUES ($1, $2::vector_store_owner_type, $3, $4, $5::file_purpose, $6, $7, $8, $9::file_storage_backend, $10, $11)
            RETURNING id, owner_type::TEXT, owner_id, filename, purpose::TEXT, content_type, size_bytes, status::TEXT,
                      status_details, content_hash, storage_backend::TEXT, storage_path, created_at, expires_at
            "#,
        )
        .bind(id)
        .bind(input.owner_type.as_str())
        .bind(input.owner_id)
        .bind(&input.filename)
        .bind(input.purpose.as_str())
        .bind(&input.content_type)
        .bind(input.size_bytes)
        .bind(&input.content_hash)
        .bind(input.storage_backend.as_str())
        .bind(&input.file_data)
        .bind(&input.storage_path)
        .fetch_one(&self.write_pool)
        .await?;

        let owner_type_str: String = row.get("owner_type");
        let purpose_str: String = row.get("purpose");
        let status_str: String = row.get("status");
        let storage_backend_str: String = row.get("storage_backend");

        Ok(File {
            id: row.get("id"),
            object: OBJECT_TYPE_FILE.to_string(),
            owner_type: owner_type_str
                .parse()
                .map_err(|e: String| DbError::Internal(e))?,
            owner_id: row.get("owner_id"),
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
    }

    async fn get_file(&self, id: Uuid) -> DbResult<Option<File>> {
        let result = sqlx::query(
            r#"
            SELECT id, owner_type::TEXT, owner_id, filename, purpose::TEXT, content_type, size_bytes, status::TEXT,
                   status_details, content_hash, storage_backend::TEXT, storage_path, created_at, expires_at
            FROM files
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        match result {
            Some(row) => {
                let owner_type_str: String = row.get("owner_type");
                let purpose_str: String = row.get("purpose");
                let status_str: String = row.get("status");
                let storage_backend_str: String = row.get("storage_backend");

                Ok(Some(File {
                    id: row.get("id"),
                    object: OBJECT_TYPE_FILE.to_string(),
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: row.get("owner_id"),
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
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
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

            let rows = match &purpose {
                Some(p) => {
                    let query = format!(
                        r#"
                        SELECT id, owner_type::TEXT, owner_id, filename, purpose::TEXT, content_type, size_bytes, status::TEXT,
                               status_details, content_hash, storage_backend::TEXT, storage_path, created_at, expires_at
                        FROM files
                        WHERE owner_type = $1::vector_store_owner_type AND owner_id = $2 AND purpose = $3::file_purpose
                        AND ROW(created_at, id) {} ROW($4, $5)
                        ORDER BY created_at {}, id {}
                        LIMIT $6
                        "#,
                        comparison, order, order
                    );
                    sqlx::query(&query)
                        .bind(owner_type.as_str())
                        .bind(owner_id)
                        .bind(p.as_str())
                        .bind(cursor.created_at)
                        .bind(cursor.id)
                        .bind(fetch_limit)
                        .fetch_all(&self.read_pool)
                        .await?
                }
                None => {
                    let query = format!(
                        r#"
                        SELECT id, owner_type::TEXT, owner_id, filename, purpose::TEXT, content_type, size_bytes, status::TEXT,
                               status_details, content_hash, storage_backend::TEXT, storage_path, created_at, expires_at
                        FROM files
                        WHERE owner_type = $1::vector_store_owner_type AND owner_id = $2
                        AND ROW(created_at, id) {} ROW($3, $4)
                        ORDER BY created_at {}, id {}
                        LIMIT $5
                        "#,
                        comparison, order, order
                    );
                    sqlx::query(&query)
                        .bind(owner_type.as_str())
                        .bind(owner_id)
                        .bind(cursor.created_at)
                        .bind(cursor.id)
                        .bind(fetch_limit)
                        .fetch_all(&self.read_pool)
                        .await?
                }
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
                        id: row.get("id"),
                        object: OBJECT_TYPE_FILE.to_string(),
                        owner_type: owner_type_str
                            .parse()
                            .map_err(|e: String| DbError::Internal(e))?,
                        owner_id: row.get("owner_id"),
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
        let rows = match &purpose {
            Some(p) => {
                let query = format!(
                    r#"
                    SELECT id, owner_type::TEXT, owner_id, filename, purpose::TEXT, content_type, size_bytes, status::TEXT,
                           status_details, content_hash, storage_backend::TEXT, storage_path, created_at, expires_at
                    FROM files
                    WHERE owner_type = $1::vector_store_owner_type AND owner_id = $2 AND purpose = $3::file_purpose
                    ORDER BY created_at {}, id {}
                    LIMIT $4
                    "#,
                    order, order
                );
                sqlx::query(&query)
                    .bind(owner_type.as_str())
                    .bind(owner_id)
                    .bind(p.as_str())
                    .bind(fetch_limit)
                    .fetch_all(&self.read_pool)
                    .await?
            }
            None => {
                let query = format!(
                    r#"
                    SELECT id, owner_type::TEXT, owner_id, filename, purpose::TEXT, content_type, size_bytes, status::TEXT,
                           status_details, content_hash, storage_backend::TEXT, storage_path, created_at, expires_at
                    FROM files
                    WHERE owner_type = $1::vector_store_owner_type AND owner_id = $2
                    ORDER BY created_at {}, id {}
                    LIMIT $3
                    "#,
                    order, order
                );
                sqlx::query(&query)
                    .bind(owner_type.as_str())
                    .bind(owner_id)
                    .bind(fetch_limit)
                    .fetch_all(&self.read_pool)
                    .await?
            }
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
                    id: row.get("id"),
                    object: OBJECT_TYPE_FILE.to_string(),
                    owner_type: owner_type_str
                        .parse()
                        .map_err(|e: String| DbError::Internal(e))?,
                    owner_id: row.get("owner_id"),
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

    async fn update_file_status(
        &self,
        id: Uuid,
        status: FileStatus,
        status_details: Option<String>,
    ) -> DbResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE files
            SET status = $1::file_status, status_details = $2
            WHERE id = $3
            "#,
        )
        .bind(status.as_str())
        .bind(&status_details)
        .bind(id)
        .execute(&self.write_pool)
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
            WHERE file_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(file_id)
        .fetch_one(&self.read_pool)
        .await?;

        Ok(result.get("count"))
    }
}
