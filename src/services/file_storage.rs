//! Pluggable file storage backends for the Files API.
//!
//! This module provides a trait-based abstraction over file storage,
//! allowing files to be stored in different backends:
//!
//! - **Database**: Store file content in the database (default, simplest)
//! - **Filesystem**: Store files on the local filesystem
//! - **S3**: Store files in S3-compatible object storage
//!
//! The choice of backend is configured via `[storage.files]` in the config.

use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use thiserror::Error;
#[cfg(feature = "s3-storage")]
use tracing::error;
use tracing::{debug, info, instrument, warn};

#[cfg(feature = "s3-storage")]
use crate::config::S3StorageConfig;
use crate::{
    config::{FileStorageBackend, FileStorageConfig, FilesystemStorageConfig},
    db::DbPool,
};

/// Errors that can occur during file storage operations.
#[derive(Debug, Error)]
pub enum FileStorageError {
    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Storage I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("S3 error: {0}")]
    S3(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("File too large: {size} bytes exceeds maximum {max} bytes")]
    FileTooLarge { size: usize, max: usize },
}

pub type FileStorageResult<T> = Result<T, FileStorageError>;

/// Trait for pluggable file storage backends.
///
/// Implementations must be `Send + Sync` to support async contexts.
#[async_trait]
pub trait FileStorage: Send + Sync {
    /// Store file content and return the storage path/key.
    ///
    /// For database storage, returns None (content stored via CreateFile.file_data).
    /// For filesystem/S3, returns the path/key where the file was stored.
    async fn store(&self, file_id: &str, content: &[u8]) -> FileStorageResult<Option<String>>;

    /// Retrieve file content by ID or path.
    ///
    /// For database storage, this is called with the file_id.
    /// For filesystem/S3, this may be called with the storage_path.
    async fn retrieve(&self, file_id_or_path: &str) -> FileStorageResult<Vec<u8>>;

    /// Delete a file from storage.
    async fn delete(&self, file_id_or_path: &str) -> FileStorageResult<()>;

    /// Check if a file exists in storage.
    async fn exists(&self, file_id_or_path: &str) -> FileStorageResult<bool>;

    /// Get the backend type name (for logging/debugging).
    fn backend_name(&self) -> &'static str;
}

/// Database file storage backend.
///
/// Stores file content directly in the `files.file_data` column.
/// This is the simplest option but may not scale for large files.
pub struct DatabaseFileStorage {
    db: Arc<DbPool>,
}

impl DatabaseFileStorage {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl FileStorage for DatabaseFileStorage {
    #[instrument(skip(self, content), fields(size = content.len()))]
    async fn store(&self, file_id: &str, content: &[u8]) -> FileStorageResult<Option<String>> {
        debug!(file_id, size = content.len(), "Storing file in database");
        // For database storage, the content is stored via CreateFile.file_data
        // in the repository layer, not here. We return None to indicate no
        // separate storage path is needed.
        //
        // The actual storage happens when FilesService.upload() is called
        // with CreateFile { file_data: Some(content), ... }
        Ok(None)
    }

    #[instrument(skip(self))]
    async fn retrieve(&self, file_id: &str) -> FileStorageResult<Vec<u8>> {
        debug!(file_id, "Retrieving file from database");
        let id = file_id
            .parse()
            .map_err(|_| FileStorageError::NotFound(format!("Invalid file ID: {}", file_id)))?;

        self.db
            .files()
            .get_file_data(id)
            .await
            .map_err(|e| FileStorageError::Database(e.to_string()))?
            .ok_or_else(|| FileStorageError::NotFound(file_id.to_string()))
    }

    #[instrument(skip(self))]
    async fn delete(&self, file_id: &str) -> FileStorageResult<()> {
        debug!(file_id, "Deleting file from database");
        // For database storage, deletion is handled by the repository layer
        // via FilesService.delete(). This method is a no-op.
        Ok(())
    }

    #[instrument(skip(self))]
    async fn exists(&self, file_id: &str) -> FileStorageResult<bool> {
        let id = file_id
            .parse()
            .map_err(|_| FileStorageError::NotFound(format!("Invalid file ID: {}", file_id)))?;

        let file = self
            .db
            .files()
            .get_file(id)
            .await
            .map_err(|e| FileStorageError::Database(e.to_string()))?;

        Ok(file.is_some())
    }

    fn backend_name(&self) -> &'static str {
        "database"
    }
}

/// Filesystem file storage backend.
///
/// Stores file content on the local filesystem.
/// Files are stored as `{base_path}/{file_id}`.
pub struct FilesystemFileStorage {
    config: FilesystemStorageConfig,
}

