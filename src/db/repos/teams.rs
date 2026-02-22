use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{
        AddTeamMember, CreateTeam, MembershipSource, Team, TeamMember, UpdateTeam, UpdateTeamMember,
    },
};

#[async_trait]
pub trait TeamRepo: Send + Sync {
    /// Create a new team within an organization.
    async fn create(&self, org_id: Uuid, input: CreateTeam) -> DbResult<Team>;

    /// Get a team by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Team>>;

    /// Get multiple teams by their IDs in a single query.
    /// Returns teams in no particular order. Missing IDs are silently ignored.
    async fn get_by_ids(&self, ids: &[Uuid]) -> DbResult<Vec<Team>>;

    /// Get a team by its slug within an organization.
    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<Team>>;

    /// List all teams in an organization.
    async fn list_by_org(&self, org_id: Uuid, params: ListParams) -> DbResult<ListResult<Team>>;

    /// Count teams in an organization.
    async fn count_by_org(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64>;

    /// Update a team's details.
    async fn update(&self, id: Uuid, input: UpdateTeam) -> DbResult<Team>;

    /// Soft-delete a team.
    async fn delete(&self, id: Uuid) -> DbResult<()>;

    // ========================================================================
    // Team membership operations
    // ========================================================================

    /// Add a user to a team.
    async fn add_member(&self, team_id: Uuid, input: AddTeamMember) -> DbResult<TeamMember>;

    /// Remove a user from a team.
    async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<()>;

    /// Remove all team memberships for a user with a specific source.
    /// Used by sync_memberships_on_login to remove JIT memberships not in current groups.
    async fn remove_memberships_by_source(
        &self,
        user_id: Uuid,
        source: MembershipSource,
        except_team_ids: &[Uuid],
    ) -> DbResult<u64>;

    /// Update a team member's role.
    async fn update_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        input: UpdateTeamMember,
    ) -> DbResult<TeamMember>;

    /// List all members of a team.
    async fn list_members(
        &self,
        team_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<TeamMember>>;

    /// Get a specific member of a team.
    async fn get_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<Option<TeamMember>>;

    /// Check if a user is a member of a team.
    async fn is_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<bool>;

    /// Count members in a team.
    async fn count_members(&self, team_id: Uuid) -> DbResult<i64>;
}
