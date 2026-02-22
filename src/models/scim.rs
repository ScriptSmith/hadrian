//! SCIM 2.0 provisioning models.
//!
//! These models support SCIM (System for Cross-domain Identity Management) 2.0
//! for automatic user provisioning and deprovisioning from identity providers
//! like Okta, Azure AD, Google Workspace, OneLogin, Keycloak, and Auth0.
//!
//! Key concepts:
//! - `OrgScimConfig`: Per-organization SCIM configuration (token, provisioning settings)
//! - `ScimUserMapping`: Maps IdP user IDs to Hadrian users (per-org)
//! - `ScimGroupMapping`: Maps IdP groups to Hadrian teams (per-org)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

// =============================================================================
// OrgScimConfig - Per-organization SCIM configuration
// =============================================================================

/// Per-organization SCIM configuration.
///
/// Enables automatic user provisioning/deprovisioning from identity providers.
/// Each organization can have at most one SCIM configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgScimConfig {
    /// Unique identifier for this SCIM configuration
    pub id: Uuid,
    /// Organization this SCIM config belongs to (one config per org)
    pub org_id: Uuid,
    /// Whether SCIM provisioning is enabled
    pub enabled: bool,
    /// Token prefix for identification (first 8 chars, like 'scim_xxxx')
    /// Note: token_hash is NOT included in the model - it's stored separately
    pub token_prefix: String,
    /// Last time the SCIM token was used for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_last_used_at: Option<DateTime<Utc>>,

    // Provisioning settings
    /// Whether to create new users when they don't exist
    pub create_users: bool,
    /// Default team to add new users to (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_team_id: Option<Uuid>,
    /// Default role for new users in the organization
    pub default_org_role: String,
    /// Default role for new users in the default team
    pub default_team_role: String,
    /// Whether to sync display name from SCIM on updates
    pub sync_display_name: bool,

    // Deprovisioning settings
    /// Whether deactivating a user deletes them entirely (vs just marking inactive)
    pub deactivate_deletes_user: bool,
    /// Whether to revoke all API keys when a user is deactivated via SCIM
    pub revoke_api_keys_on_deactivate: bool,

    // Timestamps
    /// When this config was created
    pub created_at: DateTime<Utc>,
    /// When this config was last updated
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new organization SCIM configuration.
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateOrgScimConfig {
    /// Whether SCIM provisioning is enabled (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,

    // Provisioning settings
    /// Whether to create new users when they don't exist (default: true)
    #[serde(default = "default_true")]
    pub create_users: bool,

    /// Default team to add new users to (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_team_id: Option<Uuid>,

    /// Default role for new users in the organization (default: "member")
    #[validate(length(min = 1, max = 32))]
    #[serde(default = "default_role")]
    pub default_org_role: String,

    /// Default role for new users in the default team (default: "member")
    #[validate(length(min = 1, max = 32))]
    #[serde(default = "default_role")]
    pub default_team_role: String,

    /// Whether to sync display name from SCIM on updates (default: true)
    #[serde(default = "default_true")]
    pub sync_display_name: bool,

    // Deprovisioning settings
    /// Whether deactivating a user deletes them entirely (default: false)
    #[serde(default)]
    pub deactivate_deletes_user: bool,

    /// Whether to revoke all API keys when user is deactivated (default: true)
    #[serde(default = "default_true")]
    pub revoke_api_keys_on_deactivate: bool,
}

impl Default for CreateOrgScimConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            create_users: true,
            default_team_id: None,
            default_org_role: default_role(),
            default_team_role: default_role(),
            sync_display_name: true,
            deactivate_deletes_user: false,
            revoke_api_keys_on_deactivate: true,
        }
    }
}

/// Request to update an existing organization SCIM configuration.
///
/// All fields are optional - only provided fields will be updated.
#[derive(Debug, Clone, Default, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateOrgScimConfig {
    /// Update enabled flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// Update create users flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_users: Option<bool>,

    /// Update default team (set to null to remove)
    #[serde(default, deserialize_with = "deserialize_optional_uuid")]
    pub default_team_id: Option<Option<Uuid>>,

    /// Update default org role
    #[validate(length(min = 1, max = 32))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_org_role: Option<String>,

    /// Update default team role
    #[validate(length(min = 1, max = 32))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_team_role: Option<String>,

    /// Update sync display name flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_display_name: Option<bool>,

    /// Update deactivate deletes user flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deactivate_deletes_user: Option<bool>,

    /// Update revoke API keys on deactivate flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoke_api_keys_on_deactivate: Option<bool>,
}

