use std::{borrow::Cow, sync::LazyLock};

use regex::Regex;
use validator::ValidationError;

/// Regex for validating URL-friendly slugs (lowercase alphanumeric with hyphens).
/// Examples: "my-project", "org1", "test-org-123"
pub static SLUG_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").unwrap());

/// Maximum length for a single role string
const MAX_ROLE_LENGTH: usize = 64;

/// Maximum number of roles per service account
const MAX_ROLES_COUNT: usize = 100;

/// Validate role strings for service accounts.
///
/// Ensures that:
/// - No more than MAX_ROLES_COUNT roles are provided
/// - No role is empty or whitespace-only
/// - No role exceeds MAX_ROLE_LENGTH characters
pub fn validate_roles(roles: &[String]) -> Result<(), ValidationError> {
    if roles.len() > MAX_ROLES_COUNT {
        let mut err = ValidationError::new("too_many_roles");
        err.message = Some(Cow::Owned(format!(
            "Maximum {} roles allowed",
            MAX_ROLES_COUNT
        )));
        return Err(err);
    }

    for role in roles {
        let trimmed = role.trim();
        if trimmed.is_empty() {
            let mut err = ValidationError::new("empty_role");
            err.message = Some(Cow::Borrowed(
                "Role names cannot be empty or whitespace-only",
            ));
            return Err(err);
        }
        if role.len() > MAX_ROLE_LENGTH {
            let mut err = ValidationError::new("role_too_long");
            err.message = Some(Cow::Owned(format!(
                "Role names cannot exceed {} characters",
                MAX_ROLE_LENGTH
            )));
            return Err(err);
        }
    }
    Ok(())
}
