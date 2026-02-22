use std::sync::Arc;

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams, ListResult},
    models::{ApiKey, ApiKeyWithOwner, CreateApiKey, CreatedApiKey, generate_api_key_with_prefix},
};

/// Service layer for API key operations
#[derive(Clone)]
pub struct ApiKeyService {
    db: Arc<DbPool>,
}

impl ApiKeyService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new API key with the given prefix
    /// Returns both the stored key and the raw key (only shown once)
    pub async fn create(&self, input: CreateApiKey, prefix: &str) -> DbResult<CreatedApiKey> {
        let (raw_key, key_hash) = generate_api_key_with_prefix(prefix);
        let api_key = self.db.api_keys().create(input, &key_hash).await?;
        Ok(CreatedApiKey {
            api_key,
            key: raw_key,
        })
    }

    /// Get API key by ID (without the raw key)
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ApiKey>> {
        self.db.api_keys().get_by_id(id).await
    }

    /// Get API key by hash (for authentication)
    pub async fn get_by_hash(&self, key_hash: &str) -> DbResult<Option<ApiKeyWithOwner>> {
        self.db.api_keys().get_by_hash(key_hash).await
    }

    /// List API keys for an organization
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ApiKey>> {
        self.db.api_keys().list_by_org(org_id, params).await
    }

    /// List API keys for a project
    pub async fn list_by_project(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ApiKey>> {
        self.db.api_keys().list_by_project(project_id, params).await
    }

    /// List API keys for a user
    pub async fn list_by_user(
        &self,
        user_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ApiKey>> {
        self.db.api_keys().list_by_user(user_id, params).await
    }

    /// Count API keys for an organization
    pub async fn count_by_org(&self, org_id: Uuid, include_revoked: bool) -> DbResult<i64> {
        self.db
            .api_keys()
            .count_by_org(org_id, include_revoked)
            .await
    }

    /// Count API keys for a project
    pub async fn count_by_project(&self, project_id: Uuid, include_revoked: bool) -> DbResult<i64> {
        self.db
            .api_keys()
            .count_by_project(project_id, include_revoked)
            .await
    }

    /// Count API keys for a user
    pub async fn count_by_user(&self, user_id: Uuid, include_revoked: bool) -> DbResult<i64> {
        self.db
            .api_keys()
            .count_by_user(user_id, include_revoked)
            .await
    }

    /// Revoke an API key
    pub async fn revoke(&self, id: Uuid) -> DbResult<()> {
        self.db.api_keys().revoke(id).await
    }

    /// Update the last used timestamp for an API key
    pub async fn update_last_used(&self, id: Uuid) -> DbResult<()> {
        self.db.api_keys().update_last_used(id).await
    }

    /// Revoke all active API keys owned by a user.
    ///
    /// Used by SCIM deprovisioning to revoke all API keys when a user is deactivated.
    /// Returns the number of keys that were revoked.
    pub async fn revoke_by_user(&self, user_id: Uuid) -> DbResult<u64> {
        self.db.api_keys().revoke_by_user(user_id).await
    }

    /// List API keys for a service account
    pub async fn list_by_service_account(
        &self,
        service_account_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ApiKey>> {
        self.db
            .api_keys()
            .list_by_service_account(service_account_id, params)
            .await
    }

    /// Count API keys for a service account
    pub async fn count_by_service_account(
        &self,
        service_account_id: Uuid,
        include_revoked: bool,
    ) -> DbResult<i64> {
        self.db
            .api_keys()
            .count_by_service_account(service_account_id, include_revoked)
            .await
    }

    /// Revoke all active API keys owned by a service account.
    ///
    /// Used when deleting a service account to clean up its API keys.
    /// Returns the number of keys that were revoked.
    pub async fn revoke_by_service_account(&self, service_account_id: Uuid) -> DbResult<u64> {
        self.db
            .api_keys()
            .revoke_by_service_account(service_account_id)
            .await
    }

    /// Get the key hashes for all active API keys owned by a service account.
    ///
    /// Used for cache invalidation when service account roles are updated.
    pub async fn get_key_hashes_by_service_account(
        &self,
        service_account_id: Uuid,
    ) -> DbResult<Vec<String>> {
        self.db
            .api_keys()
            .get_key_hashes_by_service_account(service_account_id)
            .await
    }

    /// Get the key hashes for all active API keys owned by a user.
    ///
    /// Used for cache invalidation when a user is removed from an organization.
    pub async fn get_key_hashes_by_user(&self, user_id: Uuid) -> DbResult<Vec<String>> {
        self.db.api_keys().get_key_hashes_by_user(user_id).await
    }

    /// Rotate an API key: create a new key with the same settings and set a grace period on the old key.
    ///
    /// During the grace period, both the old and new keys are valid.
    /// After the grace period expires, only the new key works.
    ///
    /// Returns the new API key with the raw key value (only shown once).
    pub async fn rotate(
        &self,
        old_key_id: Uuid,
        grace_period_seconds: u64,
        prefix: &str,
    ) -> DbResult<CreatedApiKey> {
        // Get the old key to copy its settings
        let old_key = self
            .db
            .api_keys()
            .get_by_id(old_key_id)
            .await?
            .ok_or(crate::db::DbError::NotFound)?;

        // Validate the old key is not already revoked
        if old_key.revoked_at.is_some() {
            return Err(crate::db::DbError::Conflict(
                "Cannot rotate a revoked API key".to_string(),
            ));
        }

        // Validate the old key is not already being rotated
        if old_key.rotation_grace_until.is_some() {
            return Err(crate::db::DbError::Conflict(
                "API key is already being rotated".to_string(),
            ));
        }

        // Calculate grace period end time
        let grace_until = Utc::now() + Duration::seconds(grace_period_seconds as i64);

        // Create the new key input with the same settings
        let new_key_input = CreateApiKey {
            name: format!("{} (rotated)", old_key.name),
            owner: old_key.owner,
            budget_limit_cents: old_key.budget_limit_cents,
            budget_period: old_key.budget_period,
            expires_at: old_key.expires_at,
            scopes: old_key.scopes,
            allowed_models: old_key.allowed_models,
            ip_allowlist: old_key.ip_allowlist,
            rate_limit_rpm: old_key.rate_limit_rpm,
            rate_limit_tpm: old_key.rate_limit_tpm,
        };

        // Generate new key
        let (raw_key, key_hash) = generate_api_key_with_prefix(prefix);

        // Perform the rotation
        let new_key = self
            .db
            .api_keys()
            .rotate(old_key_id, new_key_input, &key_hash, grace_until)
            .await?;

        Ok(CreatedApiKey {
            api_key: new_key,
            key: raw_key,
        })
    }
}
