//! Shared tests for DynamicProviderRepo implementations
//!
//! Tests are written as async functions that take a reference to a DynamicProviderRepo trait object.

use uuid::Uuid;

use crate::{
    db::{
        error::DbError,
        repos::{DynamicProviderRepo, ListParams},
    },
    models::{CreateDynamicProvider, ProviderOwner, UpdateDynamicProvider},
};

// ============================================================================
// Test Input Helpers
// ============================================================================

fn create_org_provider(name: &str, org_id: Uuid) -> CreateDynamicProvider {
    CreateDynamicProvider {
        name: name.to_string(),
        owner: ProviderOwner::Organization { org_id },
        provider_type: "open_ai".to_string(),
        base_url: "https://api.openai.com".to_string(),
        api_key: Some("vault://openai-key".to_string()),
        config: None,
        models: Some(vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()]),
    }
}

fn create_project_provider(name: &str, project_id: Uuid) -> CreateDynamicProvider {
    CreateDynamicProvider {
        name: name.to_string(),
        owner: ProviderOwner::Project { project_id },
        provider_type: "anthropic".to_string(),
        base_url: "https://api.anthropic.com".to_string(),
        api_key: None,
        config: None,
        models: None,
    }
}

fn create_user_provider(name: &str, user_id: Uuid) -> CreateDynamicProvider {
    CreateDynamicProvider {
        name: name.to_string(),
        owner: ProviderOwner::User { user_id },
        provider_type: "azure_openai".to_string(),
        base_url: "https://myinstance.openai.azure.com".to_string(),
        api_key: Some("env://AZURE_KEY".to_string()),
        config: None,
        models: Some(vec!["gpt-4".to_string()]),
    }
}

// ============================================================================
// Create Tests
// ============================================================================

pub async fn test_create_org_provider(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_provider("openai-prod", org_id);
    let provider = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create provider");

    assert_eq!(provider.name, "openai-prod");
    assert_eq!(provider.owner, ProviderOwner::Organization { org_id });
    assert_eq!(provider.provider_type, "open_ai");
    assert_eq!(provider.base_url, "https://api.openai.com");
    assert_eq!(
        provider.api_key_secret_ref,
        Some("vault://openai-key".to_string())
    );
    assert_eq!(provider.models, vec!["gpt-4", "gpt-3.5-turbo"]);
    assert!(provider.is_enabled);
}

pub async fn test_create_project_provider(repo: &dyn DynamicProviderRepo) {
    let project_id = Uuid::new_v4();
    let input = create_project_provider("anthropic-dev", project_id);
    let provider = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create provider");

    assert_eq!(provider.name, "anthropic-dev");
    assert_eq!(provider.owner, ProviderOwner::Project { project_id });
    assert_eq!(provider.provider_type, "anthropic");
    assert!(provider.api_key_secret_ref.is_none());
    assert!(provider.models.is_empty()); // None becomes empty vec
}

pub async fn test_create_user_provider(repo: &dyn DynamicProviderRepo) {
    let user_id = Uuid::new_v4();
    let input = create_user_provider("my-azure", user_id);
    let provider = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create provider");

    assert_eq!(provider.name, "my-azure");
    assert_eq!(provider.owner, ProviderOwner::User { user_id });
    assert_eq!(provider.provider_type, "azure_openai");
}

pub async fn test_create_duplicate_name_same_owner_fails(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();

    let input1 = create_org_provider("openai", org_id);
    repo.create(Uuid::new_v4(), input1)
        .await
        .expect("First create should work");

    let input2 = create_org_provider("openai", org_id);
    let result = repo.create(Uuid::new_v4(), input2).await;

    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_create_same_name_different_owners_succeeds(repo: &dyn DynamicProviderRepo) {
    let org_id1 = Uuid::new_v4();
    let org_id2 = Uuid::new_v4();

    let input1 = create_org_provider("openai", org_id1);
    let input2 = create_org_provider("openai", org_id2);

    repo.create(Uuid::new_v4(), input1)
        .await
        .expect("First create should work");
    repo.create(Uuid::new_v4(), input2)
        .await
        .expect("Second create should work (different owner)");
}

// ============================================================================
// Get By ID Tests
// ============================================================================

pub async fn test_get_by_id_found(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_provider("openai", org_id);
    let created = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");

    let fetched = repo
        .get_by_id(created.id)
        .await
        .expect("Failed to fetch")
        .expect("Should find provider");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "openai");
    assert_eq!(fetched.owner, ProviderOwner::Organization { org_id });
}

