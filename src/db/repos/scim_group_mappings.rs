//! Repository for SCIM group mappings.

use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreateScimGroupMapping, ScimGroupMapping, ScimGroupWithTeam, UpdateScimGroupMapping},
    scim::filter_to_sql::SqlFilter,
};

/// Repository for SCIM group mappings.
///
/// SCIM group mappings link SCIM groups (from the IdP) to Hadrian teams.
/// When the IdP pushes group membership changes via SCIM, we update
/// team memberships in Hadrian accordingly.
#[async_trait]
pub trait ScimGroupMappingRepo: Send + Sync {
    /// Create a new SCIM group mapping.
    ///
    /// # Arguments
    /// * `org_id` - The organization this mapping belongs to
    /// * `input` - The mapping details
    ///
    /// # Errors
    /// Returns an error if a mapping with the same (org_id, scim_group_id) already exists.
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateScimGroupMapping,
    ) -> DbResult<ScimGroupMapping>;

    /// Get a mapping by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ScimGroupMapping>>;

    /// Get a mapping by SCIM group ID within an organization.
    ///
    /// This is the primary lookup method during SCIM group operations.
    async fn get_by_scim_group_id(
        &self,
        org_id: Uuid,
        scim_group_id: &str,
    ) -> DbResult<Option<ScimGroupMapping>>;

    /// Get a mapping by team ID within an organization.
    ///
    /// Used to check if a Hadrian team has a SCIM mapping.
    async fn get_by_team_id(
        &self,
        org_id: Uuid,
        team_id: Uuid,
    ) -> DbResult<Option<ScimGroupMapping>>;

    /// List all SCIM group mappings for an organization.
    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ScimGroupMapping>>;

    /// List SCIM group mappings with optional SQL filter and offset pagination.
    ///
    /// Returns `(items, total_count)` where `total_count` is the count of records
    /// matching the filter (not the total in the org).
    ///
    /// This method is optimized for SCIM list operations:
    /// - Single query with JOIN to teams table (no N+1)
    /// - Database-level filtering using SQL WHERE clause
    /// - Proper offset pagination for SCIM's 1-based `startIndex`
    ///
    /// # Arguments
    /// * `org_id` - The organization to list mappings for
    /// * `filter` - Optional SQL filter from `filter_to_sql()`
    /// * `limit` - Maximum number of results
    /// * `offset` - Number of results to skip (0-based)
    async fn list_by_org_filtered(
        &self,
        org_id: Uuid,
        filter: Option<&SqlFilter>,
        limit: i64,
        offset: i64,
    ) -> DbResult<(Vec<ScimGroupWithTeam>, i64)>;

    /// Update a SCIM group mapping.
    async fn update(&self, id: Uuid, input: UpdateScimGroupMapping) -> DbResult<ScimGroupMapping>;

    /// Delete a SCIM group mapping (hard delete).
    async fn delete(&self, id: Uuid) -> DbResult<()>;

    /// Delete all SCIM group mappings for a team.
    ///
    /// Used when a team is deleted from Hadrian.
    async fn delete_by_team(&self, team_id: Uuid) -> DbResult<u64>;

    /// Count SCIM group mappings for an organization.
    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64>;
}
