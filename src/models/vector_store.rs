use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Maximum number of key-value pairs allowed in metadata (OpenAI limit)
const METADATA_MAX_KEYS: usize = 16;
/// Maximum length of metadata keys (OpenAI limit)
const METADATA_KEY_MAX_LENGTH: usize = 64;
/// Maximum length of metadata string values (OpenAI limit)
const METADATA_VALUE_MAX_LENGTH: usize = 512;

/// Object type identifier for File resources (OpenAI API compatibility)
pub const OBJECT_TYPE_FILE: &str = "file";
/// Object type identifier for VectorStore resources (OpenAI API compatibility)
pub const OBJECT_TYPE_VECTOR_STORE: &str = "vector_store";
/// Object type identifier for VectorStoreFile resources (OpenAI API compatibility)
pub const OBJECT_TYPE_VECTOR_STORE_FILE: &str = "vector_store.file";

/// Validates metadata according to OpenAI API limits:
/// - Maximum 16 key-value pairs
/// - Keys max 64 characters
/// - String values max 512 characters
fn validate_metadata(metadata: &HashMap<String, serde_json::Value>) -> Result<(), ValidationError> {
    if metadata.len() > METADATA_MAX_KEYS {
        let mut err = ValidationError::new("metadata_too_many_keys");
        err.message = Some(
            format!(
                "Metadata cannot have more than {} key-value pairs, got {}",
                METADATA_MAX_KEYS,
                metadata.len()
            )
            .into(),
        );
        return Err(err);
    }

    for (key, value) in metadata {
        if key.len() > METADATA_KEY_MAX_LENGTH {
            let mut err = ValidationError::new("metadata_key_too_long");
            err.message = Some(
                format!(
                    "Metadata key '{}' exceeds maximum length of {} characters",
                    key, METADATA_KEY_MAX_LENGTH
                )
                .into(),
            );
            return Err(err);
        }

        if let Some(s) = value.as_str()
            && s.len() > METADATA_VALUE_MAX_LENGTH
        {
            let mut err = ValidationError::new("metadata_value_too_long");
            err.message = Some(
                format!(
                    "Metadata value for key '{}' exceeds maximum length of {} characters",
                    key, METADATA_VALUE_MAX_LENGTH
                )
                .into(),
            );
            return Err(err);
        }
    }

    Ok(())
}

use super::prefixed_id::{chunk_id_serde, file_id_serde, vector_store_id_serde};

/// Owner type for collections (organization, team, project, or user)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreOwnerType {
    Organization,
    Team,
    Project,
    User,
}

impl VectorStoreOwnerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            VectorStoreOwnerType::Organization => "organization",
            VectorStoreOwnerType::Team => "team",
            VectorStoreOwnerType::Project => "project",
            VectorStoreOwnerType::User => "user",
        }
    }
}

impl std::str::FromStr for VectorStoreOwnerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "organization" => Ok(VectorStoreOwnerType::Organization),
            "team" => Ok(VectorStoreOwnerType::Team),
            "project" => Ok(VectorStoreOwnerType::Project),
            "user" => Ok(VectorStoreOwnerType::User),
            _ => Err(format!("Invalid vector store owner type: {}", s)),
        }
    }
}

/// VectorStore (vector store) status (OpenAI VectorStore compatible)
///
/// Indicates whether the vector store is ready for use.
///
/// ## State Transitions
///
/// ```text
/// ┌─────────────┐     files processed     ┌─────────────┐
/// │ in_progress │ ───────────────────────▶│  completed  │
/// └─────────────┘                         └─────────────┘
///                                                │
///                                                │ expiration policy
///                                                │ triggered
///                                                ▼
///                                         ┌─────────────┐
///                                         │   expired   │
///                                         └─────────────┘
/// ```
///
/// - `in_progress`: At least one file is being processed (chunked/embedded)
/// - `completed`: All files processed successfully, vector store ready for search
/// - `expired`: Vector store expired per `expires_after` policy (based on `last_active_at`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreStatus {
    /// At least one file is being processed
    InProgress,
    /// All files processed, ready for search
    #[default]
    Completed,
    /// Expired per expiration policy
    Expired,
}

impl VectorStoreStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            VectorStoreStatus::InProgress => "in_progress",
            VectorStoreStatus::Completed => "completed",
            VectorStoreStatus::Expired => "expired",
        }
    }
}

impl std::str::FromStr for VectorStoreStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "in_progress" => Ok(VectorStoreStatus::InProgress),
            "completed" => Ok(VectorStoreStatus::Completed),
            "expired" => Ok(VectorStoreStatus::Expired),
            _ => Err(format!("Invalid vector store status: {}", s)),
        }
    }
}

