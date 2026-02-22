use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{
        CreateOrgRbacPolicy, OrgRbacPolicy, OrgRbacPolicyVersion, RollbackOrgRbacPolicy,
        UpdateOrgRbacPolicy,
    },
};

/// Repository for organization RBAC policies.
///
/// Organization RBAC policies enable runtime management of authorization rules.
/// Each organization can define their own CEL-based policies that are evaluated
/// during authorization decisions.
///
/// Key features:
/// - Soft delete: Policies are soft-deleted (deleted_at timestamp) to preserve audit trails
/// - Version history: Every update creates a version record for audit and rollback
/// - Priority ordering: Policies are evaluated in priority order (highest first)
/// - CEL conditions: Policies use CEL expressions for flexible rule evaluation
///
/// All query methods (get, list, count) automatically exclude soft-deleted policies.
/// Delete operations set `deleted_at` rather than removing rows, preserving version history.
#[async_trait]
pub trait OrgRbacPolicyRepo: Send + Sync {
    // =========================================================================
    // Policy CRUD Operations
    // =========================================================================

    /// Create a new RBAC policy for an organization.
    ///
    /// # Arguments
    /// * `org_id` - The organization this policy belongs to
    /// * `input` - The policy configuration
    /// * `created_by` - User who created the policy (for version history)
    ///
    /// # Returns
    /// The created policy with version 1.
    ///
    /// # Errors
    /// Returns an error if a policy with the same name already exists in the org.
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgRbacPolicy,
        created_by: Option<Uuid>,
    ) -> DbResult<OrgRbacPolicy>;

    /// Get a policy by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgRbacPolicy>>;

    /// Get a policy by organization ID and name.
    ///
    /// Policy names are unique within an organization.
    async fn get_by_org_and_name(
        &self,
        org_id: Uuid,
        name: &str,
    ) -> DbResult<Option<OrgRbacPolicy>>;

    /// List all policies for an organization.
    ///
    /// Returns policies ordered by priority (highest first).
    /// Used by policy simulation which needs priority ordering.
    async fn list_by_org(&self, org_id: Uuid) -> DbResult<Vec<OrgRbacPolicy>>;

    /// List policies for an organization with cursor-based pagination.
    ///
    /// Returns policies ordered by created_at DESC, id DESC for stable cursor pagination.
    /// Use this for the admin API list endpoint.
    async fn list_by_org_paginated(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<OrgRbacPolicy>>;

    /// List only enabled policies for an organization.
    ///
    /// Returns policies ordered by priority (highest first).
    /// This is the primary method used during authorization evaluation.
    async fn list_enabled_by_org(&self, org_id: Uuid) -> DbResult<Vec<OrgRbacPolicy>>;

    /// List all enabled policies across all organizations.
    ///
    /// Returns policies ordered by org_id, then priority (highest first).
    /// This is used during registry initialization at startup.
    async fn list_all_enabled(&self) -> DbResult<Vec<OrgRbacPolicy>>;

    /// Update a policy.
    ///
    /// This increments the policy version and creates a version history record.
    ///
    /// # Arguments
    /// * `id` - The policy ID
    /// * `input` - The fields to update
    /// * `updated_by` - User who updated the policy (for version history)
    ///
    /// # Returns
    /// The updated policy with incremented version.
    ///
    /// # Errors
    /// Returns an error if the policy doesn't exist or name conflicts.
    async fn update(
        &self,
        id: Uuid,
        input: UpdateOrgRbacPolicy,
        updated_by: Option<Uuid>,
    ) -> DbResult<OrgRbacPolicy>;

    /// Soft-delete a policy by setting its `deleted_at` timestamp.
    ///
    /// The policy and its version history are preserved for audit purposes.
    /// The policy name becomes available for reuse in the same organization.
    /// Attempting to delete an already-deleted policy returns NotFound.
    async fn delete(&self, id: Uuid) -> DbResult<()>;

    /// Rollback a policy to a previous version.
    ///
    /// This creates a new version with the contents of the target version.
    /// The version number is incremented (not reverted to the target version).
    ///
    /// # Arguments
    /// * `id` - The policy ID
    /// * `input` - Rollback request with target version and reason
    /// * `rolled_back_by` - User who performed the rollback
    ///
    /// # Returns
    /// The policy with incremented version containing the rolled-back content.
    ///
    /// # Errors
    /// Returns an error if the policy or target version doesn't exist.
    async fn rollback(
        &self,
        id: Uuid,
        input: RollbackOrgRbacPolicy,
        rolled_back_by: Option<Uuid>,
    ) -> DbResult<OrgRbacPolicy>;

    // =========================================================================
    // Version History Operations
    // =========================================================================

    /// Get a specific version of a policy.
    async fn get_version(
        &self,
        policy_id: Uuid,
        version: i32,
    ) -> DbResult<Option<OrgRbacPolicyVersion>>;

    /// List all versions of a policy.
    ///
    /// Returns versions ordered by version number descending (newest first).
    async fn list_versions(&self, policy_id: Uuid) -> DbResult<Vec<OrgRbacPolicyVersion>>;

    /// List versions of a policy with offset-based pagination.
    ///
    /// Returns versions ordered by version number descending (newest first).
    /// Deprecated: Use `list_versions_cursor` for cursor-based pagination.
    async fn list_versions_paginated(
        &self,
        policy_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> DbResult<Vec<OrgRbacPolicyVersion>>;

    /// List versions of a policy with cursor-based pagination.
    ///
    /// Returns versions ordered by created_at DESC, id DESC for stable cursor pagination.
    async fn list_versions_cursor(
        &self,
        policy_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<OrgRbacPolicyVersion>>;

    /// Count the number of versions for a policy.
    ///
    /// More efficient than `list_versions().len()` for pagination metadata.
    async fn count_versions(&self, policy_id: Uuid) -> DbResult<i64>;

    /// Count the number of policies for an organization.
    ///
    /// Used for enforcing policy count limits per organization.
    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64>;

    /// Count the total number of policies across all organizations.
    ///
    /// Used at startup to detect configuration mismatches (e.g., RBAC disabled
    /// but org policies exist in the database).
    async fn count_all(&self) -> DbResult<i64>;
}
