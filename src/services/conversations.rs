use std::sync::Arc;

use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams, ListResult},
    models::{
        AppendMessages, Conversation, ConversationOwnerType, ConversationWithProject,
        CreateConversation, Message, UpdateConversation,
    },
};

/// Service layer for conversation operations
#[derive(Clone)]
pub struct ConversationService {
    db: Arc<DbPool>,
}

impl ConversationService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new conversation
    pub async fn create(&self, input: CreateConversation) -> DbResult<Conversation> {
        self.db.conversations().create(input).await
    }

    /// Get conversation by ID
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Conversation>> {
        self.db.conversations().get_by_id(id).await
    }

    /// Get conversation by ID, scoped to a specific organization.
    ///
    /// Use this variant when org context is available to prevent cross-org access.
    pub async fn get_by_id_and_org(
        &self,
        id: Uuid,
        org_id: Uuid,
    ) -> DbResult<Option<Conversation>> {
        self.db.conversations().get_by_id_and_org(id, org_id).await
    }

    /// List conversations by owner (project or user)
    pub async fn list_by_owner(
        &self,
        owner_type: ConversationOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Conversation>> {
        self.db
            .conversations()
            .list_by_owner(owner_type, owner_id, params)
            .await
    }

    /// List conversations by project
    pub async fn list_by_project(
        &self,
        project_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Conversation>> {
        self.list_by_owner(ConversationOwnerType::Project, project_id, params)
            .await
    }

    /// List conversations by user
    pub async fn list_by_user(
        &self,
        user_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Conversation>> {
        self.list_by_owner(ConversationOwnerType::User, user_id, params)
            .await
    }

    /// Count conversations by owner (project or user)
    pub async fn count_by_owner(
        &self,
        owner_type: ConversationOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64> {
        self.db
            .conversations()
            .count_by_owner(owner_type, owner_id, include_deleted)
            .await
    }

    /// Count conversations by project
    pub async fn count_by_project(&self, project_id: Uuid, include_deleted: bool) -> DbResult<i64> {
        self.count_by_owner(ConversationOwnerType::Project, project_id, include_deleted)
            .await
    }

    /// Count conversations by user
    pub async fn count_by_user(&self, user_id: Uuid, include_deleted: bool) -> DbResult<i64> {
        self.count_by_owner(ConversationOwnerType::User, user_id, include_deleted)
            .await
    }

    /// Update a conversation
    pub async fn update(&self, id: Uuid, input: UpdateConversation) -> DbResult<Conversation> {
        self.db.conversations().update(id, input).await
    }

    /// Append messages to a conversation
    pub async fn append_messages(&self, id: Uuid, input: AppendMessages) -> DbResult<Vec<Message>> {
        self.db.conversations().append_messages(id, input).await
    }

    /// Delete (soft-delete) a conversation
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.conversations().delete(id).await
    }

    /// List all conversations accessible to a user
    ///
    /// Returns both:
    /// - User's personal conversations (owner_type=user, owner_id=user_id)
    /// - Conversations from projects the user belongs to
    ///
    /// Results include project metadata when applicable.
    pub async fn list_accessible_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
        include_deleted: bool,
    ) -> DbResult<Vec<ConversationWithProject>> {
        self.db
            .conversations()
            .list_accessible_for_user(user_id, limit, include_deleted)
            .await
    }

    /// Set the pin order for a conversation
    ///
    /// - `pin_order = Some(n)`: Pin at position n (0 = first)
    /// - `pin_order = None`: Unpin the conversation
    pub async fn set_pin_order(&self, id: Uuid, pin_order: Option<i32>) -> DbResult<Conversation> {
        self.db.conversations().set_pin_order(id, pin_order).await
    }
}
