use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Type of actor that performed an action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AuditActorType {
    /// A user performed the action
    User,
    /// An API key was used to perform the action
    ApiKey,
    /// A service account performed the action
    ServiceAccount,
    /// An external identity without an internal user record
    ExternalUser,
    /// The system performed the action automatically
    System,
}

impl std::fmt::Display for AuditActorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditActorType::User => write!(f, "user"),
            AuditActorType::ApiKey => write!(f, "api_key"),
            AuditActorType::ServiceAccount => write!(f, "service_account"),
            AuditActorType::ExternalUser => write!(f, "external_user"),
            AuditActorType::System => write!(f, "system"),
        }
    }
}

impl std::str::FromStr for AuditActorType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(AuditActorType::User),
            "api_key" => Ok(AuditActorType::ApiKey),
            "service_account" => Ok(AuditActorType::ServiceAccount),
            "external_user" => Ok(AuditActorType::ExternalUser),
            "system" => Ok(AuditActorType::System),
            _ => Err(format!("Invalid actor type: {}", s)),
        }
    }
}

/// An audit log entry recording an admin operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AuditLog {
    /// Unique identifier for this audit log entry
    pub id: Uuid,
    /// When the action occurred
    pub timestamp: DateTime<Utc>,
    /// Type of actor that performed the action
    pub actor_type: AuditActorType,
    /// ID of the actor (user_id or api_key_id, None for system)
    pub actor_id: Option<Uuid>,
    /// The action performed (e.g., "api_key.create", "user.update")
    pub action: String,
    /// Type of resource affected (e.g., "api_key", "user", "organization")
    pub resource_type: String,
    /// ID of the affected resource
    pub resource_id: Uuid,
    /// Organization context (if applicable)
    pub org_id: Option<Uuid>,
    /// Project context (if applicable)
    pub project_id: Option<Uuid>,
    /// Additional details as JSON
    pub details: JsonValue,
    /// Client IP address
    pub ip_address: Option<String>,
    /// Client user agent
    pub user_agent: Option<String>,
}

/// Input for creating a new audit log entry
#[derive(Debug, Clone)]
pub struct CreateAuditLog {
    /// Type of actor that performed the action
    pub actor_type: AuditActorType,
    /// ID of the actor (user_id or api_key_id, None for system)
    pub actor_id: Option<Uuid>,
    /// The action performed (e.g., "api_key.create", "user.update")
    pub action: String,
    /// Type of resource affected (e.g., "api_key", "user", "organization")
    pub resource_type: String,
    /// ID of the affected resource
    pub resource_id: Uuid,
    /// Organization context (if applicable)
    pub org_id: Option<Uuid>,
    /// Project context (if applicable)
    pub project_id: Option<Uuid>,
    /// Additional details as JSON
    pub details: JsonValue,
    /// Client IP address
    pub ip_address: Option<String>,
    /// Client user agent
    pub user_agent: Option<String>,
}

/// Query parameters for listing audit logs
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
pub struct AuditLogQuery {
    /// Filter by actor type
    pub actor_type: Option<AuditActorType>,
    /// Filter by actor ID
    pub actor_id: Option<Uuid>,
    /// Filter by action (e.g., "api_key.create")
    pub action: Option<String>,
    /// Filter by resource type (e.g., "api_key")
    pub resource_type: Option<String>,
    /// Filter by resource ID
    pub resource_id: Option<Uuid>,
    /// Filter by organization ID
    pub org_id: Option<Uuid>,
    /// Filter by project ID
    pub project_id: Option<Uuid>,
    /// Start of time range (inclusive)
    pub from: Option<DateTime<Utc>>,
    /// End of time range (exclusive)
    pub to: Option<DateTime<Utc>>,
    /// Maximum number of results to return
    pub limit: Option<i64>,
    /// Cursor for pagination (cursor-based pagination)
    #[cfg_attr(
        feature = "utoipa",
        schema(example = "MTczMzU4MDgwMDAwMDphYmMxMjM0NS02Nzg5LTAxMjMtNDU2Ny0wMTIzNDU2Nzg5YWI")
    )]
    pub cursor: Option<String>,
    /// Pagination direction (forward or backward). Only used with cursor.
    #[serde(default)]
    pub direction: Option<String>,
}
