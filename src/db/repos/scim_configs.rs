//! Repository for organization SCIM configurations.

use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    db::error::DbResult,
    models::{CreateOrgScimConfig, OrgScimConfig, OrgScimConfigWithHash, UpdateOrgScimConfig},
};

/// Repository for organization SCIM configurations.
///
/// Organization SCIM configs enable automatic user provisioning and deprovisioning
/// from identity providers. Each organization can have at most one SCIM config.
///
/// Note: The token_hash is stored in the database, not in a secret manager
/// (unlike SSO client secrets), because SCIM tokens need fast lookup for
/// every provisioning request.
#[async_trait]
pub trait OrgScimConfigRepo: Send + Sync {
    /// Create a new SCIM configuration for an organization.
    ///
    /// # Arguments
    /// * `org_id` - The organization this SCIM config belongs to
    /// * `input` - The SCIM configuration details
    /// * `token_hash` - SHA-256 hash of the bearer token
    /// * `token_prefix` - First 8 characters of the token for identification
    ///
    /// # Errors
    /// Returns an error if the organization already has a SCIM config (one per org).
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgScimConfig,
        token_hash: &str,
        token_prefix: &str,
    ) -> DbResult<OrgScimConfig>;

    /// Get a SCIM configuration by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgScimConfig>>;

    /// Get a SCIM configuration by organization ID.
    ///
    /// This is the primary lookup method - each organization has at most one SCIM config.
    async fn get_by_org_id(&self, org_id: Uuid) -> DbResult<Option<OrgScimConfig>>;

    /// Get a SCIM configuration with its token hash.
    ///
    /// Used for token authentication.
    async fn get_with_hash_by_org_id(
        &self,
        org_id: Uuid,
    ) -> DbResult<Option<OrgScimConfigWithHash>>;

    /// Get a SCIM configuration by token hash.
    ///
    /// Used for bearer token authentication during SCIM requests.
    /// Returns the config and its associated organization.
    async fn get_by_token_hash(&self, token_hash: &str) -> DbResult<Option<OrgScimConfigWithHash>>;

    /// Update a SCIM configuration.
    ///
    /// # Arguments
    /// * `id` - The SCIM config ID
    /// * `input` - The fields to update
    async fn update(&self, id: Uuid, input: UpdateOrgScimConfig) -> DbResult<OrgScimConfig>;

    /// Update the token for a SCIM configuration (token rotation).
    ///
    /// # Arguments
    /// * `id` - The SCIM config ID
    /// * `token_hash` - New SHA-256 hash of the bearer token
    /// * `token_prefix` - New token prefix (first 8 characters)
    async fn rotate_token(
        &self,
        id: Uuid,
        token_hash: &str,
        token_prefix: &str,
    ) -> DbResult<OrgScimConfig>;

    /// Update the last_used_at timestamp when the token is used.
    async fn update_token_last_used(&self, id: Uuid) -> DbResult<()>;

    /// Delete a SCIM configuration (hard delete).
    async fn delete(&self, id: Uuid) -> DbResult<()>;

    /// List all enabled SCIM configurations.
    ///
    /// Used for startup validation and admin overview.
    async fn list_enabled(&self) -> DbResult<Vec<OrgScimConfig>>;
}
