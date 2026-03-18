use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreateTemplate, Template, TemplateOwnerType, UpdateTemplate},
};

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait TemplateRepo: Send + Sync {
    /// Create a new template.
    async fn create(&self, input: CreateTemplate) -> DbResult<Template>;

    /// Get a template by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Template>>;

    /// Get a template by ID, scoped to a specific organization.
    ///
    /// Verifies the template belongs to the given org by checking the owner relationship:
    /// - Organization-owned: `owner_id` matches directly
    /// - Team-owned: joins through `teams.org_id`
    /// - Project-owned: joins through `projects.org_id`
    /// - User-owned: joins through `org_memberships`
    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Template>>;

    /// List templates by owner (organization, team, project, or user).
    async fn list_by_owner(
        &self,
        owner_type: TemplateOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Template>>;

    /// List all templates accessible within an organization.
    ///
    /// Returns templates from all scopes within the org:
    /// - Organization-owned (owner_id = org_id)
    /// - Team-owned (team belongs to org)
    /// - Project-owned (project belongs to org)
    /// - User-owned (user is a member of org)
    async fn list_by_org(&self, org_id: Uuid, params: ListParams)
    -> DbResult<ListResult<Template>>;

    /// Count templates by owner.
    async fn count_by_owner(
        &self,
        owner_type: TemplateOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64>;

    /// Update a template.
    async fn update(&self, id: Uuid, input: UpdateTemplate) -> DbResult<Template>;

    /// Soft-delete a template.
    async fn delete(&self, id: Uuid) -> DbResult<()>;
}
