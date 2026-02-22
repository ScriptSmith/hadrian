use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::validators::{SLUG_REGEX, validate_roles};

/// A service account is a first-class machine identity that can own API keys
/// and carry roles for RBAC evaluation. This enables unified authorization
/// across human users and machine identities.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ServiceAccount {
    /// Unique identifier
    pub id: Uuid,
    /// Organization this service account belongs to
    pub org_id: Uuid,
    /// URL-friendly identifier (unique within org)
    pub slug: String,
    /// Display name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Roles assigned to this service account (used in RBAC evaluation)
    pub roles: Vec<String>,
    /// When the service account was created
    pub created_at: DateTime<Utc>,
    /// When the service account was last updated
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new service account
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateServiceAccount {
    /// URL-friendly identifier (lowercase alphanumeric with hyphens)
    #[validate(length(min = 1, max = 64), regex(path = *SLUG_REGEX))]
    pub slug: String,
    /// Display name
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    /// Optional description
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    /// Roles to assign (freeform strings, defaults to empty)
    #[serde(default)]
    #[validate(custom(function = "validate_roles"))]
    pub roles: Vec<String>,
}

/// Request to update an existing service account
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateServiceAccount {
    /// New display name
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
    /// New description
    #[validate(length(max = 1000))]
    pub description: Option<String>,
    /// New roles (replaces existing roles)
    #[validate(custom(function = "validate_roles"))]
    pub roles: Option<Vec<String>>,
}
