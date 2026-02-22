use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Export format for access review reports
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// JSON format (default)
    #[default]
    Json,
    /// CSV format for spreadsheet/auditor compatibility
    Csv,
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportFormat::Json => write!(f, "json"),
            ExportFormat::Csv => write!(f, "csv"),
        }
    }
}

/// Access inventory entry for a single user showing all their access rights
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserAccessInventoryEntry {
    /// User ID
    pub user_id: Uuid,
    /// User's external identifier (e.g., from SSO)
    pub external_id: String,
    /// User's email (if available)
    pub email: Option<String>,
    /// User's display name (if available)
    pub name: Option<String>,
    /// When the user was created
    pub created_at: DateTime<Utc>,
    /// Organization memberships with access details
    pub organizations: Vec<OrgAccessEntry>,
    /// Project memberships with access details
    pub projects: Vec<ProjectAccessEntry>,
    /// Summary of API key ownership
    pub api_key_summary: ApiKeySummary,
    /// Last activity timestamp (from audit logs)
    pub last_activity_at: Option<DateTime<Utc>>,
}

/// Organization access entry for access reviews
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgAccessEntry {
    /// Organization ID
    pub org_id: Uuid,
    /// Organization slug
    pub org_slug: String,
    /// Organization name
    pub org_name: String,
    /// User's role in the organization
    pub role: String,
    /// When the user was granted access (joined)
    pub granted_at: DateTime<Utc>,
}

/// Project access entry for access reviews
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProjectAccessEntry {
    /// Project ID
    pub project_id: Uuid,
    /// Project slug
    pub project_slug: String,
    /// Project name
    pub project_name: String,
    /// Organization ID the project belongs to
    pub org_id: Uuid,
    /// Organization slug the project belongs to
    pub org_slug: String,
    /// User's role in the project
    pub role: String,
    /// When the user was granted access (joined)
    pub granted_at: DateTime<Utc>,
}

/// Summary of API key ownership for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ApiKeySummary {
    /// Number of active API keys owned by the user
    pub active_count: i64,
    /// Number of revoked API keys owned by the user
    pub revoked_count: i64,
    /// Number of expired API keys owned by the user
    pub expired_count: i64,
    /// Total number of API keys
    pub total_count: i64,
}

/// Full access inventory response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AccessInventoryResponse {
    /// When this inventory was generated
    pub generated_at: DateTime<Utc>,
    /// Total number of users in the system
    pub total_users: i64,
    /// Users with their access inventory
    pub users: Vec<UserAccessInventoryEntry>,
    /// Summary statistics
    pub summary: AccessInventorySummary,
}

/// Summary statistics for the access inventory
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AccessInventorySummary {
    /// Total number of organizations
    pub total_organizations: i64,
    /// Total number of projects
    pub total_projects: i64,
    /// Total number of org memberships
    pub total_org_memberships: i64,
    /// Total number of project memberships
    pub total_project_memberships: i64,
    /// Total number of active API keys
    pub total_active_api_keys: i64,
}

/// Query parameters for access inventory endpoint
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams, utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", into_params(parameter_in = Query))]
pub struct AccessInventoryQuery {
    /// Filter by organization ID
    #[cfg_attr(feature = "utoipa", param(nullable))]
    pub org_id: Option<Uuid>,
    /// Maximum number of users to return (default: 100, max: 1000)
    #[cfg_attr(feature = "utoipa", param(default = 100, maximum = 1000))]
    pub limit: Option<i64>,
    /// Offset for pagination
    #[cfg_attr(feature = "utoipa", param(default = 0))]
    pub offset: Option<i64>,
    /// Export format (json or csv)
    #[cfg_attr(feature = "utoipa", param(default = "json"))]
    #[serde(default)]
    pub format: ExportFormat,
}

// ==================== Organization Access Report ====================

/// Query parameters for organization access report endpoint
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams, utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", into_params(parameter_in = Query))]
pub struct OrgAccessReportQuery {
    /// Export format (json or csv)
    #[cfg_attr(feature = "utoipa", param(default = "json"))]
    #[serde(default)]
    pub format: ExportFormat,
}

/// Member access entry for organization access reports
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgMemberAccessEntry {
    /// User ID
    pub user_id: Uuid,
    /// User's external identifier (e.g., from SSO)
    pub external_id: String,
    /// User's email (if available)
    pub email: Option<String>,
    /// User's name (if available)
    pub name: Option<String>,
    /// User's role in the organization
    pub role: String,
    /// When the user was granted access to the organization
    pub granted_at: DateTime<Utc>,
    /// Projects within this org that the user has access to
    pub project_access: Vec<OrgMemberProjectAccess>,
    /// API key summary for keys scoped to this org/projects
    pub api_key_summary: ApiKeySummary,
    /// Last activity timestamp within this org
    pub last_activity_at: Option<DateTime<Utc>>,
}

/// Project access within an organization for a member
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgMemberProjectAccess {
    /// Project ID
    pub project_id: Uuid,
    /// Project slug
    pub project_slug: String,
    /// Project name
    pub project_name: String,
    /// User's role in the project
    pub role: String,
    /// When the user was granted access to this project
    pub granted_at: DateTime<Utc>,
}

