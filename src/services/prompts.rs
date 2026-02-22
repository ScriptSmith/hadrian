use std::sync::Arc;

use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams, repos::ListResult},
    models::{CreatePrompt, Prompt, PromptOwnerType, UpdatePrompt},
};

/// Service layer for prompt template operations
#[derive(Clone)]
pub struct PromptService {
    db: Arc<DbPool>,
}

impl PromptService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new prompt template
    pub async fn create(&self, input: CreatePrompt) -> DbResult<Prompt> {
        self.db.prompts().create(input).await
    }

    /// Get a prompt by ID
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Prompt>> {
        self.db.prompts().get_by_id(id).await
    }

    /// Get a prompt by ID, scoped to a specific organization.
    ///
    /// Use this variant when org context is available to prevent cross-org access.
    pub async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Prompt>> {
        self.db.prompts().get_by_id_and_org(id, org_id).await
    }

    /// List prompts by owner with pagination
    pub async fn list_by_owner(
        &self,
        owner_type: PromptOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Prompt>> {
        self.db
            .prompts()
            .list_by_owner(owner_type, owner_id, params)
            .await
    }

    /// Count prompts by owner
    pub async fn count_by_owner(
        &self,
        owner_type: PromptOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64> {
        self.db
            .prompts()
            .count_by_owner(owner_type, owner_id, include_deleted)
            .await
    }

    /// Update a prompt by ID
    pub async fn update(&self, id: Uuid, input: UpdatePrompt) -> DbResult<Prompt> {
        self.db.prompts().update(id, input).await
    }

    /// Delete (soft-delete) a prompt by ID
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.prompts().delete(id).await
    }
}
