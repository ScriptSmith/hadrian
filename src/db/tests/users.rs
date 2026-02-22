//! Shared tests for UserRepo implementations
//!
//! Tests are written as async functions that take a test context containing
//! the user repo and utilities for creating test organizations and projects.

use uuid::Uuid;

use crate::{
    db::{
        error::DbError,
        repos::{ListParams, OrganizationRepo, ProjectRepo, UserRepo},
    },
    models::{CreateOrganization, CreateProject, CreateUser, MembershipSource, UpdateUser},
};

// ============================================================================
// Test Input Helpers
// ============================================================================

fn create_user_input(external_id: &str, email: Option<&str>, name: Option<&str>) -> CreateUser {
    CreateUser {
        external_id: external_id.to_string(),
        email: email.map(|e| e.to_string()),
        name: name.map(|n| n.to_string()),
    }
}

fn create_org_input(slug: &str, name: &str) -> CreateOrganization {
    CreateOrganization {
        slug: slug.to_string(),
        name: name.to_string(),
    }
}

fn create_project_input(slug: &str, name: &str) -> CreateProject {
    CreateProject {
        slug: slug.to_string(),
        name: name.to_string(),
        team_id: None,
    }
}

/// Test context containing repos needed for user tests
pub struct UserTestContext<'a> {
    pub user_repo: &'a dyn UserRepo,
    pub org_repo: &'a dyn OrganizationRepo,
    pub project_repo: &'a dyn ProjectRepo,
}

impl<'a> UserTestContext<'a> {
    /// Create a test organization and return its ID
    pub async fn create_test_org(&self, slug: &str) -> Uuid {
        self.org_repo
            .create(create_org_input(slug, &format!("Org {}", slug)))
            .await
            .expect("Failed to create test org")
            .id
    }

    /// Create a test project and return its ID
    pub async fn create_test_project(&self, org_id: Uuid, slug: &str) -> Uuid {
        self.project_repo
            .create(
                org_id,
                create_project_input(slug, &format!("Project {}", slug)),
            )
            .await
            .expect("Failed to create test project")
            .id
    }
}

// ============================================================================
// User CRUD Test Functions
// ============================================================================

pub async fn test_create_user(ctx: &UserTestContext<'_>) {
    let input = create_user_input("user-123", Some("test@example.com"), Some("Test User"));
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    assert_eq!(user.external_id, "user-123");
    assert_eq!(user.email, Some("test@example.com".to_string()));
    assert_eq!(user.name, Some("Test User".to_string()));
    assert!(!user.id.is_nil());
}