pub async fn test_get_by_id_not_found(repo: &dyn DynamicProviderRepo) {
    let result = repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

// ============================================================================
// Get By Name Tests
// ============================================================================

pub async fn test_get_by_name_found(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_provider("openai-prod", org_id);
    let created = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");

    let owner = ProviderOwner::Organization { org_id };
    let fetched = repo
        .get_by_name(&owner, "openai-prod")
        .await
        .expect("Failed to fetch")
        .expect("Should find provider");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "openai-prod");
}

pub async fn test_get_by_name_wrong_owner(repo: &dyn DynamicProviderRepo) {
    let org_id1 = Uuid::new_v4();
    let org_id2 = Uuid::new_v4();

    let input = create_org_provider("openai", org_id1);
    repo.create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");

    // Try to find with different owner
    let wrong_owner = ProviderOwner::Organization { org_id: org_id2 };
    let result = repo
        .get_by_name(&wrong_owner, "openai")
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_name_not_found(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let owner = ProviderOwner::Organization { org_id };
    let result = repo
        .get_by_name(&owner, "nonexistent")
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

// ============================================================================
// List By Org Tests
// ============================================================================

pub async fn test_list_by_org_empty(repo: &dyn DynamicProviderRepo) {
    let result = repo
        .list_by_org(Uuid::new_v4(), ListParams::default())
        .await
        .expect("Failed to list");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_by_org_multiple(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();

    let input1 = create_org_provider("alpha", org_id);
    let input2 = create_org_provider("beta", org_id);
    let input3 = create_org_provider("gamma", org_id);

    repo.create(Uuid::new_v4(), input1)
        .await
        .expect("Failed to create");
    repo.create(Uuid::new_v4(), input2)
        .await
        .expect("Failed to create");
    repo.create(Uuid::new_v4(), input3)
        .await
        .expect("Failed to create");

    let result = repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 3);
    assert!(!result.has_more);
    // Should be ordered by created_at DESC (newest first) - verify all names present
    let names: Vec<&str> = result.items.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"alpha"));
    assert!(names.contains(&"beta"));
    assert!(names.contains(&"gamma"));
}

pub async fn test_list_by_org_excludes_other_orgs(repo: &dyn DynamicProviderRepo) {
    let org_id1 = Uuid::new_v4();
    let org_id2 = Uuid::new_v4();

    let input1 = create_org_provider("provider-a", org_id1);
    let input2 = create_org_provider("provider-b", org_id2);

    repo.create(Uuid::new_v4(), input1)
        .await
        .expect("Failed to create");
    repo.create(Uuid::new_v4(), input2)
        .await
        .expect("Failed to create");

    let result = repo
        .list_by_org(org_id1, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].name, "provider-a");
}

// ============================================================================
// List By Project Tests
// ============================================================================

pub async fn test_list_by_project_empty(repo: &dyn DynamicProviderRepo) {
    let result = repo
        .list_by_project(Uuid::new_v4(), ListParams::default())
        .await
        .expect("Failed to list");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_by_project_multiple(repo: &dyn DynamicProviderRepo) {
    let project_id = Uuid::new_v4();

    let input1 = create_project_provider("anthropic-dev", project_id);
    let input2 = create_project_provider("openai-dev", project_id);

    repo.create(Uuid::new_v4(), input1)
        .await
        .expect("Failed to create");
    repo.create(Uuid::new_v4(), input2)
        .await
        .expect("Failed to create");

    let result = repo
        .list_by_project(project_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 2);
    assert!(!result.has_more);
    // Verify both names present (order is by created_at DESC)
    let names: Vec<&str> = result.items.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"anthropic-dev"));
    assert!(names.contains(&"openai-dev"));
}

