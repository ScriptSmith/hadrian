//! Shared tests for OrgRbacPolicyRepo implementations
//!
//! Tests are written as async functions that take `&dyn OrgRbacPolicyRepo`,
//! allowing the same test logic to run against both SQLite and PostgreSQL.

use uuid::Uuid;

use crate::{
    db::{
        error::DbError,
        repos::{OrgRbacPolicyRepo, OrganizationRepo, UserRepo},
    },
    models::{
        CreateOrgRbacPolicy, CreateOrganization, CreateUser, RbacPolicyEffect,
        RollbackOrgRbacPolicy, UpdateOrgRbacPolicy,
    },
};

// ============================================================================
// Test Input Helpers
// ============================================================================

fn create_test_policy_input(name: &str) -> CreateOrgRbacPolicy {
    CreateOrgRbacPolicy {
        name: name.to_string(),
        description: Some("Test policy description".to_string()),
        resource: "projects/*".to_string(),
        action: "read".to_string(),
        condition: "user.role == 'admin'".to_string(),
        effect: RbacPolicyEffect::Allow,
        priority: 10,
        enabled: true,
        reason: Some("Initial creation".to_string()),
    }
}

async fn setup_org(org_repo: &dyn OrganizationRepo) -> Uuid {
    let org = org_repo
        .create(CreateOrganization {
            slug: format!("test-org-{}", &Uuid::new_v4().to_string()[..8]),
            name: "Test Organization".to_string(),
        })
        .await
        .expect("Failed to create test organization");
    org.id
}

async fn setup_user(user_repo: &dyn UserRepo, _org_id: Uuid) -> Uuid {
    let external_id = format!("test-user-{}", &Uuid::new_v4().to_string()[..8]);
    let user = user_repo
        .create(CreateUser {
            external_id,
            email: Some(format!(
                "test-user-{}@example.com",
                &Uuid::new_v4().to_string()[..8]
            )),
            name: Some("Test User".to_string()),
        })
        .await
        .expect("Failed to create test user");
    user.id
}

// ============================================================================
// Shared Test Functions
// These are called by both SQLite and PostgreSQL test implementations
// ============================================================================

pub async fn test_create_policy(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
    user_repo: &dyn UserRepo,
) {
    let org_id = setup_org(org_repo).await;
    let user_id = setup_user(user_repo, org_id).await;

    let input = create_test_policy_input("admin-read-projects");

    let policy = policy_repo
        .create(org_id, input, Some(user_id))
        .await
        .expect("Failed to create policy");

    assert_eq!(policy.name, "admin-read-projects");
    assert_eq!(policy.org_id, org_id);
    assert_eq!(policy.resource, "projects/*");
    assert_eq!(policy.action, "read");
    assert_eq!(policy.condition, "user.role == 'admin'");
    assert_eq!(policy.effect, RbacPolicyEffect::Allow);
    assert_eq!(policy.priority, 10);
    assert!(policy.enabled);
    assert_eq!(policy.version, 1);
}

pub async fn test_create_policy_creates_version(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
    user_repo: &dyn UserRepo,
) {
    let org_id = setup_org(org_repo).await;
    let user_id = setup_user(user_repo, org_id).await;

    let input = create_test_policy_input("test-policy");

    let policy = policy_repo
        .create(org_id, input, Some(user_id))
        .await
        .expect("Failed to create policy");

    let versions = policy_repo
        .list_versions(policy.id)
        .await
        .expect("Failed to list versions");

    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);
    assert_eq!(versions[0].name, "test-policy");
    assert_eq!(versions[0].created_by, Some(user_id));
    assert_eq!(versions[0].reason, Some("Initial creation".to_string()));
}

