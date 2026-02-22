use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::validators::SLUG_REGEX;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Project {
    pub id: Uuid,
    pub org_id: Uuid,
    /// Team this project belongs to (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<Uuid>,
    pub slug: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateProject {
    /// URL-friendly identifier (lowercase alphanumeric with hyphens)
    #[validate(length(min = 1, max = 64), regex(path = *SLUG_REGEX))]
    pub slug: String,
    /// Display name
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    /// Team to assign the project to (optional)
    pub team_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateProject {
    /// New display name
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    /// Team to assign the project to (use null to remove team assignment)
    #[serde(default, deserialize_with = "deserialize_optional_team_id")]
    pub team_id: Option<Option<Uuid>>,
}

/// Custom deserializer that handles:
/// - Missing field -> None (don't update)
/// - null -> Some(None) (set to null)
/// - uuid -> Some(Some(uuid)) (set to team)
fn deserialize_optional_team_id<'de, D>(deserializer: D) -> Result<Option<Option<Uuid>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<Uuid>::deserialize(deserializer)?))
}
