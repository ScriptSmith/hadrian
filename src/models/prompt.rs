use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Owner type for prompts (organization, team, project, or user)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum PromptOwnerType {
    Organization,
    Team,
    Project,
    User,
}

impl PromptOwnerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PromptOwnerType::Organization => "organization",
            PromptOwnerType::Team => "team",
            PromptOwnerType::Project => "project",
            PromptOwnerType::User => "user",
        }
    }
}

impl std::str::FromStr for PromptOwnerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "organization" => Ok(PromptOwnerType::Organization),
            "team" => Ok(PromptOwnerType::Team),
            "project" => Ok(PromptOwnerType::Project),
            "user" => Ok(PromptOwnerType::User),
            _ => Err(format!("Invalid prompt owner type: {}", s)),
        }
    }
}

/// A reusable system prompt template
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Prompt {
    pub id: Uuid,
    pub owner_type: PromptOwnerType,
    pub owner_id: Uuid,
    /// Name of the prompt template
    pub name: String,
    /// Optional description of what this prompt does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The actual prompt content (system message template)
    pub content: String,
    /// Optional metadata (e.g., recommended temperature, max_tokens, tags)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Owner specification for creating a prompt
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromptOwner {
    Organization { organization_id: Uuid },
    Team { team_id: Uuid },
    Project { project_id: Uuid },
    User { user_id: Uuid },
}

impl PromptOwner {
    pub fn owner_type(&self) -> PromptOwnerType {
        match self {
            PromptOwner::Organization { .. } => PromptOwnerType::Organization,
            PromptOwner::Team { .. } => PromptOwnerType::Team,
            PromptOwner::Project { .. } => PromptOwnerType::Project,
            PromptOwner::User { .. } => PromptOwnerType::User,
        }
    }

    pub fn owner_id(&self) -> Uuid {
        match self {
            PromptOwner::Organization { organization_id } => *organization_id,
            PromptOwner::Team { team_id } => *team_id,
            PromptOwner::Project { project_id } => *project_id,
            PromptOwner::User { user_id } => *user_id,
        }
    }
}

/// Request to create a new prompt
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreatePrompt {
    /// Owner of the prompt
    pub owner: PromptOwner,
    /// Name of the prompt template (unique per owner)
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    /// Optional description
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    /// The actual prompt content
    #[validate(length(min = 1))]
    pub content: String,
    /// Optional metadata
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Request to update a prompt
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdatePrompt {
    /// New name (unique per owner)
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    /// New description
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    /// New content
    #[validate(length(min = 1))]
    pub content: Option<String>,
    /// New metadata (replaces existing)
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}
