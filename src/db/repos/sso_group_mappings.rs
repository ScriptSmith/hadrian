use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreateSsoGroupMapping, SsoGroupMapping, UpdateSsoGroupMapping},
};

/// Repository for SSO group mappings.
///
/// SSO group mappings define how IdP groups are mapped to Hadrian teams and roles
/// during JIT (Just-in-Time) provisioning. When a user logs in via SSO, their
/// IdP groups are looked up in this table to determine team memberships.
#[async_trait]
pub trait SsoGroupMappingRepo: Send + Sync {
    /// Create a new SSO group mapping.
    async fn create(&self, org_id: Uuid, input: CreateSsoGroupMapping)
    -> DbResult<SsoGroupMapping>;

    /// Get a mapping by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<SsoGroupMapping>>;

    /// List all mappings for an organization.
    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<SsoGroupMapping>>;

    /// List mappings for a specific SSO connection within an organization.
    async fn list_by_connection(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<SsoGroupMapping>>;

    /// Find all mappings that match any of the given IdP groups.
    ///
    /// This is the core method used during JIT provisioning to resolve a user's
    /// IdP groups to Hadrian team memberships.
    ///
    /// # Arguments
    /// * `sso_connection_name` - The SSO connection identifier
    /// * `org_id` - The organization to search within
    /// * `idp_groups` - List of IdP group names from the user's token
    ///
    /// # Returns
    /// All mappings where `idp_group` matches any of the provided group names.
    async fn find_mappings_for_groups(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        idp_groups: &[String],
    ) -> DbResult<Vec<SsoGroupMapping>>;

    /// Count mappings for an organization.
    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64>;

    /// Update a mapping.
    async fn update(&self, id: Uuid, input: UpdateSsoGroupMapping) -> DbResult<SsoGroupMapping>;

    /// Delete a mapping (hard delete - mappings don't use soft delete).
    async fn delete(&self, id: Uuid) -> DbResult<()>;

    /// Delete all mappings for a specific IdP group within an org/connection.
    ///
    /// Useful for bulk operations when an IdP group is removed or renamed.
    async fn delete_by_idp_group(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        idp_group: &str,
    ) -> DbResult<u64>;
}
