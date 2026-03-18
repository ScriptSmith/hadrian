use std::sync::Arc;

use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams, repos::ListResult},
    models::{CreateTemplate, Template, TemplateOwnerType, UpdateTemplate},
};

/// Service layer for template operations
#[derive(Clone)]
pub struct TemplateService {
    db: Arc<DbPool>,
}

impl TemplateService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new template
    pub async fn create(&self, input: CreateTemplate) -> DbResult<Template> {
        self.db.templates().create(input).await
    }

    /// Get a template by ID
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Template>> {
        self.db.templates().get_by_id(id).await
    }

    /// Get a template by ID, scoped to a specific organization.
    ///
    /// Use this variant when org context is available to prevent cross-org access.
    pub async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Template>> {
        self.db.templates().get_by_id_and_org(id, org_id).await
    }

    /// List all templates accessible within an organization
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Template>> {
        self.db.templates().list_by_org(org_id, params).await
    }

    /// List templates by owner with pagination
    pub async fn list_by_owner(
        &self,
        owner_type: TemplateOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Template>> {
        self.db
            .templates()
            .list_by_owner(owner_type, owner_id, params)
            .await
    }

    /// Count templates by owner
    pub async fn count_by_owner(
        &self,
        owner_type: TemplateOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64> {
        self.db
            .templates()
            .count_by_owner(owner_type, owner_id, include_deleted)
            .await
    }

    /// Update a template by ID
    pub async fn update(&self, id: Uuid, input: UpdateTemplate) -> DbResult<Template> {
        self.db.templates().update(id, input).await
    }

    /// Delete (soft-delete) a template by ID
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.templates().delete(id).await
    }
}