/// API key entry for organization access reports
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgApiKeyEntry {
    /// API key ID
    pub key_id: Uuid,
    /// API key name
    pub name: String,
    /// Key prefix for identification
    pub key_prefix: String,
    /// Owner type (organization, project, or user)
    pub owner_type: String,
    /// Owner ID
    pub owner_id: Uuid,
    /// Project slug if scoped to a project
    pub project_slug: Option<String>,
    /// User who owns the key (if user-owned)
    pub user_id: Option<Uuid>,
    /// User email (if user-owned)
    pub user_email: Option<String>,
    /// Whether the key is currently active
    pub is_active: bool,
    /// When the key was created
    pub created_at: DateTime<Utc>,
    /// When the key was revoked (if revoked)
    pub revoked_at: Option<DateTime<Utc>>,
    /// When the key expires (if set)
    pub expires_at: Option<DateTime<Utc>>,
    /// When the key was last used
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Access grant history entry from audit logs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AccessGrantHistoryEntry {
    /// Audit log ID
    pub log_id: Uuid,
    /// Action performed (e.g., "org_membership.create", "project_membership.create")
    pub action: String,
    /// Resource type
    pub resource_type: String,
    /// Resource ID
    pub resource_id: Uuid,
    /// Actor type who performed the action
    pub actor_type: String,
    /// Actor ID (if available)
    pub actor_id: Option<Uuid>,
    /// When the action occurred
    pub timestamp: DateTime<Utc>,
    /// Additional details
    pub details: Option<serde_json::Value>,
}

/// Organization access report response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgAccessReportResponse {
    /// When this report was generated
    pub generated_at: DateTime<Utc>,
    /// Organization ID
    pub org_id: Uuid,
    /// Organization slug
    pub org_slug: String,
    /// Organization name
    pub org_name: String,
    /// All members with their access details
    pub members: Vec<OrgMemberAccessEntry>,
    /// All API keys scoped to this organization or its projects
    pub api_keys: Vec<OrgApiKeyEntry>,
    /// Recent access grant history from audit logs
    pub access_history: Vec<AccessGrantHistoryEntry>,
    /// Summary statistics
    pub summary: OrgAccessReportSummary,
}

/// Summary statistics for organization access report
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgAccessReportSummary {
    /// Total number of org members
    pub total_members: i64,
    /// Total number of projects in the org
    pub total_projects: i64,
    /// Total number of project memberships
    pub total_project_memberships: i64,
    /// Total number of active API keys
    pub active_api_keys: i64,
    /// Total number of revoked API keys
    pub revoked_api_keys: i64,
}

// ==================== User Access Summary ====================

/// Query parameters for user access summary endpoint
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams, utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", into_params(parameter_in = Query))]
pub struct UserAccessSummaryQuery {
    /// Export format (json or csv)
    #[cfg_attr(feature = "utoipa", param(default = "json"))]
    #[serde(default)]
    pub format: ExportFormat,
}

/// User access summary response for access reviews
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserAccessSummaryResponse {
    /// When this summary was generated
    pub generated_at: DateTime<Utc>,
    /// User ID
    pub user_id: Uuid,
    /// User's external identifier
    pub external_id: String,
    /// User's email (if available)
    pub email: Option<String>,
    /// User's name (if available)
    pub name: Option<String>,
    /// When the user was created
    pub created_at: DateTime<Utc>,
    /// Organization memberships with access details
    pub organizations: Vec<UserAccessOrgEntry>,
    /// Project memberships with access details
    pub projects: Vec<UserAccessProjectEntry>,
    /// API keys owned by the user
    pub api_keys: Vec<UserAccessApiKeyEntry>,
    /// Last activity timestamp (from audit logs)
    pub last_activity_at: Option<DateTime<Utc>>,
    /// Summary statistics
    pub summary: UserAccessSummary,
}

/// Organization access entry for user access summary
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserAccessOrgEntry {
    /// Organization ID
    pub org_id: Uuid,
    /// Organization slug
    pub org_slug: String,
    /// Organization name
    pub org_name: String,
    /// User's role in the organization
    pub role: String,
    /// When the user was granted access (joined)
    pub granted_at: DateTime<Utc>,
    /// Last activity in this organization
    pub last_activity_at: Option<DateTime<Utc>>,
}

/// Project access entry for user access summary
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserAccessProjectEntry {
    /// Project ID
    pub project_id: Uuid,
    /// Project slug
    pub project_slug: String,
    /// Project name
    pub project_name: String,
    /// Organization ID the project belongs to
    pub org_id: Uuid,
    /// Organization slug the project belongs to
    pub org_slug: String,
    /// User's role in the project
    pub role: String,
    /// When the user was granted access (joined)
    pub granted_at: DateTime<Utc>,
    /// Last activity in this project
    pub last_activity_at: Option<DateTime<Utc>>,
}

