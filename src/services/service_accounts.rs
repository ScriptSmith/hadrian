use std::sync::Arc;

use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams, repos::ListResult},
    models::{CreateServiceAccount, ServiceAccount, UpdateServiceAccount},
};

/// Service layer for service account operations
#[derive(Clone)]
pub struct ServiceAccountService {
    db: Arc<DbPool>,
}

impl ServiceAccountService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new service account within an organization
    pub async fn create(
        &self,
        org_id: Uuid,
        input: CreateServiceAccount,
    ) -> DbResult<ServiceAccount> {
        self.db.service_accounts().create(org_id, input).await
    }

    /// Get service account by ID
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ServiceAccount>> {
        self.db.service_accounts().get_by_id(id).await
    }

    /// Get service account by slug within an organization
    pub async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<ServiceAccount>> {
        self.db.service_accounts().get_by_slug(org_id, slug).await
    }

    /// List service accounts for an organization with pagination
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ServiceAccount>> {
        self.db.service_accounts().list_by_org(org_id, params).await
    }

    /// Count service accounts for an organization
    pub async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        self.db.service_accounts().count_by_org(org_id).await
    }

    /// Update a service account by ID
    pub async fn update(&self, id: Uuid, input: UpdateServiceAccount) -> DbResult<ServiceAccount> {
        self.db.service_accounts().update(id, input).await
    }

    /// Delete (soft-delete) a service account by ID
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.service_accounts().delete(id).await
    }

    /// Delete a service account and revoke all its API keys atomically.
    ///
    /// This is the preferred method for deletion as it uses row locking to prevent
    /// race conditions where API keys could be created between checking the SA and
    /// revoking its keys.
    /// Returns the IDs of API keys that were revoked (for audit logging).
    pub async fn delete_with_api_key_revocation(&self, id: Uuid) -> DbResult<Vec<Uuid>> {
        self.db
            .service_accounts()
            .delete_with_api_key_revocation(id)
            .await
    }
}
