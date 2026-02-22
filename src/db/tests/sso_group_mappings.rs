//! Shared tests for SsoGroupMappingRepo implementations
//!
//! Tests are written as async functions that take a test context containing
//! the SSO group mapping repo and utilities for creating test organizations and teams.

use uuid::Uuid;

use crate::{
    db::{
        error::DbError,
        repos::{ListParams, OrganizationRepo, SsoGroupMappingRepo, TeamRepo},
    },
    models::{CreateOrganization, CreateSsoGroupMapping, CreateTeam, UpdateSsoGroupMapping},
};

// ============================================================================
// Test Input Helpers
// ============================================================================

fn create_org_input(slug: &str, name: &str) -> CreateOrganization {
    CreateOrganization {
        slug: slug.to_string(),
        name: name.to_string(),
    }
}

fn create_team_input(slug: &str, name: &str) -> CreateTeam {
    CreateTeam {
        slug: slug.to_string(),
        name: name.to_string(),
    }
}

fn create_mapping_input(
    connection: &str,
    idp_group: &str,
    team_id: Option<Uuid>,
    role: Option<&str>,
) -> CreateSsoGroupMapping {
    CreateSsoGroupMapping {
        sso_connection_name: connection.to_string(),
        idp_group: idp_group.to_string(),
        team_id,
        role: role.map(String::from),
        priority: 0,
    }
}

/// Test context containing repos needed for SSO group mapping tests
pub struct SsoGroupMappingTestContext<'a> {
    pub mapping_repo: &'a dyn SsoGroupMappingRepo,
    pub org_repo: &'a dyn OrganizationRepo,
    pub team_repo: &'a dyn TeamRepo,
}

impl<'a> SsoGroupMappingTestContext<'a> {
    /// Create a test organization and return its ID
    pub async fn create_test_org(&self, slug: &str) -> Uuid {
        self.org_repo
            .create(create_org_input(slug, &format!("Org {}", slug)))
            .await
            .expect("Failed to create test org")
            .id
    }

    /// Create a test team and return its ID
    pub async fn create_test_team(&self, org_id: Uuid, slug: &str) -> Uuid {
        self.team_repo
            .create(org_id, create_team_input(slug, &format!("Team {}", slug)))
            .await
            .expect("Failed to create test team")
            .id
    }
}

// ============================================================================
// Create Tests
// ============================================================================

pub async fn test_create_mapping_without_team(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_mapping_input("default", "Engineering", None, Some("developer"));
    let mapping = ctx
        .mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create mapping");

    assert_eq!(mapping.sso_connection_name, "default");
    assert_eq!(mapping.idp_group, "Engineering");
    assert_eq!(mapping.org_id, org_id);
    assert!(mapping.team_id.is_none());
    assert_eq!(mapping.role, Some("developer".to_string()));
}

pub async fn test_create_mapping_with_team(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let team_id = ctx.create_test_team(org_id, "engineering").await;

    let input = create_mapping_input("okta", "Engineering", Some(team_id), Some("member"));
    let mapping = ctx
        .mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create mapping");

    assert_eq!(mapping.sso_connection_name, "okta");
    assert_eq!(mapping.team_id, Some(team_id));
}

