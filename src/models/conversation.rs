use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// A chat message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Owner type for conversations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConversationOwnerType {
    Project,
    User,
}

impl ConversationOwnerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConversationOwnerType::Project => "project",
            ConversationOwnerType::User => "user",
        }
    }
}

impl std::str::FromStr for ConversationOwnerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "project" => Ok(ConversationOwnerType::Project),
            "user" => Ok(ConversationOwnerType::User),
            _ => Err(format!("Invalid owner type: {}", s)),
        }
    }
}

/// A conversation storing chat message history
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Conversation {
    pub id: Uuid,
    pub owner_type: ConversationOwnerType,
    pub owner_id: Uuid,
    pub title: String,
    /// Models used in this conversation
    #[serde(default)]
    pub models: Vec<String>,
    pub messages: Vec<Message>,
    /// Pin order for the conversation. NULL = not pinned, 0-N = pinned with order (lower = higher in list)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_order: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Owner specification for creating a conversation
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationOwner {
    Project { project_id: Uuid },
    User { user_id: Uuid },
}

impl ConversationOwner {
    pub fn owner_type(&self) -> ConversationOwnerType {
        match self {
            ConversationOwner::Project { .. } => ConversationOwnerType::Project,
            ConversationOwner::User { .. } => ConversationOwnerType::User,
        }
    }

    pub fn owner_id(&self) -> Uuid {
        match self {
            ConversationOwner::Project { project_id } => *project_id,
            ConversationOwner::User { user_id } => *user_id,
        }
    }
}

/// Request to create a new conversation
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateConversation {
    /// Owner of the conversation
    pub owner: ConversationOwner,
    /// Title of the conversation
    #[validate(length(min = 1, max = 255))]
    pub title: String,
    /// Models used in this conversation
    #[serde(default)]
    pub models: Vec<String>,
    /// Initial messages (optional)
    #[serde(default)]
    pub messages: Vec<Message>,
}

/// Request to update a conversation
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateConversation {
    /// New title
    #[validate(length(min = 1, max = 255))]
    pub title: Option<String>,
    /// New models list
    pub models: Option<Vec<String>>,
    /// Replace all messages
    pub messages: Option<Vec<Message>>,
    /// New owner (to move conversation to a different project or user)
    pub owner: Option<ConversationOwner>,
}

/// Request to append messages to a conversation
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AppendMessages {
    pub messages: Vec<Message>,
}

/// A conversation with optional project metadata
///
/// Used when listing conversations that may belong to a project,
/// so that the client can display project context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ConversationWithProject {
    #[serde(flatten)]
    pub conversation: Conversation,
    /// Project ID if this conversation belongs to a project
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
    /// Project name for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    /// Project slug for URL construction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_slug: Option<String>,
}

/// Request to set the pin order for a conversation
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SetPinOrder {
    /// The pin order (0 = first, higher = lower in list). Set to null to unpin.
    pub pin_order: Option<i32>,
}
