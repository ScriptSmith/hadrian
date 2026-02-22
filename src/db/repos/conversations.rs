use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{ListParams, ListResult};
use crate::{
    db::error::DbResult,
    models::{
        AppendMessages, Conversation, ConversationOwnerType, ConversationWithProject,
        CreateConversation, Message, UpdateConversation,
    },
};

#[async_trait]
pub trait ConversationRepo: Send + Sync {
    /// Create a new conversation
    async fn create(&self, input: CreateConversation) -> DbResult<Conversation>;

    /// Get a conversation by ID
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Conversation>>;

    /// Get a conversation by ID, scoped to a specific organization.
    ///
    /// Verifies the conversation belongs to the given org by joining through the
    /// owner relationship: project-owned conversations join through `projects.org_id`,
    /// and user-owned conversations join through `org_memberships`.
    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Conversation>>;

    /// List conversations by owner (project or user)
    ///
    /// Note: Conversations are ordered by `updated_at` (not `created_at`) since
    /// recently-used conversations should appear first. The cursor uses `updated_at`
    /// as its timestamp component.
    async fn list_by_owner(
        &self,
        owner_type: ConversationOwnerType,
        owner_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<Conversation>>;

    /// Count conversations by owner (project or user)
    async fn count_by_owner(
        &self,
        owner_type: ConversationOwnerType,
        owner_id: Uuid,
        include_deleted: bool,
    ) -> DbResult<i64>;

    /// Update a conversation
    async fn update(&self, id: Uuid, input: UpdateConversation) -> DbResult<Conversation>;

    /// Append messages to a conversation
    async fn append_messages(&self, id: Uuid, input: AppendMessages) -> DbResult<Vec<Message>>;

    /// Delete (soft-delete) a conversation
    async fn delete(&self, id: Uuid) -> DbResult<()>;

    /// Set the pin order for a conversation
    ///
    /// - `pin_order = Some(n)`: Pin at position n (0 = first)
    /// - `pin_order = None`: Unpin the conversation
    async fn set_pin_order(&self, id: Uuid, pin_order: Option<i32>) -> DbResult<Conversation>;

    /// List all conversations accessible to a user
    ///
    /// Returns both:
    /// - User's personal conversations (owner_type=user, owner_id=user_id)
    /// - Conversations from projects the user belongs to
    ///
    /// Results include project metadata when applicable.
    /// Note: Does not support cursor-based pagination due to multi-source complexity.
    async fn list_accessible_for_user(
        &self,
        user_id: Uuid,
        limit: i64,
        include_deleted: bool,
    ) -> DbResult<Vec<ConversationWithProject>>;

    // ==================== Retention Operations ====================

    /// Hard-delete conversations that were soft-deleted before the given cutoff date.
    ///
    /// Only deletes conversations where `deleted_at < cutoff`.
    /// This permanently removes conversations that have been in the trash for
    /// longer than the retention period.
    ///
    /// Deletes in batches to avoid locking the database.
    /// Returns the total number of records deleted.
    async fn hard_delete_soft_deleted_before(
        &self,
        cutoff: DateTime<Utc>,
        batch_size: u32,
        max_deletes: u64,
    ) -> DbResult<u64>;
}