/// File processing status within a vector store (OpenAI VectorStoreFile compatible)
///
/// Tracks the processing state when a file is added to a vector store.
/// This is separate from `FileStatus` which tracks the file upload itself.
///
/// ## State Transitions
///
/// ```text
///                              ┌─────────────┐
///              success         │  completed  │
///           ┌─────────────────▶└─────────────┘
///           │
/// ┌─────────────┐
/// │ in_progress │──────────────▶┌─────────────┐
/// └─────────────┘   error       │   failed    │
///           │                   └─────────────┘
///           │
///           │   user request   ┌─────────────┐
///           └─────────────────▶│  cancelled  │
///                              └─────────────┘
/// ```
///
/// - `in_progress`: File is being chunked and embeddings are being generated
/// - `completed`: File successfully processed, chunks stored in vector store
/// - `failed`: Processing failed (see `last_error` for details)
/// - `cancelled`: Processing was cancelled by user request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreFileStatus {
    /// File is being chunked and embedded
    #[default]
    InProgress,
    /// Processing complete, chunks stored
    Completed,
    /// Processing cancelled by user
    Cancelled,
    /// Processing failed (see last_error)
    Failed,
}

impl VectorStoreFileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            VectorStoreFileStatus::InProgress => "in_progress",
            VectorStoreFileStatus::Completed => "completed",
            VectorStoreFileStatus::Cancelled => "cancelled",
            VectorStoreFileStatus::Failed => "failed",
        }
    }
}

impl std::str::FromStr for VectorStoreFileStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "in_progress" => Ok(VectorStoreFileStatus::InProgress),
            "completed" => Ok(VectorStoreFileStatus::Completed),
            "cancelled" => Ok(VectorStoreFileStatus::Cancelled),
            "failed" => Ok(VectorStoreFileStatus::Failed),
            _ => Err(format!("Invalid vector store file status: {}", s)),
        }
    }
}

/// Storage backend for vector store files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum StorageBackend {
    /// File content stored directly in the database
    #[default]
    Database,
    /// File content stored on the local filesystem
    Filesystem,
    /// File content stored in S3-compatible object storage
    S3,
}

impl StorageBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageBackend::Database => "database",
            StorageBackend::Filesystem => "filesystem",
            StorageBackend::S3 => "s3",
        }
    }
}

impl std::str::FromStr for StorageBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "database" => Ok(StorageBackend::Database),
            "filesystem" => Ok(StorageBackend::Filesystem),
            "s3" => Ok(StorageBackend::S3),
            _ => Err(format!("Invalid storage backend: {}", s)),
        }
    }
}

/// File purpose (OpenAI Files API compatible)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum FilePurpose {
    /// For use with Assistants and Responses API file_search tool
    #[default]
    Assistants,
    /// For Batch API
    Batch,
    /// For fine-tuning
    FineTune,
    /// For vision models
    Vision,
}

impl FilePurpose {
    pub fn as_str(&self) -> &'static str {
        match self {
            FilePurpose::Assistants => "assistants",
            FilePurpose::Batch => "batch",
            FilePurpose::FineTune => "fine-tune",
            FilePurpose::Vision => "vision",
        }
    }
}

impl std::str::FromStr for FilePurpose {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "assistants" => Ok(FilePurpose::Assistants),
            "batch" => Ok(FilePurpose::Batch),
            "fine-tune" => Ok(FilePurpose::FineTune),
            "vision" => Ok(FilePurpose::Vision),
            _ => Err(format!("Invalid file purpose: {}", s)),
        }
    }
}

