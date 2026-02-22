use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{ApiKey, ApiKeyWithOwner, CachedApiKey, CreateApiKey},
};

#[async_trait]
pub trait ApiKeyRepo: Send + Sync {
    async fn create(&self, input: CreateApiKey, key_hash: &str) -> DbResult<ApiKey>;
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ApiKey>>;
    async fn get_by_hash(&self, key_hash: &str) -> DbResult<Option<ApiKeyWithOwner>>;
    async fn list_by_org(&self, org_id: Uuid, params: ListParams) -> DbResult<ListResult<ApiKey>>;
    async fn count_by_org(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64>;
    async fn list_by_team(&self, team_id: Uuid, params: ListParams)
    -> DbResult<ListResult<ApiKey>>;
    async fn count_by_team(&self, team_id: Uuid, include_deleted: bool) -> DbResult<i64>;
    async fn list_by_project(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ApiKey>>;
    async fn count_by_project(&self, project_id: Uuid, include_deleted: bool) -> DbResult<i64>;
    async fn list_by_user(&self, user_id: Uuid, params: ListParams)
    -> DbResult<ListResult<ApiKey>>;
    async fn count_by_user(&self, user_id: Uuid, include_deleted: bool) -> DbResult<i64>;
    async fn revoke(&self, id: Uuid) -> DbResult<()>;
    async fn update_last_used(&self, id: Uuid) -> DbResult<()>;

    /// Revoke all active API keys owned by a user.
    ///
    /// Used by SCIM deprovisioning to revoke all API keys when a user is deactivated.
    /// Returns the number of keys that were revoked.
    async fn revoke_by_user(&self, user_id: Uuid) -> DbResult<u64>;

    /// List API keys owned by a service account
    async fn list_by_service_account(
        &self,
        service_account_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ApiKey>>;

    /// Count API keys owned by a service account.
    ///
    /// When `include_revoked` is false, only active (non-revoked) keys are counted.
    async fn count_by_service_account(
        &self,
        service_account_id: Uuid,
        include_revoked: bool,
    ) -> DbResult<i64>;

    /// Revoke all active API keys owned by a service account.
    ///
    /// Used when deleting a service account to clean up its API keys.
    /// Returns the number of keys that were revoked.
    async fn revoke_by_service_account(&self, service_account_id: Uuid) -> DbResult<u64>;

    /// Rotate an API key: create new key and set grace period on old key.
    ///
    /// This operation:
    /// 1. Sets `rotation_grace_until` on the old key
    /// 2. Creates a new key with the same settings and `rotated_from_key_id` pointing to the old key
    ///
    /// Both keys remain valid during the grace period, after which the old key stops working.
    async fn rotate(
        &self,
        old_key_id: Uuid,
        new_key_input: CreateApiKey,
        new_key_hash: &str,
        grace_until: DateTime<Utc>,
    ) -> DbResult<ApiKey>;

    /// Get the key hashes for all active API keys owned by a service account.
    ///
    /// Used for cache invalidation when service account roles are updated.
    async fn get_key_hashes_by_service_account(
        &self,
        service_account_id: Uuid,
    ) -> DbResult<Vec<String>>;

    /// Get the key hashes for all active API keys owned by a user.
    ///
    /// Used for cache invalidation when a user is removed from an organization.
    async fn get_key_hashes_by_user(&self, user_id: Uuid) -> DbResult<Vec<String>>;
}

impl From<ApiKeyWithOwner> for CachedApiKey {
    fn from(key: ApiKeyWithOwner) -> Self {
        CachedApiKey {
            key: key.key,
            org_id: key.org_id,
            team_id: key.team_id,
            project_id: key.project_id,
            user_id: key.user_id,
            service_account_id: key.service_account_id,
            service_account_roles: key.service_account_roles,
        }
    }
}
