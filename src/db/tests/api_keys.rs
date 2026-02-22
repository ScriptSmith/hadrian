//! Shared tests for ApiKeyRepo implementations
//!
//! Tests are written as async functions that take a test context containing
//! the api key repo and utilities for creating test organizations and projects.

use chrono::Utc;
use uuid::Uuid;

use crate::{
    db::{
        error::DbError,
        repos::{ApiKeyRepo, ListParams, OrganizationRepo, ProjectRepo},
    },
    models::{ApiKeyOwner, BudgetPeriod, CreateApiKey, CreateOrganization, CreateProject},
};

// ============================================================================
// Test Input Helpers
// ============================================================================

fn create_org_api_key(name: &str, org_id: Uuid) -> CreateApiKey {
    CreateApiKey {
        name: name.to_string(),
        owner: ApiKeyOwner::Organization { org_id },
        budget_limit_cents: None,
        budget_period: None,
        expires_at: None,
        scopes: None,
        allowed_models: None,
        ip_allowlist: None,
        rate_limit_rpm: None,
        rate_limit_tpm: None,
    }
}

fn create_project_api_key(name: &str, project_id: Uuid) -> CreateApiKey {
    CreateApiKey {
        name: name.to_string(),
        owner: ApiKeyOwner::Project { project_id },
        budget_limit_cents: None,
        budget_period: None,
        expires_at: None,
        scopes: None,
        allowed_models: None,
        ip_allowlist: None,
        rate_limit_rpm: None,
        rate_limit_tpm: None,
    }
}

