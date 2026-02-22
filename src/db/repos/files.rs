use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreateFile, File, FilePurpose, FileStatus, VectorStoreOwnerType},
};

/// Repository trait for files (OpenAI Files API) operations
#[async_trait]
pub trait FilesRepo: Send + Sync {
    /// Create a new file
    async fn create_file(&self, input: CreateFile) -> DbResult<File>;

    /// Get a file by ID
    async fn get_file(&self, id: Uuid) -> DbResult<Option<File>>;

    /// Get file data (for files stored in DB)
    async fn get_file_data(&self, id: Uuid) -> DbResult<Option<Vec<u8>>>;

    /// List files by owner, optionally filtered by purpose
    async fn list_files(
        &self,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
        purpose: Option<FilePurpose>,
        params: ListParams,
    ) -> DbResult<ListResult<File>>;

    /// Delete a file
    async fn delete_file(&self, id: Uuid) -> DbResult<()>;

    /// Update file status
    async fn update_file_status(
        &self,
        id: Uuid,
        status: FileStatus,
        status_details: Option<String>,
    ) -> DbResult<()>;

    /// Count references to a file across collections
    /// Used to determine if a file can be deleted
    async fn count_file_references(&self, file_id: Uuid) -> DbResult<i64>;
}