/// Internal struct that includes the token_hash for database operations.
/// This is NOT exposed via the API - only used internally.
#[derive(Debug, Clone)]
pub struct OrgScimConfigWithHash {
    /// The public SCIM config
    pub config: OrgScimConfig,
    /// SHA-256 hash of the SCIM bearer token
    pub token_hash: String,
}

/// Result of creating a new SCIM config (includes the raw token, shown only once)
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreatedOrgScimConfig {
    /// The created SCIM configuration
    pub config: OrgScimConfig,
    /// The raw SCIM bearer token (only shown once at creation time!)
    /// Format: "scim_<random_base64>" (e.g., "scim_Abc123XyzDef456...")
    pub token: String,
}

// =============================================================================
// ScimUserMapping - Maps SCIM external IDs to Hadrian users
// =============================================================================

/// Maps a SCIM external ID to a Hadrian user (per-organization).
///
/// This allows:
/// - The same user to have different SCIM IDs in different organizations
/// - Tracking SCIM-specific "active" status separately from user deletion
/// - Efficient lookup by SCIM external ID during provisioning
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ScimUserMapping {
    /// Unique identifier for this mapping
    pub id: Uuid,
    /// Organization this mapping belongs to
    pub org_id: Uuid,
    /// SCIM external ID from the IdP (e.g., Okta user ID like '00u1a2b3c4d5e6f7g8h9')
    pub scim_external_id: String,
    /// Hadrian user this maps to
    pub user_id: Uuid,
    /// SCIM "active" status (can be false while user still exists)
    pub active: bool,
    /// When this mapping was created
    pub created_at: DateTime<Utc>,
    /// When this mapping was last updated
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new SCIM user mapping.
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateScimUserMapping {
    /// SCIM external ID from the IdP
    #[validate(length(min = 1, max = 255))]
    pub scim_external_id: String,
    /// Hadrian user to map to
    pub user_id: Uuid,
    /// Initial active status (default: true)
    #[serde(default = "default_true")]
    pub active: bool,
}

/// Request to update a SCIM user mapping.
#[derive(Debug, Clone, Default, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateScimUserMapping {
    /// Update active status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

/// Combined SCIM user mapping with the associated Hadrian user.
///
/// Used for efficient database queries that JOIN mappings with users,
/// avoiding N+1 queries when listing SCIM users.
#[derive(Debug, Clone)]
pub struct ScimUserWithMapping {
    /// The SCIM mapping record
    pub mapping: ScimUserMapping,
    /// The associated Hadrian user
    pub user: super::User,
}

// =============================================================================
// ScimGroupMapping - Maps SCIM groups to Hadrian teams
// =============================================================================

/// Maps a SCIM group to a Hadrian team (per-organization).
///
/// When a SCIM group is pushed from the IdP, it maps to a Hadrian team.
/// Group membership changes in the IdP trigger team membership updates in Hadrian.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ScimGroupMapping {
    /// Unique identifier for this mapping
    pub id: Uuid,
    /// Organization this mapping belongs to
    pub org_id: Uuid,
    /// SCIM group ID from the IdP
    pub scim_group_id: String,
    /// Hadrian team this maps to
    pub team_id: Uuid,
    /// Display name from SCIM (for reference)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// When this mapping was created
    pub created_at: DateTime<Utc>,
    /// When this mapping was last updated
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new SCIM group mapping.
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateScimGroupMapping {
    /// SCIM group ID from the IdP
    #[validate(length(min = 1, max = 255))]
    pub scim_group_id: String,
    /// Hadrian team to map to
    pub team_id: Uuid,
    /// Display name from SCIM (optional)
    #[validate(length(max = 255))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// Request to update a SCIM group mapping.
#[derive(Debug, Clone, Default, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateScimGroupMapping {
    /// Update the team this group maps to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<Uuid>,
    /// Update display name
    #[validate(length(max = 255))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub display_name: Option<Option<String>>,
}

/// Combined SCIM group mapping with the associated Hadrian team.
///
/// Used for efficient database queries that JOIN mappings with teams,
/// avoiding N+1 queries when listing SCIM groups.
#[derive(Debug, Clone)]
pub struct ScimGroupWithTeam {
    /// The SCIM mapping record
    pub mapping: ScimGroupMapping,
    /// The associated Hadrian team
    pub team: super::Team,
}

// =============================================================================
// Helper functions
// =============================================================================

fn default_true() -> bool {
    true
}

fn default_role() -> String {
    "member".to_string()
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