impl FilePurpose {
    /// Validate that a file extension is allowed for this purpose.
    ///
    /// Returns `Ok(())` if the extension is valid, or `Err` with a message explaining
    /// what file types are allowed for this purpose.
    pub fn validate_file_extension(&self, filename: &str) -> Result<(), String> {
        let extension = std::path::Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match self {
            FilePurpose::FineTune | FilePurpose::Batch => {
                // Fine-tune and batch require JSONL files
                if extension.as_deref() == Some("jsonl") {
                    Ok(())
                } else {
                    let ext_display = extension
                        .map(|e| format!(".{}", e))
                        .unwrap_or_else(|| "(no extension)".to_string());
                    Err(format!(
                        "File type '{}' is not allowed for {} purpose. Only .jsonl files are accepted.",
                        ext_display,
                        self.as_str()
                    ))
                }
            }
            FilePurpose::Vision => {
                // Vision requires image files
                let allowed = ["png", "jpg", "jpeg", "gif", "webp"];
                if extension
                    .as_ref()
                    .is_some_and(|e| allowed.contains(&e.as_str()))
                {
                    Ok(())
                } else {
                    let ext_display = extension
                        .map(|e| format!(".{}", e))
                        .unwrap_or_else(|| "(no extension)".to_string());
                    Err(format!(
                        "File type '{}' is not allowed for vision purpose. Allowed types: .png, .jpg, .jpeg, .gif, .webp",
                        ext_display
                    ))
                }
            }
            FilePurpose::Assistants => {
                // Assistants allows documents, code, and images but not executables
                let blocked = [
                    "exe", "dll", "so", "dylib", "bat", "cmd", "sh", "ps1", "msi", "com", "scr",
                    "pif", "vbs", "vbe", "js", "jse", "ws", "wsf", "wsc", "wsh", "msc",
                ];
                if extension
                    .as_ref()
                    .is_some_and(|e| blocked.contains(&e.as_str()))
                {
                    let ext_display = extension
                        .map(|e| format!(".{}", e))
                        .unwrap_or_else(|| "(no extension)".to_string());
                    Err(format!(
                        "File type '{}' is not allowed for assistants purpose. Executable files are not permitted.",
                        ext_display
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Validate file content by checking magic bytes against declared extension.
    ///
    /// This prevents attacks where an executable is renamed to a benign extension
    /// (e.g., renaming a `.exe` to `.jpg`). Checks are performed for:
    /// - **Assistants**: Reject files with executable magic bytes regardless of extension
    /// - **Vision**: Verify the file contains valid image magic bytes
    /// - **Batch/FineTune**: Verify the file starts with valid UTF-8 (JSONL content)
    pub fn validate_file_content(&self, data: &[u8]) -> Result<(), String> {
        // Check for executable magic bytes regardless of purpose
        if is_executable_magic(data) {
            return Err(
                "File content appears to be an executable binary. Executable files are not permitted.".to_string(),
            );
        }

        match self {
            FilePurpose::Vision => {
                // Vision files must have image magic bytes
                if !is_image_magic(data) {
                    return Err(
                        "File content does not match a recognized image format (PNG, JPEG, GIF, WebP). \
                         The file may have been renamed."
                            .to_string(),
                    );
                }
                Ok(())
            }
            FilePurpose::Batch | FilePurpose::FineTune => {
                // JSONL must be valid UTF-8 text
                if std::str::from_utf8(data).is_err() {
                    return Err(
                        "File content is not valid UTF-8 text. JSONL files must contain valid UTF-8."
                            .to_string(),
                    );
                }
                Ok(())
            }
            FilePurpose::Assistants => Ok(()),
        }
    }
}

/// Check if file content has executable magic bytes.
fn is_executable_magic(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }

    // PE (Windows .exe, .dll, .scr, .com)
    if data.starts_with(b"MZ") {
        return true;
    }
    // ELF (Linux .so, executables)
    if data.starts_with(b"\x7fELF") {
        return true;
    }
    // Mach-O (macOS .dylib, executables) - both 32 and 64 bit, both endians
    if data.starts_with(&[0xFE, 0xED, 0xFA, 0xCE])
        || data.starts_with(&[0xFE, 0xED, 0xFA, 0xCF])
        || data.starts_with(&[0xCE, 0xFA, 0xED, 0xFE])
        || data.starts_with(&[0xCF, 0xFA, 0xED, 0xFE])
    {
        return true;
    }
    // Mach-O universal binary (fat binary)
    if data.starts_with(&[0xCA, 0xFE, 0xBA, 0xBE]) {
        return true;
    }
    // MSI (Microsoft Installer) / OLE Compound File
    if data.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        return true;
    }

    false
}

/// Check if file content has image magic bytes (PNG, JPEG, GIF, WebP).
fn is_image_magic(data: &[u8]) -> bool {
    if data.len() < 3 {
        return false;
    }

    // PNG
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return true;
    }
    // JPEG
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return true;
    }
    // GIF87a / GIF89a
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return true;
    }
    // WebP (RIFF....WEBP)
    if data.len() >= 12 && data.starts_with(b"RIFF") && data[8..12] == *b"WEBP" {
        return true;
    }

    false
}

/// File upload status (OpenAI Files API compatible)
///
/// Tracks the state of a file uploaded via the Files API.
/// Note: This status is **deprecated** in OpenAI's API but still returned for compatibility.
///
/// ## State Transitions
///
/// ```text
/// ┌──────────┐     validation      ┌───────────┐
/// │ uploaded │ ──────────────────▶ │ processed │
/// └──────────┘     success         └───────────┘
///       │
///       │ validation failed
///       ▼
/// ┌──────────┐
/// │  error   │
/// └──────────┘
/// ```
///
/// - `uploaded`: File received and stored, initial validation pending
/// - `processed`: File validated and ready for use (e.g., adding to vector stores)
/// - `error`: File validation failed (see `status_details` for reason)
///
/// ## Important
///
/// This is the **file upload** status, not the vector store processing status.
/// When a file is added to a vector store, its processing status is tracked
/// separately via `VectorStoreFileStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    /// File received and stored
    #[default]
    Uploaded,
    /// File validated and ready for use
    Processed,
    /// File validation failed
    Error,
}

impl FileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileStatus::Uploaded => "uploaded",
            FileStatus::Processed => "processed",
            FileStatus::Error => "error",
        }
    }
}