// ============================================================================
// List By User Tests
// ============================================================================

pub async fn test_list_by_user_empty(repo: &dyn DynamicProviderRepo) {
    let result = repo
        .list_by_user(Uuid::new_v4(), ListParams::default())
        .await
        .expect("Failed to list");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_by_user_multiple(repo: &dyn DynamicProviderRepo) {
    let user_id = Uuid::new_v4();

    let input1 = create_user_provider("my-azure", user_id);
    let input2 = create_user_provider("my-openai", user_id);

    repo.create(Uuid::new_v4(), input1)
        .await
        .expect("Failed to create");
    repo.create(Uuid::new_v4(), input2)
        .await
        .expect("Failed to create");

    let result = repo
        .list_by_user(user_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 2);
    assert!(!result.has_more);
}

// ============================================================================
// List Enabled Tests
// ============================================================================

pub async fn test_list_enabled_by_user(repo: &dyn DynamicProviderRepo) {
    let user_id = Uuid::new_v4();

    let input1 = create_user_provider("enabled-one", user_id);
    repo.create(Uuid::new_v4(), input1)
        .await
        .expect("Failed to create");

    let input2 = create_user_provider("enabled-two", user_id);
    repo.create(Uuid::new_v4(), input2)
        .await
        .expect("Failed to create");

    // Create a disabled user provider
    let input3 = create_user_provider("disabled", user_id);
    let disabled = repo
        .create(Uuid::new_v4(), input3)
        .await
        .expect("Failed to create");
    repo.update(
        disabled.id,
        UpdateDynamicProvider {
            base_url: None,
            api_key: None,
            config: None,
            models: None,
            is_enabled: Some(false),
        },
    )
    .await
    .expect("Failed to disable");

    // Create a provider for a different user
    let other_user_id = Uuid::new_v4();
    let input4 = create_user_provider("other-user", other_user_id);
    repo.create(Uuid::new_v4(), input4)
        .await
        .expect("Failed to create");

    let result = repo
        .list_enabled_by_user(user_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 2);
    let names: Vec<&str> = result.items.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"enabled-one"));
    assert!(names.contains(&"enabled-two"));
    assert!(!names.contains(&"disabled"));
    assert!(!names.contains(&"other-user"));
}

pub async fn test_list_enabled_by_user_empty(repo: &dyn DynamicProviderRepo) {
    let result = repo
        .list_enabled_by_user(Uuid::new_v4(), ListParams::default())
        .await
        .expect("Failed to list");

    assert!(result.items.is_empty());
}

pub async fn test_list_enabled_by_org(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();

    let input1 = create_org_provider("enabled-one", org_id);
    repo.create(Uuid::new_v4(), input1)
        .await
        .expect("Failed to create");

    let input2 = create_org_provider("enabled-two", org_id);
    repo.create(Uuid::new_v4(), input2)
        .await
        .expect("Failed to create");

    // Create a disabled provider
    let mut input3 = create_org_provider("disabled", org_id);
    input3.name = "disabled".to_string();
    let disabled = repo
        .create(Uuid::new_v4(), input3)
        .await
        .expect("Failed to create");
    repo.update(
        disabled.id,
        UpdateDynamicProvider {
            base_url: None,
            api_key: None,
            config: None,
            models: None,
            is_enabled: Some(false),
        },
    )
    .await
    .expect("Failed to disable");

    // Create a provider in a different org
    let other_org_id = Uuid::new_v4();
    let input4 = create_org_provider("other-org", other_org_id);
    repo.create(Uuid::new_v4(), input4)
        .await
        .expect("Failed to create");

    let result = repo
        .list_enabled_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 2);
    let names: Vec<&str> = result.items.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"enabled-one"));
    assert!(names.contains(&"enabled-two"));
    assert!(!names.contains(&"disabled"));
    assert!(!names.contains(&"other-org"));
}

