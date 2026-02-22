use async_trait::async_trait;
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{CreateDynamicProvider, DynamicProvider, ProviderOwner, UpdateDynamicProvider},
};

#[async_trait]
pub trait DynamicProviderRepo: Send + Sync {
    async fn create(&self, id: Uuid, input: CreateDynamicProvider) -> DbResult<DynamicProvider>;
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<DynamicProvider>>;
    async fn get_by_name(
        &self,
        owner: &ProviderOwner,
        name: &str,
    ) -> DbResult<Option<DynamicProvider>>;
    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>>;
    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64>;
    async fn list_by_team(
        &self,
        team_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>>;
    async fn count_by_team(&self, team_id: Uuid) -> DbResult<i64>;
    async fn list_by_project(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>>;
    async fn count_by_project(&self, project_id: Uuid) -> DbResult<i64>;
    async fn list_by_user(
        &self,
        user_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>>;
    async fn count_by_user(&self, user_id: Uuid) -> DbResult<i64>;
    /// List enabled providers for a user with cursor-based pagination
    async fn list_enabled_by_user(
        &self,
        user_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>>;
    /// List enabled providers for an organization with cursor-based pagination
    async fn list_enabled_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>>;
    /// List enabled providers for a project with cursor-based pagination
    async fn list_enabled_by_project(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>>;
    /// List enabled providers for a team with cursor-based pagination
    async fn list_enabled_by_team(
        &self,
        team_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<DynamicProvider>>;
    async fn update(&self, id: Uuid, input: UpdateDynamicProvider) -> DbResult<DynamicProvider>;
    async fn delete(&self, id: Uuid) -> DbResult<()>;
}