impl std::str::FromStr for FileStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "uploaded" => Ok(FileStatus::Uploaded),
            "processed" => Ok(FileStatus::Processed),
            "error" => Ok(FileStatus::Error),
            _ => Err(format!("Invalid file status: {}", s)),
        }
    }
}

/// A file (OpenAI Files API compatible)
/// Files are uploaded via the Files API and can then be added to vector stores
///
/// ## OpenAI Compatibility Notes
///
/// - `id` is serialized with `file-` prefix (e.g., `file-550e8400-e29b-41d4-a716-446655440000`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct File {
    /// File ID (serialized with `file-` prefix for OpenAI compatibility)
    #[serde(with = "file_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "file-550e8400-e29b-41d4-a716-446655440000"))]
    pub id: Uuid,
    /// Object type identifier (always "file" for API compatibility)
    #[serde(default = "default_file_object")]
    pub object: String,
    /// **Hadrian Extension:** Owner type for multi-tenancy (organization, project, or user)
    pub owner_type: VectorStoreOwnerType,
    /// **Hadrian Extension:** Owner ID for multi-tenancy
    pub owner_id: Uuid,
    pub filename: String,
    pub purpose: FilePurpose,
    /// **Hadrian Extension:** MIME content type of the file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(rename = "bytes")]
    pub size_bytes: i64,
    pub status: FileStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_details: Option<String>,
    /// SHA-256 hash of file content for deduplication (64 hex characters).
    /// Used to detect duplicate files when adding to collections.
    #[serde(skip)]
    pub content_hash: Option<String>,
    /// Storage backend for the file content
    #[serde(skip)]
    pub storage_backend: StorageBackend,
    /// Storage path for filesystem/S3 backends (None for database backend)
    #[serde(skip)]
    pub storage_path: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

fn default_file_object() -> String {
    OBJECT_TYPE_FILE.to_string()
}

/// Request to create a new file
#[derive(Debug, Clone, Validate)]
pub struct CreateFile {
    pub owner_type: VectorStoreOwnerType,
    pub owner_id: Uuid,
    #[validate(length(min = 1, max = 255))]
    pub filename: String,
    pub purpose: FilePurpose,
    pub content_type: Option<String>,
    pub size_bytes: i64,
    /// SHA-256 hash of file content for deduplication (64 hex characters)
    pub content_hash: Option<String>,
    pub storage_backend: StorageBackend,
    /// File data (only when storage_backend = Database)
    pub file_data: Option<Vec<u8>>,
    /// Storage path (for filesystem/S3 backends)
    pub storage_path: Option<String>,
}

/// File counts for a vector store (OpenAI-compatible).
///
/// These counts are **derived statistics** recalculated from the current state of
/// vector store files. They are NOT incremented/decremented manually, which prevents
/// race conditions and ensures eventual consistency.
///
/// ## When Counts Update
///
/// `file_counts` is automatically refreshed after these operations:
///
/// 1. **File added** (`add_file`): New file link created with `in_progress` status
/// 2. **Status changed** (`update_vector_store_file_status`): File transitions to
///    `completed`, `failed`, or `cancelled`
/// 3. **Usage updated** (`update_vector_store_file_usage`): After processing completes
/// 4. **File removed** (`remove_file`): Soft-delete excludes file from counts
///
/// ## Count Definitions
///
/// - `in_progress`: Files currently being processed (chunking, embedding)
/// - `completed`: Files successfully processed and searchable
/// - `failed`: Files that encountered processing errors
/// - `cancelled`: Files whose processing was cancelled
/// - `total`: Sum of all non-deleted files (in_progress + completed + failed + cancelled)
///
/// ## Implementation Note
///
/// Counts are calculated via SQL aggregation over `vector_store_files` where
/// `deleted_at IS NULL`. Soft-deleted files are excluded from all counts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileCounts {
    /// Number of files whose processing was cancelled
    pub cancelled: i32,
    /// Number of files successfully processed and searchable
    pub completed: i32,
    /// Number of files that encountered processing errors
    pub failed: i32,
    /// Number of files currently being processed
    pub in_progress: i32,
    /// Total number of files (sum of all statuses)
    pub total: i32,
}

/// Expiration policy for collections
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ExpiresAfter {
    /// Anchor timestamp for expiration ("last_active_at")
    pub anchor: String,
    /// Number of days after anchor before expiration
    pub days: i32,
}

/// Error codes for file processing failures (OpenAI-compatible)
///
/// These codes indicate why a file failed to be added to a vector store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum FileErrorCode {
    /// Internal server error during processing
    ServerError,
    /// File type is not supported for vector store processing
    UnsupportedFile,
    /// File content is invalid or corrupted
    InvalidFile,
}