pub async fn test_list_enabled_by_org_empty(repo: &dyn DynamicProviderRepo) {
    let result = repo
        .list_enabled_by_org(Uuid::new_v4(), ListParams::default())
        .await
        .expect("Failed to list");

    assert!(result.items.is_empty());
}

pub async fn test_list_enabled_by_project(repo: &dyn DynamicProviderRepo) {
    let project_id = Uuid::new_v4();

    let input1 = create_project_provider("enabled-one", project_id);
    repo.create(Uuid::new_v4(), input1)
        .await
        .expect("Failed to create");

    let input2 = create_project_provider("enabled-two", project_id);
    repo.create(Uuid::new_v4(), input2)
        .await
        .expect("Failed to create");

    // Create a disabled project provider
    let input3 = create_project_provider("disabled", project_id);
    let disabled = repo
        .create(Uuid::new_v4(), input3)
        .await
        .expect("Failed to create");
    repo.update(
        disabled.id,
        UpdateDynamicProvider {
            base_url: None,
            api_key: None,
            config: None,
            models: None,
            is_enabled: Some(false),
        },
    )
    .await
    .expect("Failed to disable");

    // Create a provider in a different project
    let other_project_id = Uuid::new_v4();
    let input4 = create_project_provider("other-project", other_project_id);
    repo.create(Uuid::new_v4(), input4)
        .await
        .expect("Failed to create");

    let result = repo
        .list_enabled_by_project(project_id, ListParams::default())
        .await
        .expect("Failed to list");

    assert_eq!(result.items.len(), 2);
    let names: Vec<&str> = result.items.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"enabled-one"));
    assert!(names.contains(&"enabled-two"));
    assert!(!names.contains(&"disabled"));
    assert!(!names.contains(&"other-project"));
}

pub async fn test_list_enabled_by_project_empty(repo: &dyn DynamicProviderRepo) {
    let result = repo
        .list_enabled_by_project(Uuid::new_v4(), ListParams::default())
        .await
        .expect("Failed to list");

    assert!(result.items.is_empty());
}

// ============================================================================
// Update Tests
// ============================================================================

pub async fn test_update_base_url(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_provider("openai", org_id);
    let created = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");

    let update = UpdateDynamicProvider {
        base_url: Some("https://new.openai.com".to_string()),
        api_key: None,
        config: None,
        models: None,
        is_enabled: None,
    };

    let updated = repo
        .update(created.id, update)
        .await
        .expect("Failed to update");

    assert_eq!(updated.base_url, "https://new.openai.com");
    assert!(updated.updated_at >= created.updated_at);
    // Other fields unchanged
    assert_eq!(updated.name, "openai");
    assert!(updated.is_enabled);
}

pub async fn test_update_api_key(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_provider("openai", org_id);
    let created = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");

    let update = UpdateDynamicProvider {
        base_url: None,
        api_key: Some("vault://new-key".to_string()),
        config: None,
        models: None,
        is_enabled: None,
    };

    let updated = repo
        .update(created.id, update)
        .await
        .expect("Failed to update");

    assert_eq!(
        updated.api_key_secret_ref,
        Some("vault://new-key".to_string())
    );
}

pub async fn test_update_models(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_provider("openai", org_id);
    let created = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");
    assert_eq!(created.models, vec!["gpt-4", "gpt-3.5-turbo"]);

    let update = UpdateDynamicProvider {
        base_url: None,
        api_key: None,
        config: None,
        models: Some(vec!["o1".to_string(), "o1-mini".to_string()]),
        is_enabled: None,
    };

    let updated = repo
        .update(created.id, update)
        .await
        .expect("Failed to update");

    assert_eq!(updated.models, vec!["o1", "o1-mini"]);
}