pub async fn test_create_duplicate_mapping_fails(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_mapping_input("default", "Admins", None, Some("admin"));
    ctx.mapping_repo
        .create(org_id, input.clone())
        .await
        .expect("First create should succeed");

    let result = ctx.mapping_repo.create(org_id, input).await;
    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_same_group_different_teams_allowed(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let team1_id = ctx.create_test_team(org_id, "team-1").await;
    let team2_id = ctx.create_test_team(org_id, "team-2").await;

    // Same IdP group can map to multiple teams
    let input1 = create_mapping_input("default", "Engineering", Some(team1_id), Some("member"));
    let input2 = create_mapping_input("default", "Engineering", Some(team2_id), Some("member"));

    ctx.mapping_repo
        .create(org_id, input1)
        .await
        .expect("First mapping should succeed");
    ctx.mapping_repo
        .create(org_id, input2)
        .await
        .expect("Second mapping to different team should succeed");
}

pub async fn test_same_group_different_orgs_allowed(ctx: &SsoGroupMappingTestContext<'_>) {
    let org1_id = ctx.create_test_org("org-1").await;
    let org2_id = ctx.create_test_org("org-2").await;

    let input = create_mapping_input("default", "Engineering", None, Some("member"));

    ctx.mapping_repo
        .create(org1_id, input.clone())
        .await
        .expect("Mapping in org 1 should succeed");
    ctx.mapping_repo
        .create(org2_id, input)
        .await
        .expect("Same mapping in org 2 should succeed");
}

// ============================================================================
// Get/List Tests
// ============================================================================

pub async fn test_get_by_id(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_mapping_input("default", "Test", None, None);
    let created = ctx
        .mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create");

    let fetched = ctx
        .mapping_repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get")
        .expect("Should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.idp_group, "Test");
}

pub async fn test_get_by_id_not_found(ctx: &SsoGroupMappingTestContext<'_>) {
    let result = ctx
        .mapping_repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");
    assert!(result.is_none());
}

pub async fn test_list_by_org_empty(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let result = ctx
        .mapping_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_by_org(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for group in ["Group1", "Group2", "Group3"] {
        let input = create_mapping_input("default", group, None, None);
        ctx.mapping_repo
            .create(org_id, input)
            .await
            .expect("Failed to create");
    }

    let result = ctx
        .mapping_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 3);
}

pub async fn test_list_by_org_filters_by_org(ctx: &SsoGroupMappingTestContext<'_>) {
    let org1_id = ctx.create_test_org("org-1").await;
    let org2_id = ctx.create_test_org("org-2").await;

    // Create mappings in both orgs
    ctx.mapping_repo
        .create(
            org1_id,
            create_mapping_input("default", "Group1", None, None),
        )
        .await
        .expect("Failed to create");
    ctx.mapping_repo
        .create(
            org2_id,
            create_mapping_input("default", "Group2", None, None),
        )
        .await
        .expect("Failed to create");

    let result = ctx
        .mapping_repo
        .list_by_org(org1_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].idp_group, "Group1");
}

pub async fn test_list_by_connection(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    // Create mappings for different connections
    for (conn, group) in [
        ("okta", "OktaGroup"),
        ("azure", "AzureGroup"),
        ("okta", "OktaGroup2"),
    ] {
        let input = create_mapping_input(conn, group, None, None);
        ctx.mapping_repo
            .create(org_id, input)
            .await
            .expect("Failed to create");
    }

    let okta_result = ctx
        .mapping_repo
        .list_by_connection("okta", org_id, ListParams::default())
        .await
        .expect("Failed to list");
    let azure_result = ctx
        .mapping_repo
        .list_by_connection("azure", org_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(okta_result.items.len(), 2);
    assert_eq!(azure_result.items.len(), 1);
}

pub async fn test_list_by_connection_filters_by_org(ctx: &SsoGroupMappingTestContext<'_>) {
    let org1_id = ctx.create_test_org("org-1").await;
    let org2_id = ctx.create_test_org("org-2").await;

    // Same connection name in different orgs
    ctx.mapping_repo
        .create(org1_id, create_mapping_input("okta", "Group1", None, None))
        .await
        .expect("Failed to create");
    ctx.mapping_repo
        .create(org2_id, create_mapping_input("okta", "Group2", None, None))
        .await
        .expect("Failed to create");

    let result = ctx
        .mapping_repo
        .list_by_connection("okta", org1_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].idp_group, "Group1");
}

// ============================================================================
// Find Mappings for Groups Tests
// ============================================================================

pub async fn test_find_mappings_for_groups(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let team_id = ctx.create_test_team(org_id, "engineering").await;

    // Create several mappings
    for (group, team, role) in [
        ("Engineering", Some(team_id), Some("developer")),
        ("Admins", None, Some("admin")),
        ("Support", None, Some("support")),
    ] {
        let input = create_mapping_input("default", group, team, role);
        ctx.mapping_repo
            .create(org_id, input)
            .await
            .expect("Failed to create");
    }

    // Find mappings for a subset of groups
    let user_groups = vec!["Engineering".to_string(), "Admins".to_string()];
    let mappings = ctx
        .mapping_repo
        .find_mappings_for_groups("default", org_id, &user_groups)
        .await
        .expect("Failed to find mappings");

    assert_eq!(mappings.len(), 2);
    let group_names: Vec<&str> = mappings.iter().map(|m| m.idp_group.as_str()).collect();
    assert!(group_names.contains(&"Engineering"));
    assert!(group_names.contains(&"Admins"));
    assert!(!group_names.contains(&"Support"));
}

pub async fn test_find_mappings_empty_groups(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let mappings = ctx
        .mapping_repo
        .find_mappings_for_groups("default", org_id, &[])
        .await
        .expect("Failed to find mappings");

    assert!(mappings.is_empty());
}

pub async fn test_find_mappings_no_matches(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_mapping_input("default", "Engineering", None, None);
    ctx.mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create");

    let mappings = ctx
        .mapping_repo
        .find_mappings_for_groups("default", org_id, &["NonExistent".to_string()])
        .await
        .expect("Failed to find mappings");

    assert!(mappings.is_empty());
}

pub async fn test_find_mappings_wrong_connection(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_mapping_input("okta", "Engineering", None, None);
    ctx.mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create");

    // Search with different connection name
    let mappings = ctx
        .mapping_repo
        .find_mappings_for_groups("azure", org_id, &["Engineering".to_string()])
        .await
        .expect("Failed to find mappings");

    assert!(mappings.is_empty());
}

pub async fn test_find_mappings_returns_multiple_for_same_group(
    ctx: &SsoGroupMappingTestContext<'_>,
) {
    let org_id = ctx.create_test_org("test-org").await;
    let team1_id = ctx.create_test_team(org_id, "team-1").await;
    let team2_id = ctx.create_test_team(org_id, "team-2").await;

    // Same IdP group maps to multiple teams
    ctx.mapping_repo
        .create(
            org_id,
            create_mapping_input("default", "Engineering", Some(team1_id), Some("member")),
        )
        .await
        .expect("Failed to create");
    ctx.mapping_repo
        .create(
            org_id,
            create_mapping_input("default", "Engineering", Some(team2_id), Some("lead")),
        )
        .await
        .expect("Failed to create");

    let mappings = ctx
        .mapping_repo
        .find_mappings_for_groups("default", org_id, &["Engineering".to_string()])
        .await
        .expect("Failed to find mappings");

    assert_eq!(mappings.len(), 2);
    let team_ids: Vec<Option<Uuid>> = mappings.iter().map(|m| m.team_id).collect();
    assert!(team_ids.contains(&Some(team1_id)));
    assert!(team_ids.contains(&Some(team2_id)));
}

// ============================================================================
// Count Tests
// ============================================================================

pub async fn test_count_by_org_empty(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let count = ctx
        .mapping_repo
        .count_by_org(org_id)
        .await
        .expect("Failed to count");

    assert_eq!(count, 0);
}

pub async fn test_count_by_org(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..3 {
        let input = create_mapping_input("default", &format!("Group{}", i), None, None);
        ctx.mapping_repo
            .create(org_id, input)
            .await
            .expect("Failed to create");
    }

    let count = ctx
        .mapping_repo
        .count_by_org(org_id)
        .await
        .expect("Failed to count");

    assert_eq!(count, 3);
}

pub async fn test_count_filters_by_org(ctx: &SsoGroupMappingTestContext<'_>) {
    let org1_id = ctx.create_test_org("org-1").await;
    let org2_id = ctx.create_test_org("org-2").await;

    // Create mappings in both orgs
    for i in 0..2 {
        ctx.mapping_repo
            .create(
                org1_id,
                create_mapping_input("default", &format!("Org1Group{}", i), None, None),
            )
            .await
            .expect("Failed to create");
    }
    ctx.mapping_repo
        .create(
            org2_id,
            create_mapping_input("default", "Org2Group", None, None),
        )
        .await
        .expect("Failed to create");

    assert_eq!(
        ctx.mapping_repo
            .count_by_org(org1_id)
            .await
            .expect("Failed to count"),
        2
    );
    assert_eq!(
        ctx.mapping_repo
            .count_by_org(org2_id)
            .await
            .expect("Failed to count"),
        1
    );
}

// ============================================================================
// Update Tests
// ============================================================================

pub async fn test_update_mapping(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let team_id = ctx.create_test_team(org_id, "new-team").await;

    let input = create_mapping_input("default", "OldGroup", None, Some("old-role"));
    let created = ctx
        .mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create");

    let updated = ctx
        .mapping_repo
        .update(
            created.id,
            UpdateSsoGroupMapping {
                idp_group: Some("NewGroup".to_string()),
                team_id: Some(Some(team_id)),
                role: Some(Some("new-role".to_string())),
                priority: None,
            },
        )
        .await
        .expect("Failed to update");

    assert_eq!(updated.idp_group, "NewGroup");
    assert_eq!(updated.team_id, Some(team_id));
    assert_eq!(updated.role, Some("new-role".to_string()));
}

pub async fn test_update_partial(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_mapping_input("default", "Group", None, Some("old-role"));
    let created = ctx
        .mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create");

    // Only update role, leave other fields unchanged
    let updated = ctx
        .mapping_repo
        .update(
            created.id,
            UpdateSsoGroupMapping {
                idp_group: None,
                team_id: None,
                role: Some(Some("new-role".to_string())),
                priority: None,
            },
        )
        .await
        .expect("Failed to update");

    assert_eq!(updated.idp_group, "Group"); // Unchanged
    assert_eq!(updated.role, Some("new-role".to_string())); // Changed
}

pub async fn test_update_clear_optional_fields(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let team_id = ctx.create_test_team(org_id, "team").await;

    let input = create_mapping_input("default", "Group", Some(team_id), Some("role"));
    let created = ctx
        .mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create");

    // Clear team_id and role by setting to None
    let updated = ctx
        .mapping_repo
        .update(
            created.id,
            UpdateSsoGroupMapping {
                idp_group: None,
                team_id: Some(None), // Clear team_id
                role: Some(None),    // Clear role
                priority: None,
            },
        )
        .await
        .expect("Failed to update");

    assert!(updated.team_id.is_none());
    assert!(updated.role.is_none());
}

pub async fn test_update_no_changes(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_mapping_input("default", "Group", None, Some("role"));
    let created = ctx
        .mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create");

    // Update with no changes
    let updated = ctx
        .mapping_repo
        .update(
            created.id,
            UpdateSsoGroupMapping {
                idp_group: None,
                team_id: None,
                role: None,
                priority: None,
            },
        )
        .await
        .expect("Failed to update");

    assert_eq!(updated.idp_group, created.idp_group);
    assert_eq!(updated.role, created.role);
}

pub async fn test_update_not_found(ctx: &SsoGroupMappingTestContext<'_>) {
    let result = ctx
        .mapping_repo
        .update(
            Uuid::new_v4(),
            UpdateSsoGroupMapping {
                idp_group: Some("New".to_string()),
                team_id: None,
                role: None,
                priority: None,
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

// ============================================================================
// Delete Tests
// ============================================================================

pub async fn test_delete_mapping(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_mapping_input("default", "ToDelete", None, None);
    let created = ctx
        .mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create");

    ctx.mapping_repo
        .delete(created.id)
        .await
        .expect("Failed to delete");

    let result = ctx
        .mapping_repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed");
    assert!(result.is_none());
}

pub async fn test_delete_not_found(ctx: &SsoGroupMappingTestContext<'_>) {
    let result = ctx.mapping_repo.delete(Uuid::new_v4()).await;
    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_delete_by_idp_group(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let team1_id = ctx.create_test_team(org_id, "team-1").await;
    let team2_id = ctx.create_test_team(org_id, "team-2").await;

    // Create multiple mappings for same IdP group (different teams)
    for team_id in [team1_id, team2_id] {
        let input = create_mapping_input("default", "Engineering", Some(team_id), None);
        ctx.mapping_repo
            .create(org_id, input)
            .await
            .expect("Failed to create");
    }

    // Also create a mapping for a different group
    let input = create_mapping_input("default", "Other", None, None);
    ctx.mapping_repo
        .create(org_id, input)
        .await
        .expect("Failed to create");

    // Delete all Engineering mappings
    let deleted_count = ctx
        .mapping_repo
        .delete_by_idp_group("default", org_id, "Engineering")
        .await
        .expect("Failed to delete");

    assert_eq!(deleted_count, 2);

    // Verify only "Other" remains
    let remaining = ctx
        .mapping_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list");
    assert_eq!(remaining.items.len(), 1);
    assert_eq!(remaining.items[0].idp_group, "Other");
}

pub async fn test_delete_by_idp_group_filters_by_connection(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    // Same group name in different connections
    ctx.mapping_repo
        .create(
            org_id,
            create_mapping_input("okta", "Engineering", None, None),
        )
        .await
        .expect("Failed to create");
    ctx.mapping_repo
        .create(
            org_id,
            create_mapping_input("azure", "Engineering", None, None),
        )
        .await
        .expect("Failed to create");

    // Delete only from okta connection
    let deleted_count = ctx
        .mapping_repo
        .delete_by_idp_group("okta", org_id, "Engineering")
        .await
        .expect("Failed to delete");

    assert_eq!(deleted_count, 1);

    // Azure mapping should still exist
    let remaining = ctx
        .mapping_repo
        .list_by_connection("azure", org_id, ListParams::default())
        .await
        .expect("Failed to list");
    assert_eq!(remaining.items.len(), 1);
}

pub async fn test_delete_by_idp_group_no_matches(ctx: &SsoGroupMappingTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let deleted_count = ctx
        .mapping_repo
        .delete_by_idp_group("default", org_id, "NonExistent")
        .await
        .expect("Failed to delete");

    assert_eq!(deleted_count, 0);
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use super::*;
    use crate::db::{
        sqlite::{SqliteOrganizationRepo, SqliteSsoGroupMappingRepo, SqliteTeamRepo},
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repos() -> (
        SqliteSsoGroupMappingRepo,
        SqliteOrganizationRepo,
        SqliteTeamRepo,
    ) {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        (
            SqliteSsoGroupMappingRepo::new(pool.clone()),
            SqliteOrganizationRepo::new(pool.clone()),
            SqliteTeamRepo::new(pool),
        )
    }

    macro_rules! sqlite_test {
        ($name:ident) => {
            #[tokio::test]
            async fn $name() {
                let (mapping_repo, org_repo, team_repo) = create_repos().await;
                let ctx = SsoGroupMappingTestContext {
                    mapping_repo: &mapping_repo,
                    org_repo: &org_repo,
                    team_repo: &team_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    // Create tests
    sqlite_test!(test_create_mapping_without_team);
    sqlite_test!(test_create_mapping_with_team);
    sqlite_test!(test_create_duplicate_mapping_fails);
    sqlite_test!(test_same_group_different_teams_allowed);
    sqlite_test!(test_same_group_different_orgs_allowed);

    // Get/List tests
    sqlite_test!(test_get_by_id);
    sqlite_test!(test_get_by_id_not_found);
    sqlite_test!(test_list_by_org_empty);
    sqlite_test!(test_list_by_org);
    sqlite_test!(test_list_by_org_filters_by_org);
    sqlite_test!(test_list_by_connection);
    sqlite_test!(test_list_by_connection_filters_by_org);

    // Find mappings for groups tests
    sqlite_test!(test_find_mappings_for_groups);
    sqlite_test!(test_find_mappings_empty_groups);
    sqlite_test!(test_find_mappings_no_matches);
    sqlite_test!(test_find_mappings_wrong_connection);
    sqlite_test!(test_find_mappings_returns_multiple_for_same_group);

    // Count tests
    sqlite_test!(test_count_by_org_empty);
    sqlite_test!(test_count_by_org);
    sqlite_test!(test_count_filters_by_org);

    // Update tests
    sqlite_test!(test_update_mapping);
    sqlite_test!(test_update_partial);
    sqlite_test!(test_update_clear_optional_fields);
    sqlite_test!(test_update_no_changes);
    sqlite_test!(test_update_not_found);

    // Delete tests
    sqlite_test!(test_delete_mapping);
    sqlite_test!(test_delete_not_found);
    sqlite_test!(test_delete_by_idp_group);
    sqlite_test!(test_delete_by_idp_group_filters_by_connection);
    sqlite_test!(test_delete_by_idp_group_no_matches);
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use super::*;
    use crate::db::{
        postgres::{PostgresOrganizationRepo, PostgresSsoGroupMappingRepo, PostgresTeamRepo},
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let pool = create_isolated_postgres_pool().await;
                run_postgres_migrations(&pool).await;
                let mapping_repo = PostgresSsoGroupMappingRepo::new(pool.clone(), None);
                let org_repo = PostgresOrganizationRepo::new(pool.clone(), None);
                let team_repo = PostgresTeamRepo::new(pool, None);
                let ctx = SsoGroupMappingTestContext {
                    mapping_repo: &mapping_repo,
                    org_repo: &org_repo,
                    team_repo: &team_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    // Create tests
    postgres_test!(test_create_mapping_without_team);
    postgres_test!(test_create_mapping_with_team);
    postgres_test!(test_create_duplicate_mapping_fails);
    postgres_test!(test_same_group_different_teams_allowed);
    postgres_test!(test_same_group_different_orgs_allowed);

    // Get/List tests
    postgres_test!(test_get_by_id);
    postgres_test!(test_get_by_id_not_found);
    postgres_test!(test_list_by_org_empty);
    postgres_test!(test_list_by_org);
    postgres_test!(test_list_by_org_filters_by_org);
    postgres_test!(test_list_by_connection);
    postgres_test!(test_list_by_connection_filters_by_org);

    // Find mappings for groups tests
    postgres_test!(test_find_mappings_for_groups);
    postgres_test!(test_find_mappings_empty_groups);
    postgres_test!(test_find_mappings_no_matches);
    postgres_test!(test_find_mappings_wrong_connection);
    postgres_test!(test_find_mappings_returns_multiple_for_same_group);

    // Count tests
    postgres_test!(test_count_by_org_empty);
    postgres_test!(test_count_by_org);
    postgres_test!(test_count_filters_by_org);

    // Update tests
    postgres_test!(test_update_mapping);
    postgres_test!(test_update_partial);
    postgres_test!(test_update_clear_optional_fields);
    postgres_test!(test_update_no_changes);
    postgres_test!(test_update_not_found);

    // Delete tests
    postgres_test!(test_delete_mapping);
    postgres_test!(test_delete_not_found);
    postgres_test!(test_delete_by_idp_group);
    postgres_test!(test_delete_by_idp_group_filters_by_connection);
    postgres_test!(test_delete_by_idp_group_no_matches);
}
