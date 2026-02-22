use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// SSO Group Mapping - maps an IdP group to a Hadrian team and/or role.
///
/// When a user logs in via SSO, their IdP groups are looked up in this table
/// to determine which teams they should be added to and with what role.
///
/// Multiple mappings per IdP group are allowed, enabling a single IdP group
/// to grant membership to multiple Hadrian teams.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SsoGroupMapping {
    /// Unique identifier for this mapping
    pub id: Uuid,
    /// Which SSO connection this mapping applies to (from config, defaults to 'default')
    pub sso_connection_name: String,
    /// The IdP group name exactly as it appears in the groups claim
    pub idp_group: String,
    /// Organization this mapping belongs to (mappings are org-scoped)
    pub org_id: Uuid,
    /// Team to add users to when they have this IdP group (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<Uuid>,
    /// Role to assign (within the team if team_id is set, otherwise org-level role)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Priority for role precedence (higher = wins when multiple mappings target same team)
    pub priority: i32,
    /// When this mapping was created
    pub created_at: DateTime<Utc>,
    /// When this mapping was last updated
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new SSO group mapping.
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateSsoGroupMapping {
    /// Which SSO connection this mapping applies to (defaults to 'default')
    #[validate(length(min = 1, max = 64))]
    #[serde(default = "default_connection_name")]
    pub sso_connection_name: String,
    /// The IdP group name exactly as it appears in the groups claim
    #[validate(length(min = 1, max = 512))]
    pub idp_group: String,
    /// Team to add users to when they have this IdP group (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<Uuid>,
    /// Role to assign (within the team if team_id is set, otherwise org-level role)
    #[validate(length(min = 1, max = 32))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Priority for role precedence (higher = wins when multiple mappings target same team)
    /// Defaults to 0 if not specified.
    #[serde(default)]
    pub priority: i32,
}

fn default_connection_name() -> String {
    "default".to_string()
}

/// Request to update an existing SSO group mapping.
///
/// All fields are optional - only provided fields will be updated.
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateSsoGroupMapping {
    /// Update the IdP group name
    #[validate(length(min = 1, max = 512))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idp_group: Option<String>,
    /// Update the team assignment (set to null to remove team assignment)
    #[serde(default, deserialize_with = "deserialize_optional_uuid")]
    pub team_id: Option<Option<Uuid>>,
    /// Update the role (set to null to remove role assignment)
    #[validate(length(min = 1, max = 32))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub role: Option<Option<String>>,
    /// Update the priority (higher = wins when multiple mappings target same team)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
}

/// Custom deserializer for Option<Option<Uuid>> to distinguish between:
/// - Field not present in JSON -> None (don't update)
/// - Field present as null -> Some(None) (set to NULL)
/// - Field present with value -> Some(Some(uuid)) (set to value)
fn deserialize_optional_uuid<'de, D>(deserializer: D) -> Result<Option<Option<Uuid>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

/// Custom deserializer for Option<Option<String>> to distinguish between:
/// - Field not present in JSON -> None (don't update)
/// - Field present as null -> Some(None) (set to NULL)
/// - Field present with value -> Some(Some(string)) (set to value)
fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

/// Resolved membership from SSO group mappings.
///
/// Returned by the group mapping service when resolving a user's IdP groups
/// to Hadrian team memberships.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ResolvedMembership {
    /// Team ID to add the user to
    pub team_id: Uuid,
    /// Role to assign within the team
    pub role: String,
    /// The IdP group that triggered this membership
    pub from_idp_group: String,
}