pub async fn test_update_is_enabled(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_provider("openai", org_id);
    let created = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");
    assert!(created.is_enabled);

    let update = UpdateDynamicProvider {
        base_url: None,
        api_key: None,
        config: None,
        models: None,
        is_enabled: Some(false),
    };

    let updated = repo
        .update(created.id, update)
        .await
        .expect("Failed to update");

    assert!(!updated.is_enabled);
}

pub async fn test_update_multiple_fields(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_provider("openai", org_id);
    let created = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");

    let update = UpdateDynamicProvider {
        base_url: Some("https://updated.api.com".to_string()),
        api_key: Some("vault://updated-key".to_string()),
        config: None,
        models: Some(vec!["new-model".to_string()]),
        is_enabled: Some(false),
    };

    let updated = repo
        .update(created.id, update)
        .await
        .expect("Failed to update");

    assert_eq!(updated.base_url, "https://updated.api.com");
    assert_eq!(
        updated.api_key_secret_ref,
        Some("vault://updated-key".to_string())
    );
    assert_eq!(updated.models, vec!["new-model"]);
    assert!(!updated.is_enabled);
}

pub async fn test_update_nonexistent_returns_not_found(repo: &dyn DynamicProviderRepo) {
    let update = UpdateDynamicProvider {
        base_url: Some("https://test.com".to_string()),
        api_key: None,
        config: None,
        models: None,
        is_enabled: None,
    };

    let result = repo.update(Uuid::new_v4(), update).await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

// ============================================================================
// Delete Tests
// ============================================================================

pub async fn test_delete_existing(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_provider("openai", org_id);
    let created = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");

    repo.delete(created.id).await.expect("Failed to delete");

    let fetched = repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed");
    assert!(fetched.is_none());
}

pub async fn test_delete_nonexistent_succeeds(repo: &dyn DynamicProviderRepo) {
    // Should not error even for nonexistent ID
    repo.delete(Uuid::new_v4())
        .await
        .expect("Delete should succeed");
}

// ============================================================================
// Models Serialization Tests
// ============================================================================

pub async fn test_models_empty_vec(repo: &dyn DynamicProviderRepo) {
    let org_id = Uuid::new_v4();
    let input = CreateDynamicProvider {
        name: "test".to_string(),
        owner: ProviderOwner::Organization { org_id },
        provider_type: "open_ai".to_string(),
        base_url: "https://api.openai.com".to_string(),
        api_key: None,
        config: None,
        models: Some(vec![]),
    };

    let created = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");
    assert!(created.models.is_empty());

    let fetched = repo
        .get_by_id(created.id)
        .await
        .expect("Query failed")
        .expect("Should find");
    assert!(fetched.models.is_empty());
}

pub async fn test_models_none_becomes_empty(repo: &dyn DynamicProviderRepo) {
    let project_id = Uuid::new_v4();
    let input = create_project_provider("test", project_id); // models is None

    let created = repo
        .create(Uuid::new_v4(), input)
        .await
        .expect("Failed to create");
    assert!(created.models.is_empty());
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use crate::db::{
        sqlite::SqliteDynamicProviderRepo,
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repo() -> SqliteDynamicProviderRepo {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        SqliteDynamicProviderRepo::new(pool)
    }

    macro_rules! sqlite_test {
        ($name:ident) => {
            #[tokio::test]
            async fn $name() {
                let repo = create_repo().await;
                super::$name(&repo).await;
            }
        };
    }

    // Create tests
    sqlite_test!(test_create_org_provider);
    sqlite_test!(test_create_project_provider);
    sqlite_test!(test_create_user_provider);
    sqlite_test!(test_create_duplicate_name_same_owner_fails);
    sqlite_test!(test_create_same_name_different_owners_succeeds);

    // Get by ID tests
    sqlite_test!(test_get_by_id_found);
    sqlite_test!(test_get_by_id_not_found);

    // Get by name tests
    sqlite_test!(test_get_by_name_found);
    sqlite_test!(test_get_by_name_wrong_owner);
    sqlite_test!(test_get_by_name_not_found);

    // List by org tests
    sqlite_test!(test_list_by_org_empty);
    sqlite_test!(test_list_by_org_multiple);
    sqlite_test!(test_list_by_org_excludes_other_orgs);

    // List by project tests
    sqlite_test!(test_list_by_project_empty);
    sqlite_test!(test_list_by_project_multiple);

    // List by user tests
    sqlite_test!(test_list_by_user_empty);
    sqlite_test!(test_list_by_user_multiple);

    // List enabled tests
    sqlite_test!(test_list_enabled_by_user);
    sqlite_test!(test_list_enabled_by_user_empty);
    sqlite_test!(test_list_enabled_by_org);
    sqlite_test!(test_list_enabled_by_org_empty);
    sqlite_test!(test_list_enabled_by_project);
    sqlite_test!(test_list_enabled_by_project_empty);

    // Update tests
    sqlite_test!(test_update_base_url);
    sqlite_test!(test_update_api_key);
    sqlite_test!(test_update_models);
    sqlite_test!(test_update_is_enabled);
    sqlite_test!(test_update_multiple_fields);
    sqlite_test!(test_update_nonexistent_returns_not_found);

    // Delete tests
    sqlite_test!(test_delete_existing);
    sqlite_test!(test_delete_nonexistent_succeeds);

    // Models serialization tests
    sqlite_test!(test_models_empty_vec);
    sqlite_test!(test_models_none_becomes_empty);
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use crate::db::{
        postgres::PostgresDynamicProviderRepo,
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let pool = create_isolated_postgres_pool().await;
                run_postgres_migrations(&pool).await;
                let repo = PostgresDynamicProviderRepo::new(pool, None);
                super::$name(&repo).await;
            }
        };
    }

    // Create tests
    postgres_test!(test_create_org_provider);
    postgres_test!(test_create_project_provider);
    postgres_test!(test_create_user_provider);
    postgres_test!(test_create_duplicate_name_same_owner_fails);
    postgres_test!(test_create_same_name_different_owners_succeeds);

    // Get by ID tests
    postgres_test!(test_get_by_id_found);
    postgres_test!(test_get_by_id_not_found);

    // Get by name tests
    postgres_test!(test_get_by_name_found);
    postgres_test!(test_get_by_name_wrong_owner);
    postgres_test!(test_get_by_name_not_found);

    // List by org tests
    postgres_test!(test_list_by_org_empty);
    postgres_test!(test_list_by_org_multiple);
    postgres_test!(test_list_by_org_excludes_other_orgs);

    // List by project tests
    postgres_test!(test_list_by_project_empty);
    postgres_test!(test_list_by_project_multiple);

    // List by user tests
    postgres_test!(test_list_by_user_empty);
    postgres_test!(test_list_by_user_multiple);

    // List enabled tests
    postgres_test!(test_list_enabled_by_user);
    postgres_test!(test_list_enabled_by_user_empty);
    postgres_test!(test_list_enabled_by_org);
    postgres_test!(test_list_enabled_by_org_empty);
    postgres_test!(test_list_enabled_by_project);
    postgres_test!(test_list_enabled_by_project_empty);

    // Update tests
    postgres_test!(test_update_base_url);
    postgres_test!(test_update_api_key);
    postgres_test!(test_update_models);
    postgres_test!(test_update_is_enabled);
    postgres_test!(test_update_multiple_fields);
    postgres_test!(test_update_nonexistent_returns_not_found);

    // Delete tests
    postgres_test!(test_delete_existing);
    postgres_test!(test_delete_nonexistent_succeeds);

    // Models serialization tests
    postgres_test!(test_models_empty_vec);
    postgres_test!(test_models_none_becomes_empty);
}
