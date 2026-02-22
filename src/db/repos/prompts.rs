use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreatePrompt, Prompt, PromptOwnerType, UpdatePrompt},
};

#[async_trait]
pub trait PromptRepo: Send + Sync {
    /// Create a new prompt.
    async fn create(&self, input: CreatePrompt) -> DbResult<Prompt>;

    /// Get a prompt by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Prompt>>;

    /// Get a prompt by ID, scoped to a specific organization.
    ///
    /// Verifies the prompt belongs to the given org by checking the owner relationship:
    /// - Organization-owned: `owner_id` matches directly
    /// - Team-owned: joins through `teams.org_id`
    /// - Project-owned: joins through `projects.org_id`
    /// - User-owned: joins through `org_memberships`
    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Prompt>>;

    /// List prompts by owner (organization, team, project, or user).
    async fn list_by_owner(
        &self,
        owner_type: PromptOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Prompt>>;

    /// Count prompts by owner.
    async fn count_by_owner(
        &self,
        owner_type: PromptOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64>;

    /// Update a prompt.
    async fn update(&self, id: Uuid, input: UpdatePrompt) -> DbResult<Prompt>;

    /// Soft-delete a prompt.
    async fn delete(&self, id: Uuid) -> DbResult<()>;
}