/// API key entry for user access summary
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserAccessApiKeyEntry {
    /// API key ID
    pub key_id: Uuid,
    /// API key name
    pub name: String,
    /// Key prefix for identification
    pub key_prefix: String,
    /// Owner type (organization, project, or user)
    pub owner_type: String,
    /// Owner ID
    pub owner_id: Uuid,
    /// Whether the key is currently active
    pub is_active: bool,
    /// When the key was created
    pub created_at: DateTime<Utc>,
    /// When the key was revoked (if revoked)
    pub revoked_at: Option<DateTime<Utc>>,
    /// When the key expires (if set)
    pub expires_at: Option<DateTime<Utc>>,
    /// When the key was last used
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Summary statistics for user access
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserAccessSummary {
    /// Total number of organizations the user belongs to
    pub total_organizations: i64,
    /// Total number of projects the user belongs to
    pub total_projects: i64,
    /// Number of active API keys
    pub active_api_keys: i64,
    /// Number of revoked API keys
    pub revoked_api_keys: i64,
    /// Number of expired API keys
    pub expired_api_keys: i64,
}

// ==================== Stale Access Detection ====================

/// Query parameters for stale access detection endpoint
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams, utoipa::ToSchema))]
#[cfg_attr(feature = "utoipa", into_params(parameter_in = Query))]
pub struct StaleAccessQuery {
    /// Number of days of inactivity to consider access stale (default: 90)
    #[cfg_attr(feature = "utoipa", param(default = 90, minimum = 1, maximum = 365))]
    pub inactive_days: Option<i64>,
    /// Filter by organization ID
    #[cfg_attr(feature = "utoipa", param(nullable))]
    pub org_id: Option<Uuid>,
    /// Maximum number of results to return (default: 100, max: 1000)
    #[cfg_attr(feature = "utoipa", param(default = 100, maximum = 1000))]
    pub limit: Option<i64>,
    /// Export format (json or csv)
    #[cfg_attr(feature = "utoipa", param(default = "json"))]
    #[serde(default)]
    pub format: ExportFormat,
}

/// Response for stale access detection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StaleAccessResponse {
    /// When this report was generated
    pub generated_at: DateTime<Utc>,
    /// Number of days used as the inactivity threshold
    pub inactive_days_threshold: i64,
    /// Cutoff date (activity before this date is considered stale)
    pub cutoff_date: DateTime<Utc>,
    /// Users with stale access (no recent activity)
    pub stale_users: Vec<StaleUserEntry>,
    /// API keys with no recent usage
    pub stale_api_keys: Vec<StaleApiKeyEntry>,
    /// Users with access but zero recorded activity
    pub never_active_users: Vec<NeverActiveUserEntry>,
    /// Summary statistics
    pub summary: StaleAccessSummary,
}

/// A user identified as having stale access
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StaleUserEntry {
    /// User ID
    pub user_id: Uuid,
    /// User's external identifier
    pub external_id: String,
    /// User's email (if available)
    pub email: Option<String>,
    /// User's name (if available)
    pub name: Option<String>,
    /// When the user was created
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp (before the cutoff)
    pub last_activity_at: Option<DateTime<Utc>>,
    /// Days since last activity
    pub days_inactive: i64,
    /// Number of organization memberships
    pub org_count: i64,
    /// Number of project memberships
    pub project_count: i64,
    /// Number of active API keys
    pub active_api_keys: i64,
}

/// An API key identified as stale (not used recently)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StaleApiKeyEntry {
    /// API key ID
    pub key_id: Uuid,
    /// API key name
    pub name: String,
    /// Key prefix for identification
    pub key_prefix: String,
    /// Owner type (organization, project, or user)
    pub owner_type: String,
    /// Owner ID
    pub owner_id: Uuid,
    /// When the key was created
    pub created_at: DateTime<Utc>,
    /// When the key was last used (if ever)
    pub last_used_at: Option<DateTime<Utc>>,
    /// Days since last use (or since creation if never used)
    pub days_inactive: i64,
    /// Whether the key has ever been used
    pub never_used: bool,
}

/// A user with access who has never had any recorded activity
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct NeverActiveUserEntry {
    /// User ID
    pub user_id: Uuid,
    /// User's external identifier
    pub external_id: String,
    /// User's email (if available)
    pub email: Option<String>,
    /// User's name (if available)
    pub name: Option<String>,
    /// When the user was created
    pub created_at: DateTime<Utc>,
    /// Days since account creation
    pub days_since_creation: i64,
    /// Number of organization memberships
    pub org_count: i64,
    /// Number of project memberships
    pub project_count: i64,
    /// Number of active API keys
    pub active_api_keys: i64,
}

/// Summary statistics for stale access detection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct StaleAccessSummary {
    /// Total users scanned
    pub total_users_scanned: i64,
    /// Number of users with stale access
    pub stale_users_count: i64,
    /// Number of users who have never been active
    pub never_active_users_count: i64,
    /// Total active API keys scanned
    pub total_api_keys_scanned: i64,
    /// Number of stale API keys
    pub stale_api_keys_count: i64,
    /// Number of API keys never used
    pub never_used_api_keys_count: i64,
}
