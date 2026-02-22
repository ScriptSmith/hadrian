use std::{collections::HashMap, sync::Arc};

use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult, ListParams, repos::ListResult},
    models::{CreateSsoGroupMapping, ResolvedMembership, SsoGroupMapping, UpdateSsoGroupMapping},
};

/// Service layer for SSO group mapping operations.
///
/// This service handles the resolution of IdP groups to Hadrian team memberships
/// during JIT (Just-in-Time) provisioning. When a user logs in via SSO, their
/// IdP groups are looked up to determine which teams they should be added to.
#[derive(Clone)]
pub struct SsoGroupMappingService {
    db: Arc<DbPool>,
}

impl SsoGroupMappingService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new SSO group mapping.
    pub async fn create(
        &self,
        org_id: Uuid,
        input: CreateSsoGroupMapping,
    ) -> DbResult<SsoGroupMapping> {
        self.db.sso_group_mappings().create(org_id, input).await
    }

    /// Get a mapping by its ID.
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<SsoGroupMapping>> {
        self.db.sso_group_mappings().get_by_id(id).await
    }

    /// List all mappings for an organization.
    pub async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<SsoGroupMapping>> {
        self.db
            .sso_group_mappings()
            .list_by_org(org_id, params)
            .await
    }

    /// List mappings for a specific SSO connection within an organization.
    pub async fn list_by_connection(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<SsoGroupMapping>> {
        self.db
            .sso_group_mappings()
            .list_by_connection(sso_connection_name, org_id, params)
            .await
    }

    /// Count mappings for an organization.
    pub async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        self.db.sso_group_mappings().count_by_org(org_id).await
    }

    /// Update a mapping.
    pub async fn update(
        &self,
        id: Uuid,
        input: UpdateSsoGroupMapping,
    ) -> DbResult<SsoGroupMapping> {
        self.db.sso_group_mappings().update(id, input).await
    }

    /// Delete a mapping.
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.sso_group_mappings().delete(id).await
    }

    /// Delete all mappings for a specific IdP group within an org/connection.
    pub async fn delete_by_idp_group(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        idp_group: &str,
    ) -> DbResult<u64> {
        self.db
            .sso_group_mappings()
            .delete_by_idp_group(sso_connection_name, org_id, idp_group)
            .await
    }

    /// Resolve a user's IdP groups to Hadrian team memberships.
    ///
    /// This is the core method used during JIT provisioning. It looks up all
    /// configured mappings that match the user's IdP groups and returns the
    /// resolved team memberships.
    ///
    /// # Arguments
    /// * `sso_connection_name` - The SSO connection identifier (from config, defaults to "default")
    /// * `org_id` - The organization to resolve memberships within
    /// * `idp_groups` - List of IdP group names from the user's token
    /// * `default_role` - Default role to use when a mapping doesn't specify one
    ///
    /// # Returns
    /// A list of resolved memberships, each containing a team ID, role, and the
    /// IdP group that triggered the membership. Returns an empty list if no
    /// mappings match or if `idp_groups` is empty.
    ///
    /// # Behavior
    /// - Mappings without a `team_id` are skipped (they represent org-level roles only)
    /// - Mappings without a `role` use the provided `default_role`
    /// - Multiple mappings can match the same IdP group (e.g., one group → multiple teams)
    /// - When multiple mappings target the same team, the highest priority mapping wins
    /// - Each team appears at most once in the result
    ///
    /// # Priority Resolution
    /// Mappings are processed in priority order (highest first). When a user belongs
    /// to multiple IdP groups that map to the same team with different roles, the
    /// mapping with the highest `priority` value determines the role. If priorities
    /// are equal, the first matching group (alphabetically) wins.
    ///
    /// # Example
    /// ```ignore
    /// let memberships = service.resolve_memberships(
    ///     "default",
    ///     org_id,
    ///     &["Engineering".to_string(), "Platform".to_string()],
    ///     "member",
    /// ).await?;
    ///
    /// for m in memberships {
    ///     println!("Add to team {} with role {} (from group {})",
    ///         m.team_id, m.role, m.from_idp_group);
    /// }
    /// ```
    pub async fn resolve_memberships(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        idp_groups: &[String],
        default_role: &str,
    ) -> DbResult<Vec<ResolvedMembership>> {
        // Early return for empty groups
        if idp_groups.is_empty() {
            return Ok(Vec::new());
        }

        // Find all mappings that match the user's IdP groups
        // Results are ordered by priority DESC, idp_group ASC, created_at ASC
        let mappings = self
            .db
            .sso_group_mappings()
            .find_mappings_for_groups(sso_connection_name, org_id, idp_groups)
            .await?;

        // Deduplicate by team_id, keeping the highest priority mapping for each team.
        // Since mappings are ordered by priority DESC, the first occurrence wins.
        let mut seen_teams: HashMap<Uuid, ResolvedMembership> = HashMap::new();

        for mapping in mappings {
            // Skip mappings without a team_id (org-level role only)
            let Some(team_id) = mapping.team_id else {
                continue;
            };

            // Only keep the first (highest priority) mapping for each team
            seen_teams
                .entry(team_id)
                .or_insert_with(|| ResolvedMembership {
                    team_id,
                    role: mapping.role.unwrap_or_else(|| default_role.to_string()),
                    from_idp_group: mapping.idp_group,
                });
        }

        // Sort by team_id for deterministic ordering
        let mut memberships: Vec<_> = seen_teams.into_values().collect();
        memberships.sort_by_key(|m| m.team_id);
        Ok(memberships)
    }

    /// Get all unique IdP groups that have been configured for an organization.
    ///
    /// Useful for admin UIs to show which groups have mappings configured.
    pub async fn get_configured_groups(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
    ) -> DbResult<Vec<String>> {
        let mappings = self
            .db
            .sso_group_mappings()
            .list_by_connection(sso_connection_name, org_id, ListParams::default())
            .await?;

        let mut groups: Vec<String> = mappings.items.into_iter().map(|m| m.idp_group).collect();

        // Deduplicate and sort
        groups.sort();
        groups.dedup();

        Ok(groups)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::models::SsoGroupMapping;

    // Note: Integration tests are in src/db/tests/sso_group_mappings.rs
    // These unit tests focus on the service logic (filtering, default role handling)

    #[test]
    fn test_resolved_membership_creation() {
        let membership = ResolvedMembership {
            team_id: Uuid::new_v4(),
            role: "admin".to_string(),
            from_idp_group: "Engineering".to_string(),
        };

        assert_eq!(membership.role, "admin");
        assert_eq!(membership.from_idp_group, "Engineering");
    }

    /// Test the deduplication logic that happens in resolve_memberships.
    /// This tests the pure transformation logic without needing a database.
    #[test]
    fn test_deduplication_keeps_highest_priority() {
        let team_a = Uuid::new_v4();
        let team_b = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let now = Utc::now();

        // Simulate mappings returned from the database, ordered by priority DESC
        // Team A has two mappings: one from "SeniorEngineers" (priority 10) and one from "Engineers" (priority 5)
        // Team B has one mapping from "Engineers" (priority 5)
        let mappings = vec![
            SsoGroupMapping {
                id: Uuid::new_v4(),
                sso_connection_name: "default".to_string(),
                idp_group: "SeniorEngineers".to_string(),
                org_id,
                team_id: Some(team_a),
                role: Some("lead".to_string()),
                priority: 10,
                created_at: now,
                updated_at: now,
            },
            SsoGroupMapping {
                id: Uuid::new_v4(),
                sso_connection_name: "default".to_string(),
                idp_group: "Engineers".to_string(),
                org_id,
                team_id: Some(team_a),
                role: Some("member".to_string()),
                priority: 5,
                created_at: now,
                updated_at: now,
            },
            SsoGroupMapping {
                id: Uuid::new_v4(),
                sso_connection_name: "default".to_string(),
                idp_group: "Engineers".to_string(),
                org_id,
                team_id: Some(team_b),
                role: Some("member".to_string()),
                priority: 5,
                created_at: now,
                updated_at: now,
            },
        ];

        // Apply the same deduplication logic as resolve_memberships
        let default_role = "member";
        let mut seen_teams: HashMap<Uuid, ResolvedMembership> = HashMap::new();

        for mapping in mappings {
            let Some(team_id) = mapping.team_id else {
                continue;
            };

            seen_teams
                .entry(team_id)
                .or_insert_with(|| ResolvedMembership {
                    team_id,
                    role: mapping.role.unwrap_or_else(|| default_role.to_string()),
                    from_idp_group: mapping.idp_group,
                });
        }

        let memberships: Vec<_> = seen_teams.into_values().collect();

        // Should have exactly 2 memberships (one per team)
        assert_eq!(memberships.len(), 2);

        // Find the membership for team_a
        let team_a_membership = memberships.iter().find(|m| m.team_id == team_a).unwrap();
        // Should have role "lead" from "SeniorEngineers" (priority 10), not "member" from "Engineers" (priority 5)
        assert_eq!(team_a_membership.role, "lead");
        assert_eq!(team_a_membership.from_idp_group, "SeniorEngineers");

        // Find the membership for team_b
        let team_b_membership = memberships.iter().find(|m| m.team_id == team_b).unwrap();
        assert_eq!(team_b_membership.role, "member");
        assert_eq!(team_b_membership.from_idp_group, "Engineers");
    }

    /// Test that mappings without team_id are skipped
    #[test]
    fn test_deduplication_skips_org_level_mappings() {
        let team_a = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let now = Utc::now();

        let mappings = vec![
            // Org-level mapping (no team_id) - should be skipped
            SsoGroupMapping {
                id: Uuid::new_v4(),
                sso_connection_name: "default".to_string(),
                idp_group: "Admins".to_string(),
                org_id,
                team_id: None,
                role: Some("admin".to_string()),
                priority: 100,
                created_at: now,
                updated_at: now,
            },
            // Team-level mapping - should be included
            SsoGroupMapping {
                id: Uuid::new_v4(),
                sso_connection_name: "default".to_string(),
                idp_group: "Engineers".to_string(),
                org_id,
                team_id: Some(team_a),
                role: Some("member".to_string()),
                priority: 5,
                created_at: now,
                updated_at: now,
            },
        ];

        let default_role = "member";
        let mut seen_teams: HashMap<Uuid, ResolvedMembership> = HashMap::new();

        for mapping in mappings {
            let Some(team_id) = mapping.team_id else {
                continue;
            };

            seen_teams
                .entry(team_id)
                .or_insert_with(|| ResolvedMembership {
                    team_id,
                    role: mapping.role.unwrap_or_else(|| default_role.to_string()),
                    from_idp_group: mapping.idp_group,
                });
        }

        let memberships: Vec<_> = seen_teams.into_values().collect();

        // Should have exactly 1 membership (org-level mapping was skipped)
        assert_eq!(memberships.len(), 1);
        assert_eq!(memberships[0].team_id, team_a);
    }

    /// Test that default_role is used when mapping has no role
    #[test]
    fn test_deduplication_uses_default_role() {
        let team_a = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let now = Utc::now();

        let mappings = vec![SsoGroupMapping {
            id: Uuid::new_v4(),
            sso_connection_name: "default".to_string(),
            idp_group: "Engineers".to_string(),
            org_id,
            team_id: Some(team_a),
            role: None, // No role specified
            priority: 5,
            created_at: now,
            updated_at: now,
        }];

        let default_role = "contributor";
        let mut seen_teams: HashMap<Uuid, ResolvedMembership> = HashMap::new();

        for mapping in mappings {
            let Some(team_id) = mapping.team_id else {
                continue;
            };

            seen_teams
                .entry(team_id)
                .or_insert_with(|| ResolvedMembership {
                    team_id,
                    role: mapping.role.unwrap_or_else(|| default_role.to_string()),
                    from_idp_group: mapping.idp_group,
                });
        }

        let memberships: Vec<_> = seen_teams.into_values().collect();

        assert_eq!(memberships.len(), 1);
        assert_eq!(memberships[0].role, "contributor"); // Used default_role
    }
}
