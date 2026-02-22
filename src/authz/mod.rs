//! Authorization module providing policy-based access control.
//!
//! This module implements authorization using:
//! - Roles from IdP (JWT claims)
//! - Policies defined in configuration with CEL conditions
//! - Per-organization policies stored in the database
//!
//! The authorization flow:
//! 1. Extract roles from JWT claims (mapped via config if needed)
//! 2. Build a subject with user info and roles
//! 3. Evaluate configured policies in priority order
//! 4. For org-scoped requests, evaluate org-specific policies
//! 5. Return allow/deny based on first matching policy (or default effect)

mod engine;
mod error;
mod registry;

pub use engine::{
    AuthzEngine, AuthzResult, PolicyContext, RequestContext, Subject, SystemPolicySimulationResult,
    SystemSimulationResult, TimeContext,
};
pub use error::AuthzError;
#[cfg(feature = "cel")]
pub use registry::CompiledOrgPolicy;
pub use registry::{PolicyRegistry, PolicyRegistryError};

/// Match a pattern against a value.
///
/// Supports three matching modes:
/// - `*` matches any value (full wildcard)
/// - `foo*` matches any value starting with `foo` (prefix wildcard)
/// - `foo` matches only the exact string `foo` (exact match)
///
/// # Examples
///
/// ```ignore
/// assert!(pattern_matches("*", "anything"));
/// assert!(pattern_matches("team*", "teams"));
/// assert!(pattern_matches("team*", "team_admin"));
/// assert!(!pattern_matches("team*", "project"));
/// assert!(pattern_matches("team", "team"));
/// assert!(!pattern_matches("team", "teams"));
/// ```
pub(crate) fn pattern_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        true
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        value.starts_with(prefix)
    } else {
        pattern == value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matches_full_wildcard() {
        // Full wildcard matches anything
        assert!(pattern_matches("*", "anything"));
        assert!(pattern_matches("*", ""));
        assert!(pattern_matches("*", "team"));
        assert!(pattern_matches("*", "organizations"));
    }

    #[test]
    fn test_pattern_matches_prefix_wildcard() {
        // Prefix wildcard matches values starting with prefix
        assert!(pattern_matches("team*", "team"));
        assert!(pattern_matches("team*", "teams"));
        assert!(pattern_matches("team*", "team_admin"));
        assert!(pattern_matches("team*", "team_member"));
        assert!(pattern_matches("team*", "team123"));

        // Does not match values not starting with prefix
        assert!(!pattern_matches("team*", "project"));
        assert!(!pattern_matches("team*", ""));
        assert!(!pattern_matches("team*", "ateam"));
        assert!(!pattern_matches("team*", "Team")); // case sensitive

        // Edge cases
        assert!(pattern_matches("*", "anything")); // just "*" is full wildcard
        assert!(pattern_matches("a*", "a"));
        assert!(pattern_matches("a*", "abc"));
    }

    #[test]
    fn test_pattern_matches_exact() {
        // Exact match only matches identical strings
        assert!(pattern_matches("team", "team"));
        assert!(pattern_matches("organizations", "organizations"));
        assert!(pattern_matches("", ""));

        // Does not match partial strings
        assert!(!pattern_matches("team", "teams"));
        assert!(!pattern_matches("team", "tea"));
        assert!(!pattern_matches("team", "Team")); // case sensitive
        assert!(!pattern_matches("team", "team_admin"));
    }
}
