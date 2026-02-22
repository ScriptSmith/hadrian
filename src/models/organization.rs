use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::validators::SLUG_REGEX;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Organization {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateOrganization {
    /// URL-friendly identifier (lowercase alphanumeric with hyphens)
    #[validate(length(min = 1, max = 64), regex(path = *SLUG_REGEX))]
    pub slug: String,
    /// Display name
    #[validate(length(min = 1, max = 255))]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateOrganization {
    /// New display name
    #[validate(length(min = 1, max = 255))]
    pub name: Option<String>,
}