impl FilesystemFileStorage {
    pub fn new(config: FilesystemStorageConfig) -> FileStorageResult<Self> {
        let storage = Self { config };

        // Ensure the storage directory exists if create_dir is enabled
        if storage.config.create_dir {
            let path = Path::new(&storage.config.path);
            if !path.exists() {
                info!(path = %storage.config.path, "Creating file storage directory");
                std::fs::create_dir_all(path)?;

                // Set directory permissions on Unix
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(
                        path,
                        std::fs::Permissions::from_mode(storage.config.dir_mode),
                    )?;
                }
            }
        }

        Ok(storage)
    }

    fn file_path(&self, file_id: &str) -> std::path::PathBuf {
        self.config.file_path(file_id)
    }
}

#[async_trait]
impl FileStorage for FilesystemFileStorage {
    #[instrument(skip(self, content), fields(size = content.len()))]
    async fn store(&self, file_id: &str, content: &[u8]) -> FileStorageResult<Option<String>> {
        let path = self.file_path(file_id);
        debug!(file_id, path = %path.display(), size = content.len(), "Storing file on filesystem");

        // Write to a temp file first, then rename for atomicity
        let temp_path = path.with_extension("tmp");

        tokio::fs::write(&temp_path, content).await?;

        // Set file permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(
                &temp_path,
                std::fs::Permissions::from_mode(self.config.file_mode),
            )
            .await?;
        }

        // Atomic rename
        tokio::fs::rename(&temp_path, &path).await?;

        info!(file_id, path = %path.display(), "File stored successfully");
        Ok(Some(path.to_string_lossy().to_string()))
    }

    #[instrument(skip(self))]
    async fn retrieve(&self, file_id_or_path: &str) -> FileStorageResult<Vec<u8>> {
        // If the input looks like a path (contains separator), use it directly
        // Otherwise, treat it as a file ID and construct the path
        let path = if file_id_or_path.contains(std::path::MAIN_SEPARATOR)
            || file_id_or_path.contains('/')
        {
            std::path::PathBuf::from(file_id_or_path)
        } else {
            self.file_path(file_id_or_path)
        };

        debug!(path = %path.display(), "Retrieving file from filesystem");

        match tokio::fs::read(&path).await {
            Ok(content) => Ok(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(FileStorageError::NotFound(
                path.to_string_lossy().to_string(),
            )),
            Err(e) => Err(FileStorageError::Io(e)),
        }
    }

    #[instrument(skip(self))]
    async fn delete(&self, file_id_or_path: &str) -> FileStorageResult<()> {
        let path = if file_id_or_path.contains(std::path::MAIN_SEPARATOR)
            || file_id_or_path.contains('/')
        {
            std::path::PathBuf::from(file_id_or_path)
        } else {
            self.file_path(file_id_or_path)
        };

        debug!(path = %path.display(), "Deleting file from filesystem");

        match tokio::fs::remove_file(&path).await {
            Ok(()) => {
                info!(path = %path.display(), "File deleted successfully");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                warn!(path = %path.display(), "File not found during deletion");
                Ok(()) // Idempotent - not an error if already gone
            }
            Err(e) => Err(FileStorageError::Io(e)),
        }
    }

    #[instrument(skip(self))]
    async fn exists(&self, file_id_or_path: &str) -> FileStorageResult<bool> {
        let path = if file_id_or_path.contains(std::path::MAIN_SEPARATOR)
            || file_id_or_path.contains('/')
        {
            std::path::PathBuf::from(file_id_or_path)
        } else {
            self.file_path(file_id_or_path)
        };

        Ok(tokio::fs::metadata(&path).await.is_ok())
    }

    fn backend_name(&self) -> &'static str {
        "filesystem"
    }
}

/// S3-compatible object storage backend.
///
/// Stores file content in an S3 bucket. Supports:
/// - AWS S3
/// - MinIO
/// - Cloudflare R2
/// - DigitalOcean Spaces
/// - Any S3-compatible service
///
/// Requires the `s3-storage` feature.
#[cfg(feature = "s3-storage")]
pub struct S3FileStorage {
    config: S3StorageConfig,
    client: aws_sdk_s3::Client,
}