pub async fn test_create_duplicate_name_fails(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    policy_repo
        .create(org_id, create_test_policy_input("duplicate-name"), None)
        .await
        .expect("First policy should succeed");

    let result = policy_repo
        .create(org_id, create_test_policy_input("duplicate-name"), None)
        .await;

    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_get_by_id(policy_repo: &dyn OrgRbacPolicyRepo, org_repo: &dyn OrganizationRepo) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(org_id, create_test_policy_input("get-test"), None)
        .await
        .expect("Failed to create policy");

    let fetched = policy_repo
        .get_by_id(created.id)
        .await
        .expect("Failed to fetch policy")
        .expect("Policy should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "get-test");
}

pub async fn test_get_by_id_not_found(policy_repo: &dyn OrgRbacPolicyRepo) {
    let result = policy_repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_org_and_name(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    policy_repo
        .create(org_id, create_test_policy_input("named-policy"), None)
        .await
        .expect("Failed to create policy");

    let fetched = policy_repo
        .get_by_org_and_name(org_id, "named-policy")
        .await
        .expect("Failed to fetch policy")
        .expect("Policy should exist");

    assert_eq!(fetched.name, "named-policy");
    assert_eq!(fetched.org_id, org_id);
}

pub async fn test_list_by_org(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;
    let other_org_id = setup_org(org_repo).await;

    // Create policies with different priorities
    let mut input1 = create_test_policy_input("policy-1");
    input1.priority = 5;
    let mut input2 = create_test_policy_input("policy-2");
    input2.priority = 20;
    let mut input3 = create_test_policy_input("policy-3");
    input3.priority = 10;

    policy_repo
        .create(org_id, input1, None)
        .await
        .expect("Failed to create policy 1");
    policy_repo
        .create(org_id, input2, None)
        .await
        .expect("Failed to create policy 2");
    policy_repo
        .create(org_id, input3, None)
        .await
        .expect("Failed to create policy 3");
    policy_repo
        .create(
            other_org_id,
            create_test_policy_input("other-org-policy"),
            None,
        )
        .await
        .expect("Failed to create other org policy");

    let policies = policy_repo
        .list_by_org(org_id)
        .await
        .expect("Failed to list policies");

    assert_eq!(policies.len(), 3);
    // Should be ordered by priority DESC
    assert_eq!(policies[0].name, "policy-2"); // priority 20
    assert_eq!(policies[1].name, "policy-3"); // priority 10
    assert_eq!(policies[2].name, "policy-1"); // priority 5
}

pub async fn test_list_enabled_by_org(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let mut enabled_input = create_test_policy_input("enabled-policy");
    enabled_input.enabled = true;

    let mut disabled_input = create_test_policy_input("disabled-policy");
    disabled_input.enabled = false;

    policy_repo
        .create(org_id, enabled_input, None)
        .await
        .expect("Failed to create enabled policy");
    policy_repo
        .create(org_id, disabled_input, None)
        .await
        .expect("Failed to create disabled policy");

    let policies = policy_repo
        .list_enabled_by_org(org_id)
        .await
        .expect("Failed to list enabled policies");

    assert_eq!(policies.len(), 1);
    assert_eq!(policies[0].name, "enabled-policy");
}

pub async fn test_update_policy(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
    user_repo: &dyn UserRepo,
) {
    let org_id = setup_org(org_repo).await;
    let user_id = setup_user(user_repo, org_id).await;

    let created = policy_repo
        .create(org_id, create_test_policy_input("update-test"), None)
        .await
        .expect("Failed to create policy");

    assert_eq!(created.version, 1);

    let update = UpdateOrgRbacPolicy {
        name: Some("updated-name".to_string()),
        priority: Some(100),
        reason: Some("Updated priority".to_string()),
        ..Default::default()
    };

    let updated = policy_repo
        .update(created.id, update, Some(user_id))
        .await
        .expect("Failed to update policy");

    assert_eq!(updated.name, "updated-name");
    assert_eq!(updated.priority, 100);
    assert_eq!(updated.version, 2);
    // Unchanged fields should remain the same
    assert_eq!(updated.condition, created.condition);
}

pub async fn test_update_creates_version(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
    user_repo: &dyn UserRepo,
) {
    let org_id = setup_org(org_repo).await;
    let user_id = setup_user(user_repo, org_id).await;

    let created = policy_repo
        .create(org_id, create_test_policy_input("version-test"), None)
        .await
        .expect("Failed to create policy");

    let update = UpdateOrgRbacPolicy {
        condition: Some("user.department == 'engineering'".to_string()),
        reason: Some("Changed condition".to_string()),
        ..Default::default()
    };

    policy_repo
        .update(created.id, update, Some(user_id))
        .await
        .expect("Failed to update policy");

    let versions = policy_repo
        .list_versions(created.id)
        .await
        .expect("Failed to list versions");

    assert_eq!(versions.len(), 2);
    // Versions should be ordered by version DESC
    assert_eq!(versions[0].version, 2);
    assert_eq!(versions[0].condition, "user.department == 'engineering'");
    assert_eq!(versions[0].reason, Some("Changed condition".to_string()));
    assert_eq!(versions[1].version, 1);
}

pub async fn test_update_not_found(policy_repo: &dyn OrgRbacPolicyRepo) {
    let result = policy_repo
        .update(Uuid::new_v4(), UpdateOrgRbacPolicy::default(), None)
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_delete_policy(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(org_id, create_test_policy_input("delete-test"), None)
        .await
        .expect("Failed to create policy");

    policy_repo
        .delete(created.id)
        .await
        .expect("Failed to delete policy");

    let fetched = policy_repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed");
    assert!(fetched.is_none());
}

pub async fn test_delete_preserves_versions(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(
            org_id,
            create_test_policy_input("version-preserve-test"),
            None,
        )
        .await
        .expect("Failed to create policy");

    // Update to create version 2
    policy_repo
        .update(
            created.id,
            UpdateOrgRbacPolicy {
                priority: Some(50),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("Failed to update policy");

    // Verify versions exist
    let versions_before = policy_repo
        .list_versions(created.id)
        .await
        .expect("Failed to list versions");
    assert_eq!(versions_before.len(), 2);

    // Delete (soft-delete) policy
    policy_repo
        .delete(created.id)
        .await
        .expect("Failed to delete policy");

    // Versions should be preserved after soft-delete (for audit purposes)
    let versions_after = policy_repo
        .list_versions(created.id)
        .await
        .expect("Failed to list versions");
    assert_eq!(versions_after.len(), 2);
}

pub async fn test_delete_allows_name_reuse(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    // Create policy
    let created = policy_repo
        .create(org_id, create_test_policy_input("reusable-name"), None)
        .await
        .expect("Failed to create policy");

    // Delete (soft-delete) policy
    policy_repo
        .delete(created.id)
        .await
        .expect("Failed to delete policy");

    // Should be able to create a new policy with the same name
    let new_policy = policy_repo
        .create(org_id, create_test_policy_input("reusable-name"), None)
        .await
        .expect("Should be able to reuse name after delete");

    assert_eq!(new_policy.name, "reusable-name");
    assert_ne!(new_policy.id, created.id); // Different ID
}

pub async fn test_update_deleted_policy_fails(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(
            org_id,
            create_test_policy_input("update-deleted-test"),
            None,
        )
        .await
        .expect("Failed to create policy");

    // Delete the policy
    policy_repo
        .delete(created.id)
        .await
        .expect("Failed to delete policy");

    // Attempt to update the deleted policy should fail
    let result = policy_repo
        .update(
            created.id,
            UpdateOrgRbacPolicy {
                priority: Some(100),
                ..Default::default()
            },
            None,
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_rollback_deleted_policy_fails(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(
            org_id,
            create_test_policy_input("rollback-deleted-test"),
            None,
        )
        .await
        .expect("Failed to create policy");

    // Delete the policy
    policy_repo
        .delete(created.id)
        .await
        .expect("Failed to delete policy");

    // Attempt to rollback the deleted policy should fail
    let result = policy_repo
        .rollback(
            created.id,
            RollbackOrgRbacPolicy {
                target_version: 1,
                reason: None,
            },
            None,
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_list_excludes_deleted(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    // Create two policies
    policy_repo
        .create(org_id, create_test_policy_input("active-policy"), None)
        .await
        .expect("Failed to create policy");

    let to_delete = policy_repo
        .create(org_id, create_test_policy_input("deleted-policy"), None)
        .await
        .expect("Failed to create policy");

    // Delete one policy
    policy_repo
        .delete(to_delete.id)
        .await
        .expect("Failed to delete policy");

    // List should only return active policy
    let policies = policy_repo
        .list_by_org(org_id)
        .await
        .expect("Failed to list policies");

    assert_eq!(policies.len(), 1);
    assert_eq!(policies[0].name, "active-policy");
}

pub async fn test_count_excludes_deleted(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    // Create two policies
    policy_repo
        .create(org_id, create_test_policy_input("count-active"), None)
        .await
        .expect("Failed to create policy");

    let to_delete = policy_repo
        .create(org_id, create_test_policy_input("count-deleted"), None)
        .await
        .expect("Failed to create policy");

    // Delete one policy
    policy_repo
        .delete(to_delete.id)
        .await
        .expect("Failed to delete policy");

    // Count should only include active policy
    let count = policy_repo
        .count_by_org(org_id)
        .await
        .expect("Failed to count policies");

    assert_eq!(count, 1);
}

pub async fn test_count_all(policy_repo: &dyn OrgRbacPolicyRepo, org_repo: &dyn OrganizationRepo) {
    // Initially zero
    let count = policy_repo
        .count_all()
        .await
        .expect("Failed to count policies");
    assert_eq!(count, 0);

    // Create policies in two different orgs
    let org1_id = setup_org(org_repo).await;
    let org2_id = setup_org(org_repo).await;

    policy_repo
        .create(org1_id, create_test_policy_input("policy-a"), None)
        .await
        .expect("Failed to create policy");
    policy_repo
        .create(org1_id, create_test_policy_input("policy-b"), None)
        .await
        .expect("Failed to create policy");
    policy_repo
        .create(org2_id, create_test_policy_input("policy-c"), None)
        .await
        .expect("Failed to create policy");

    let count = policy_repo
        .count_all()
        .await
        .expect("Failed to count policies");
    assert_eq!(count, 3);

    // Delete one — count_all should exclude soft-deleted
    let policies = policy_repo
        .list_by_org(org1_id)
        .await
        .expect("Failed to list");
    policy_repo
        .delete(policies[0].id)
        .await
        .expect("Failed to delete");

    let count = policy_repo
        .count_all()
        .await
        .expect("Failed to count policies");
    assert_eq!(count, 2);
}

pub async fn test_delete_not_found(policy_repo: &dyn OrgRbacPolicyRepo) {
    let result = policy_repo.delete(Uuid::new_v4()).await;
    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_rollback(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
    user_repo: &dyn UserRepo,
) {
    let org_id = setup_org(org_repo).await;
    let user_id = setup_user(user_repo, org_id).await;

    // Create initial policy
    let created = policy_repo
        .create(org_id, create_test_policy_input("rollback-test"), None)
        .await
        .expect("Failed to create policy");

    assert_eq!(created.condition, "user.role == 'admin'");

    // Update to change condition
    policy_repo
        .update(
            created.id,
            UpdateOrgRbacPolicy {
                condition: Some("user.role == 'superadmin'".to_string()),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("Failed to update policy");

    // Rollback to version 1
    let rollback_input = RollbackOrgRbacPolicy {
        target_version: 1,
        reason: Some("Reverting to original condition".to_string()),
    };

    let rolled_back = policy_repo
        .rollback(created.id, rollback_input, Some(user_id))
        .await
        .expect("Failed to rollback policy");

    // Should have original condition but new version number
    assert_eq!(rolled_back.condition, "user.role == 'admin'");
    assert_eq!(rolled_back.version, 3);
}

pub async fn test_rollback_creates_version(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(
            org_id,
            create_test_policy_input("rollback-version-test"),
            None,
        )
        .await
        .expect("Failed to create policy");

    // Update twice
    policy_repo
        .update(
            created.id,
            UpdateOrgRbacPolicy {
                priority: Some(50),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("Failed to update policy");

    policy_repo
        .update(
            created.id,
            UpdateOrgRbacPolicy {
                priority: Some(100),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("Failed to update policy");

    // Rollback to version 1
    policy_repo
        .rollback(
            created.id,
            RollbackOrgRbacPolicy {
                target_version: 1,
                reason: None,
            },
            None,
        )
        .await
        .expect("Failed to rollback policy");

    let versions = policy_repo
        .list_versions(created.id)
        .await
        .expect("Failed to list versions");

    assert_eq!(versions.len(), 4);
    assert_eq!(versions[0].version, 4); // rollback version
    assert!(
        versions[0]
            .reason
            .as_ref()
            .unwrap()
            .contains("Rolled back to version 1")
    );
    assert_eq!(versions[0].priority, 10); // original priority
}

pub async fn test_rollback_not_found_policy(policy_repo: &dyn OrgRbacPolicyRepo) {
    let result = policy_repo
        .rollback(
            Uuid::new_v4(),
            RollbackOrgRbacPolicy {
                target_version: 1,
                reason: None,
            },
            None,
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_rollback_not_found_version(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(
            org_id,
            create_test_policy_input("rollback-missing-version"),
            None,
        )
        .await
        .expect("Failed to create policy");

    let result = policy_repo
        .rollback(
            created.id,
            RollbackOrgRbacPolicy {
                target_version: 999,
                reason: None,
            },
            None,
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_get_version(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(org_id, create_test_policy_input("get-version-test"), None)
        .await
        .expect("Failed to create policy");

    let version = policy_repo
        .get_version(created.id, 1)
        .await
        .expect("Failed to get version")
        .expect("Version should exist");

    assert_eq!(version.version, 1);
    assert_eq!(version.name, "get-version-test");
}

pub async fn test_get_version_not_found(policy_repo: &dyn OrgRbacPolicyRepo) {
    let result = policy_repo
        .get_version(Uuid::new_v4(), 1)
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_list_versions_paginated(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(org_id, create_test_policy_input("paginated-test"), None)
        .await
        .expect("Failed to create policy");

    // Create 4 more versions (total 5)
    for i in 0..4 {
        policy_repo
            .update(
                created.id,
                UpdateOrgRbacPolicy {
                    priority: Some(i * 10),
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("Failed to update policy");
    }

    // Get first page
    let page1 = policy_repo
        .list_versions_paginated(created.id, 2, 0)
        .await
        .expect("Failed to get page 1");

    assert_eq!(page1.len(), 2);
    assert_eq!(page1[0].version, 5); // newest first
    assert_eq!(page1[1].version, 4);

    // Get second page
    let page2 = policy_repo
        .list_versions_paginated(created.id, 2, 2)
        .await
        .expect("Failed to get page 2");

    assert_eq!(page2.len(), 2);
    assert_eq!(page2[0].version, 3);
    assert_eq!(page2[1].version, 2);

    // Get third page
    let page3 = policy_repo
        .list_versions_paginated(created.id, 2, 4)
        .await
        .expect("Failed to get page 3");

    assert_eq!(page3.len(), 1);
    assert_eq!(page3[0].version, 1);
}

pub async fn test_count_versions(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(org_id, create_test_policy_input("count-test"), None)
        .await
        .expect("Failed to create policy");

    // Initially 1 version
    let count = policy_repo
        .count_versions(created.id)
        .await
        .expect("Failed to count versions");
    assert_eq!(count, 1);

    // Create 3 more versions (total 4)
    for i in 0..3 {
        policy_repo
            .update(
                created.id,
                UpdateOrgRbacPolicy {
                    priority: Some(i * 10),
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("Failed to update policy");
    }

    let count = policy_repo
        .count_versions(created.id)
        .await
        .expect("Failed to count versions");
    assert_eq!(count, 4);

    // Non-existent policy returns 0
    let count = policy_repo
        .count_versions(Uuid::new_v4())
        .await
        .expect("Failed to count versions");
    assert_eq!(count, 0);
}

pub async fn test_policy_effect_deny(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let mut input = create_test_policy_input("deny-policy");
    input.effect = RbacPolicyEffect::Deny;

    let policy = policy_repo
        .create(org_id, input, None)
        .await
        .expect("Failed to create policy");

    assert_eq!(policy.effect, RbacPolicyEffect::Deny);

    let fetched = policy_repo
        .get_by_id(policy.id)
        .await
        .expect("Failed to fetch policy")
        .expect("Policy should exist");

    assert_eq!(fetched.effect, RbacPolicyEffect::Deny);
}

pub async fn test_update_description_to_none(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org_id = setup_org(org_repo).await;

    let created = policy_repo
        .create(org_id, create_test_policy_input("description-test"), None)
        .await
        .expect("Failed to create policy");

    assert!(created.description.is_some());

    let updated = policy_repo
        .update(
            created.id,
            UpdateOrgRbacPolicy {
                description: Some(None), // Set to null
                ..Default::default()
            },
            None,
        )
        .await
        .expect("Failed to update policy");

    assert!(updated.description.is_none());
}

pub async fn test_list_all_enabled(
    policy_repo: &dyn OrgRbacPolicyRepo,
    org_repo: &dyn OrganizationRepo,
) {
    let org1_id = setup_org(org_repo).await;
    let org2_id = setup_org(org_repo).await;

    // Create enabled policies in both orgs
    let mut enabled1 = create_test_policy_input("org1-enabled");
    enabled1.enabled = true;
    policy_repo
        .create(org1_id, enabled1, None)
        .await
        .expect("Failed to create policy");

    let mut disabled1 = create_test_policy_input("org1-disabled");
    disabled1.enabled = false;
    policy_repo
        .create(org1_id, disabled1, None)
        .await
        .expect("Failed to create policy");

    let mut enabled2 = create_test_policy_input("org2-enabled");
    enabled2.enabled = true;
    policy_repo
        .create(org2_id, enabled2, None)
        .await
        .expect("Failed to create policy");

    let all_enabled = policy_repo
        .list_all_enabled()
        .await
        .expect("Failed to list all enabled");

    // Should only include enabled policies from both orgs
    assert_eq!(all_enabled.len(), 2);
    let names: Vec<_> = all_enabled.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"org1-enabled"));
    assert!(names.contains(&"org2-enabled"));
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use super::*;
    use crate::db::{
        sqlite::{SqliteOrgRbacPolicyRepo, SqliteOrganizationRepo, SqliteUserRepo},
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repos() -> (
        SqliteOrgRbacPolicyRepo,
        SqliteOrganizationRepo,
        SqliteUserRepo,
    ) {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        (
            SqliteOrgRbacPolicyRepo::new(pool.clone()),
            SqliteOrganizationRepo::new(pool.clone()),
            SqliteUserRepo::new(pool),
        )
    }

    #[tokio::test]
    async fn sqlite_create_policy() {
        let (policy_repo, org_repo, user_repo) = create_repos().await;
        test_create_policy(&policy_repo, &org_repo, &user_repo).await;
    }

    #[tokio::test]
    async fn sqlite_create_policy_creates_version() {
        let (policy_repo, org_repo, user_repo) = create_repos().await;
        test_create_policy_creates_version(&policy_repo, &org_repo, &user_repo).await;
    }

    #[tokio::test]
    async fn sqlite_create_duplicate_name_fails() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_create_duplicate_name_fails(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_get_by_id() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_get_by_id(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_get_by_id_not_found() {
        let (policy_repo, _, _) = create_repos().await;
        test_get_by_id_not_found(&policy_repo).await;
    }

    #[tokio::test]
    async fn sqlite_get_by_org_and_name() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_get_by_org_and_name(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_list_by_org() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_list_by_org(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_list_enabled_by_org() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_list_enabled_by_org(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_update_policy() {
        let (policy_repo, org_repo, user_repo) = create_repos().await;
        test_update_policy(&policy_repo, &org_repo, &user_repo).await;
    }

    #[tokio::test]
    async fn sqlite_update_creates_version() {
        let (policy_repo, org_repo, user_repo) = create_repos().await;
        test_update_creates_version(&policy_repo, &org_repo, &user_repo).await;
    }

    #[tokio::test]
    async fn sqlite_update_not_found() {
        let (policy_repo, _, _) = create_repos().await;
        test_update_not_found(&policy_repo).await;
    }

    #[tokio::test]
    async fn sqlite_delete_policy() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_delete_policy(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_delete_preserves_versions() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_delete_preserves_versions(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_delete_allows_name_reuse() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_delete_allows_name_reuse(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_update_deleted_policy_fails() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_update_deleted_policy_fails(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_rollback_deleted_policy_fails() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_rollback_deleted_policy_fails(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_list_excludes_deleted() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_list_excludes_deleted(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_count_excludes_deleted() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_count_excludes_deleted(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_delete_not_found() {
        let (policy_repo, _, _) = create_repos().await;
        test_delete_not_found(&policy_repo).await;
    }

    #[tokio::test]
    async fn sqlite_rollback() {
        let (policy_repo, org_repo, user_repo) = create_repos().await;
        test_rollback(&policy_repo, &org_repo, &user_repo).await;
    }

    #[tokio::test]
    async fn sqlite_rollback_creates_version() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_rollback_creates_version(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_rollback_not_found_policy() {
        let (policy_repo, _, _) = create_repos().await;
        test_rollback_not_found_policy(&policy_repo).await;
    }

    #[tokio::test]
    async fn sqlite_rollback_not_found_version() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_rollback_not_found_version(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_get_version() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_get_version(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_get_version_not_found() {
        let (policy_repo, _, _) = create_repos().await;
        test_get_version_not_found(&policy_repo).await;
    }

    #[tokio::test]
    async fn sqlite_list_versions_paginated() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_list_versions_paginated(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_count_versions() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_count_versions(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_policy_effect_deny() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_policy_effect_deny(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_update_description_to_none() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_update_description_to_none(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_list_all_enabled() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_list_all_enabled(&policy_repo, &org_repo).await;
    }

    #[tokio::test]
    async fn sqlite_count_all() {
        let (policy_repo, org_repo, _) = create_repos().await;
        test_count_all(&policy_repo, &org_repo).await;
    }
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use super::*;
    use crate::db::{
        postgres::{PostgresOrgRbacPolicyRepo, PostgresOrganizationRepo, PostgresUserRepo},
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    async fn create_repos() -> (
        PostgresOrgRbacPolicyRepo,
        PostgresOrganizationRepo,
        PostgresUserRepo,
    ) {
        let pool = create_isolated_postgres_pool().await;
        run_postgres_migrations(&pool).await;
        (
            PostgresOrgRbacPolicyRepo::new(pool.clone(), None),
            PostgresOrganizationRepo::new(pool.clone(), None),
            PostgresUserRepo::new(pool, None),
        )
    }

    macro_rules! postgres_test {
        ($name:ident, $test_fn:ident, all) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let (policy_repo, org_repo, user_repo) = create_repos().await;
                $test_fn(&policy_repo, &org_repo, &user_repo).await;
            }
        };
        ($name:ident, $test_fn:ident, policy_org) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let (policy_repo, org_repo, _) = create_repos().await;
                $test_fn(&policy_repo, &org_repo).await;
            }
        };
        ($name:ident, $test_fn:ident, policy_only) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let (policy_repo, _, _) = create_repos().await;
                $test_fn(&policy_repo).await;
            }
        };
    }

    // Tests that need all repos (policy, org, user)
    postgres_test!(postgres_create_policy, test_create_policy, all);
    postgres_test!(
        postgres_create_policy_creates_version,
        test_create_policy_creates_version,
        all
    );
    postgres_test!(postgres_update_policy, test_update_policy, all);
    postgres_test!(
        postgres_update_creates_version,
        test_update_creates_version,
        all
    );
    postgres_test!(postgres_rollback, test_rollback, all);

    // Tests that need policy and org repos
    postgres_test!(
        postgres_create_duplicate_name_fails,
        test_create_duplicate_name_fails,
        policy_org
    );
    postgres_test!(postgres_get_by_id, test_get_by_id, policy_org);
    postgres_test!(
        postgres_get_by_org_and_name,
        test_get_by_org_and_name,
        policy_org
    );
    postgres_test!(postgres_list_by_org, test_list_by_org, policy_org);
    postgres_test!(
        postgres_list_enabled_by_org,
        test_list_enabled_by_org,
        policy_org
    );
    postgres_test!(postgres_delete_policy, test_delete_policy, policy_org);
    postgres_test!(
        postgres_delete_preserves_versions,
        test_delete_preserves_versions,
        policy_org
    );
    postgres_test!(
        postgres_delete_allows_name_reuse,
        test_delete_allows_name_reuse,
        policy_org
    );
    postgres_test!(
        postgres_update_deleted_policy_fails,
        test_update_deleted_policy_fails,
        policy_org
    );
    postgres_test!(
        postgres_rollback_deleted_policy_fails,
        test_rollback_deleted_policy_fails,
        policy_org
    );
    postgres_test!(
        postgres_list_excludes_deleted,
        test_list_excludes_deleted,
        policy_org
    );
    postgres_test!(
        postgres_count_excludes_deleted,
        test_count_excludes_deleted,
        policy_org
    );
    postgres_test!(
        postgres_rollback_creates_version,
        test_rollback_creates_version,
        policy_org
    );
    postgres_test!(
        postgres_rollback_not_found_version,
        test_rollback_not_found_version,
        policy_org
    );
    postgres_test!(postgres_get_version, test_get_version, policy_org);
    postgres_test!(
        postgres_list_versions_paginated,
        test_list_versions_paginated,
        policy_org
    );
    postgres_test!(postgres_count_versions, test_count_versions, policy_org);
    postgres_test!(
        postgres_policy_effect_deny,
        test_policy_effect_deny,
        policy_org
    );
    postgres_test!(
        postgres_update_description_to_none,
        test_update_description_to_none,
        policy_org
    );
    postgres_test!(postgres_list_all_enabled, test_list_all_enabled, policy_org);
    postgres_test!(postgres_count_all, test_count_all, policy_org);

    // Tests that only need policy repo
    postgres_test!(
        postgres_get_by_id_not_found,
        test_get_by_id_not_found,
        policy_only
    );
    postgres_test!(
        postgres_update_not_found,
        test_update_not_found,
        policy_only
    );
    postgres_test!(
        postgres_delete_not_found,
        test_delete_not_found,
        policy_only
    );
    postgres_test!(
        postgres_rollback_not_found_policy,
        test_rollback_not_found_policy,
        policy_only
    );
    postgres_test!(
        postgres_get_version_not_found,
        test_get_version_not_found,
        policy_only
    );
}