impl FileErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileErrorCode::ServerError => "server_error",
            FileErrorCode::UnsupportedFile => "unsupported_file",
            FileErrorCode::InvalidFile => "invalid_file",
        }
    }
}

impl std::str::FromStr for FileErrorCode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "server_error" => Ok(FileErrorCode::ServerError),
            "unsupported_file" => Ok(FileErrorCode::UnsupportedFile),
            "invalid_file" => Ok(FileErrorCode::InvalidFile),
            _ => Err(format!("Invalid file error code: {}", s)),
        }
    }
}

/// File error information (OpenAI-compatible)
///
/// Returned when a file fails to be processed for a vector store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileError {
    /// Error code indicating the type of failure
    pub code: FileErrorCode,
    /// Human-readable description of the error
    pub message: String,
}

/// Static chunking configuration (OpenAI-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StaticChunkingConfig {
    #[serde(default = "default_max_chunk_size_tokens")]
    pub max_chunk_size_tokens: i32,
    #[serde(default = "default_chunk_overlap_tokens")]
    pub chunk_overlap_tokens: i32,
}

impl Default for StaticChunkingConfig {
    fn default() -> Self {
        Self {
            max_chunk_size_tokens: default_max_chunk_size_tokens(),
            chunk_overlap_tokens: default_chunk_overlap_tokens(),
        }
    }
}

fn default_max_chunk_size_tokens() -> i32 {
    800
}

fn default_chunk_overlap_tokens() -> i32 {
    400
}

/// Chunking strategy configuration (OpenAI-compatible)
/// OpenAI schema: {"type": "auto"} or {"type": "static", "static": {...}}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChunkingStrategy {
    #[default]
    Auto,
    Static {
        #[serde(rename = "static")]
        config: StaticChunkingConfig,
    },
}

/// A vector store for RAG.
///
/// Follows OpenAI VectorStore schema with Hadrian extensions for multi-tenancy
/// and embedding configuration.
///
/// ## Hadrian Extensions
///
/// The following fields are **Hadrian extensions** not present in the standard OpenAI API:
/// - `owner_type`, `owner_id`: Multi-tenancy support (organization, project, or user ownership)
/// - `description`: Human-readable description of the vector store
/// - `embedding_model`: The embedding model used for this vector store (immutable after creation)
/// - `embedding_dimensions`: Vector dimensions for the embedding model (immutable after creation)
/// - `updated_at`: Timestamp of last modification
///
/// ## OpenAI Compatibility Notes
///
/// - `id` is serialized with `vs_` prefix (e.g., `vs_550e8400-e29b-41d4-a716-446655440000`)
/// - `created_at` uses ISO 8601 format (OpenAI uses Unix timestamps)
/// - `expires_at`, `last_active_at` use ISO 8601 format (OpenAI uses Unix timestamps)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStore {
    /// Vector store ID (serialized with `vs_` prefix for OpenAI compatibility)
    #[serde(with = "vector_store_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "vs_550e8400-e29b-41d4-a716-446655440000"))]
    pub id: Uuid,
    /// Object type identifier (always "vector_store" for API compatibility)
    #[serde(default = "default_vector_store_object")]
    pub object: String,
    /// **Hadrian Extension:** Owner type for multi-tenancy (organization, project, or user)
    pub owner_type: VectorStoreOwnerType,
    /// **Hadrian Extension:** Owner ID for multi-tenancy
    pub owner_id: Uuid,
    pub name: String,
    /// **Hadrian Extension:** Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: VectorStoreStatus,
    /// **Hadrian Extension:** Embedding model used for this vector store (immutable after creation)
    pub embedding_model: String,
    /// **Hadrian Extension:** Embedding dimensions for this vector store (immutable after creation)
    pub embedding_dimensions: i32,
    /// Total storage usage in bytes across all files in this vector store.
    ///
    /// This represents the sum of chunk text content sizes (not including embedding vectors).
    /// It may differ from the sum of original file sizes due to chunking and text extraction.
    /// Updated automatically when files are processed.
    pub usage_bytes: i64,
    pub file_counts: FileCounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_after: Option<ExpiresAfter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    /// **Hadrian Extension:** Timestamp of last modification
    pub updated_at: DateTime<Utc>,
}

fn default_vector_store_object() -> String {
    OBJECT_TYPE_VECTOR_STORE.to_string()
}

