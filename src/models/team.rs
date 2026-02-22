use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::{MembershipSource, validators::SLUG_REGEX};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Team {
    pub id: Uuid,
    pub org_id: Uuid,
    pub slug: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateTeam {
    /// URL-friendly identifier (lowercase alphanumeric with hyphens)
    #[validate(length(min = 1, max = 64), regex(path = *SLUG_REGEX))]
    pub slug: String,
    /// Display name
    #[validate(length(min = 1, max = 255))]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateTeam {
    /// New display name
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
}

/// Team membership for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TeamMembership {
    /// Team ID
    pub team_id: Uuid,
    /// Team slug
    pub team_slug: String,
    /// Team name
    pub team_name: String,
    /// Organization ID the team belongs to
    pub org_id: Uuid,
    /// User's role in the team
    pub role: String,
    /// Source of this membership (manual, jit, scim)
    pub source: MembershipSource,
    /// When the user joined the team
    pub joined_at: DateTime<Utc>,
}

/// Member of a team (user with role)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TeamMember {
    /// User ID
    pub user_id: Uuid,
    /// User's external ID
    pub external_id: String,
    /// User's email (if available)
    pub email: Option<String>,
    /// User's name (if available)
    pub name: Option<String>,
    /// User's role in the team
    pub role: String,
    /// When the user joined the team
    pub joined_at: DateTime<Utc>,
}

/// Request to add a member to a team
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AddTeamMember {
    /// User ID to add to the team
    pub user_id: Uuid,
    /// Role to assign (defaults to 'member')
    #[validate(length(min = 1, max = 64))]
    #[serde(default = "default_role")]
    pub role: String,
    /// Source of this membership (defaults to 'manual' for API calls)
    #[serde(default)]
    pub source: MembershipSource,
}

fn default_role() -> String {
    "member".to_string()
}

/// Request to update a team member's role
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateTeamMember {
    /// New role to assign
    #[validate(length(min = 1, max = 64))]
    pub role: String,
}
