use std::sync::Arc;

use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams, ListResult},
    models::{
        AddFileToVectorStore, CreateVectorStore, FileError, UpdateVectorStore, VectorStore,
        VectorStoreFile, VectorStoreFileStatus, VectorStoreOwner, VectorStoreOwnerType,
    },
};

/// Service layer for vector store operations.
///
/// Vector stores are used for RAG (Retrieval Augmented Generation) and provide
/// storage for documents that can be searched and retrieved to augment LLM prompts.
#[derive(Clone)]
pub struct VectorStoresService {
    db: Arc<DbPool>,
}

impl VectorStoresService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    // ==================== Vector Stores CRUD ====================

    /// Create a new vector_store.
    ///
    /// VectorStore names must be unique within the owner scope.
    pub async fn create(&self, input: CreateVectorStore) -> DbResult<VectorStore> {
        self.db.vector_stores().create_vector_store(input).await
    }

    /// Get a vector store by ID.
    ///
    /// Returns None if the vector store doesn't exist or has been deleted.
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<VectorStore>> {
        self.db.vector_stores().get_vector_store(id).await
    }

    /// Get a vector store by ID, scoped to a specific organization.
    ///
    /// Use this variant when org context is available to prevent cross-org access.
    pub async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<VectorStore>> {
        self.db.vector_stores().get_by_id_and_org(id, org_id).await
    }

    /// Get a vector store by owner and name.
    ///
    /// Returns None if the vector store doesn't exist or has been deleted.
    pub async fn get_by_name(
        &self,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
        name: &str,
    ) -> DbResult<Option<VectorStore>> {
        self.db
            .vector_stores()
            .get_vector_store_by_name(owner_type, owner_id, name)
            .await
    }

    /// List collections for an owner (organization, project, or user).
    ///
    /// Vector stores are ordered by `updated_at` DESC (most recently used first).
    pub async fn list(
        &self,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<VectorStore>> {
        self.db
            .vector_stores()
            .list_vector_stores(owner_type, owner_id, params)
            .await
    }

    /// Convenience method to list collections by owner enum.
    pub async fn list_by_owner(
        &self,
        owner: &VectorStoreOwner,
        params: ListParams,
    ) -> DbResult<ListResult<VectorStore>> {
        self.list(owner.owner_type(), owner.owner_id(), params)
            .await
    }

    /// List collections accessible to a user based on their memberships.
    ///
    /// Returns collections owned by:
    /// - The user directly
    /// - Organizations the user is a member of
    /// - Teams the user is a member of
    /// - Projects the user has access to
    ///
    /// Vector stores are ordered by `updated_at` DESC (most recently used first).
    pub async fn list_accessible(
        &self,
        user_id: Option<Uuid>,
        org_ids: &[Uuid],
        team_ids: &[Uuid],
        project_ids: &[Uuid],
        params: ListParams,
    ) -> DbResult<ListResult<VectorStore>> {
        self.db
            .vector_stores()
            .list_accessible_vector_stores(user_id, org_ids, team_ids, project_ids, params)
            .await
    }

    /// List all vector stores (no owner filter).
    ///
    /// Used when auth is disabled to return all vector stores.
    /// Vector stores are ordered by `updated_at` DESC (most recently used first).
    pub async fn list_all(&self, params: ListParams) -> DbResult<ListResult<VectorStore>> {
        self.db.vector_stores().list_all_vector_stores(params).await
    }

    /// Update a vector store's metadata.
    pub async fn update(&self, id: Uuid, input: UpdateVectorStore) -> DbResult<VectorStore> {
        self.db.vector_stores().update_vector_store(id, input).await
    }

    /// Delete a vector store (soft delete).
    ///
    /// Files and chunks are cascade-deleted when the vector store is permanently removed.
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.vector_stores().delete_vector_store(id).await
    }

    /// Update the vector store's last_active_at timestamp.
    ///
    /// Call this when the vector store is accessed (e.g., searched).
    /// This affects expiration for collections with `expires_after` policy.
    pub async fn touch(&self, id: Uuid) -> DbResult<()> {
        self.db.vector_stores().touch_vector_store(id).await
    }

    // ==================== VectorStore Files CRUD ====================
    // Note: File upload/download operations use FilesService.
    // This section manages the link between files and collections.

    /// Add a file to a vector_store.
    ///
    /// The file must exist in the Files API (see FilesService). This creates
    /// a link with `in_progress` status. Call `process_file` in DocumentProcessor
    /// to chunk and embed the file content, which will update the status to
    /// `completed` or `failed`.
    pub async fn add_file(&self, input: AddFileToVectorStore) -> DbResult<VectorStoreFile> {
        let file = self
            .db
            .vector_stores()
            .add_file_to_vector_store(input)
            .await?;

        // Update vector store stats (file_counts.in_progress increases)
        self.db
            .vector_stores()
            .update_vector_store_stats(file.vector_store_id)
            .await?;

        Ok(file)
    }

    /// Get a vector store file link by ID.
    pub async fn get_vector_store_file(&self, id: Uuid) -> DbResult<Option<VectorStoreFile>> {
        self.db.vector_stores().get_vector_store_file(id).await
    }

    /// List files linked to a vector_store.
    pub async fn list_vector_store_files(
        &self,
        vector_store_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<VectorStoreFile>> {
        self.db
            .vector_stores()
            .list_vector_store_files(vector_store_id, params)
            .await
    }

    /// Update a file's processing status.
    ///
    /// This also updates the vector store's file_counts statistics.
    pub async fn update_vector_store_file_status(
        &self,
        id: Uuid,
        status: VectorStoreFileStatus,
        error: Option<FileError>,
    ) -> DbResult<()> {
        self.db
            .vector_stores()
            .update_vector_store_file_status(id, status, error)
            .await?;

        // Get the file to find its vector_store_id
        if let Some(file) = self.db.vector_stores().get_vector_store_file(id).await? {
            self.db
                .vector_stores()
                .update_vector_store_stats(file.vector_store_id)
                .await?;
        }

        Ok(())
    }

    /// Update a file's usage bytes (after processing).
    ///
    /// This also updates the vector store's total usage_bytes.
    pub async fn update_vector_store_file_usage(&self, id: Uuid, usage_bytes: i64) -> DbResult<()> {
        self.db
            .vector_stores()
            .update_vector_store_file_usage(id, usage_bytes)
            .await?;

        // Get the file to find its vector_store_id
        if let Some(file) = self.db.vector_stores().get_vector_store_file(id).await? {
            self.db
                .vector_stores()
                .update_vector_store_stats(file.vector_store_id)
                .await?;
        }

        Ok(())
    }

    /// Find a vector store file by file ID (for idempotency).
    ///
    /// Checks if a specific file is already in the vector_store. This provides true
    /// idempotency - adding the same file_id to the same vector store twice returns
    /// the existing entry instead of creating a duplicate.
    ///
    /// Returns the existing VectorStoreFile if found, None otherwise.
    pub async fn find_by_file_id(
        &self,
        vector_store_id: Uuid,
        file_id: Uuid,
    ) -> DbResult<Option<VectorStoreFile>> {
        self.db
            .vector_stores()
            .find_vector_store_file_by_file_id(vector_store_id, file_id)
            .await
    }

    /// Find a vector store file by content hash and owner (for same-owner deduplication).
    ///
    /// Checks if any file with the same content hash AND same owner already exists
    /// in the vector_store. This prevents users from accidentally adding duplicate
    /// content to a vector store while avoiding cross-user deduplication issues.
    ///
    /// Returns the existing VectorStoreFile if found, None otherwise.
    pub async fn find_by_content_hash_and_owner(
        &self,
        vector_store_id: Uuid,
        content_hash: &str,
        owner_type: VectorStoreOwnerType,
        owner_id: Uuid,
    ) -> DbResult<Option<VectorStoreFile>> {
        self.db
            .vector_stores()
            .find_vector_store_file_by_content_hash_and_owner(
                vector_store_id,
                content_hash,
                owner_type,
                owner_id,
            )
            .await
    }

    /// Remove a file from a vector_store.
    ///
    /// Note: Chunks associated with this file must be deleted from the vector store
    /// separately using the VectorStore trait's `delete_chunks_by_file` method.
    /// Also updates the vector store's statistics.
    /// The actual file in the Files API is NOT deleted - only the link is removed.
    pub async fn remove_file(&self, id: Uuid) -> DbResult<()> {
        // Get the file to find its vector_store_id before deletion
        let vector_store_id = self
            .db
            .vector_stores()
            .get_vector_store_file(id)
            .await?
            .map(|f| f.vector_store_id);

        self.db
            .vector_stores()
            .remove_file_from_vector_store(id)
            .await?;

        // Update vector store stats after deletion
        if let Some(vector_store_id) = vector_store_id {
            self.db
                .vector_stores()
                .update_vector_store_stats(vector_store_id)
                .await?;
        }

        Ok(())
    }

    // ==================== Utility Methods ====================
    // Note: Chunk operations (create, get, delete) are handled by the VectorStore trait,
    // as chunks are stored in the vector database (pgvector/Qdrant), not the relational database.

    /// Clean up soft-deleted vector_store_files that reference a specific file.
    ///
    /// This is called before deleting a file to ensure the foreign key constraint
    /// doesn't fail. Only removes records that have already been soft-deleted
    /// (i.e., the file was already removed from the vector store).
    pub async fn cleanup_soft_deleted_references(&self, file_id: Uuid) -> DbResult<u64> {
        self.db
            .vector_stores()
            .hard_delete_soft_deleted_references(file_id)
            .await
    }

    /// Recalculate and update vector store statistics.
    ///
    /// This updates `usage_bytes` and `file_counts` based on current file state.
    /// Called automatically by file operations, but can be called manually if needed.
    pub async fn refresh_stats(&self, vector_store_id: Uuid) -> DbResult<()> {
        self.db
            .vector_stores()
            .update_vector_store_stats(vector_store_id)
            .await
    }

    /// Check if a vector store exists and is accessible.
    ///
    /// Returns true if the vector store exists and hasn't been deleted.
    pub async fn exists(&self, id: Uuid) -> DbResult<bool> {
        Ok(self
            .db
            .vector_stores()
            .get_vector_store(id)
            .await?
            .is_some())
    }

    /// Check if a user has access to a vector_store.
    ///
    /// Access is granted if:
    /// - The vector store is owned by the user directly
    /// - The vector store is owned by an organization the user belongs to
    /// - The vector store is owned by a project the user belongs to
    ///
    /// Note: This method checks ownership hierarchy. For more complex RBAC,
    /// use the policy engine.
    pub async fn user_has_access(&self, user_id: Uuid, vector_store_id: Uuid) -> DbResult<bool> {
        let vector_store = match self
            .db
            .vector_stores()
            .get_vector_store(vector_store_id)
            .await?
        {
            Some(c) => c,
            None => return Ok(false),
        };

        match vector_store.owner_type {
            VectorStoreOwnerType::User => {
                // Direct ownership
                Ok(vector_store.owner_id == user_id)
            }
            VectorStoreOwnerType::Organization => {
                // Check if user is a member of the organization
                let members = self
                    .db
                    .users()
                    .list_org_members(vector_store.owner_id, ListParams::default())
                    .await?;
                Ok(members.items.iter().any(|u| u.id == user_id))
            }
            VectorStoreOwnerType::Team => {
                // Check if user is a member of the team
                let members = self
                    .db
                    .teams()
                    .list_members(vector_store.owner_id, ListParams::default())
                    .await?;
                Ok(members.items.iter().any(|m| m.user_id == user_id))
            }
            VectorStoreOwnerType::Project => {
                // Check if user is a member of the project
                let members = self
                    .db
                    .users()
                    .list_project_members(vector_store.owner_id, ListParams::default())
                    .await?;
                Ok(members.items.iter().any(|u| u.id == user_id))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests would go here, using a test database
    // Unit tests are limited since the service primarily delegates to the repository

    #[test]
    fn test_service_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<VectorStoresService>();
    }
}
