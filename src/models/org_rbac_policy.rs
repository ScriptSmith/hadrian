use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// RBAC policy effect.
///
/// Determines whether a matching policy allows or denies access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum RbacPolicyEffect {
    /// Allow access when condition matches
    Allow,
    /// Deny access when condition matches (default - fail closed)
    #[default]
    Deny,
}

impl std::fmt::Display for RbacPolicyEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RbacPolicyEffect::Allow => write!(f, "allow"),
            RbacPolicyEffect::Deny => write!(f, "deny"),
        }
    }
}

impl std::str::FromStr for RbacPolicyEffect {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "allow" => Ok(RbacPolicyEffect::Allow),
            "deny" => Ok(RbacPolicyEffect::Deny),
            _ => Err(format!("Invalid RBAC policy effect: {}", s)),
        }
    }
}

/// Organization RBAC Policy.
///
/// Per-organization authorization policy that uses CEL expressions for evaluation.
/// Policies are evaluated in priority order (highest first) and the first matching
/// policy determines the access decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgRbacPolicy {
    /// Unique identifier for this policy
    pub id: Uuid,
    /// Organization this policy belongs to
    pub org_id: Uuid,
    /// Human-readable name for this policy (unique per org among non-deleted policies)
    pub name: String,
    /// Optional description of what this policy does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Resource pattern to match (e.g., "projects/*", "teams/engineering/*", "*")
    pub resource: String,
    /// Action pattern to match (e.g., "read", "write", "delete", "*")
    pub action: String,
    /// CEL expression that must evaluate to true for the policy to apply
    pub condition: String,
    /// Policy effect when condition matches
    pub effect: RbacPolicyEffect,
    /// Priority for evaluation order (higher = evaluated first)
    pub priority: i32,
    /// Whether this policy is active
    pub enabled: bool,
    /// Version number (incremented on each update)
    pub version: i32,
    /// When this policy was created
    pub created_at: DateTime<Utc>,
    /// When this policy was last updated
    pub updated_at: DateTime<Utc>,
    /// When this policy was soft-deleted (None if active)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Version history record for an RBAC policy.
///
/// Every update to a policy creates a new version record, enabling audit
/// trails and rollback to previous configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgRbacPolicyVersion {
    /// Unique identifier for this version record
    pub id: Uuid,
    /// The policy this version belongs to
    pub policy_id: Uuid,
    /// Version number (matches the policy's version at time of creation)
    pub version: i32,
    /// Policy name at this version
    pub name: String,
    /// Policy description at this version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Resource pattern at this version
    pub resource: String,
    /// Action pattern at this version
    pub action: String,
    /// CEL condition at this version
    pub condition: String,
    /// Policy effect at this version
    pub effect: RbacPolicyEffect,
    /// Priority at this version
    pub priority: i32,
    /// Enabled state at this version
    pub enabled: bool,
    /// User who created this version (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<Uuid>,
    /// Reason for the change
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// When this version was created
    pub created_at: DateTime<Utc>,
}

/// Request to create a new organization RBAC policy.
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateOrgRbacPolicy {
    /// Human-readable name for this policy (unique per org)
    #[validate(length(min = 1, max = 128))]
    pub name: String,

    /// Optional description of what this policy does
    #[validate(length(max = 1024))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Resource pattern to match (e.g., "projects/*", "teams/engineering/*", "*")
    #[validate(length(min = 1, max = 128))]
    #[serde(default = "default_wildcard")]
    pub resource: String,

    /// Action pattern to match (e.g., "read", "write", "delete", "*")
    #[validate(length(min = 1, max = 64))]
    #[serde(default = "default_wildcard")]
    pub action: String,

    /// CEL expression that must evaluate to true for the policy to apply
    #[validate(length(min = 1, max = 4096))]
    pub condition: String,

    /// Policy effect when condition matches (defaults to 'deny')
    #[serde(default)]
    pub effect: RbacPolicyEffect,

    /// Priority for evaluation order (higher = evaluated first, defaults to 0)
    /// Valid range: -1000 to 1000
    #[validate(range(min = -1000, max = 1000))]
    #[serde(default)]
    pub priority: i32,

    /// Whether this policy is active (defaults to true)
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Reason for creating this policy (stored in version history)
    #[validate(length(max = 512))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

fn default_wildcard() -> String {
    "*".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for CreateOrgRbacPolicy {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            resource: default_wildcard(),
            action: default_wildcard(),
            condition: String::new(),
            effect: RbacPolicyEffect::default(),
            priority: 0,
            enabled: true,
            reason: None,
        }
    }
}

/// Request to update an existing organization RBAC policy.
///
/// All fields are optional - only provided fields will be updated.
/// Each update increments the policy version and creates a version history record.
#[derive(Debug, Clone, Default, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateOrgRbacPolicy {
    /// Update the policy name
    #[validate(length(min = 1, max = 128))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Update the description (set to null to remove)
    #[validate(length(max = 1024))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub description: Option<Option<String>>,

    /// Update the resource pattern
    #[validate(length(min = 1, max = 128))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,

    /// Update the action pattern
    #[validate(length(min = 1, max = 64))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,

    /// Update the CEL condition
    #[validate(length(min = 1, max = 4096))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,

    /// Update the policy effect
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<RbacPolicyEffect>,

    /// Update the priority (valid range: -1000 to 1000)
    #[validate(range(min = -1000, max = 1000))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,

    /// Update the enabled state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    /// Reason for the update (stored in version history)
    #[validate(length(max = 512))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Request to rollback an RBAC policy to a previous version.
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RollbackOrgRbacPolicy {
    /// Target version number to rollback to
    pub target_version: i32,

    /// Reason for the rollback (stored in version history)
    #[validate(length(max = 512))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
