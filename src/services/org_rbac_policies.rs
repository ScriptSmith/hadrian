use std::sync::Arc;

use thiserror::Error;
use uuid::Uuid;

use crate::{
    authz::{AuthzEngine, PolicyRegistry},
    db::{DbPool, DbResult, ListParams, ListResult},
    models::{
        CreateOrgRbacPolicy, OrgRbacPolicy, OrgRbacPolicyVersion, RollbackOrgRbacPolicy,
        UpdateOrgRbacPolicy,
    },
};

/// Service layer for organization RBAC policy operations.
///
/// This service handles CRUD operations for per-organization RBAC policies,
/// including CEL expression validation and version management.
#[derive(Clone)]
pub struct OrgRbacPolicyService {
    db: Arc<DbPool>,
    max_expression_length: usize,
}

impl OrgRbacPolicyService {
    pub fn new(db: Arc<DbPool>, max_expression_length: usize) -> Self {
        Self {
            db,
            max_expression_length,
        }
    }

    /// Create a new RBAC policy for an organization.
    ///
    /// The CEL condition is validated before the policy is created.
    ///
    /// # Arguments
    /// * `org_id` - The organization this policy belongs to
    /// * `input` - The policy configuration
    /// * `created_by` - User who created the policy (for version history)
    ///
    /// # Errors
    /// Returns an error if:
    /// - The CEL condition is invalid
    /// - A policy with the same name already exists in the org
    pub async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgRbacPolicy,
        created_by: Option<Uuid>,
    ) -> Result<OrgRbacPolicy, OrgRbacPolicyError> {
        // Validate the CEL expression (with length limit) before saving
        AuthzEngine::validate_expression_with_max_length(
            &input.condition,
            self.max_expression_length,
        )?;

        let policy = self
            .db
            .org_rbac_policies()
            .create(org_id, input, created_by)
            .await?;

        Ok(policy)
    }

    /// Get a policy by its ID.
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgRbacPolicy>> {
        self.db.org_rbac_policies().get_by_id(id).await
    }

    /// Get a policy by organization ID and name.
    ///
    /// Policy names are unique within an organization.
    pub async fn get_by_org_and_name(
        &self,
        org_id: Uuid,
        name: &str,
    ) -> DbResult<Option<OrgRbacPolicy>> {
        self.db
            .org_rbac_policies()
            .get_by_org_and_name(org_id, name)
            .await
    }

    /// List all policies for an organization.
    ///
    /// Returns policies ordered by priority (highest first).
    /// Used by policy simulation which needs priority ordering.
    pub async fn list_by_org(&self, org_id: Uuid) -> DbResult<Vec<OrgRbacPolicy>> {
        self.db.org_rbac_policies().list_by_org(org_id).await
    }

    /// List policies for an organization with cursor-based pagination.
    ///
    /// Returns policies ordered by created_at DESC, id DESC for stable cursor pagination.
    /// Use this for the admin API list endpoint.
    pub async fn list_by_org_paginated(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<OrgRbacPolicy>> {
        self.db
            .org_rbac_policies()
            .list_by_org_paginated(org_id, params)
            .await
    }

    /// List only enabled policies for an organization.
    ///
    /// Returns policies ordered by priority (highest first).
    /// This is the primary method used during authorization evaluation.
    pub async fn list_enabled_by_org(&self, org_id: Uuid) -> DbResult<Vec<OrgRbacPolicy>> {
        self.db
            .org_rbac_policies()
            .list_enabled_by_org(org_id)
            .await
    }

    /// List all enabled policies across all organizations.
    ///
    /// Returns policies ordered by org_id, then priority (highest first).
    /// This is used during registry initialization at startup.
    pub async fn list_all_enabled(&self) -> DbResult<Vec<OrgRbacPolicy>> {
        self.db.org_rbac_policies().list_all_enabled().await
    }

    /// Update a policy.
    ///
    /// If the condition is being updated, it is validated before saving.
    /// This increments the policy version and creates a version history record.
    ///
    /// # Arguments
    /// * `id` - The policy ID
    /// * `input` - The fields to update
    /// * `updated_by` - User who updated the policy (for version history)
    ///
    /// # Errors
    /// Returns an error if:
    /// - The policy doesn't exist
    /// - The new CEL condition is invalid
    /// - Name conflicts with another policy in the same org
    pub async fn update(
        &self,
        id: Uuid,
        input: UpdateOrgRbacPolicy,
        updated_by: Option<Uuid>,
    ) -> Result<OrgRbacPolicy, OrgRbacPolicyError> {
        // Validate the CEL expression (with length limit) if it's being updated
        if let Some(ref condition) = input.condition {
            AuthzEngine::validate_expression_with_max_length(
                condition,
                self.max_expression_length,
            )?;
        }

        let policy = self
            .db
            .org_rbac_policies()
            .update(id, input, updated_by)
            .await?;

        Ok(policy)
    }

    /// Delete a policy (hard delete).
    ///
    /// This also deletes all version history for the policy.
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.org_rbac_policies().delete(id).await
    }

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
    /// # Errors
    /// Returns an error if:
    /// - The policy or target version doesn't exist
    /// - The target version's CEL condition is invalid
    pub async fn rollback(
        &self,
        id: Uuid,
        input: RollbackOrgRbacPolicy,
        rolled_back_by: Option<Uuid>,
    ) -> Result<OrgRbacPolicy, OrgRbacPolicyError> {
        // Fetch the target version and validate its CEL expression
        // This protects against CEL engine changes that could invalidate old expressions
        let target_version = self
            .db
            .org_rbac_policies()
            .get_version(id, input.target_version)
            .await?
            .ok_or(OrgRbacPolicyError::NotFound)?;

        AuthzEngine::validate_expression_with_max_length(
            &target_version.condition,
            self.max_expression_length,
        )?;

        let policy = self
            .db
            .org_rbac_policies()
            .rollback(id, input, rolled_back_by)
            .await?;

        Ok(policy)
    }

    /// Get a specific version of a policy.
    pub async fn get_version(
        &self,
        policy_id: Uuid,
        version: i32,
    ) -> DbResult<Option<OrgRbacPolicyVersion>> {
        self.db
            .org_rbac_policies()
            .get_version(policy_id, version)
            .await
    }

    /// List all versions of a policy.
    ///
    /// Returns versions ordered by version number descending (newest first).
    pub async fn list_versions(&self, policy_id: Uuid) -> DbResult<Vec<OrgRbacPolicyVersion>> {
        self.db.org_rbac_policies().list_versions(policy_id).await
    }

    /// List versions of a policy with offset-based pagination.
    ///
    /// Returns versions ordered by version number descending (newest first).
    /// Deprecated: Use `list_versions_cursor` for cursor-based pagination.
    pub async fn list_versions_paginated(
        &self,
        policy_id: Uuid,
        limit: u32,
        offset: u32,
    ) -> DbResult<Vec<OrgRbacPolicyVersion>> {
        self.db
            .org_rbac_policies()
            .list_versions_paginated(policy_id, limit, offset)
            .await
    }

    /// List versions of a policy with cursor-based pagination.
    ///
    /// Returns versions ordered by created_at DESC, id DESC for stable cursor pagination.
    pub async fn list_versions_cursor(
        &self,
        policy_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<OrgRbacPolicyVersion>> {
        self.db
            .org_rbac_policies()
            .list_versions_cursor(policy_id, params)
            .await
    }

    /// Count the number of versions for a policy.
    ///
    /// More efficient than `list_versions().len()` for pagination metadata.
    pub async fn count_versions(&self, policy_id: Uuid) -> DbResult<i64> {
        self.db.org_rbac_policies().count_versions(policy_id).await
    }

    /// Count the number of policies for an organization.
    ///
    /// Used for enforcing policy count limits per organization.
    pub async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        self.db.org_rbac_policies().count_by_org(org_id).await
    }

    /// Refresh the policy registry cache for an organization.
    ///
    /// Call this after creating, updating, or deleting policies to ensure
    /// the in-memory cache reflects the latest database state.
    ///
    /// If no registry is provided (e.g., RBAC is disabled), this is a no-op.
    pub async fn refresh_registry(
        &self,
        org_id: Uuid,
        registry: Option<&PolicyRegistry>,
    ) -> Result<(), OrgRbacPolicyError> {
        let Some(registry) = registry else {
            return Ok(());
        };

        let policies = self.list_enabled_by_org(org_id).await?;
        registry
            .refresh_org_policies(org_id, policies)
            .await
            .map_err(|e| OrgRbacPolicyError::RegistryRefresh(e.to_string()))?;

        Ok(())
    }

    /// Remove an organization from the policy registry cache.
    ///
    /// Call this when an organization is deleted or all its policies are removed.
    ///
    /// If no registry is provided (e.g., RBAC is disabled), this is a no-op.
    pub async fn remove_org_from_registry(&self, org_id: Uuid, registry: Option<&PolicyRegistry>) {
        if let Some(registry) = registry {
            registry.remove_org(org_id).await;
        }
    }
}

/// Errors that can occur during RBAC policy operations.
#[derive(Debug, Error)]
pub enum OrgRbacPolicyError {
    #[error("Database error: {0}")]
    Database(#[from] crate::db::DbError),

    #[error("Invalid CEL expression: {0}")]
    InvalidCondition(String),

    #[error("RBAC policy not found")]
    NotFound,

    #[error("Failed to refresh policy registry: {0}")]
    RegistryRefresh(String),
}

impl From<crate::authz::AuthzError> for OrgRbacPolicyError {
    fn from(e: crate::authz::AuthzError) -> Self {
        OrgRbacPolicyError::InvalidCondition(e.to_string())
    }
}
