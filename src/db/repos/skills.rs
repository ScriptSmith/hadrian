use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreateSkill, Skill, SkillOwnerType, UpdateSkill},
};

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait SkillRepo: Send + Sync {
    /// Create a new skill with its full file set. `input.files` is stored
    /// verbatim — callers (service layer) must have already enforced the
    /// spec invariants (SKILL.md present, paths valid, total-size limit).
    async fn create(&self, input: CreateSkill) -> DbResult<Skill>;

    /// Get a skill by ID, including all bundled files.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Skill>>;

    /// Get a skill by ID, scoped to a specific organization.
    ///
    /// Verifies the skill belongs to the given org by checking the owner relationship:
    /// - Organization-owned: `owner_id` matches directly
    /// - Team-owned: joins through `teams.org_id`
    /// - Project-owned: joins through `projects.org_id`
    /// - User-owned: joins through `org_memberships`
    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Skill>>;

    /// List skills by owner. Results populate `files_manifest` (not `files`).
    async fn list_by_owner(
        &self,
        owner_type: SkillOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Skill>>;

    /// List all skills accessible within an organization.
    ///
    /// Returns skills from all scopes within the org:
    /// - Organization-owned (owner_id = org_id)
    /// - Team-owned (team belongs to org)
    /// - Project-owned (project belongs to org)
    /// - User-owned (user is a member of org)
    async fn list_by_org(&self, org_id: Uuid, params: ListParams) -> DbResult<ListResult<Skill>>;

    /// Count skills by owner.
    async fn count_by_owner(
        &self,
        owner_type: SkillOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64>;

    /// Update a skill. When `input.files` is `Some(_)`, the full file set is
    /// replaced (existing rows in `skill_files` are removed, then the new
    /// set is inserted) and `total_bytes` is recomputed.
    async fn update(&self, id: Uuid, input: UpdateSkill) -> DbResult<Skill>;

    /// Soft-delete a skill.
    async fn delete(&self, id: Uuid) -> DbResult<()>;
}