/// A file in a vector store (links File to VectorStore).
///
/// Follows OpenAI VectorStoreFile schema with Hadrian extensions.
///
/// ## Hadrian Extensions
///
/// The following fields are **Hadrian extensions** not present in the standard OpenAI API:
/// - `internal_id`: Internal junction record ID (not serialized, used for database operations)
/// - `updated_at`: Timestamp of last modification
///
/// ## OpenAI Compatibility Notes
///
/// - `id` is the Files API file ID (matches OpenAI behavior where the vector store file ID
///   is the same as the underlying file ID)
/// - `vector_store_id` is serialized with `vs_` prefix
/// - `created_at` uses ISO 8601 format (OpenAI uses Unix timestamps)
/// - `last_error` is optional (OpenAI requires it, can be null)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStoreFile {
    /// Internal junction record ID (not exposed in API, used for database operations)
    #[serde(skip)]
    pub internal_id: Uuid,
    /// Vector store file ID - this is the Files API file ID (matches OpenAI behavior)
    #[serde(rename = "id", with = "file_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "file-550e8400-e29b-41d4-a716-446655440000"))]
    pub file_id: Uuid,
    /// Object type identifier (always "vector_store.file" for API compatibility)
    #[serde(default = "default_vector_store_file_object")]
    pub object: String,
    /// The vector store this file belongs to (vector_store_id in OpenAI API)
    #[serde(rename = "vector_store_id", with = "vector_store_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "vs_550e8400-e29b-41d4-a716-446655440000"))]
    pub vector_store_id: Uuid,
    pub status: VectorStoreFileStatus,
    /// Storage usage in bytes for this file's chunks.
    ///
    /// This represents the sum of chunk text content sizes (not including embedding vectors).
    /// It may differ from the original file size due to chunking and text extraction.
    /// Set to 0 until processing completes.
    pub usage_bytes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<FileError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunking_strategy: Option<ChunkingStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, serde_json::Value>>,
    pub created_at: DateTime<Utc>,
    /// **Hadrian Extension:** Timestamp of last modification
    pub updated_at: DateTime<Utc>,
}

fn default_vector_store_file_object() -> String {
    OBJECT_TYPE_VECTOR_STORE_FILE.to_string()
}

/// A chunk of a file (processed text segment)
/// Note: Chunks are stored in the vector database (pgvector/Qdrant), not the relational database.
/// This struct is used for in-memory operations and API responses.
///
/// ## OpenAI Compatibility Notes
///
/// - `id` is serialized with `chunk_` prefix (e.g., `chunk_550e8400-e29b-41d4-a716-446655440000`)
/// - `file_id` is serialized with `file-` prefix
/// - `vector_store_id` is serialized with `vs_` prefix
#[allow(dead_code)] // Public API type for vector store chunk operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStoreChunk {
    /// Chunk ID (serialized with `chunk_` prefix)
    #[serde(with = "chunk_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "chunk_550e8400-e29b-41d4-a716-446655440000"))]
    pub id: Uuid,
    /// File this chunk belongs to (serialized with `file-` prefix)
    #[serde(with = "file_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "file-550e8400-e29b-41d4-a716-446655440000"))]
    pub file_id: Uuid,
    /// VectorStore (vector store) this chunk belongs to (serialized with `vs_` prefix)
    #[serde(with = "vector_store_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "vs_550e8400-e29b-41d4-a716-446655440000"))]
    pub vector_store_id: Uuid,
    pub chunk_index: i32,
    pub content: String,
    pub token_count: i32,
    pub char_start: i32,
    pub char_end: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Owner specification for creating a vector store
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VectorStoreOwner {
    Organization { organization_id: Uuid },
    Team { team_id: Uuid },
    Project { project_id: Uuid },
    User { user_id: Uuid },
}

impl VectorStoreOwner {
    pub fn owner_type(&self) -> VectorStoreOwnerType {
        match self {
            VectorStoreOwner::Organization { .. } => VectorStoreOwnerType::Organization,
            VectorStoreOwner::Team { .. } => VectorStoreOwnerType::Team,
            VectorStoreOwner::Project { .. } => VectorStoreOwnerType::Project,
            VectorStoreOwner::User { .. } => VectorStoreOwnerType::User,
        }
    }

    pub fn owner_id(&self) -> Uuid {
        match self {
            VectorStoreOwner::Organization { organization_id } => *organization_id,
            VectorStoreOwner::Team { team_id } => *team_id,
            VectorStoreOwner::Project { project_id } => *project_id,
            VectorStoreOwner::User { user_id } => *user_id,
        }
    }
}

fn default_embedding_model() -> String {
    "text-embedding-3-small".to_string()
}

fn default_embedding_dimensions() -> i32 {
    1536
}