#[cfg(feature = "s3-storage")]
impl S3FileStorage {
    pub async fn new(config: S3StorageConfig) -> FileStorageResult<Self> {
        info!(bucket = %config.bucket, "Initializing S3 file storage");

        let mut sdk_config_builder = aws_config::defaults(aws_config::BehaviorVersion::latest());

        // Set region if specified
        if let Some(region) = &config.region {
            sdk_config_builder = sdk_config_builder.region(aws_config::Region::new(region.clone()));
        }

        // Set credentials if specified in config
        if let (Some(access_key), Some(secret_key)) =
            (&config.access_key_id, &config.secret_access_key)
        {
            let credentials = aws_credential_types::Credentials::new(
                access_key.clone(),
                secret_key.clone(),
                None, // session token
                None, // expiry
                "hadrian-config",
            );
            sdk_config_builder = sdk_config_builder.credentials_provider(credentials);
        }

        let sdk_config = sdk_config_builder.load().await;

        // Build S3 client with custom endpoint if specified
        let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&sdk_config);

        if let Some(endpoint) = &config.endpoint {
            s3_config_builder = s3_config_builder.endpoint_url(endpoint);
        }

        if config.force_path_style {
            s3_config_builder = s3_config_builder.force_path_style(true);
        }

        let client = aws_sdk_s3::Client::from_conf(s3_config_builder.build());

        Ok(Self { config, client })
    }

    fn object_key(&self, file_id: &str) -> String {
        self.config.file_key(file_id)
    }
}

#[cfg(feature = "s3-storage")]
#[async_trait]
impl FileStorage for S3FileStorage {
    #[instrument(skip(self, content), fields(size = content.len(), bucket = %self.config.bucket))]
    async fn store(&self, file_id: &str, content: &[u8]) -> FileStorageResult<Option<String>> {
        let key = self.object_key(file_id);
        debug!(file_id, key, size = content.len(), "Storing file in S3");

        let mut request = self
            .client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .body(aws_sdk_s3::primitives::ByteStream::from(content.to_vec()));

        // Set storage class if specified
        if let Some(storage_class) = &self.config.storage_class {
            request = request.storage_class(storage_class.as_str().into());
        }

        // Set server-side encryption if configured
        if let Some(sse) = &self.config.server_side_encryption {
            match sse {
                crate::config::S3ServerSideEncryption::Aes256 => {
                    request = request
                        .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::Aes256);
                }
                crate::config::S3ServerSideEncryption::Kms { key_id } => {
                    request = request
                        .server_side_encryption(aws_sdk_s3::types::ServerSideEncryption::AwsKms)
                        .ssekms_key_id(key_id);
                }
            }
        }

        request.send().await.map_err(|e| {
            error!(error = %e, "Failed to upload to S3");
            FileStorageError::S3(e.to_string())
        })?;

        info!(file_id, key, bucket = %self.config.bucket, "File stored in S3");
        Ok(Some(key))
    }

    #[instrument(skip(self), fields(bucket = %self.config.bucket))]
    async fn retrieve(&self, file_id_or_key: &str) -> FileStorageResult<Vec<u8>> {
        // If it looks like an S3 key (contains the prefix or no slashes), use it directly
        // Otherwise, construct the key from the file ID
        let key = if file_id_or_key.contains('/') || self.config.key_prefix.is_none() {
            file_id_or_key.to_string()
        } else {
            self.object_key(file_id_or_key)
        };

        debug!(key, "Retrieving file from S3");

        let result = self
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                if e.to_string().contains("NoSuchKey") || e.to_string().contains("NotFound") {
                    FileStorageError::NotFound(key.clone())
                } else {
                    error!(error = %e, "Failed to download from S3");
                    FileStorageError::S3(e.to_string())
                }
            })?;

        let content = result
            .body
            .collect()
            .await
            .map_err(|e| FileStorageError::S3(format!("Failed to read S3 response body: {}", e)))?
            .to_vec();

        Ok(content)
    }

    #[instrument(skip(self), fields(bucket = %self.config.bucket))]
    async fn delete(&self, file_id_or_key: &str) -> FileStorageResult<()> {
        let key = if file_id_or_key.contains('/') || self.config.key_prefix.is_none() {
            file_id_or_key.to_string()
        } else {
            self.object_key(file_id_or_key)
        };

        debug!(key, "Deleting file from S3");

        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to delete from S3");
                FileStorageError::S3(e.to_string())
            })?;

        info!(key, bucket = %self.config.bucket, "File deleted from S3");
        Ok(())
    }

    #[instrument(skip(self), fields(bucket = %self.config.bucket))]
    async fn exists(&self, file_id_or_key: &str) -> FileStorageResult<bool> {
        let key = if file_id_or_key.contains('/') || self.config.key_prefix.is_none() {
            file_id_or_key.to_string()
        } else {
            self.object_key(file_id_or_key)
        };

        match self
            .client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.to_string().contains("NotFound") || e.to_string().contains("NoSuchKey") {
                    Ok(false)
                } else {
                    Err(FileStorageError::S3(e.to_string()))
                }
            }
        }
    }

    fn backend_name(&self) -> &'static str {
        "s3"
    }
}

