use std::sync::Arc;

use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams, ListResult},
    models::{CreateOrganization, Organization, UpdateOrganization},
};

/// Service layer for organization operations
#[derive(Clone)]
pub struct OrganizationService {
    db: Arc<DbPool>,
}

impl OrganizationService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new organization
    pub async fn create(&self, input: CreateOrganization) -> DbResult<Organization> {
        self.db.organizations().create(input).await
    }

    /// Get organization by ID
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Organization>> {
        self.db.organizations().get_by_id(id).await
    }

    /// Get organization by slug
    pub async fn get_by_slug(&self, slug: &str) -> DbResult<Option<Organization>> {
        self.db.organizations().get_by_slug(slug).await
    }

    /// List organizations with pagination
    pub async fn list(&self, params: ListParams) -> DbResult<ListResult<Organization>> {
        self.db.organizations().list(params).await
    }

    /// Count organizations
    pub async fn count(&self, include_deleted: bool) -> DbResult<i64> {
        self.db.organizations().count(include_deleted).await
    }

    /// Update an organization by ID
    pub async fn update(&self, id: Uuid, input: UpdateOrganization) -> DbResult<Organization> {
        self.db.organizations().update(id, input).await
    }

    /// Delete (soft-delete) an organization by ID
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.organizations().delete(id).await
    }
}
