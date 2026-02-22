use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{
        AddFileToVectorStore, CreateVectorStore, FileError, UpdateVectorStore, VectorStore,
        VectorStoreFile, VectorStoreFileStatus, VectorStoreOwnerType,
    },
};

/// Repository trait for collections (vector stores) operations
#[async_trait]
pub trait VectorStoresRepo: Send + Sync {
    // ==================== Vector Stores CRUD ====================

    /// Create a new vector store
    async fn create_vector_store(&self, input: CreateVectorStore) -> DbResult<VectorStore>;

    /// Get a vector store by ID
    async fn get_vector_store(&self, id: Uuid) -> DbResult<Option<VectorStore>>;

    /// Get a vector store by ID, scoped to a specific organization.
    ///
    /// Verifies the vector store belongs to the given org by checking the owner relationship:
    /// - Organization-owned: `owner_id` matches directly
    /// - Team-owned: joins through `teams.org_id`
    /// - Project-owned: joins through `projects.org_id`
    /// - User-owned: joins through `org_memberships`
    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<VectorStore>>;

    /// Get a vector store by owner and name
    async fn get_vector_store_by_name(
        &self,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
        name: &str,
    ) -> DbResult<Option<VectorStore>>;

    /// List collections by owner
    ///
    /// Note: Vector stores are ordered by `updated_at` DESC (most recently used first).
    async fn list_vector_stores(
        &self,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<VectorStore>>;

    /// List collections accessible to a user based on their memberships.
    ///
    /// Returns collections where:
    /// - The user owns the vector store directly
    /// - The vector store belongs to an organization the user is a member of
    /// - The vector store belongs to a team the user is a member of
    /// - The vector store belongs to a project the user has access to
    ///
    /// Note: Vector stores are ordered by `updated_at` DESC (most recently used first).
    async fn list_accessible_vector_stores(
        &self,
        user_id: Option<Uuid>,
        org_ids: &[Uuid],
        team_ids: &[Uuid],
        project_ids: &[Uuid],
        params: ListParams,
    ) -> DbResult<ListResult<VectorStore>>;

    /// List all vector stores (no owner filter).
    ///
    /// Used when auth is disabled to return all vector stores.
    /// Note: Vector stores are ordered by `updated_at` DESC (most recently used first).
    async fn list_all_vector_stores(&self, params: ListParams)
    -> DbResult<ListResult<VectorStore>>;

    /// Update a vector store
    async fn update_vector_store(
        &self,
        id: Uuid,
        input: UpdateVectorStore,
    ) -> DbResult<VectorStore>;

    /// Delete a vector store (soft delete - sets deleted_at)
    async fn delete_vector_store(&self, id: Uuid) -> DbResult<()>;

    /// Hard delete a vector store (for cleanup job)
    async fn hard_delete_vector_store(&self, id: Uuid) -> DbResult<()>;

    /// List soft-deleted collections older than the given timestamp
    /// Used by the cleanup job
    async fn list_deleted_vector_stores(
        &self,
        older_than: DateTime<Utc>,
    ) -> DbResult<Vec<VectorStore>>;

    /// Update vector store's last_active_at timestamp
    async fn touch_vector_store(&self, id: Uuid) -> DbResult<()>;

    // ==================== VectorStore Files CRUD ====================

    /// Add a file to a vector store (creates a VectorStoreFile link)
    async fn add_file_to_vector_store(
        &self,
        input: AddFileToVectorStore,
    ) -> DbResult<VectorStoreFile>;

    /// Get a vector store file by ID
    async fn get_vector_store_file(&self, id: Uuid) -> DbResult<Option<VectorStoreFile>>;

    /// Find a vector store file by file ID (for idempotency).
    ///
    /// Checks if a specific file is already in the vector_store. This provides true
    /// idempotency - adding the same file_id to the same vector store twice returns
    /// the existing entry instead of creating a duplicate.
    ///
    /// Returns the existing VectorStoreFile if found, None otherwise.
    /// Only returns non-deleted entries (deleted_at IS NULL).
    async fn find_vector_store_file_by_file_id(
        &self,
        vector_store_id: Uuid,
        file_id: Uuid,
    ) -> DbResult<Option<VectorStoreFile>>;

    /// Find a vector store file by content hash and owner (for same-owner deduplication).
    ///
    /// Checks if any file with the same content hash AND same owner already exists
    /// in the vector_store. This prevents users from accidentally adding duplicate
    /// content to a vector store while avoiding cross-user deduplication issues.
    ///
    /// Returns the existing VectorStoreFile if found, None otherwise.
    /// Only returns non-deleted entries (deleted_at IS NULL).
    async fn find_vector_store_file_by_content_hash_and_owner(
        &self,
        vector_store_id: Uuid,
        content_hash: &str,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
    ) -> DbResult<Option<VectorStoreFile>>;

    /// List files in a vector store
    async fn list_vector_store_files(
        &self,
        vector_store_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<VectorStoreFile>>;

    /// Update vector store file status and optionally set error
    async fn update_vector_store_file_status(
        &self,
        id: Uuid,
        status: VectorStoreFileStatus,
        error: Option<FileError>,
    ) -> DbResult<()>;

    /// Update vector store file usage bytes after processing
    async fn update_vector_store_file_usage(&self, id: Uuid, usage_bytes: i64) -> DbResult<()>;

    /// Remove a file from a vector store (soft delete - sets deleted_at)
    async fn remove_file_from_vector_store(&self, id: Uuid) -> DbResult<()>;

    /// List soft-deleted vector store files older than the given timestamp
    /// Used by the cleanup job to find files ready for hard deletion
    async fn list_deleted_vector_store_files(
        &self,
        older_than: DateTime<Utc>,
    ) -> DbResult<Vec<VectorStoreFile>>;

    /// Hard delete a vector store file record (for cleanup job)
    /// This permanently removes the vector_store_files link after chunks have been deleted
    async fn hard_delete_vector_store_file(&self, id: Uuid) -> DbResult<()>;

    /// Hard delete all soft-deleted vector store files that reference a specific file.
    /// Used when deleting a file to clean up any soft-deleted references first.
    async fn hard_delete_soft_deleted_references(&self, file_id: Uuid) -> DbResult<u64>;

    // ==================== Aggregates ====================
    // Note: Chunk operations (create, get, delete) are handled by the VectorStore trait,
    // as chunks are stored in the vector database (pgvector/Qdrant), not the relational database.

    /// Recalculate and update vector store statistics (usage_bytes, file_counts)
    /// Call this after file status changes
    async fn update_vector_store_stats(&self, vector_store_id: Uuid) -> DbResult<()>;
}