/// Create a file storage backend from configuration.
pub async fn create_file_storage(
    config: &FileStorageConfig,
    db: Arc<DbPool>,
) -> FileStorageResult<Arc<dyn FileStorage>> {
    match config.backend {
        FileStorageBackend::Database => {
            info!("Using database file storage backend");
            Ok(Arc::new(DatabaseFileStorage::new(db)))
        }
        FileStorageBackend::Filesystem => {
            let fs_config = config.filesystem.clone().ok_or_else(|| {
                FileStorageError::Config(
                    "Filesystem backend requires [storage.files.filesystem] config".to_string(),
                )
            })?;
            info!(path = %fs_config.path, "Using filesystem file storage backend");
            Ok(Arc::new(FilesystemFileStorage::new(fs_config)?))
        }
        #[cfg(feature = "s3-storage")]
        FileStorageBackend::S3 => {
            let s3_config = config.s3.clone().ok_or_else(|| {
                FileStorageError::Config(
                    "S3 backend requires [storage.files.s3] config".to_string(),
                )
            })?;
            info!(bucket = %s3_config.bucket, "Using S3 file storage backend");
            Ok(Arc::new(S3FileStorage::new(s3_config).await?))
        }
        #[cfg(not(feature = "s3-storage"))]
        FileStorageBackend::S3 => Err(FileStorageError::Config(
            "S3 file storage backend requires the 's3-storage' feature. \
                Rebuild with: cargo build --features s3-storage"
                .to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_filesystem_storage_file_path() {
        let config = FilesystemStorageConfig {
            path: "/var/hadrian/files".to_string(),
            create_dir: false,
            file_mode: 0o600,
            dir_mode: 0o700,
        };
        let storage = FilesystemFileStorage { config };

        assert_eq!(
            storage.file_path("abc-123"),
            std::path::PathBuf::from("/var/hadrian/files/abc-123")
        );
    }

    #[tokio::test]
    async fn test_filesystem_storage_store_and_retrieve() {
        let temp_dir = TempDir::new().unwrap();
        let config = FilesystemStorageConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            create_dir: true,
            file_mode: 0o600,
            dir_mode: 0o700,
        };

        let storage = FilesystemFileStorage::new(config).unwrap();

        // Store a file
        let content = b"Hello, world!";
        let path = storage.store("test-file-id", content).await.unwrap();
        assert!(path.is_some());

        // Retrieve it
        let retrieved = storage.retrieve("test-file-id").await.unwrap();
        assert_eq!(retrieved, content);

        // Check exists
        assert!(storage.exists("test-file-id").await.unwrap());
        assert!(!storage.exists("nonexistent").await.unwrap());

        // Delete it
        storage.delete("test-file-id").await.unwrap();
        assert!(!storage.exists("test-file-id").await.unwrap());

        // Delete again should be idempotent
        storage.delete("test-file-id").await.unwrap();
    }

    #[tokio::test]
    async fn test_filesystem_storage_retrieve_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config = FilesystemStorageConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            create_dir: true,
            file_mode: 0o600,
            dir_mode: 0o700,
        };

        let storage = FilesystemFileStorage::new(config).unwrap();

        let result = storage.retrieve("nonexistent").await;
        assert!(matches!(result, Err(FileStorageError::NotFound(_))));
    }

    #[cfg(feature = "s3-storage")]
    #[test]
    fn test_s3_object_key_generation() {
        let config = S3StorageConfig {
            bucket: "test-bucket".to_string(),
            region: Some("us-east-1".to_string()),
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            force_path_style: false,
            key_prefix: Some("hadrian/files/".to_string()),
            storage_class: None,
            server_side_encryption: None,
        };

        // We can't instantiate S3FileStorage without actual AWS credentials,
        // but we can test the key generation via the config
        assert_eq!(config.file_key("abc-123"), "hadrian/files/abc-123");
    }

    #[test]
    fn test_database_storage_backend_name() {
        // DatabaseFileStorage requires a DbPool which we don't have in unit tests,
        // but we can verify the trait is implemented correctly
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DatabaseFileStorage>();
        assert_send_sync::<FilesystemFileStorage>();
        #[cfg(feature = "s3-storage")]
        assert_send_sync::<S3FileStorage>();
    }
}
