use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Owner type for templates (organization, team, project, or user)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TemplateOwnerType {
    Organization,
    Team,
    Project,
    User,
}

impl TemplateOwnerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TemplateOwnerType::Organization => "organization",
            TemplateOwnerType::Team => "team",
            TemplateOwnerType::Project => "project",
            TemplateOwnerType::User => "user",
        }
    }
}

impl std::str::FromStr for TemplateOwnerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "organization" => Ok(TemplateOwnerType::Organization),
            "team" => Ok(TemplateOwnerType::Team),
            "project" => Ok(TemplateOwnerType::Project),
            "user" => Ok(TemplateOwnerType::User),
            _ => Err(format!("Invalid template owner type: {}", s)),
        }
    }
}

/// A reusable system template
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Template {
    pub id: Uuid,
    pub owner_type: TemplateOwnerType,
    pub owner_id: Uuid,
    /// Name of the template
    pub name: String,
    /// Optional description of what this template does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The actual template content (system message template)
    pub content: String,
    /// Optional metadata (e.g., recommended temperature, max_tokens, tags)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Owner specification for creating a template
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemplateOwner {
    Organization { organization_id: Uuid },
    Team { team_id: Uuid },
    Project { project_id: Uuid },
    User { user_id: Uuid },
}

impl TemplateOwner {
    pub fn owner_type(&self) -> TemplateOwnerType {
        match self {
            TemplateOwner::Organization { .. } => TemplateOwnerType::Organization,
            TemplateOwner::Team { .. } => TemplateOwnerType::Team,
            TemplateOwner::Project { .. } => TemplateOwnerType::Project,
            TemplateOwner::User { .. } => TemplateOwnerType::User,
        }
    }

    pub fn owner_id(&self) -> Uuid {
        match self {
            TemplateOwner::Organization { organization_id } => *organization_id,
            TemplateOwner::Team { team_id } => *team_id,
            TemplateOwner::Project { project_id } => *project_id,
            TemplateOwner::User { user_id } => *user_id,
        }
    }
}

/// Request to create a new template
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateTemplate {
    /// Owner of the template
    pub owner: TemplateOwner,
    /// Name of the template (unique per owner)
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    /// Optional description
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    /// The actual template content
    #[validate(length(min = 1))]
    pub content: String,
    /// Optional metadata
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Request to update a template
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateTemplate {
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
