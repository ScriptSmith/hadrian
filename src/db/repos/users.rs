use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{
        CreateUser, MembershipSource, TeamMembership, UpdateUser, User, UserOrgMembership,
        UserProjectMembership,
    },
};

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn create(&self, input: CreateUser) -> DbResult<User>;
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<User>>;
    async fn get_by_external_id(&self, external_id: &str) -> DbResult<Option<User>>;
    async fn list(&self, params: ListParams) -> DbResult<ListResult<User>>;
    async fn count(&self, include_deleted: bool) -> DbResult<i64>;
    async fn update(&self, id: Uuid, input: UpdateUser) -> DbResult<User>;

    // Organization memberships
    async fn add_to_org(
        &self,
        user_id: Uuid,
        org_id: Uuid,
        role: &str,
        source: MembershipSource,
    ) -> DbResult<()>;
    async fn update_org_member_role(&self, user_id: Uuid, org_id: Uuid, role: &str)
    -> DbResult<()>;
    async fn remove_from_org(&self, user_id: Uuid, org_id: Uuid) -> DbResult<()>;
    /// Remove all org memberships for a user with a specific source.
    /// Used by sync_memberships_on_login to remove JIT memberships not in current groups.
    async fn remove_org_memberships_by_source(
        &self,
        user_id: Uuid,
        source: MembershipSource,
        except_org_ids: &[Uuid],
    ) -> DbResult<u64>;
    async fn list_org_members(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<User>>;
    async fn count_org_members(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64>;

    // Project memberships
    async fn add_to_project(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        role: &str,
        source: MembershipSource,
    ) -> DbResult<()>;
    async fn update_project_member_role(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        role: &str,
    ) -> DbResult<()>;
    async fn remove_from_project(&self, user_id: Uuid, project_id: Uuid) -> DbResult<()>;
    /// Remove all project memberships for a user with a specific source.
    /// Used by sync_memberships_on_login to remove JIT memberships not in current groups.
    async fn remove_project_memberships_by_source(
        &self,
        user_id: Uuid,
        source: MembershipSource,
        except_project_ids: &[Uuid],
    ) -> DbResult<u64>;
    async fn list_project_members(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<User>>;
    async fn count_project_members(&self, project_id: Uuid, include_deleted: bool)
    -> DbResult<i64>;

    // ==================== GDPR Export Methods ====================

    /// Get all organization memberships for a user (for GDPR data export)
    async fn get_org_memberships_for_user(&self, user_id: Uuid)
    -> DbResult<Vec<UserOrgMembership>>;

    /// Get all project memberships for a user (for GDPR data export)
    async fn get_project_memberships_for_user(
        &self,
        user_id: Uuid,
    ) -> DbResult<Vec<UserProjectMembership>>;

    /// Get all team memberships for a user (for GDPR data export)
    async fn get_team_memberships_for_user(&self, user_id: Uuid) -> DbResult<Vec<TeamMembership>>;

    // ==================== GDPR Deletion Methods ====================

    /// Hard delete a user and all associated data (GDPR Article 17 - Right to Erasure)
    ///
    /// This permanently deletes:
    /// - User record
    /// - Organization memberships (via CASCADE)
    /// - Project memberships (via CASCADE)
    /// - API keys owned by the user
    /// - Conversations owned by the user
    /// - Dynamic providers owned by the user
    /// - Usage records for user's API keys
    ///
    /// Returns the number of deleted records across all tables.
    async fn hard_delete(&self, user_id: Uuid) -> DbResult<UserDeletionResult>;
}

/// Result of a user deletion operation (GDPR erasure)
#[derive(Debug, Clone, Default)]
pub struct UserDeletionResult {
    /// Number of API keys deleted
    pub api_keys_deleted: u64,
    /// Number of conversations deleted
    pub conversations_deleted: u64,
    /// Number of dynamic providers deleted
    pub dynamic_providers_deleted: u64,
    /// Number of usage records deleted
    pub usage_records_deleted: u64,
    /// Whether the user record was deleted
    pub user_deleted: bool,
}
