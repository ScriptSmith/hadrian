//! Shared tests for TeamRepo implementations
//!
//! Tests are written as async functions that take a test context containing
//! both the team repo and utilities for creating test organizations and users.

use uuid::Uuid;

use crate::{
    db::{
        error::DbError,
        repos::{ListParams, OrganizationRepo, TeamRepo, UserRepo},
    },
    models::{
        AddTeamMember, CreateOrganization, CreateTeam, CreateUser, MembershipSource, UpdateTeam,
        UpdateTeamMember,
    },
};

// ============================================================================
// Test Input Helpers
// ============================================================================

fn create_team_input(slug: &str, name: &str) -> CreateTeam {
    CreateTeam {
        slug: slug.to_string(),
        name: name.to_string(),
    }
}

fn create_org_input(slug: &str, name: &str) -> CreateOrganization {
    CreateOrganization {
        slug: slug.to_string(),
        name: name.to_string(),
    }
}

fn create_user_input(external_id: &str) -> CreateUser {
    CreateUser {
        external_id: external_id.to_string(),
        email: Some(format!("{}@example.com", external_id)),
        name: Some(format!("User {}", external_id)),
    }
}

/// Test context containing repos needed for team tests
pub struct TeamTestContext<'a> {
    pub team_repo: &'a dyn TeamRepo,
    pub org_repo: &'a dyn OrganizationRepo,
    pub user_repo: &'a dyn UserRepo,
}

impl<'a> TeamTestContext<'a> {
    /// Create a test organization and return its ID
    pub async fn create_test_org(&self, slug: &str) -> Uuid {
        self.org_repo
            .create(create_org_input(slug, &format!("Org {}", slug)))
            .await
            .expect("Failed to create test org")
            .id
    }

    /// Create a test user and return their ID
    pub async fn create_test_user(&self, external_id: &str) -> Uuid {
        self.user_repo
            .create(create_user_input(external_id))
            .await
            .expect("Failed to create test user")
            .id
    }
}

// ============================================================================
// Team CRUD Tests
// ============================================================================

pub async fn test_create_team(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_team_input("test-team", "Test Team");
    let team = ctx
        .team_repo
        .create(org_id, input)
        .await
        .expect("Failed to create team");

    assert_eq!(team.slug, "test-team");
    assert_eq!(team.name, "Test Team");
    assert_eq!(team.org_id, org_id);
    assert!(!team.id.is_nil());
}

