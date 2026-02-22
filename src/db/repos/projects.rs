use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreateProject, Project, UpdateProject},
};

#[async_trait]
pub trait ProjectRepo: Send + Sync {
    async fn create(&self, org_id: Uuid, input: CreateProject) -> DbResult<Project>;
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Project>>;
    /// Get a project by ID, scoped to a specific organization.
    ///
    /// Prevents cross-org IDOR attacks by verifying the project belongs to the given org.
    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Project>>;
    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<Project>>;
    async fn list_by_org(&self, org_id: Uuid, params: ListParams) -> DbResult<ListResult<Project>>;
    async fn count_by_org(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64>;
    async fn update(&self, id: Uuid, input: UpdateProject) -> DbResult<Project>;
    async fn delete(&self, id: Uuid) -> DbResult<()>;
}
