//! Repository for SCIM user mappings.

use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreateScimUserMapping, ScimUserMapping, ScimUserWithMapping, UpdateScimUserMapping},
    scim::filter_to_sql::SqlFilter,
};

/// Repository for SCIM user mappings.
///
/// SCIM user mappings link SCIM external IDs (from the IdP) to Hadrian users.
/// This enables:
/// - Looking up Hadrian users by SCIM ID during provisioning
/// - Tracking SCIM "active" status separately from user existence
/// - Supporting the same user in multiple organizations with different SCIM IDs
#[async_trait]
pub trait ScimUserMappingRepo: Send + Sync {
    /// Create a new SCIM user mapping.
    ///
    /// # Arguments
    /// * `org_id` - The organization this mapping belongs to
    /// * `input` - The mapping details
    ///
    /// # Errors
    /// Returns an error if a mapping with the same (org_id, scim_external_id) already exists.
    async fn create(&self, org_id: Uuid, input: CreateScimUserMapping)
    -> DbResult<ScimUserMapping>;

    /// Get a mapping by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ScimUserMapping>>;

    /// Get a mapping by SCIM external ID within an organization.
    ///
    /// This is the primary lookup method during SCIM provisioning.
    async fn get_by_scim_external_id(
        &self,
        org_id: Uuid,
        scim_external_id: &str,
    ) -> DbResult<Option<ScimUserMapping>>;

    /// Get a mapping by user ID within an organization.
    ///
    /// Used to check if a Hadrian user has a SCIM mapping in a given org.
    async fn get_by_user_id(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> DbResult<Option<ScimUserMapping>>;

    /// List all SCIM user mappings for an organization.
    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ScimUserMapping>>;

    /// List SCIM user mappings with optional SQL filter and offset pagination.
    ///
    /// Returns `(items, total_count)` where `total_count` is the count of records
    /// matching the filter (not the total in the org).
    ///
    /// This method is optimized for SCIM list operations:
    /// - Single query with JOIN to users table (no N+1)
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
    ) -> DbResult<(Vec<ScimUserWithMapping>, i64)>;

    /// List all SCIM user mappings for a user across all organizations.
    ///
    /// Used for GDPR data export.
    async fn list_by_user(&self, user_id: Uuid) -> DbResult<Vec<ScimUserMapping>>;

    /// Update a SCIM user mapping.
    async fn update(&self, id: Uuid, input: UpdateScimUserMapping) -> DbResult<ScimUserMapping>;

    /// Update the active status of a mapping.
    ///
    /// This is the most common update during SCIM provisioning (activate/deactivate user).
    async fn set_active(&self, id: Uuid, active: bool) -> DbResult<ScimUserMapping>;

    /// Delete a SCIM user mapping (hard delete).
    async fn delete(&self, id: Uuid) -> DbResult<()>;

    /// Delete all SCIM user mappings for a user.
    ///
    /// Used when a user is deleted from Hadrian.
    async fn delete_by_user(&self, user_id: Uuid) -> DbResult<u64>;

    /// Count SCIM user mappings for an organization.
    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64>;
}