pub async fn test_create_duplicate_slug_same_org_fails(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_team_input("duplicate", "First Team");
    ctx.team_repo
        .create(org_id, input)
        .await
        .expect("Failed to create first team");

    let input2 = create_team_input("duplicate", "Second Team");
    let result = ctx.team_repo.create(org_id, input2).await;

    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_create_same_slug_different_orgs_succeeds(ctx: &TeamTestContext<'_>) {
    let org1_id = ctx.create_test_org("org-1").await;
    let org2_id = ctx.create_test_org("org-2").await;

    let input1 = create_team_input("same-slug", "Team in Org 1");
    let team1 = ctx
        .team_repo
        .create(org1_id, input1)
        .await
        .expect("Failed to create team in org 1");

    let input2 = create_team_input("same-slug", "Team in Org 2");
    let team2 = ctx
        .team_repo
        .create(org2_id, input2)
        .await
        .expect("Failed to create team in org 2");

    assert_eq!(team1.slug, team2.slug);
    assert_ne!(team1.id, team2.id);
    assert_ne!(team1.org_id, team2.org_id);
}

pub async fn test_get_by_id(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_team_input("get-test", "Get Test Team");
    let created = ctx
        .team_repo
        .create(org_id, input)
        .await
        .expect("Failed to create team");

    let fetched = ctx
        .team_repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get team")
        .expect("Team should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.org_id, org_id);
    assert_eq!(fetched.slug, "get-test");
    assert_eq!(fetched.name, "Get Test Team");
}

pub async fn test_get_by_id_not_found(ctx: &TeamTestContext<'_>) {
    let result = ctx
        .team_repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_slug(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_team_input("slug-test", "Slug Test Team");
    let created = ctx
        .team_repo
        .create(org_id, input)
        .await
        .expect("Failed to create team");

    let fetched = ctx
        .team_repo
        .get_by_slug(org_id, "slug-test")
        .await
        .expect("Failed to get team")
        .expect("Team should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.slug, "slug-test");
}

pub async fn test_get_by_slug_wrong_org(ctx: &TeamTestContext<'_>) {
    let org1_id = ctx.create_test_org("org-1").await;
    let org2_id = ctx.create_test_org("org-2").await;

    let input = create_team_input("team-slug", "Test Team");
    ctx.team_repo
        .create(org1_id, input)
        .await
        .expect("Failed to create team");

    let result = ctx
        .team_repo
        .get_by_slug(org2_id, "team-slug")
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_slug_not_found(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let result = ctx
        .team_repo
        .get_by_slug(org_id, "nonexistent")
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_list_by_org_empty(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let result = ctx
        .team_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list teams");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_by_org_with_teams(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..3 {
        ctx.team_repo
            .create(
                org_id,
                create_team_input(&format!("team-{}", i), &format!("Team {}", i)),
            )
            .await
            .expect("Failed to create team");
    }

    let result = ctx
        .team_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list teams");

    assert_eq!(result.items.len(), 3);
    assert!(!result.has_more);
}

pub async fn test_list_by_org_filters_by_org(ctx: &TeamTestContext<'_>) {
    let org1_id = ctx.create_test_org("org-1").await;
    let org2_id = ctx.create_test_org("org-2").await;

    ctx.team_repo
        .create(org1_id, create_team_input("team-1", "Org1 Team"))
        .await
        .expect("Failed to create team");
    ctx.team_repo
        .create(org2_id, create_team_input("team-2", "Org2 Team"))
        .await
        .expect("Failed to create team");

    let org1_result = ctx
        .team_repo
        .list_by_org(org1_id, ListParams::default())
        .await
        .expect("Failed to list");
    let org2_result = ctx
        .team_repo
        .list_by_org(org2_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(org1_result.items.len(), 1);
    assert_eq!(org1_result.items[0].name, "Org1 Team");
    assert_eq!(org2_result.items.len(), 1);
    assert_eq!(org2_result.items[0].name, "Org2 Team");
}

pub async fn test_list_by_org_with_pagination(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..5 {
        ctx.team_repo
            .create(
                org_id,
                create_team_input(&format!("team-{}", i), &format!("Team {}", i)),
            )
            .await
            .expect("Failed to create team");
    }

    let page1 = ctx
        .team_repo
        .list_by_org(
            org_id,
            ListParams {
                limit: Some(2),
                ..Default::default()
            },
        )
        .await
        .expect("Failed to list page 1");

    let page2 = ctx
        .team_repo
        .list_by_org(
            org_id,
            ListParams {
                limit: Some(2),
                cursor: page1.cursors.next.clone(),
                ..Default::default()
            },
        )
        .await
        .expect("Failed to list page 2");

    assert_eq!(page1.items.len(), 2);
    assert_eq!(page2.items.len(), 2);
    assert!(page1.has_more);
    assert!(page2.has_more);
    assert_ne!(page1.items[0].id, page2.items[0].id);
}

pub async fn test_count_by_org_empty(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let count = ctx
        .team_repo
        .count_by_org(org_id, false)
        .await
        .expect("Failed to count");
    assert_eq!(count, 0);
}

pub async fn test_count_by_org_with_teams(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..3 {
        ctx.team_repo
            .create(
                org_id,
                create_team_input(&format!("team-{}", i), &format!("Team {}", i)),
            )
            .await
            .expect("Failed to create team");
    }

    let count = ctx
        .team_repo
        .count_by_org(org_id, false)
        .await
        .expect("Failed to count");
    assert_eq!(count, 3);
}

pub async fn test_update_name(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .team_repo
        .create(org_id, create_team_input("update-test", "Original Name"))
        .await
        .expect("Failed to create team");

    let updated = ctx
        .team_repo
        .update(
            created.id,
            UpdateTeam {
                name: Some("Updated Name".to_string()),
            },
        )
        .await
        .expect("Failed to update team");

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.slug, "update-test");
    assert_eq!(updated.name, "Updated Name");
    assert!(updated.updated_at >= created.updated_at);
}

pub async fn test_update_no_changes(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .team_repo
        .create(org_id, create_team_input("no-change", "Original"))
        .await
        .expect("Failed to create team");

    let result = ctx
        .team_repo
        .update(created.id, UpdateTeam { name: None })
        .await
        .expect("Failed to update team");

    assert_eq!(result.name, "Original");
}

pub async fn test_update_not_found(ctx: &TeamTestContext<'_>) {
    let result = ctx
        .team_repo
        .update(
            Uuid::new_v4(),
            UpdateTeam {
                name: Some("New Name".to_string()),
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_delete(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .team_repo
        .create(org_id, create_team_input("delete-test", "To Delete"))
        .await
        .expect("Failed to create team");

    ctx.team_repo
        .delete(created.id)
        .await
        .expect("Failed to delete team");

    let result = ctx
        .team_repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed");
    assert!(result.is_none());

    let result = ctx
        .team_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list");
    assert!(result.items.is_empty());
}

pub async fn test_delete_not_found(ctx: &TeamTestContext<'_>) {
    let result = ctx.team_repo.delete(Uuid::new_v4()).await;
    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_delete_already_deleted(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .team_repo
        .create(org_id, create_team_input("double-delete", "Delete Twice"))
        .await
        .expect("Failed to create team");

    ctx.team_repo
        .delete(created.id)
        .await
        .expect("First delete should succeed");
    let result = ctx.team_repo.delete(created.id).await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_count_excludes_deleted(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let team1 = ctx
        .team_repo
        .create(org_id, create_team_input("team-1", "Team 1"))
        .await
        .expect("Failed to create team 1");
    ctx.team_repo
        .create(org_id, create_team_input("team-2", "Team 2"))
        .await
        .expect("Failed to create team 2");

    ctx.team_repo
        .delete(team1.id)
        .await
        .expect("Failed to delete");

    let count = ctx
        .team_repo
        .count_by_org(org_id, false)
        .await
        .expect("Failed to count");
    assert_eq!(count, 1);

    let count_all = ctx
        .team_repo
        .count_by_org(org_id, true)
        .await
        .expect("Failed to count all");
    assert_eq!(count_all, 2);
}

pub async fn test_list_include_deleted(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let team1 = ctx
        .team_repo
        .create(org_id, create_team_input("team-1", "Team 1"))
        .await
        .expect("Failed to create team 1");
    ctx.team_repo
        .create(org_id, create_team_input("team-2", "Team 2"))
        .await
        .expect("Failed to create team 2");

    ctx.team_repo
        .delete(team1.id)
        .await
        .expect("Failed to delete");

    let active = ctx
        .team_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list active");
    assert_eq!(active.items.len(), 1);

    let all = ctx
        .team_repo
        .list_by_org(
            org_id,
            ListParams {
                include_deleted: true,
                ..Default::default()
            },
        )
        .await
        .expect("Failed to list all");
    assert_eq!(all.items.len(), 2);
}

pub async fn test_get_by_slug_excludes_deleted(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .team_repo
        .create(org_id, create_team_input("deleted-slug", "Will Be Deleted"))
        .await
        .expect("Failed to create team");

    ctx.team_repo
        .delete(created.id)
        .await
        .expect("Failed to delete");

    let result = ctx
        .team_repo
        .get_by_slug(org_id, "deleted-slug")
        .await
        .expect("Query should succeed");
    assert!(result.is_none());
}

pub async fn test_update_deleted_team_fails(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .team_repo
        .create(
            org_id,
            create_team_input("update-deleted", "Will Be Deleted"),
        )
        .await
        .expect("Failed to create team");

    ctx.team_repo
        .delete(created.id)
        .await
        .expect("Failed to delete");

    let result = ctx
        .team_repo
        .update(
            created.id,
            UpdateTeam {
                name: Some("New Name".to_string()),
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

// ============================================================================
// Team Membership Tests
// ============================================================================

pub async fn test_add_member(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("test-user").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    let member = ctx
        .team_repo
        .add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

    assert_eq!(member.user_id, user_id);
    assert_eq!(member.role, "member");
    assert_eq!(member.external_id, "test-user");
}

pub async fn test_add_member_with_admin_role(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("admin-user").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    let member = ctx
        .team_repo
        .add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "admin".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

    assert_eq!(member.role, "admin");
}

pub async fn test_add_member_duplicate_fails(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("test-user").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    ctx.team_repo
        .add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

    let result = ctx
        .team_repo
        .add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_remove_member(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("test-user").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    ctx.team_repo
        .add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

    ctx.team_repo
        .remove_member(team.id, user_id)
        .await
        .expect("Failed to remove member");

    let is_member = ctx
        .team_repo
        .is_member(team.id, user_id)
        .await
        .expect("Failed to check membership");
    assert!(!is_member);
}

pub async fn test_remove_member_not_found(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("test-user").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    let result = ctx.team_repo.remove_member(team.id, user_id).await;
    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_update_member_role(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("test-user").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    ctx.team_repo
        .add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

    let updated = ctx
        .team_repo
        .update_member_role(
            team.id,
            user_id,
            UpdateTeamMember {
                role: "admin".to_string(),
            },
        )
        .await
        .expect("Failed to update role");

    assert_eq!(updated.role, "admin");
}

pub async fn test_update_member_role_not_found(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("test-user").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    let result = ctx
        .team_repo
        .update_member_role(
            team.id,
            user_id,
            UpdateTeamMember {
                role: "admin".to_string(),
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_list_members_empty(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    let result = ctx
        .team_repo
        .list_members(team.id, ListParams::default())
        .await
        .expect("Failed to list members");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_members(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    for i in 0..3 {
        let user_id = ctx.create_test_user(&format!("user-{}", i)).await;
        ctx.team_repo
            .add_member(
                team.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");
    }

    let result = ctx
        .team_repo
        .list_members(team.id, ListParams::default())
        .await
        .expect("Failed to list members");

    assert_eq!(result.items.len(), 3);
}

pub async fn test_list_members_with_pagination(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    for i in 0..5 {
        let user_id = ctx.create_test_user(&format!("user-{}", i)).await;
        ctx.team_repo
            .add_member(
                team.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");
    }

    let page1 = ctx
        .team_repo
        .list_members(
            team.id,
            ListParams {
                limit: Some(2),
                ..Default::default()
            },
        )
        .await
        .expect("Failed to list page 1");

    let page2 = ctx
        .team_repo
        .list_members(
            team.id,
            ListParams {
                limit: Some(2),
                cursor: page1.cursors.next.clone(),
                ..Default::default()
            },
        )
        .await
        .expect("Failed to list page 2");

    assert_eq!(page1.items.len(), 2);
    assert_eq!(page2.items.len(), 2);
    assert!(page1.has_more);
    assert!(page2.has_more);
    assert_ne!(page1.items[0].user_id, page2.items[0].user_id);
}

pub async fn test_get_member(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("test-user").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    ctx.team_repo
        .add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "admin".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

    let member = ctx
        .team_repo
        .get_member(team.id, user_id)
        .await
        .expect("Failed to get member")
        .expect("Member should exist");

    assert_eq!(member.user_id, user_id);
    assert_eq!(member.role, "admin");
    assert_eq!(member.external_id, "test-user");
}

pub async fn test_get_member_not_found(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("test-user").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    let result = ctx
        .team_repo
        .get_member(team.id, user_id)
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_is_member(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("test-user").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    assert!(
        !ctx.team_repo
            .is_member(team.id, user_id)
            .await
            .expect("Failed to check membership")
    );

    ctx.team_repo
        .add_member(
            team.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add member");

    assert!(
        ctx.team_repo
            .is_member(team.id, user_id)
            .await
            .expect("Failed to check membership")
    );
}

pub async fn test_count_members_empty(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    let count = ctx
        .team_repo
        .count_members(team.id)
        .await
        .expect("Failed to count");
    assert_eq!(count, 0);
}

pub async fn test_count_members(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let team = ctx
        .team_repo
        .create(org_id, create_team_input("test-team", "Test Team"))
        .await
        .expect("Failed to create team");

    for i in 0..3 {
        let user_id = ctx.create_test_user(&format!("user-{}", i)).await;
        ctx.team_repo
            .add_member(
                team.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");
    }

    let count = ctx
        .team_repo
        .count_members(team.id)
        .await
        .expect("Failed to count");
    assert_eq!(count, 3);
}

pub async fn test_user_can_be_in_multiple_teams(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = ctx.create_test_user("multi-team-user").await;

    let team1 = ctx
        .team_repo
        .create(org_id, create_team_input("team-1", "Team 1"))
        .await
        .expect("Failed to create team 1");
    let team2 = ctx
        .team_repo
        .create(org_id, create_team_input("team-2", "Team 2"))
        .await
        .expect("Failed to create team 2");

    ctx.team_repo
        .add_member(
            team1.id,
            AddTeamMember {
                user_id,
                role: "member".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add to team 1");
    ctx.team_repo
        .add_member(
            team2.id,
            AddTeamMember {
                user_id,
                role: "admin".to_string(),
                source: MembershipSource::Manual,
            },
        )
        .await
        .expect("Failed to add to team 2");

    assert!(
        ctx.team_repo
            .is_member(team1.id, user_id)
            .await
            .expect("Failed to check")
    );
    assert!(
        ctx.team_repo
            .is_member(team2.id, user_id)
            .await
            .expect("Failed to check")
    );

    let member1 = ctx
        .team_repo
        .get_member(team1.id, user_id)
        .await
        .expect("Query failed")
        .expect("Should be member");
    let member2 = ctx
        .team_repo
        .get_member(team2.id, user_id)
        .await
        .expect("Query failed")
        .expect("Should be member");

    assert_eq!(member1.role, "member");
    assert_eq!(member2.role, "admin");
}

pub async fn test_members_isolated_by_team(ctx: &TeamTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let team1 = ctx
        .team_repo
        .create(org_id, create_team_input("team-1", "Team 1"))
        .await
        .expect("Failed to create team 1");
    let team2 = ctx
        .team_repo
        .create(org_id, create_team_input("team-2", "Team 2"))
        .await
        .expect("Failed to create team 2");

    // Add 2 users to team 1
    for i in 0..2 {
        let user_id = ctx.create_test_user(&format!("t1-user-{}", i)).await;
        ctx.team_repo
            .add_member(
                team1.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");
    }

    // Add 3 users to team 2
    for i in 0..3 {
        let user_id = ctx.create_test_user(&format!("t2-user-{}", i)).await;
        ctx.team_repo
            .add_member(
                team2.id,
                AddTeamMember {
                    user_id,
                    role: "member".to_string(),
                    source: MembershipSource::Manual,
                },
            )
            .await
            .expect("Failed to add member");
    }

    assert_eq!(
        ctx.team_repo
            .count_members(team1.id)
            .await
            .expect("Failed to count"),
        2
    );
    assert_eq!(
        ctx.team_repo
            .count_members(team2.id)
            .await
            .expect("Failed to count"),
        3
    );
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use super::*;
    use crate::db::{
        sqlite::{SqliteOrganizationRepo, SqliteTeamRepo, SqliteUserRepo},
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repos() -> (SqliteTeamRepo, SqliteOrganizationRepo, SqliteUserRepo) {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        (
            SqliteTeamRepo::new(pool.clone()),
            SqliteOrganizationRepo::new(pool.clone()),
            SqliteUserRepo::new(pool),
        )
    }

    macro_rules! sqlite_test {
        ($name:ident) => {
            #[tokio::test]
            async fn $name() {
                let (team_repo, org_repo, user_repo) = create_repos().await;
                let ctx = TeamTestContext {
                    team_repo: &team_repo,
                    org_repo: &org_repo,
                    user_repo: &user_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    // Team CRUD tests
    sqlite_test!(test_create_team);
    sqlite_test!(test_create_duplicate_slug_same_org_fails);
    sqlite_test!(test_create_same_slug_different_orgs_succeeds);
    sqlite_test!(test_get_by_id);
    sqlite_test!(test_get_by_id_not_found);
    sqlite_test!(test_get_by_slug);
    sqlite_test!(test_get_by_slug_wrong_org);
    sqlite_test!(test_get_by_slug_not_found);
    sqlite_test!(test_list_by_org_empty);
    sqlite_test!(test_list_by_org_with_teams);
    sqlite_test!(test_list_by_org_filters_by_org);
    sqlite_test!(test_list_by_org_with_pagination);
    sqlite_test!(test_count_by_org_empty);
    sqlite_test!(test_count_by_org_with_teams);
    sqlite_test!(test_update_name);
    sqlite_test!(test_update_no_changes);
    sqlite_test!(test_update_not_found);
    sqlite_test!(test_delete);
    sqlite_test!(test_delete_not_found);
    sqlite_test!(test_delete_already_deleted);
    sqlite_test!(test_count_excludes_deleted);
    sqlite_test!(test_list_include_deleted);
    sqlite_test!(test_get_by_slug_excludes_deleted);
    sqlite_test!(test_update_deleted_team_fails);

    // Team membership tests
    sqlite_test!(test_add_member);
    sqlite_test!(test_add_member_with_admin_role);
    sqlite_test!(test_add_member_duplicate_fails);
    sqlite_test!(test_remove_member);
    sqlite_test!(test_remove_member_not_found);
    sqlite_test!(test_update_member_role);
    sqlite_test!(test_update_member_role_not_found);
    sqlite_test!(test_list_members_empty);
    sqlite_test!(test_list_members);
    sqlite_test!(test_list_members_with_pagination);
    sqlite_test!(test_get_member);
    sqlite_test!(test_get_member_not_found);
    sqlite_test!(test_is_member);
    sqlite_test!(test_count_members_empty);
    sqlite_test!(test_count_members);
    sqlite_test!(test_user_can_be_in_multiple_teams);
    sqlite_test!(test_members_isolated_by_team);
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use super::*;
    use crate::db::{
        postgres::{PostgresOrganizationRepo, PostgresTeamRepo, PostgresUserRepo},
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let pool = create_isolated_postgres_pool().await;
                run_postgres_migrations(&pool).await;
                let team_repo = PostgresTeamRepo::new(pool.clone(), None);
                let org_repo = PostgresOrganizationRepo::new(pool.clone(), None);
                let user_repo = PostgresUserRepo::new(pool, None);
                let ctx = TeamTestContext {
                    team_repo: &team_repo,
                    org_repo: &org_repo,
                    user_repo: &user_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    // Team CRUD tests
    postgres_test!(test_create_team);
    postgres_test!(test_create_duplicate_slug_same_org_fails);
    postgres_test!(test_create_same_slug_different_orgs_succeeds);
    postgres_test!(test_get_by_id);
    postgres_test!(test_get_by_id_not_found);
    postgres_test!(test_get_by_slug);
    postgres_test!(test_get_by_slug_wrong_org);
    postgres_test!(test_get_by_slug_not_found);
    postgres_test!(test_list_by_org_empty);
    postgres_test!(test_list_by_org_with_teams);
    postgres_test!(test_list_by_org_filters_by_org);
    postgres_test!(test_list_by_org_with_pagination);
    postgres_test!(test_count_by_org_empty);
    postgres_test!(test_count_by_org_with_teams);
    postgres_test!(test_update_name);
    postgres_test!(test_update_no_changes);
    postgres_test!(test_update_not_found);
    postgres_test!(test_delete);
    postgres_test!(test_delete_not_found);
    postgres_test!(test_delete_already_deleted);
    postgres_test!(test_count_excludes_deleted);
    postgres_test!(test_list_include_deleted);
    postgres_test!(test_get_by_slug_excludes_deleted);
    postgres_test!(test_update_deleted_team_fails);

    // Team membership tests
    postgres_test!(test_add_member);
    postgres_test!(test_add_member_with_admin_role);
    postgres_test!(test_add_member_duplicate_fails);
    postgres_test!(test_remove_member);
    postgres_test!(test_remove_member_not_found);
    postgres_test!(test_update_member_role);
    postgres_test!(test_update_member_role_not_found);
    postgres_test!(test_list_members_empty);
    postgres_test!(test_list_members);
    postgres_test!(test_list_members_with_pagination);
    postgres_test!(test_get_member);
    postgres_test!(test_get_member_not_found);
    postgres_test!(test_is_member);
    postgres_test!(test_count_members_empty);
    postgres_test!(test_count_members);
    postgres_test!(test_user_can_be_in_multiple_teams);
    postgres_test!(test_members_isolated_by_team);
}
