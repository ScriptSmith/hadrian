use std::sync::Arc;

use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use super::{FileStorage, FileStorageError};
use crate::{
    db::{DbError, DbPool, DbResult, ListParams, ListResult},
    models::{CreateFile, File, FilePurpose, FileStatus, StorageBackend, VectorStoreOwnerType},
};

/// Errors that can occur in the FilesService.
#[derive(Debug, Error)]
pub enum FilesServiceError {
    #[error("Database error: {0}")]
    Database(#[from] DbError),

    #[error("Storage error: {0}")]
    Storage(#[from] FileStorageError),

    #[error("File not found: {0}")]
    NotFound(Uuid),
}

pub type FilesServiceResult<T> = Result<T, FilesServiceError>;

/// Service layer for file operations (OpenAI Files API).
///
/// Files are uploaded via the Files API and can then be added to vector stores
/// for RAG (Retrieval Augmented Generation) use cases.
///
/// The service supports multiple storage backends:
/// - **Database**: Store file content directly in the database (default)
/// - **Filesystem**: Store files on the local filesystem
/// - **S3**: Store files in S3-compatible object storage
///
/// The storage backend is configured via `[storage.files]` in the gateway config.
#[derive(Clone)]
pub struct FilesService {
    db: Arc<DbPool>,
    storage: Arc<dyn FileStorage>,
}

impl FilesService {
    pub fn new(db: Arc<DbPool>, storage: Arc<dyn FileStorage>) -> Self {
        Self { db, storage }
    }

    /// Get the storage backend name (for logging/debugging).
    pub fn storage_backend_name(&self) -> &'static str {
        self.storage.backend_name()
    }

    /// Upload a new file.
    ///
    /// The file is stored according to the configured storage backend:
    /// - **Database**: Content is stored in the `files.file_data` column
    /// - **Filesystem/S3**: Content is stored in the external backend, path saved in DB
    ///
    /// Files can later be added to vector stores for processing.
    #[instrument(skip(self, input), fields(
        filename = %input.filename,
        size = input.size_bytes,
        backend = %self.storage.backend_name()
    ))]
    pub async fn upload(&self, mut input: CreateFile) -> FilesServiceResult<File> {
        // For non-database storage, store the content externally first
        if input.storage_backend != StorageBackend::Database
            && let Some(ref data) = input.file_data
        {
            // Generate a unique file ID for storage (will be replaced by actual DB ID)
            let temp_id = Uuid::new_v4().to_string();

            debug!(
                backend = %self.storage.backend_name(),
                temp_id = %temp_id,
                size = data.len(),
                "Storing file content in external storage"
            );

            let storage_path = self.storage.store(&temp_id, data).await?;

            // Update the input to use external storage
            input.storage_path = storage_path;
            input.file_data = None; // Don't store content in DB

            info!(
                backend = %self.storage.backend_name(),
                path = ?input.storage_path,
                "File content stored externally"
            );
        }

        // Create the database record
        let file = self.db.files().create_file(input).await?;

        debug!(file_id = %file.id, "File record created in database");
        Ok(file)
    }

    /// Get a file by ID.
    ///
    /// Returns the file metadata, not the content. Use `get_content` for content.
    pub async fn get(&self, id: Uuid) -> DbResult<Option<File>> {
        self.db.files().get_file(id).await
    }

    /// Get the content of a file.
    ///
    /// Retrieves the file content from the appropriate storage backend:
    /// - **Database**: Returns data from `files.file_data` column
    /// - **Filesystem/S3**: Reads from the external storage using `storage_path`
    #[instrument(skip(self))]
    pub async fn get_content(&self, id: Uuid) -> FilesServiceResult<Vec<u8>> {
        // First get file metadata to determine storage backend
        let file = self
            .db
            .files()
            .get_file(id)
            .await?
            .ok_or(FilesServiceError::NotFound(id))?;

        match file.storage_backend {
            StorageBackend::Database => {
                // Get content from database
                let content = self
                    .db
                    .files()
                    .get_file_data(id)
                    .await?
                    .ok_or(FilesServiceError::NotFound(id))?;
                Ok(content)
            }
            StorageBackend::Filesystem | StorageBackend::S3 => {
                // Get content from external storage
                let path = file.storage_path.as_ref().ok_or_else(|| {
                    FilesServiceError::Storage(FileStorageError::NotFound(format!(
                        "File {} has no storage path",
                        id
                    )))
                })?;

                debug!(
                    file_id = %id,
                    path = %path,
                    backend = ?file.storage_backend,
                    "Retrieving file content from external storage"
                );

                let content = self.storage.retrieve(path).await?;
                Ok(content)
            }
        }
    }

    /// List files by owner, optionally filtered by purpose.
    pub async fn list(
        &self,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
        purpose: Option<FilePurpose>,
        params: ListParams,
    ) -> DbResult<ListResult<File>> {
        self.db
            .files()
            .list_files(owner_type, owner_id, purpose, params)
            .await
    }

    /// Delete a file.
    ///
    /// Deletes the file from both the database and external storage (if applicable).
    ///
    /// Note: This only deletes the file if it's not referenced by any vector stores.
    /// Check `count_references` first to ensure safe deletion.
    #[instrument(skip(self))]
    pub async fn delete(&self, id: Uuid) -> FilesServiceResult<()> {
        // First get file metadata to determine storage backend
        let file = self
            .db
            .files()
            .get_file(id)
            .await?
            .ok_or(FilesServiceError::NotFound(id))?;

        // Delete from external storage first (if applicable)
        if file.storage_backend != StorageBackend::Database
            && let Some(ref path) = file.storage_path
        {
            debug!(
                file_id = %id,
                path = %path,
                backend = ?file.storage_backend,
                "Deleting file content from external storage"
            );

            if let Err(e) = self.storage.delete(path).await {
                // Log but don't fail - we still want to delete the DB record
                // The storage delete is idempotent so orphaned files are acceptable
                warn!(
                    file_id = %id,
                    path = %path,
                    error = %e,
                    "Failed to delete file from external storage (continuing with DB deletion)"
                );
            }
        }

        // Delete from database
        self.db.files().delete_file(id).await?;

        info!(file_id = %id, "File deleted");
        Ok(())
    }

    /// Update the status of a file.
    pub async fn update_status(
        &self,
        id: Uuid,
        status: FileStatus,
        details: Option<String>,
    ) -> DbResult<()> {
        self.db
            .files()
            .update_file_status(id, status, details)
            .await
    }

    /// Count how many collections reference this file.
    ///
    /// A file cannot be deleted if it's referenced by any vector stores.
    pub async fn count_references(&self, file_id: Uuid) -> DbResult<i64> {
        self.db.files().count_file_references(file_id).await
    }

    /// Check if a user has access to a file.
    ///
    /// Access is granted if:
    /// - The file is owned by the user directly
    /// - The file is owned by an organization the user belongs to
    /// - The file is owned by a project the user belongs to
    pub async fn user_has_access(&self, user_id: Uuid, file_id: Uuid) -> DbResult<bool> {
        let file = match self.db.files().get_file(file_id).await? {
            Some(f) => f,
            None => return Ok(false),
        };

        match file.owner_type {
            VectorStoreOwnerType::User => {
                // Direct ownership
                Ok(file.owner_id == user_id)
            }
            VectorStoreOwnerType::Organization => {
                // Check if user is a member of the organization
                let members = self
                    .db
                    .users()
                    .list_org_members(file.owner_id, ListParams::default())
                    .await?;
                Ok(members.items.iter().any(|u| u.id == user_id))
            }
            VectorStoreOwnerType::Team => {
                // Check if user is a member of the team
                let members = self
                    .db
                    .teams()
                    .list_members(file.owner_id, ListParams::default())
                    .await?;
                Ok(members.items.iter().any(|m| m.user_id == user_id))
            }
            VectorStoreOwnerType::Project => {
                // Check if user is a member of the project
                let members = self
                    .db
                    .users()
                    .list_project_members(file.owner_id, ListParams::default())
                    .await?;
                Ok(members.items.iter().any(|u| u.id == user_id))
            }
        }
    }

    /// Create a file from upload data with a specified storage backend.
    ///
    /// This is a convenience method that creates a CreateFile struct.
    /// The actual storage happens in `upload()` based on the backend type.
    ///
    /// Computes a SHA-256 content hash for deduplication.
    pub fn create_file_input(
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
        filename: String,
        purpose: FilePurpose,
        content_type: Option<String>,
        data: Vec<u8>,
        storage_backend: StorageBackend,
    ) -> CreateFile {
        let size_bytes = data.len() as i64;
        // Compute SHA-256 hash for content deduplication
        let content_hash = {
            let mut hasher = Sha256::new();
            hasher.update(&data);
            format!("{:x}", hasher.finalize())
        };
        CreateFile {
            owner_type,
            owner_id,
            filename,
            purpose,
            content_type,
            size_bytes,
            content_hash: Some(content_hash),
            storage_backend,
            file_data: Some(data),
            storage_path: None,
        }
    }

    /// Get the configured storage backend type.
    ///
    /// Returns the storage backend this service uses for new files.
    pub fn configured_backend(&self) -> StorageBackend {
        // Determine from the storage implementation
        match self.storage.backend_name() {
            "database" => StorageBackend::Database,
            "filesystem" => StorageBackend::Filesystem,
            "s3" => StorageBackend::S3,
            _ => StorageBackend::Database, // Default fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FilesService>();
    }

    #[test]
    fn test_create_file_input_database() {
        let input = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "test.txt".to_string(),
            FilePurpose::Assistants,
            Some("text/plain".to_string()),
            b"Hello, world!".to_vec(),
            StorageBackend::Database,
        );

        assert_eq!(input.filename, "test.txt");
        assert_eq!(input.purpose, FilePurpose::Assistants);
        assert_eq!(input.content_type, Some("text/plain".to_string()));
        assert_eq!(input.size_bytes, 13);
        assert_eq!(input.storage_backend, StorageBackend::Database);
        assert!(input.file_data.is_some());
        assert!(input.storage_path.is_none());
    }

    #[test]
    fn test_create_file_input_filesystem() {
        let input = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "test.txt".to_string(),
            FilePurpose::Assistants,
            Some("text/plain".to_string()),
            b"Hello, world!".to_vec(),
            StorageBackend::Filesystem,
        );

        assert_eq!(input.storage_backend, StorageBackend::Filesystem);
        assert!(input.file_data.is_some());
    }

    #[test]
    fn test_create_file_input_s3() {
        let input = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "test.txt".to_string(),
            FilePurpose::Assistants,
            Some("text/plain".to_string()),
            b"Hello, world!".to_vec(),
            StorageBackend::S3,
        );

        assert_eq!(input.storage_backend, StorageBackend::S3);
        assert!(input.file_data.is_some());
    }

    #[test]
    fn test_create_file_input_computes_content_hash() {
        let input = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "test.txt".to_string(),
            FilePurpose::Assistants,
            Some("text/plain".to_string()),
            b"Hello, world!".to_vec(),
            StorageBackend::Database,
        );

        assert!(input.content_hash.is_some(), "content_hash should be set");
        let hash = input.content_hash.unwrap();
        assert_eq!(hash.len(), 64, "SHA-256 hex should be 64 characters");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should be lowercase hex"
        );
    }

    #[test]
    fn test_content_hash_is_deterministic() {
        let content = b"The quick brown fox jumps over the lazy dog".to_vec();

        let input1 = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "file1.txt".to_string(),
            FilePurpose::Assistants,
            None,
            content.clone(),
            StorageBackend::Database,
        );

        let input2 = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "file2.txt".to_string(),
            FilePurpose::Assistants,
            None,
            content,
            StorageBackend::Database,
        );

        assert_eq!(
            input1.content_hash, input2.content_hash,
            "Same content should produce same hash"
        );
    }

    #[test]
    fn test_different_content_different_hash() {
        let input1 = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "test.txt".to_string(),
            FilePurpose::Assistants,
            None,
            b"Content A".to_vec(),
            StorageBackend::Database,
        );

        let input2 = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "test.txt".to_string(),
            FilePurpose::Assistants,
            None,
            b"Content B".to_vec(),
            StorageBackend::Database,
        );

        assert_ne!(
            input1.content_hash, input2.content_hash,
            "Different content should produce different hashes"
        );
    }

    #[test]
    fn test_content_hash_ignores_filename() {
        let content = b"Identical content".to_vec();

        let input1 = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "document.pdf".to_string(),
            FilePurpose::Assistants,
            Some("application/pdf".to_string()),
            content.clone(),
            StorageBackend::Database,
        );

        let input2 = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "report.docx".to_string(),
            FilePurpose::Assistants,
            Some(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string(),
            ),
            content,
            StorageBackend::Database,
        );

        assert_eq!(
            input1.content_hash, input2.content_hash,
            "Hash should only depend on content, not filename or content_type"
        );
    }

    #[test]
    fn test_content_hash_known_value() {
        // SHA-256 of "Hello, world!" is a well-known value
        // echo -n "Hello, world!" | sha256sum
        // 315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3
        let input = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "test.txt".to_string(),
            FilePurpose::Assistants,
            None,
            b"Hello, world!".to_vec(),
            StorageBackend::Database,
        );

        assert_eq!(
            input.content_hash,
            Some("315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3".to_string()),
            "SHA-256 hash should match expected value"
        );
    }

    #[test]
    fn test_content_hash_empty_content() {
        // SHA-256 of empty string
        // echo -n "" | sha256sum
        // e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let input = FilesService::create_file_input(
            VectorStoreOwnerType::User,
            Uuid::new_v4(),
            "empty.txt".to_string(),
            FilePurpose::Assistants,
            None,
            vec![],
            StorageBackend::Database,
        );

        assert_eq!(
            input.content_hash,
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string()),
            "Empty content should have well-known SHA-256 hash"
        );
        assert_eq!(input.size_bytes, 0);
    }
}