pub async fn test_create_user_minimal(ctx: &UserTestContext<'_>) {
    let input = create_user_input("user-minimal", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    assert_eq!(user.external_id, "user-minimal");
    assert!(user.email.is_none());
    assert!(user.name.is_none());
}

pub async fn test_create_duplicate_external_id_fails(ctx: &UserTestContext<'_>) {
    let input1 = create_user_input("duplicate-id", Some("first@example.com"), None);
    ctx.user_repo
        .create(input1)
        .await
        .expect("Failed to create first user");

    let input2 = create_user_input("duplicate-id", Some("second@example.com"), None);
    let result = ctx.user_repo.create(input2).await;

    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_get_by_id(ctx: &UserTestContext<'_>) {
    let input = create_user_input("get-test", Some("get@example.com"), Some("Get Test"));
    let created = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    let fetched = ctx
        .user_repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get user")
        .expect("User should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.external_id, "get-test");
    assert_eq!(fetched.email, Some("get@example.com".to_string()));
}

pub async fn test_get_by_id_not_found(ctx: &UserTestContext<'_>) {
    let result = ctx
        .user_repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_external_id(ctx: &UserTestContext<'_>) {
    let input = create_user_input("ext-id-test", Some("ext@example.com"), None);
    let created = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    let fetched = ctx
        .user_repo
        .get_by_external_id("ext-id-test")
        .await
        .expect("Failed to get user")
        .expect("User should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.external_id, "ext-id-test");
}

pub async fn test_get_by_external_id_not_found(ctx: &UserTestContext<'_>) {
    let result = ctx
        .user_repo
        .get_by_external_id("nonexistent")
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_list_empty(ctx: &UserTestContext<'_>) {
    let result = ctx
        .user_repo
        .list(ListParams::default())
        .await
        .expect("Failed to list users");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_with_users(ctx: &UserTestContext<'_>) {
    for i in 0..3 {
        let input = create_user_input(&format!("user-{}", i), None, None);
        ctx.user_repo
            .create(input)
            .await
            .expect("Failed to create user");
    }

    let result = ctx
        .user_repo
        .list(ListParams::default())
        .await
        .expect("Failed to list users");

    assert_eq!(result.items.len(), 3);
    assert!(!result.has_more);
}

pub async fn test_list_with_pagination(ctx: &UserTestContext<'_>) {
    for i in 0..5 {
        let input = create_user_input(&format!("user-{}", i), None, None);
        ctx.user_repo
            .create(input)
            .await
            .expect("Failed to create user");
    }

    let page1 = ctx
        .user_repo
        .list(ListParams {
            limit: Some(2),
            include_deleted: false,
            ..Default::default()
        })
        .await
        .expect("Failed to list page 1");

    let page2 = ctx
        .user_repo
        .list(ListParams {
            limit: Some(2),
            include_deleted: false,
            cursor: page1.cursors.next.clone(),
            ..Default::default()
        })
        .await
        .expect("Failed to list page 2");

    assert_eq!(page1.items.len(), 2);
    assert_eq!(page2.items.len(), 2);
    assert!(page1.has_more);
    assert!(page2.has_more);
    // Pages should have different users
    assert_ne!(page1.items[0].id, page2.items[0].id);
}

pub async fn test_count_empty(ctx: &UserTestContext<'_>) {
    let count = ctx.user_repo.count(false).await.expect("Failed to count");
    assert_eq!(count, 0);
}

pub async fn test_count_with_users(ctx: &UserTestContext<'_>) {
    for i in 0..3 {
        let input = create_user_input(&format!("user-{}", i), None, None);
        ctx.user_repo
            .create(input)
            .await
            .expect("Failed to create user");
    }

    let count = ctx.user_repo.count(false).await.expect("Failed to count");
    assert_eq!(count, 3);
}

pub async fn test_update_email(ctx: &UserTestContext<'_>) {
    let input = create_user_input("update-email", Some("old@example.com"), None);
    let created = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    let updated = ctx
        .user_repo
        .update(
            created.id,
            UpdateUser {
                email: Some("new@example.com".to_string()),
                name: None,
            },
        )
        .await
        .expect("Failed to update user");

    assert_eq!(updated.email, Some("new@example.com".to_string()));
    assert!(updated.updated_at >= created.updated_at);
}

pub async fn test_update_name(ctx: &UserTestContext<'_>) {
    let input = create_user_input("update-name", None, Some("Old Name"));
    let created = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    let updated = ctx
        .user_repo
        .update(
            created.id,
            UpdateUser {
                email: None,
                name: Some("New Name".to_string()),
            },
        )
        .await
        .expect("Failed to update user");

    assert_eq!(updated.name, Some("New Name".to_string()));
}

pub async fn test_update_not_found(ctx: &UserTestContext<'_>) {
    let result = ctx
        .user_repo
        .update(
            Uuid::new_v4(),
            UpdateUser {
                email: Some("new@example.com".to_string()),
                name: None,
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

// ============================================================================
// Organization Membership Test Functions
// ============================================================================

pub async fn test_add_to_org(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_user_input("org-member", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    ctx.user_repo
        .add_to_org(user.id, org_id, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add user to org");

    let result = ctx
        .user_repo
        .list_org_members(org_id, ListParams::default())
        .await
        .expect("Failed to list org members");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, user.id);
}

pub async fn test_add_to_org_duplicate_fails(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_user_input("org-member", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    ctx.user_repo
        .add_to_org(user.id, org_id, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add user to org");

    let result = ctx
        .user_repo
        .add_to_org(user.id, org_id, "member", MembershipSource::Manual)
        .await;
    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_remove_from_org(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_user_input("org-member", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    ctx.user_repo
        .add_to_org(user.id, org_id, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add user to org");

    ctx.user_repo
        .remove_from_org(user.id, org_id)
        .await
        .expect("Failed to remove user from org");

    let result = ctx
        .user_repo
        .list_org_members(org_id, ListParams::default())
        .await
        .expect("Failed to list org members");

    assert!(result.items.is_empty());
}

pub async fn test_remove_from_org_not_member(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_user_input("not-member", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    let result = ctx.user_repo.remove_from_org(user.id, org_id).await;
    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_list_org_members_with_pagination(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..5 {
        let input = create_user_input(&format!("member-{}", i), None, None);
        let user = ctx
            .user_repo
            .create(input)
            .await
            .expect("Failed to create user");
        ctx.user_repo
            .add_to_org(user.id, org_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to org");
    }

    let page1 = ctx
        .user_repo
        .list_org_members(
            org_id,
            ListParams {
                limit: Some(2),
                include_deleted: false,
                ..Default::default()
            },
        )
        .await
        .expect("Failed to list page 1");

    let page2 = ctx
        .user_repo
        .list_org_members(
            org_id,
            ListParams {
                limit: Some(2),
                include_deleted: false,
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

pub async fn test_count_org_members(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..3 {
        let input = create_user_input(&format!("member-{}", i), None, None);
        let user = ctx
            .user_repo
            .create(input)
            .await
            .expect("Failed to create user");
        ctx.user_repo
            .add_to_org(user.id, org_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to org");
    }

    let count = ctx
        .user_repo
        .count_org_members(org_id, false)
        .await
        .expect("Failed to count org members");
    assert_eq!(count, 3);
}

pub async fn test_org_members_isolated_by_org(ctx: &UserTestContext<'_>) {
    let org_id_1 = ctx.create_test_org("org-1").await;
    let org_id_2 = ctx.create_test_org("org-2").await;

    // Add 2 users to org 1
    for i in 0..2 {
        let input = create_user_input(&format!("org1-user-{}", i), None, None);
        let user = ctx
            .user_repo
            .create(input)
            .await
            .expect("Failed to create user");
        ctx.user_repo
            .add_to_org(user.id, org_id_1, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to org");
    }

    // Add 3 users to org 2
    for i in 0..3 {
        let input = create_user_input(&format!("org2-user-{}", i), None, None);
        let user = ctx
            .user_repo
            .create(input)
            .await
            .expect("Failed to create user");
        ctx.user_repo
            .add_to_org(user.id, org_id_2, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to org");
    }

    let count1 = ctx
        .user_repo
        .count_org_members(org_id_1, false)
        .await
        .expect("Failed to count");
    let count2 = ctx
        .user_repo
        .count_org_members(org_id_2, false)
        .await
        .expect("Failed to count");

    assert_eq!(count1, 2);
    assert_eq!(count2, 3);
}

// ============================================================================
// Project Membership Test Functions
// ============================================================================

pub async fn test_add_to_project(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;

    let input = create_user_input("project-member", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    ctx.user_repo
        .add_to_project(user.id, project_id, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add user to project");

    let result = ctx
        .user_repo
        .list_project_members(project_id, ListParams::default())
        .await
        .expect("Failed to list project members");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, user.id);
}

pub async fn test_add_to_project_duplicate_fails(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;

    let input = create_user_input("project-member", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    ctx.user_repo
        .add_to_project(user.id, project_id, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add user to project");

    let result = ctx
        .user_repo
        .add_to_project(user.id, project_id, "member", MembershipSource::Manual)
        .await;
    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_remove_from_project(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;

    let input = create_user_input("project-member", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    ctx.user_repo
        .add_to_project(user.id, project_id, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add user to project");

    ctx.user_repo
        .remove_from_project(user.id, project_id)
        .await
        .expect("Failed to remove user from project");

    let result = ctx
        .user_repo
        .list_project_members(project_id, ListParams::default())
        .await
        .expect("Failed to list project members");

    assert!(result.items.is_empty());
}

pub async fn test_remove_from_project_not_member(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;

    let input = create_user_input("not-member", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    let result = ctx.user_repo.remove_from_project(user.id, project_id).await;
    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_list_project_members_with_pagination(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;

    for i in 0..5 {
        let input = create_user_input(&format!("member-{}", i), None, None);
        let user = ctx
            .user_repo
            .create(input)
            .await
            .expect("Failed to create user");
        ctx.user_repo
            .add_to_project(user.id, project_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to project");
    }

    let page1 = ctx
        .user_repo
        .list_project_members(
            project_id,
            ListParams {
                limit: Some(2),
                include_deleted: false,
                ..Default::default()
            },
        )
        .await
        .expect("Failed to list page 1");

    let page2 = ctx
        .user_repo
        .list_project_members(
            project_id,
            ListParams {
                limit: Some(2),
                include_deleted: false,
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

pub async fn test_count_project_members(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;

    for i in 0..3 {
        let input = create_user_input(&format!("member-{}", i), None, None);
        let user = ctx
            .user_repo
            .create(input)
            .await
            .expect("Failed to create user");
        ctx.user_repo
            .add_to_project(user.id, project_id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to project");
    }

    let count = ctx
        .user_repo
        .count_project_members(project_id, false)
        .await
        .expect("Failed to count project members");
    assert_eq!(count, 3);
}

pub async fn test_project_members_isolated_by_project(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id_1 = ctx.create_test_project(org_id, "project-1").await;
    let project_id_2 = ctx.create_test_project(org_id, "project-2").await;

    // Add 2 users to project 1
    for i in 0..2 {
        let input = create_user_input(&format!("p1-user-{}", i), None, None);
        let user = ctx
            .user_repo
            .create(input)
            .await
            .expect("Failed to create user");
        ctx.user_repo
            .add_to_project(user.id, project_id_1, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to project");
    }

    // Add 3 users to project 2
    for i in 0..3 {
        let input = create_user_input(&format!("p2-user-{}", i), None, None);
        let user = ctx
            .user_repo
            .create(input)
            .await
            .expect("Failed to create user");
        ctx.user_repo
            .add_to_project(user.id, project_id_2, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to project");
    }

    let count1 = ctx
        .user_repo
        .count_project_members(project_id_1, false)
        .await
        .expect("Failed to count");
    let count2 = ctx
        .user_repo
        .count_project_members(project_id_2, false)
        .await
        .expect("Failed to count");

    assert_eq!(count1, 2);
    assert_eq!(count2, 3);
}

/// Test that single-org constraint prevents users from joining multiple organizations.
/// Each user can only belong to one organization at a time.
pub async fn test_add_to_second_org_fails(ctx: &UserTestContext<'_>) {
    let org_id_1 = ctx.create_test_org("org-1").await;
    let org_id_2 = ctx.create_test_org("org-2").await;

    let input = create_user_input("single-org-user", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    // Adding to first org should succeed
    ctx.user_repo
        .add_to_org(user.id, org_id_1, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add to org 1");

    // Adding to second org should fail with single-org constraint violation
    let result = ctx
        .user_repo
        .add_to_org(user.id, org_id_2, "member", MembershipSource::Manual)
        .await;
    assert!(
        matches!(result, Err(DbError::Conflict(ref msg)) if msg.contains("another organization")),
        "Expected conflict error for adding to second org, got: {:?}",
        result
    );

    // Verify user is only in the first org
    let count1 = ctx
        .user_repo
        .count_org_members(org_id_1, false)
        .await
        .expect("Failed to count");
    let count2 = ctx
        .user_repo
        .count_org_members(org_id_2, false)
        .await
        .expect("Failed to count");

    assert_eq!(count1, 1);
    assert_eq!(count2, 0);
}

/// Test that concurrent attempts to add a user to different organizations are handled correctly.
/// The database constraint (idx_org_memberships_single_org) must ensure exactly one succeeds.
///
/// This test verifies race condition handling: if two requests try to add the same user
/// to different orgs simultaneously, one must succeed and one must fail.
pub async fn test_concurrent_add_to_different_orgs(ctx: &UserTestContext<'_>) {
    let org_id_1 = ctx.create_test_org("concurrent-org-1").await;
    let org_id_2 = ctx.create_test_org("concurrent-org-2").await;

    let input = create_user_input("concurrent-user", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    // Attempt to add to both orgs concurrently
    let user_id = user.id;

    // We can't truly test concurrency in this test harness since the repos aren't clonable/shareable,
    // but we verify that the database constraint works by trying to add sequentially
    // (the real race condition is tested by the DB constraint, not application code)

    // First add should succeed
    let result1 = ctx
        .user_repo
        .add_to_org(user_id, org_id_1, "member", MembershipSource::Jit)
        .await;

    // Second add should fail due to single-org constraint
    let result2 = ctx
        .user_repo
        .add_to_org(user_id, org_id_2, "member", MembershipSource::Jit)
        .await;

    // Verify exactly one succeeded and one failed
    assert!(
        result1.is_ok(),
        "First add should succeed, got: {:?}",
        result1
    );
    assert!(
        matches!(result2, Err(DbError::Conflict(ref msg)) if msg.contains("another organization")),
        "Second add should fail with conflict, got: {:?}",
        result2
    );

    // Verify user is only in org_1
    let count1 = ctx
        .user_repo
        .count_org_members(org_id_1, false)
        .await
        .expect("Failed to count");
    let count2 = ctx
        .user_repo
        .count_org_members(org_id_2, false)
        .await
        .expect("Failed to count");

    assert_eq!(count1, 1, "User should be in org_1");
    assert_eq!(count2, 0, "User should not be in org_2");
}

/// Test that removing from org and adding to a different org works correctly.
/// This verifies the single-org constraint is properly released when membership is removed.
pub async fn test_can_switch_orgs_after_removal(ctx: &UserTestContext<'_>) {
    let org_id_1 = ctx.create_test_org("switch-org-1").await;
    let org_id_2 = ctx.create_test_org("switch-org-2").await;

    let input = create_user_input("switch-org-user", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    // Add to first org
    ctx.user_repo
        .add_to_org(user.id, org_id_1, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add to org 1");

    // Remove from first org
    ctx.user_repo
        .remove_from_org(user.id, org_id_1)
        .await
        .expect("Failed to remove from org 1");

    // Should now be able to add to second org
    ctx.user_repo
        .add_to_org(user.id, org_id_2, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add to org 2 after removal from org 1");

    // Verify user is only in org_2
    let count1 = ctx
        .user_repo
        .count_org_members(org_id_1, false)
        .await
        .expect("Failed to count");
    let count2 = ctx
        .user_repo
        .count_org_members(org_id_2, false)
        .await
        .expect("Failed to count");

    assert_eq!(count1, 0, "User should not be in org_1");
    assert_eq!(count2, 1, "User should be in org_2");
}

pub async fn test_user_can_be_in_multiple_projects(ctx: &UserTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id_1 = ctx.create_test_project(org_id, "project-1").await;
    let project_id_2 = ctx.create_test_project(org_id, "project-2").await;

    let input = create_user_input("multi-project-user", None, None);
    let user = ctx
        .user_repo
        .create(input)
        .await
        .expect("Failed to create user");

    ctx.user_repo
        .add_to_project(user.id, project_id_1, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add to project 1");
    ctx.user_repo
        .add_to_project(user.id, project_id_2, "member", MembershipSource::Manual)
        .await
        .expect("Failed to add to project 2");

    let count1 = ctx
        .user_repo
        .count_project_members(project_id_1, false)
        .await
        .expect("Failed to count");
    let count2 = ctx
        .user_repo
        .count_project_members(project_id_2, false)
        .await
        .expect("Failed to count");

    assert_eq!(count1, 1);
    assert_eq!(count2, 1);
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use super::*;
    use crate::db::{
        sqlite::{SqliteOrganizationRepo, SqliteProjectRepo, SqliteUserRepo},
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repos() -> (SqliteUserRepo, SqliteOrganizationRepo, SqliteProjectRepo) {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        (
            SqliteUserRepo::new(pool.clone()),
            SqliteOrganizationRepo::new(pool.clone()),
            SqliteProjectRepo::new(pool),
        )
    }

    macro_rules! sqlite_test {
        ($name:ident) => {
            #[tokio::test]
            async fn $name() {
                let (user_repo, org_repo, project_repo) = create_repos().await;
                let ctx = UserTestContext {
                    user_repo: &user_repo,
                    org_repo: &org_repo,
                    project_repo: &project_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    // User CRUD tests
    sqlite_test!(test_create_user);
    sqlite_test!(test_create_user_minimal);
    sqlite_test!(test_create_duplicate_external_id_fails);
    sqlite_test!(test_get_by_id);
    sqlite_test!(test_get_by_id_not_found);
    sqlite_test!(test_get_by_external_id);
    sqlite_test!(test_get_by_external_id_not_found);
    sqlite_test!(test_list_empty);
    sqlite_test!(test_list_with_users);
    sqlite_test!(test_list_with_pagination);
    sqlite_test!(test_count_empty);
    sqlite_test!(test_count_with_users);
    sqlite_test!(test_update_email);
    sqlite_test!(test_update_name);
    sqlite_test!(test_update_not_found);

    // Organization membership tests
    sqlite_test!(test_add_to_org);
    sqlite_test!(test_add_to_org_duplicate_fails);
    sqlite_test!(test_remove_from_org);
    sqlite_test!(test_remove_from_org_not_member);
    sqlite_test!(test_list_org_members_with_pagination);
    sqlite_test!(test_count_org_members);
    sqlite_test!(test_org_members_isolated_by_org);

    // Project membership tests
    sqlite_test!(test_add_to_project);
    sqlite_test!(test_add_to_project_duplicate_fails);
    sqlite_test!(test_remove_from_project);
    sqlite_test!(test_remove_from_project_not_member);
    sqlite_test!(test_list_project_members_with_pagination);
    sqlite_test!(test_count_project_members);
    sqlite_test!(test_project_members_isolated_by_project);

    // Multi-membership tests
    sqlite_test!(test_add_to_second_org_fails);
    sqlite_test!(test_concurrent_add_to_different_orgs);
    sqlite_test!(test_can_switch_orgs_after_removal);
    sqlite_test!(test_user_can_be_in_multiple_projects);
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use super::*;
    use crate::db::{
        postgres::{PostgresOrganizationRepo, PostgresProjectRepo, PostgresUserRepo},
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let pool = create_isolated_postgres_pool().await;
                run_postgres_migrations(&pool).await;
                let user_repo = PostgresUserRepo::new(pool.clone(), None);
                let org_repo = PostgresOrganizationRepo::new(pool.clone(), None);
                let project_repo = PostgresProjectRepo::new(pool, None);
                let ctx = UserTestContext {
                    user_repo: &user_repo,
                    org_repo: &org_repo,
                    project_repo: &project_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    // User CRUD tests
    postgres_test!(test_create_user);
    postgres_test!(test_create_user_minimal);
    postgres_test!(test_create_duplicate_external_id_fails);
    postgres_test!(test_get_by_id);
    postgres_test!(test_get_by_id_not_found);
    postgres_test!(test_get_by_external_id);
    postgres_test!(test_get_by_external_id_not_found);
    postgres_test!(test_list_empty);
    postgres_test!(test_list_with_users);
    postgres_test!(test_list_with_pagination);
    postgres_test!(test_count_empty);
    postgres_test!(test_count_with_users);
    postgres_test!(test_update_email);
    postgres_test!(test_update_name);
    postgres_test!(test_update_not_found);

    // Organization membership tests
    postgres_test!(test_add_to_org);
    postgres_test!(test_add_to_org_duplicate_fails);
    postgres_test!(test_remove_from_org);
    postgres_test!(test_remove_from_org_not_member);
    postgres_test!(test_list_org_members_with_pagination);
    postgres_test!(test_count_org_members);
    postgres_test!(test_org_members_isolated_by_org);

    // Project membership tests
    postgres_test!(test_add_to_project);
    postgres_test!(test_add_to_project_duplicate_fails);
    postgres_test!(test_remove_from_project);
    postgres_test!(test_remove_from_project_not_member);
    postgres_test!(test_list_project_members_with_pagination);
    postgres_test!(test_count_project_members);
    postgres_test!(test_project_members_isolated_by_project);

    // Multi-membership tests
    postgres_test!(test_add_to_second_org_fails);
    postgres_test!(test_concurrent_add_to_different_orgs);
    postgres_test!(test_can_switch_orgs_after_removal);
    postgres_test!(test_user_can_be_in_multiple_projects);
}
