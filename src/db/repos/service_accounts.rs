use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreateServiceAccount, ServiceAccount, UpdateServiceAccount},
};

#[async_trait]
pub trait ServiceAccountRepo: Send + Sync {
    /// Create a new service account within an organization.
    async fn create(&self, org_id: Uuid, input: CreateServiceAccount) -> DbResult<ServiceAccount>;

    /// Get a service account by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ServiceAccount>>;

    /// Get a service account by its slug within an organization.
    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<ServiceAccount>>;

    /// List all service accounts in an organization.
    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ServiceAccount>>;

    /// Count service accounts in an organization.
    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64>;

    /// Update a service account's details.
    async fn update(&self, id: Uuid, input: UpdateServiceAccount) -> DbResult<ServiceAccount>;

    /// Soft-delete a service account.
    async fn delete(&self, id: Uuid) -> DbResult<()>;

    /// Delete (soft-delete) a service account and revoke all its API keys atomically.
    ///
    /// This operation is performed in a single transaction with row locking to prevent
    /// race conditions where API keys could be created between checking the SA and
    /// revoking its keys.
    /// Returns the IDs of API keys that were revoked (for audit logging).
    async fn delete_with_api_key_revocation(&self, id: Uuid) -> DbResult<Vec<Uuid>>;
}