/// Request to create a new vector store.
///
/// ## Hadrian Extensions
///
/// The following fields are **Hadrian extensions** not present in the standard OpenAI API:
/// - `owner`: Multi-tenancy support (required, specifies organization/project/user ownership)
/// - `embedding_model`: Custom embedding model selection (default: text-embedding-3-small)
/// - `embedding_dimensions`: Custom embedding dimensions (default: 1536)
///
/// ## OpenAI Compatibility Notes
///
/// - `name` is optional (OpenAI-compatible), but a name will be auto-generated if not provided
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateVectorStore {
    /// **Hadrian Extension:** Owner of the vector store (required for multi-tenancy)
    pub owner: VectorStoreOwner,
    /// A list of File IDs that the vector store should use (max 500).
    /// Files will be attached to the vector store after creation.
    #[serde(default)]
    #[validate(length(max = 500))]
    pub file_ids: Vec<Uuid>,
    /// Name of the vector store (optional, auto-generated if not provided)
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    /// Description of the vector store
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    /// **Hadrian Extension:** Embedding model to use (immutable after creation, default: text-embedding-3-small)
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    /// **Hadrian Extension:** Embedding dimensions (immutable after creation, default: 1536)
    #[serde(default = "default_embedding_dimensions")]
    pub embedding_dimensions: i32,
    /// Custom metadata (up to 16 key-value pairs, keys max 64 chars, string values max 512 chars)
    #[validate(custom(function = "validate_metadata"))]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    /// Expiration policy
    pub expires_after: Option<ExpiresAfter>,
    /// The chunking strategy used to chunk the file(s). If not set, will use the `auto` strategy.
    /// Only applicable if `file_ids` is non-empty.
    pub chunking_strategy: Option<ChunkingStrategy>,
}

/// Request to update a vector store (vector store).
///
/// ## Hadrian Extensions
///
/// The following field is a **Hadrian extension** not present in the standard OpenAI API:
/// - `description`: Human-readable description
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateVectorStore {
    /// New name
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    /// **Hadrian Extension:** New description
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    /// New metadata (replaces existing, up to 16 key-value pairs, keys max 64 chars, string values max 512 chars)
    #[validate(custom(function = "validate_metadata"))]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    /// New expiration policy
    pub expires_after: Option<ExpiresAfter>,
}

/// Request to add a file to a vector store (create a vector store file)
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AddFileToVectorStore {
    pub vector_store_id: Uuid,
    /// Reference to an existing file from the Files API
    pub file_id: Uuid,
    pub chunking_strategy: Option<ChunkingStrategy>,
    pub attributes: Option<HashMap<String, serde_json::Value>>,
}

/// Request to create chunks for a file
/// Note: This is used to create chunks in the vector database, not the relational database.
/// The embedding is generated and stored alongside the chunk in the vector store.
#[allow(dead_code)] // Public API type for vector store chunk creation
#[derive(Debug, Clone)]
pub struct CreateChunk {
    pub file_id: Uuid,
    pub vector_store_id: Uuid,
    pub chunk_index: i32,
    pub content: String,
    pub token_count: i32,
    pub char_start: i32,
    pub char_end: i32,
    pub metadata: Option<serde_json::Value>,
}

