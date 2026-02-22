use std::sync::Arc;

use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams, repos::ListResult},
    models::{CreateProject, Project, UpdateProject},
};

/// Service layer for project operations
#[derive(Clone)]
pub struct ProjectService {
    db: Arc<DbPool>,
}

impl ProjectService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new project within an organization
    pub async fn create(&self, org_id: Uuid, input: CreateProject) -> DbResult<Project> {
        self.db.projects().create(org_id, input).await
    }

    /// Get project by ID
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Project>> {
        self.db.projects().get_by_id(id).await
    }

    /// Get project by ID, scoped to a specific organization.
    ///
    /// Use this variant when org context is available to prevent cross-org access.
    pub async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Project>> {
        self.db.projects().get_by_id_and_org(id, org_id).await
    }

    /// Get project by slug within an organization
    pub async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<Project>> {
        self.db.projects().get_by_slug(org_id, slug).await
    }

    /// List projects for an organization with pagination
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Project>> {
        self.db.projects().list_by_org(org_id, params).await
    }

    /// Count projects for an organization
    pub async fn count_by_org(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64> {
        self.db
            .projects()
            .count_by_org(org_id, include_deleted)
            .await
    }

    /// Update a project by ID
    pub async fn update(&self, id: Uuid, input: UpdateProject) -> DbResult<Project> {
        self.db.projects().update(id, input).await
    }

    /// Delete (soft-delete) a project by ID
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.projects().delete(id).await
    }
}
