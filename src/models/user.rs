use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Source of a membership (how it was created).
///
/// This enum tracks the origin of org, team, and project memberships,
/// which is critical for `sync_memberships_on_login` to work correctly.
/// JIT-created memberships can be removed when no longer present in IdP groups,
/// while manual and SCIM memberships are preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum MembershipSource {
    /// Created manually via admin API or UI
    #[default]
    Manual,
    /// Created via JIT provisioning (SSO login)
    Jit,
    /// Created via SCIM provisioning (IdP push)
    Scim,
}

impl MembershipSource {
    /// Convert to string for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Jit => "jit",
            Self::Scim => "scim",
        }
    }

    /// Parse from database string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "manual" => Some(Self::Manual),
            "jit" => Some(Self::Jit),
            "scim" => Some(Self::Scim),
            _ => None,
        }
    }
}

impl fmt::Display for MembershipSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct User {
    pub id: Uuid,
    /// External identifier (e.g., from SSO provider)
    pub external_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateUser {
    /// External identifier (e.g., from SSO provider)
    #[validate(length(min = 1, max = 255))]
    pub external_id: String,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateUser {
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
}

// ==================== GDPR Export Types ====================

/// Organization membership for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserOrgMembership {
    /// Organization ID
    pub org_id: Uuid,
    /// Organization slug
    pub org_slug: String,
    /// Organization name
    pub org_name: String,
    /// User's role in the organization
    pub role: String,
    /// Source of this membership (manual, jit, scim)
    pub source: MembershipSource,
    /// When the user joined the organization
    pub joined_at: DateTime<Utc>,
}

/// Project membership for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserProjectMembership {
    /// Project ID
    pub project_id: Uuid,
    /// Project slug
    pub project_slug: String,
    /// Project name
    pub project_name: String,
    /// Organization ID the project belongs to
    pub org_id: Uuid,
    /// User's role in the project
    pub role: String,
    /// Source of this membership (manual, jit, scim)
    pub source: MembershipSource,
    /// When the user joined the project
    pub joined_at: DateTime<Utc>,
}

/// User memberships (organizations, teams, and projects)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserMemberships {
    /// Organizations the user belongs to
    pub organizations: Vec<UserOrgMembership>,
    /// Teams the user belongs to
    pub teams: Vec<super::TeamMembership>,
    /// Projects the user belongs to
    pub projects: Vec<UserProjectMembership>,
}

/// API key export data (excludes sensitive hash)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ExportedApiKey {
    /// API key ID
    pub id: Uuid,
    /// Key prefix for identification
    pub key_prefix: String,
    /// User-assigned name
    pub name: String,
    /// Budget limit in cents (if set)
    pub budget_limit_cents: Option<i64>,
    /// Budget period (daily/monthly)
    pub budget_period: Option<String>,
    /// When the key was created
    pub created_at: DateTime<Utc>,
    /// When the key expires (if set)
    pub expires_at: Option<DateTime<Utc>>,
    /// When the key was revoked (if revoked)
    pub revoked_at: Option<DateTime<Utc>>,
    /// Last time the key was used
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Usage summary for export
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ExportedUsageSummary {
    /// Total cost in microcents
    pub total_cost_microcents: i64,
    /// Total tokens used
    pub total_tokens: i64,
    /// Total number of API requests
    pub request_count: i64,
    /// First request timestamp
    pub first_request_at: Option<DateTime<Utc>>,
    /// Last request timestamp
    pub last_request_at: Option<DateTime<Utc>>,
}

/// Session export data for user data export
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ExportedSession {
    /// Session ID
    pub id: Uuid,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session expires
    pub expires_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_activity: Option<DateTime<Utc>>,
    /// Device description (e.g., "Chrome 120 on Windows")
    pub device_description: Option<String>,
    /// Client IP address
    pub ip_address: Option<String>,
}

/// Complete user data export (GDPR Article 15 - Right of Access)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserDataExport {
    /// When this export was generated
    pub exported_at: DateTime<Utc>,
    /// User profile information
    pub user: User,
    /// Organization and project memberships
    pub memberships: UserMemberships,
    /// API keys owned by the user (excludes sensitive key hash)
    pub api_keys: Vec<ExportedApiKey>,
    /// Conversations owned by the user
    pub conversations: Vec<super::Conversation>,
    /// Active sessions (when enhanced session management is enabled)
    pub sessions: Vec<ExportedSession>,
    /// Aggregated usage summary
    pub usage_summary: ExportedUsageSummary,
    /// Audit logs where user was the actor (actions performed)
    pub audit_logs: Vec<super::AuditLog>,
}

/// Result of a user deletion operation (GDPR Article 17 - Right to Erasure)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserDeletionResponse {
    /// Whether the user was successfully deleted
    pub deleted: bool,
    /// ID of the deleted user
    pub user_id: Uuid,
    /// Number of API keys deleted
    pub api_keys_deleted: u64,
    /// Number of conversations deleted
    pub conversations_deleted: u64,
    /// Number of dynamic providers deleted
    pub dynamic_providers_deleted: u64,
    /// Number of usage records deleted
    pub usage_records_deleted: u64,
}