fn create_user_api_key(name: &str, user_id: Uuid) -> CreateApiKey {
    CreateApiKey {
        name: name.to_string(),
        owner: ApiKeyOwner::User { user_id },
        budget_limit_cents: None,
        budget_period: None,
        expires_at: None,
        scopes: None,
        allowed_models: None,
        ip_allowlist: None,
        rate_limit_rpm: None,
        rate_limit_tpm: None,
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

/// Test context containing repos needed for api key tests
pub struct ApiKeyTestContext<'a> {
    pub api_key_repo: &'a dyn ApiKeyRepo,
    pub org_repo: &'a dyn OrganizationRepo,
    pub project_repo: &'a dyn ProjectRepo,
}

impl<'a> ApiKeyTestContext<'a> {
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
// Create API Key Tests
// ============================================================================

pub async fn test_create_org_api_key(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let input = create_org_api_key("Test Key", org_id);
    let key_hash = "abcdef123456789";

    let key = ctx
        .api_key_repo
        .create(input, key_hash)
        .await
        .expect("Failed to create API key");

    assert_eq!(key.name, "Test Key");
    assert_eq!(key.key_prefix, "abcdef12"); // First 8 chars of hash
    assert!(matches!(key.owner, ApiKeyOwner::Organization { org_id: id } if id == org_id));
    assert!(key.revoked_at.is_none());
    assert!(key.last_used_at.is_none());
}

pub async fn test_create_project_api_key(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;
    let input = create_project_api_key("Project Key", project_id);
    let key_hash = "projecthash12345";

    let key = ctx
        .api_key_repo
        .create(input, key_hash)
        .await
        .expect("Failed to create API key");

    assert_eq!(key.name, "Project Key");
    assert!(matches!(key.owner, ApiKeyOwner::Project { project_id: id } if id == project_id));
}

pub async fn test_create_user_api_key(ctx: &ApiKeyTestContext<'_>) {
    let user_id = Uuid::new_v4();
    let input = create_user_api_key("User Key", user_id);
    let key_hash = "userhash12345678";

    let key = ctx
        .api_key_repo
        .create(input, key_hash)
        .await
        .expect("Failed to create API key");

    assert_eq!(key.name, "User Key");
    assert!(matches!(key.owner, ApiKeyOwner::User { user_id: id } if id == user_id));
}

pub async fn test_create_api_key_with_budget(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let input = CreateApiKey {
        name: "Budget Key".to_string(),
        owner: ApiKeyOwner::Organization { org_id },
        budget_limit_cents: Some(10000), // $100
        budget_period: Some(BudgetPeriod::Monthly),
        expires_at: None,
        scopes: None,
        allowed_models: None,
        ip_allowlist: None,
        rate_limit_rpm: None,
        rate_limit_tpm: None,
    };

    let key = ctx
        .api_key_repo
        .create(input, "budgethash123456")
        .await
        .expect("Failed to create API key");

    assert_eq!(key.budget_limit_cents, Some(10000));
    assert_eq!(key.budget_period, Some(BudgetPeriod::Monthly));
}

pub async fn test_create_duplicate_hash_fails(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let key_hash = "duplicatehash123";

    ctx.api_key_repo
        .create(create_org_api_key("First Key", org_id), key_hash)
        .await
        .expect("First key should succeed");

    let result = ctx
        .api_key_repo
        .create(create_org_api_key("Second Key", org_id), key_hash)
        .await;

    assert!(matches!(result, Err(DbError::Conflict(_))));
}

// ============================================================================
// Get By ID Tests
// ============================================================================

pub async fn test_get_by_id(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let created = ctx
        .api_key_repo
        .create(create_org_api_key("Get Test", org_id), "gettesthash12345")
        .await
        .expect("Failed to create key");

    let fetched = ctx
        .api_key_repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed")
        .expect("Key should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "Get Test");
    assert!(matches!(fetched.owner, ApiKeyOwner::Organization { .. }));
}

pub async fn test_get_by_id_not_found(ctx: &ApiKeyTestContext<'_>) {
    let result = ctx
        .api_key_repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_id_returns_revoked_key(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let created = ctx
        .api_key_repo
        .create(create_org_api_key("Revoke Test", org_id), "revoketesthash")
        .await
        .expect("Failed to create key");

    ctx.api_key_repo
        .revoke(created.id)
        .await
        .expect("Failed to revoke key");

    // get_by_id should still return revoked keys
    let fetched = ctx
        .api_key_repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed")
        .expect("Key should exist");

    assert_eq!(fetched.id, created.id);
    assert!(fetched.revoked_at.is_some());
}

// ============================================================================
// Get By Hash Tests
// ============================================================================

pub async fn test_get_by_hash(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let key_hash = "hashfortesting12";
    let created = ctx
        .api_key_repo
        .create(create_org_api_key("Hash Test", org_id), key_hash)
        .await
        .expect("Failed to create key");

    let fetched = ctx
        .api_key_repo
        .get_by_hash(key_hash)
        .await
        .expect("Query should succeed")
        .expect("Key should exist");

    assert_eq!(fetched.key.id, created.id);
    assert_eq!(fetched.key.name, "Hash Test");
    assert_eq!(fetched.org_id, Some(org_id));
    assert!(fetched.project_id.is_none());
    assert!(fetched.user_id.is_none());
}

pub async fn test_get_by_hash_not_found(ctx: &ApiKeyTestContext<'_>) {
    let result = ctx
        .api_key_repo
        .get_by_hash("nonexistenthash1")
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_hash_excludes_revoked(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let key_hash = "revokedhash12345";
    let created = ctx
        .api_key_repo
        .create(create_org_api_key("Revoked Key", org_id), key_hash)
        .await
        .expect("Failed to create key");

    ctx.api_key_repo
        .revoke(created.id)
        .await
        .expect("Failed to revoke key");

    // get_by_hash should NOT return revoked keys
    let result = ctx
        .api_key_repo
        .get_by_hash(key_hash)
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_hash_project_key_includes_org_id(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;
    let key_hash = "projectkeyhash12";

    ctx.api_key_repo
        .create(create_project_api_key("Project Key", project_id), key_hash)
        .await
        .expect("Failed to create key");

    let fetched = ctx
        .api_key_repo
        .get_by_hash(key_hash)
        .await
        .expect("Query should succeed")
        .expect("Key should exist");

    // For project keys, org_id should be looked up from the project
    assert_eq!(fetched.org_id, Some(org_id));
    assert_eq!(fetched.project_id, Some(project_id));
    assert!(fetched.user_id.is_none());
}

pub async fn test_get_by_hash_user_key(ctx: &ApiKeyTestContext<'_>) {
    let user_id = Uuid::new_v4();
    let key_hash = "userkeyhashhash1";

    ctx.api_key_repo
        .create(create_user_api_key("User Key", user_id), key_hash)
        .await
        .expect("Failed to create key");

    let fetched = ctx
        .api_key_repo
        .get_by_hash(key_hash)
        .await
        .expect("Query should succeed")
        .expect("Key should exist");

    // For user keys, org_id and project_id should be None
    assert!(fetched.org_id.is_none());
    assert!(fetched.project_id.is_none());
    assert_eq!(fetched.user_id, Some(user_id));
}

// ============================================================================
// List By Org Tests
// ============================================================================

pub async fn test_list_by_org_empty(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let result = ctx
        .api_key_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list keys");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_by_org(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..3 {
        ctx.api_key_repo
            .create(
                create_org_api_key(&format!("Key {}", i), org_id),
                &format!("hash{:016}", i),
            )
            .await
            .expect("Failed to create key");
    }

    let result = ctx
        .api_key_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list keys");

    assert_eq!(result.items.len(), 3);
    assert!(!result.has_more);
}

pub async fn test_list_by_org_only_returns_org_keys(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("org-1").await;
    let other_org_id = ctx.create_test_org("org-2").await;

    ctx.api_key_repo
        .create(create_org_api_key("Our Key", org_id), "ourkeyhash123456")
        .await
        .expect("Failed to create key");

    ctx.api_key_repo
        .create(
            create_org_api_key("Other Key", other_org_id),
            "otherkeyhash1234",
        )
        .await
        .expect("Failed to create key");

    let result = ctx
        .api_key_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list keys");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].name, "Our Key");
}

pub async fn test_list_by_org_pagination(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..5 {
        ctx.api_key_repo
            .create(
                create_org_api_key(&format!("Key {}", i), org_id),
                &format!("paginationhash{:02}", i),
            )
            .await
            .expect("Failed to create key");
    }

    let page1 = ctx
        .api_key_repo
        .list_by_org(
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
        .api_key_repo
        .list_by_org(
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

pub async fn test_count_by_org(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..3 {
        ctx.api_key_repo
            .create(
                create_org_api_key(&format!("Key {}", i), org_id),
                &format!("countorg{:08}", i),
            )
            .await
            .expect("Failed to create key");
    }

    let count = ctx
        .api_key_repo
        .count_by_org(org_id, false)
        .await
        .expect("Failed to count keys");

    assert_eq!(count, 3);
}

// ============================================================================
// List By Project Tests
// ============================================================================

pub async fn test_list_by_project(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;

    for i in 0..3 {
        ctx.api_key_repo
            .create(
                create_project_api_key(&format!("Project Key {}", i), project_id),
                &format!("projecthash{:05}", i),
            )
            .await
            .expect("Failed to create key");
    }

    let result = ctx
        .api_key_repo
        .list_by_project(project_id, ListParams::default())
        .await
        .expect("Failed to list keys");

    assert_eq!(result.items.len(), 3);
    assert!(!result.has_more);
}

pub async fn test_count_by_project(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;

    for i in 0..2 {
        ctx.api_key_repo
            .create(
                create_project_api_key(&format!("Key {}", i), project_id),
                &format!("projcount{:06}", i),
            )
            .await
            .expect("Failed to create key");
    }

    let count = ctx
        .api_key_repo
        .count_by_project(project_id, false)
        .await
        .expect("Failed to count keys");

    assert_eq!(count, 2);
}

// ============================================================================
// List By User Tests
// ============================================================================

pub async fn test_list_by_user(ctx: &ApiKeyTestContext<'_>) {
    let user_id = Uuid::new_v4();

    for i in 0..3 {
        ctx.api_key_repo
            .create(
                create_user_api_key(&format!("User Key {}", i), user_id),
                &format!("userhash{:07}", i),
            )
            .await
            .expect("Failed to create key");
    }

    let result = ctx
        .api_key_repo
        .list_by_user(user_id, ListParams::default())
        .await
        .expect("Failed to list keys");

    assert_eq!(result.items.len(), 3);
    assert!(!result.has_more);
}

pub async fn test_count_by_user(ctx: &ApiKeyTestContext<'_>) {
    let user_id = Uuid::new_v4();

    for i in 0..4 {
        ctx.api_key_repo
            .create(
                create_user_api_key(&format!("Key {}", i), user_id),
                &format!("usercount{:05}", i),
            )
            .await
            .expect("Failed to create key");
    }

    let count = ctx
        .api_key_repo
        .count_by_user(user_id, false)
        .await
        .expect("Failed to count keys");

    assert_eq!(count, 4);
}

// ============================================================================
// Revoke Tests
// ============================================================================

pub async fn test_revoke(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let created = ctx
        .api_key_repo
        .create(create_org_api_key("To Revoke", org_id), "revokekeyhash123")
        .await
        .expect("Failed to create key");

    assert!(created.revoked_at.is_none());

    ctx.api_key_repo
        .revoke(created.id)
        .await
        .expect("Failed to revoke key");

    let fetched = ctx
        .api_key_repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed")
        .expect("Key should exist");

    assert!(fetched.revoked_at.is_some());
}

pub async fn test_revoke_nonexistent_key_succeeds(ctx: &ApiKeyTestContext<'_>) {
    // revoke doesn't check if the key exists, it just sets is_active=0
    ctx.api_key_repo
        .revoke(Uuid::new_v4())
        .await
        .expect("Revoke should succeed even for non-existent key");
}

// ============================================================================
// Update Last Used Tests
// ============================================================================

pub async fn test_update_last_used(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let created = ctx
        .api_key_repo
        .create(
            create_org_api_key("Last Used Test", org_id),
            "lastusedkey1234",
        )
        .await
        .expect("Failed to create key");

    assert!(created.last_used_at.is_none());

    ctx.api_key_repo
        .update_last_used(created.id)
        .await
        .expect("Failed to update last_used");

    let fetched = ctx
        .api_key_repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed")
        .expect("Key should exist");

    assert!(fetched.last_used_at.is_some());
}

// ============================================================================
// List Includes Revoked Tests
// ============================================================================

pub async fn test_list_includes_revoked_keys(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let key1 = ctx
        .api_key_repo
        .create(create_org_api_key("Active Key", org_id), "activekey1234567")
        .await
        .expect("Failed to create key");

    let key2 = ctx
        .api_key_repo
        .create(
            create_org_api_key("Revoked Key", org_id),
            "revokedkey123456",
        )
        .await
        .expect("Failed to create key");

    ctx.api_key_repo
        .revoke(key2.id)
        .await
        .expect("Failed to revoke key");

    // list_by_org includes all keys (active and revoked)
    let result = ctx
        .api_key_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list keys");

    assert_eq!(result.items.len(), 2);

    let active_key = result.items.iter().find(|k| k.id == key1.id).unwrap();
    let revoked_key = result.items.iter().find(|k| k.id == key2.id).unwrap();

    assert!(active_key.revoked_at.is_none());
    assert!(revoked_key.revoked_at.is_some());
}

pub async fn test_count_includes_revoked_keys(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let key1 = ctx
        .api_key_repo
        .create(create_org_api_key("Key 1", org_id), "countrevoked1234")
        .await
        .expect("Failed to create key");

    ctx.api_key_repo
        .create(create_org_api_key("Key 2", org_id), "countrevoked5678")
        .await
        .expect("Failed to create key");

    ctx.api_key_repo
        .revoke(key1.id)
        .await
        .expect("Failed to revoke key");

    // count_by_org includes all keys (active and revoked)
    let count = ctx
        .api_key_repo
        .count_by_org(org_id, false)
        .await
        .expect("Failed to count keys");

    assert_eq!(count, 2);
}

// ============================================================================
// Budget Period Tests
// ============================================================================

pub async fn test_budget_period_daily(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let input = CreateApiKey {
        name: "Daily Budget".to_string(),
        owner: ApiKeyOwner::Organization { org_id },
        budget_limit_cents: Some(5000),
        budget_period: Some(BudgetPeriod::Daily),
        expires_at: None,
        scopes: None,
        allowed_models: None,
        ip_allowlist: None,
        rate_limit_rpm: None,
        rate_limit_tpm: None,
    };

    let created = ctx
        .api_key_repo
        .create(input, "dailybudgethash1")
        .await
        .expect("Failed to create key");

    let fetched = ctx
        .api_key_repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed")
        .expect("Key should exist");

    assert_eq!(fetched.budget_period, Some(BudgetPeriod::Daily));
}

// ============================================================================
// Owner Parsing Tests
// ============================================================================

pub async fn test_owner_parsing(ctx: &ApiKeyTestContext<'_>) {
    // Test the internal parse_owner function via create/get cycle
    let org_id = ctx.create_test_org("test-org").await;
    let project_id = ctx.create_test_project(org_id, "test-project").await;
    let user_id = Uuid::new_v4();

    let org_key = ctx
        .api_key_repo
        .create(create_org_api_key("Org", org_id), "parseorg12345678")
        .await
        .expect("Failed to create org key");

    let proj_key = ctx
        .api_key_repo
        .create(
            create_project_api_key("Project", project_id),
            "parseproj1234567",
        )
        .await
        .expect("Failed to create project key");

    let user_key = ctx
        .api_key_repo
        .create(create_user_api_key("User", user_id), "parseuser1234567")
        .await
        .expect("Failed to create user key");

    let fetched_org = ctx
        .api_key_repo
        .get_by_id(org_key.id)
        .await
        .unwrap()
        .unwrap();
    let fetched_proj = ctx
        .api_key_repo
        .get_by_id(proj_key.id)
        .await
        .unwrap()
        .unwrap();
    let fetched_user = ctx
        .api_key_repo
        .get_by_id(user_key.id)
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(
        fetched_org.owner,
        ApiKeyOwner::Organization { org_id: id } if id == org_id
    ));
    assert!(matches!(
        fetched_proj.owner,
        ApiKeyOwner::Project { project_id: id } if id == project_id
    ));
    assert!(matches!(
        fetched_user.owner,
        ApiKeyOwner::User { user_id: id } if id == user_id
    ));
}

// ============================================================================
// Rotation Tests
// ============================================================================

pub async fn test_rotate_creates_new_key(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let old_key = ctx
        .api_key_repo
        .create(
            create_org_api_key("Original Key", org_id),
            "originalhash1234",
        )
        .await
        .expect("Failed to create old key");

    let grace_until = Utc::now() + chrono::Duration::hours(24);
    let new_key_input = create_org_api_key("Original Key (rotated)", org_id);

    let new_key = ctx
        .api_key_repo
        .rotate(old_key.id, new_key_input, "newkeyhash123456", grace_until)
        .await
        .expect("Failed to rotate key");

    assert_eq!(new_key.name, "Original Key (rotated)");
    assert_eq!(new_key.rotated_from_key_id, Some(old_key.id));
    assert!(new_key.rotation_grace_until.is_none()); // New key doesn't have grace period
    assert!(matches!(new_key.owner, ApiKeyOwner::Organization { org_id: id } if id == org_id));
}

pub async fn test_rotate_sets_grace_until_on_old_key(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let old_key = ctx
        .api_key_repo
        .create(
            create_org_api_key("Key to Rotate", org_id),
            "keytorotate12345",
        )
        .await
        .expect("Failed to create old key");

    assert!(old_key.rotation_grace_until.is_none());

    let grace_until = Utc::now() + chrono::Duration::hours(1);
    let new_key_input = create_org_api_key("Key to Rotate (rotated)", org_id);

    ctx.api_key_repo
        .rotate(old_key.id, new_key_input, "rotatedkey123456", grace_until)
        .await
        .expect("Failed to rotate key");

    // Fetch the old key and verify grace_until is set
    let updated_old_key = ctx
        .api_key_repo
        .get_by_id(old_key.id)
        .await
        .expect("Query should succeed")
        .expect("Old key should still exist");

    assert!(updated_old_key.rotation_grace_until.is_some());
}

pub async fn test_old_key_works_during_grace_period(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let old_key = ctx
        .api_key_repo
        .create(create_org_api_key("Grace Test", org_id), "gracetesthash123")
        .await
        .expect("Failed to create old key");

    // Set grace period to 1 hour from now
    let grace_until = Utc::now() + chrono::Duration::hours(1);
    let new_key_input = create_org_api_key("Grace Test (rotated)", org_id);

    ctx.api_key_repo
        .rotate(old_key.id, new_key_input, "newgracetesthash", grace_until)
        .await
        .expect("Failed to rotate key");

    // Old key should still be retrievable by hash during grace period
    let result = ctx
        .api_key_repo
        .get_by_hash("gracetesthash123")
        .await
        .expect("Query should succeed");

    assert!(
        result.is_some(),
        "Old key should still work during grace period"
    );
}

pub async fn test_old_key_fails_after_grace_period(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let old_key = ctx
        .api_key_repo
        .create(
            create_org_api_key("Expired Grace", org_id),
            "expiredgracehash",
        )
        .await
        .expect("Failed to create old key");

    // Set grace period to 1 second ago (already expired)
    let grace_until = Utc::now() - chrono::Duration::seconds(1);
    let new_key_input = create_org_api_key("Expired Grace (rotated)", org_id);

    ctx.api_key_repo
        .rotate(old_key.id, new_key_input, "newexpiredhash12", grace_until)
        .await
        .expect("Failed to rotate key");

    // Old key should NOT be retrievable by hash after grace period expired
    let result = ctx
        .api_key_repo
        .get_by_hash("expiredgracehash")
        .await
        .expect("Query should succeed");

    assert!(
        result.is_none(),
        "Old key should not work after grace period expired"
    );
}

pub async fn test_new_key_works_after_rotation(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let old_key = ctx
        .api_key_repo
        .create(
            create_org_api_key("New Key Test", org_id),
            "newkeytesthash12",
        )
        .await
        .expect("Failed to create old key");

    let grace_until = Utc::now() + chrono::Duration::hours(1);
    let new_key_hash = "rotatednewkey123";

    let new_key = ctx
        .api_key_repo
        .rotate(
            old_key.id,
            create_org_api_key("New Key Test (rotated)", org_id),
            new_key_hash,
            grace_until,
        )
        .await
        .expect("Failed to rotate key");

    // New key should be retrievable by its hash
    let result = ctx
        .api_key_repo
        .get_by_hash(new_key_hash)
        .await
        .expect("Query should succeed");

    assert!(result.is_some());
    assert_eq!(result.unwrap().key.id, new_key.id);
}

// ============================================================================
// Get Key Hashes By User Tests
// ============================================================================

pub async fn test_get_key_hashes_by_user(ctx: &ApiKeyTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;
    let user_id = Uuid::new_v4();

    // Create two active user-owned keys
    let key1 = ctx
        .api_key_repo
        .create(
            create_user_api_key("User Key 1", user_id),
            "userhash_1_test",
        )
        .await
        .expect("Failed to create key");
    ctx.api_key_repo
        .create(
            create_user_api_key("User Key 2", user_id),
            "userhash_2_test",
        )
        .await
        .expect("Failed to create key");

    // Create an org key (different owner_type, should not appear)
    ctx.api_key_repo
        .create(create_org_api_key("Org Key", org_id), "orghash_not_user1")
        .await
        .expect("Failed to create key");

    // Create a key for another user (should not appear)
    ctx.api_key_repo
        .create(
            create_user_api_key("Other User", Uuid::new_v4()),
            "otheruserhash123",
        )
        .await
        .expect("Failed to create key");

    let hashes = ctx
        .api_key_repo
        .get_key_hashes_by_user(user_id)
        .await
        .expect("Failed to get key hashes");

    assert_eq!(hashes.len(), 2);
    assert!(hashes.contains(&"userhash_1_test".to_string()));
    assert!(hashes.contains(&"userhash_2_test".to_string()));

    // Revoke one key — should be excluded from results
    ctx.api_key_repo
        .revoke(key1.id)
        .await
        .expect("Failed to revoke");

    let hashes_after = ctx
        .api_key_repo
        .get_key_hashes_by_user(user_id)
        .await
        .expect("Failed to get key hashes");

    assert_eq!(hashes_after.len(), 1);
    assert!(hashes_after.contains(&"userhash_2_test".to_string()));
}

pub async fn test_get_key_hashes_by_user_empty(ctx: &ApiKeyTestContext<'_>) {
    let hashes = ctx
        .api_key_repo
        .get_key_hashes_by_user(Uuid::new_v4())
        .await
        .expect("Failed to get key hashes");

    assert!(hashes.is_empty());
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use super::*;
    use crate::db::{
        sqlite::{SqliteApiKeyRepo, SqliteOrganizationRepo, SqliteProjectRepo},
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repos() -> (SqliteApiKeyRepo, SqliteOrganizationRepo, SqliteProjectRepo) {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        (
            SqliteApiKeyRepo::new(pool.clone()),
            SqliteOrganizationRepo::new(pool.clone()),
            SqliteProjectRepo::new(pool),
        )
    }

    macro_rules! sqlite_test {
        ($name:ident) => {
            #[tokio::test]
            async fn $name() {
                let (api_key_repo, org_repo, project_repo) = create_repos().await;
                let ctx = ApiKeyTestContext {
                    api_key_repo: &api_key_repo,
                    org_repo: &org_repo,
                    project_repo: &project_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    // Create tests
    sqlite_test!(test_create_org_api_key);
    sqlite_test!(test_create_project_api_key);
    sqlite_test!(test_create_user_api_key);
    sqlite_test!(test_create_api_key_with_budget);
    sqlite_test!(test_create_duplicate_hash_fails);

    // Get by ID tests
    sqlite_test!(test_get_by_id);
    sqlite_test!(test_get_by_id_not_found);
    sqlite_test!(test_get_by_id_returns_revoked_key);

    // Get by hash tests
    sqlite_test!(test_get_by_hash);
    sqlite_test!(test_get_by_hash_not_found);
    sqlite_test!(test_get_by_hash_excludes_revoked);
    sqlite_test!(test_get_by_hash_project_key_includes_org_id);
    sqlite_test!(test_get_by_hash_user_key);

    // List by org tests
    sqlite_test!(test_list_by_org_empty);
    sqlite_test!(test_list_by_org);
    sqlite_test!(test_list_by_org_only_returns_org_keys);
    sqlite_test!(test_list_by_org_pagination);
    sqlite_test!(test_count_by_org);

    // List by project tests
    sqlite_test!(test_list_by_project);
    sqlite_test!(test_count_by_project);

    // List by user tests
    sqlite_test!(test_list_by_user);
    sqlite_test!(test_count_by_user);

    // Revoke tests
    sqlite_test!(test_revoke);
    sqlite_test!(test_revoke_nonexistent_key_succeeds);

    // Update last used tests
    sqlite_test!(test_update_last_used);

    // List includes revoked tests
    sqlite_test!(test_list_includes_revoked_keys);
    sqlite_test!(test_count_includes_revoked_keys);

    // Budget period tests
    sqlite_test!(test_budget_period_daily);

    // Owner parsing tests
    sqlite_test!(test_owner_parsing);

    // Rotation tests
    sqlite_test!(test_rotate_creates_new_key);
    sqlite_test!(test_rotate_sets_grace_until_on_old_key);
    sqlite_test!(test_old_key_works_during_grace_period);
    sqlite_test!(test_old_key_fails_after_grace_period);
    sqlite_test!(test_new_key_works_after_rotation);

    // Get key hashes by user tests
    sqlite_test!(test_get_key_hashes_by_user);
    sqlite_test!(test_get_key_hashes_by_user_empty);
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use super::*;
    use crate::db::{
        postgres::{PostgresApiKeyRepo, PostgresOrganizationRepo, PostgresProjectRepo},
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let pool = create_isolated_postgres_pool().await;
                run_postgres_migrations(&pool).await;
                let api_key_repo = PostgresApiKeyRepo::new(pool.clone(), None);
                let org_repo = PostgresOrganizationRepo::new(pool.clone(), None);
                let project_repo = PostgresProjectRepo::new(pool, None);
                let ctx = ApiKeyTestContext {
                    api_key_repo: &api_key_repo,
                    org_repo: &org_repo,
                    project_repo: &project_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    // Create tests
    postgres_test!(test_create_org_api_key);
    postgres_test!(test_create_project_api_key);
    postgres_test!(test_create_user_api_key);
    postgres_test!(test_create_api_key_with_budget);
    postgres_test!(test_create_duplicate_hash_fails);

    // Get by ID tests
    postgres_test!(test_get_by_id);
    postgres_test!(test_get_by_id_not_found);
    postgres_test!(test_get_by_id_returns_revoked_key);

    // Get by hash tests
    postgres_test!(test_get_by_hash);
    postgres_test!(test_get_by_hash_not_found);
    postgres_test!(test_get_by_hash_excludes_revoked);
    postgres_test!(test_get_by_hash_project_key_includes_org_id);
    postgres_test!(test_get_by_hash_user_key);

    // List by org tests
    postgres_test!(test_list_by_org_empty);
    postgres_test!(test_list_by_org);
    postgres_test!(test_list_by_org_only_returns_org_keys);
    postgres_test!(test_list_by_org_pagination);
    postgres_test!(test_count_by_org);

    // List by project tests
    postgres_test!(test_list_by_project);
    postgres_test!(test_count_by_project);

    // List by user tests
    postgres_test!(test_list_by_user);
    postgres_test!(test_count_by_user);

    // Revoke tests
    postgres_test!(test_revoke);
    postgres_test!(test_revoke_nonexistent_key_succeeds);

    // Update last used tests
    postgres_test!(test_update_last_used);

    // List includes revoked tests
    postgres_test!(test_list_includes_revoked_keys);
    postgres_test!(test_count_includes_revoked_keys);

    // Budget period tests
    postgres_test!(test_budget_period_daily);

    // Owner parsing tests
    postgres_test!(test_owner_parsing);

    // Rotation tests
    postgres_test!(test_rotate_creates_new_key);
    postgres_test!(test_rotate_sets_grace_until_on_old_key);
    postgres_test!(test_old_key_works_during_grace_period);
    postgres_test!(test_old_key_fails_after_grace_period);
    postgres_test!(test_new_key_works_after_rotation);

    // Get key hashes by user tests
    postgres_test!(test_get_key_hashes_by_user);
    postgres_test!(test_get_key_hashes_by_user_empty);
}