/// Validation result for multi-store search
/// Ensures all stores in a search have compatible embedding configurations
#[allow(dead_code)] // Public API type for multi-store search validation
#[derive(Debug, Clone)]
pub struct SearchValidation {
    /// The validated collections
    pub stores: Vec<VectorStore>,
    /// Common embedding model (must match across all stores)
    pub embedding_model: String,
    /// Common embedding dimensions (must match across all stores)
    pub embedding_dimensions: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunking_strategy_auto_serialization() {
        let strategy = ChunkingStrategy::Auto;
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(json, r#"{"type":"auto"}"#);
    }

    #[test]
    fn test_chunking_strategy_static_serialization() {
        let strategy = ChunkingStrategy::Static {
            config: StaticChunkingConfig {
                max_chunk_size_tokens: 1600,
                chunk_overlap_tokens: 400,
            },
        };
        let json = serde_json::to_string(&strategy).unwrap();
        // OpenAI format: {"type": "static", "static": {"max_chunk_size_tokens": ..., "chunk_overlap_tokens": ...}}
        assert_eq!(
            json,
            r#"{"type":"static","static":{"max_chunk_size_tokens":1600,"chunk_overlap_tokens":400}}"#
        );
    }

    #[test]
    fn test_chunking_strategy_static_deserialization() {
        // OpenAI format input
        let json = r#"{"type":"static","static":{"max_chunk_size_tokens":1600,"chunk_overlap_tokens":400}}"#;
        let strategy: ChunkingStrategy = serde_json::from_str(json).unwrap();
        match strategy {
            ChunkingStrategy::Static { config } => {
                assert_eq!(config.max_chunk_size_tokens, 1600);
                assert_eq!(config.chunk_overlap_tokens, 400);
            }
            _ => panic!("Expected Static variant"),
        }
    }

    #[test]
    fn test_chunking_strategy_auto_deserialization() {
        let json = r#"{"type":"auto"}"#;
        let strategy: ChunkingStrategy = serde_json::from_str(json).unwrap();
        assert!(matches!(strategy, ChunkingStrategy::Auto));
    }

    // ==================== File Content Validation ====================

    #[test]
    fn test_executable_pe_rejected() {
        let mut data = b"MZ".to_vec();
        data.extend_from_slice(&[0u8; 100]);
        assert!(
            FilePurpose::Assistants
                .validate_file_content(&data)
                .is_err()
        );
        assert!(FilePurpose::Vision.validate_file_content(&data).is_err());
        assert!(FilePurpose::Batch.validate_file_content(&data).is_err());
    }

    #[test]
    fn test_executable_elf_rejected() {
        let mut data = b"\x7fELF".to_vec();
        data.extend_from_slice(&[0u8; 100]);
        assert!(
            FilePurpose::Assistants
                .validate_file_content(&data)
                .is_err()
        );
    }

    #[test]
    fn test_executable_macho_rejected() {
        for magic in [
            [0xFE, 0xED, 0xFA, 0xCE],
            [0xFE, 0xED, 0xFA, 0xCF],
            [0xCE, 0xFA, 0xED, 0xFE],
            [0xCF, 0xFA, 0xED, 0xFE],
        ] {
            let mut data = magic.to_vec();
            data.extend_from_slice(&[0u8; 100]);
            assert!(
                FilePurpose::Assistants
                    .validate_file_content(&data)
                    .is_err(),
                "Mach-O {:02X?} should be rejected",
                magic
            );
        }
    }

    #[test]
    fn test_executable_fat_binary_rejected() {
        let mut data = vec![0xCA, 0xFE, 0xBA, 0xBE];
        data.extend_from_slice(&[0u8; 100]);
        assert!(
            FilePurpose::Assistants
                .validate_file_content(&data)
                .is_err()
        );
    }

    #[test]
    fn test_executable_msi_rejected() {
        let mut data = vec![0xD0, 0xCF, 0x11, 0xE0];
        data.extend_from_slice(&[0u8; 100]);
        assert!(
            FilePurpose::Assistants
                .validate_file_content(&data)
                .is_err()
        );
    }

    #[test]
    fn test_short_data_not_rejected_as_executable() {
        // Data shorter than 4 bytes should not be mistakenly flagged as executable
        assert!(FilePurpose::Assistants.validate_file_content(b"MZ").is_ok());
        assert!(
            FilePurpose::Assistants
                .validate_file_content(b"abc")
                .is_ok()
        );
    }

    #[test]
    fn test_vision_png_accepted() {
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
        assert!(FilePurpose::Vision.validate_file_content(&data).is_ok());
    }

    #[test]
    fn test_vision_jpeg_accepted() {
        let mut data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        data.extend_from_slice(&[0u8; 100]);
        assert!(FilePurpose::Vision.validate_file_content(&data).is_ok());
    }

    #[test]
    fn test_vision_gif_accepted() {
        let mut data87 = b"GIF87a".to_vec();
        data87.extend_from_slice(&[0u8; 100]);
        assert!(FilePurpose::Vision.validate_file_content(&data87).is_ok());

        let mut data89 = b"GIF89a".to_vec();
        data89.extend_from_slice(&[0u8; 100]);
        assert!(FilePurpose::Vision.validate_file_content(&data89).is_ok());
    }

    #[test]
    fn test_vision_webp_accepted() {
        let mut data = b"RIFF".to_vec();
        data.extend_from_slice(&[0u8; 4]); // file size
        data.extend_from_slice(b"WEBP");
        data.extend_from_slice(&[0u8; 100]);
        assert!(FilePurpose::Vision.validate_file_content(&data).is_ok());
    }

    #[test]
    fn test_vision_non_image_rejected() {
        let data = b"This is just plain text, not an image.";
        assert!(FilePurpose::Vision.validate_file_content(data).is_err());
    }

    #[test]
    fn test_vision_truncated_data_rejected() {
        // Less than 3 bytes can't be a valid image
        assert!(FilePurpose::Vision.validate_file_content(b"AB").is_err());
    }

    #[test]
    fn test_batch_valid_utf8_accepted() {
        let data = b"{\"method\": \"POST\"}\n{\"method\": \"GET\"}";
        assert!(FilePurpose::Batch.validate_file_content(data).is_ok());
        assert!(FilePurpose::FineTune.validate_file_content(data).is_ok());
    }

    #[test]
    fn test_batch_invalid_utf8_rejected() {
        let data = [0xFF, 0xFE, 0x00, 0x01, 0x80, 0x81];
        assert!(FilePurpose::Batch.validate_file_content(&data).is_err());
        assert!(FilePurpose::FineTune.validate_file_content(&data).is_err());
    }

    #[test]
    fn test_assistants_non_executable_accepted() {
        let data = b"Hello, this is a document.";
        assert!(FilePurpose::Assistants.validate_file_content(data).is_ok());
    }

    #[test]
    fn test_assistants_executable_rejected() {
        let mut data = b"\x7fELF".to_vec();
        data.extend_from_slice(&[0u8; 100]);
        assert!(
            FilePurpose::Assistants
                .validate_file_content(&data)
                .is_err()
        );
    }
}
