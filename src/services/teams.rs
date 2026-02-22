use std::sync::Arc;

use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams, repos::ListResult},
    models::{AddTeamMember, CreateTeam, Team, TeamMember, UpdateTeam, UpdateTeamMember},
};

/// Service layer for team operations
#[derive(Clone)]
pub struct TeamService {
    db: Arc<DbPool>,
}

impl TeamService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new team within an organization
    pub async fn create(&self, org_id: Uuid, input: CreateTeam) -> DbResult<Team> {
        self.db.teams().create(org_id, input).await
    }

    /// Get team by ID
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Team>> {
        self.db.teams().get_by_id(id).await
    }

    /// Get multiple teams by their IDs in a single query.
    /// Returns teams in no particular order. Missing IDs are silently ignored.
    pub async fn get_by_ids(&self, ids: &[Uuid]) -> DbResult<Vec<Team>> {
        self.db.teams().get_by_ids(ids).await
    }

    /// Get team by slug within an organization
    pub async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<Team>> {
        self.db.teams().get_by_slug(org_id, slug).await
    }

    /// List teams for an organization with pagination
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Team>> {
        self.db.teams().list_by_org(org_id, params).await
    }

    /// Count teams for an organization
    pub async fn count_by_org(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64> {
        self.db.teams().count_by_org(org_id, include_deleted).await
    }

    /// Update a team by ID
    pub async fn update(&self, id: Uuid, input: UpdateTeam) -> DbResult<Team> {
        self.db.teams().update(id, input).await
    }

    /// Delete (soft-delete) a team by ID
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.teams().delete(id).await
    }

    // ========================================================================
    // Team membership operations
    // ========================================================================

    /// Add a user to a team
    pub async fn add_member(&self, team_id: Uuid, input: AddTeamMember) -> DbResult<TeamMember> {
        self.db.teams().add_member(team_id, input).await
    }

    /// Remove a user from a team
    pub async fn remove_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<()> {
        self.db.teams().remove_member(team_id, user_id).await
    }

    /// Update a team member's role
    pub async fn update_member_role(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        input: UpdateTeamMember,
    ) -> DbResult<TeamMember> {
        self.db
            .teams()
            .update_member_role(team_id, user_id, input)
            .await
    }

    /// List all members of a team
    pub async fn list_members(
        &self,
        team_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<TeamMember>> {
        self.db.teams().list_members(team_id, params).await
    }

    /// Get a specific member of a team
    pub async fn get_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<Option<TeamMember>> {
        self.db.teams().get_member(team_id, user_id).await
    }

    /// Check if a user is a member of a team
    pub async fn is_member(&self, team_id: Uuid, user_id: Uuid) -> DbResult<bool> {
        self.db.teams().is_member(team_id, user_id).await
    }

    /// Count members in a team
    pub async fn count_members(&self, team_id: Uuid) -> DbResult<i64> {
        self.db.teams().count_members(team_id).await
    }
}
