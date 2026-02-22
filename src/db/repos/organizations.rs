use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreateOrganization, Organization, UpdateOrganization},
};

#[async_trait]
pub trait OrganizationRepo: Send + Sync {
    async fn create(&self, input: CreateOrganization) -> DbResult<Organization>;
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Organization>>;
    async fn get_by_slug(&self, slug: &str) -> DbResult<Option<Organization>>;
    async fn list(&self, params: ListParams) -> DbResult<ListResult<Organization>>;
    async fn count(&self, include_deleted: bool) -> DbResult<i64>;
    async fn update(&self, id: Uuid, input: UpdateOrganization) -> DbResult<Organization>;
    async fn delete(&self, id: Uuid) -> DbResult<()>;
}
