//! Shared tests for ModelPricingRepo implementations
//!
//! Tests are written as async functions that take a reference to a ModelPricingRepo trait object.

use uuid::Uuid;

use crate::{
    db::{
        error::DbError,
        repos::{ListParams, ModelPricingRepo},
    },
    models::{CreateModelPricing, PricingOwner, PricingSource, UpdateModelPricing},
};

// ============================================================================
// Test Input Helpers
// ============================================================================

fn create_global_pricing(provider: &str, model: &str) -> CreateModelPricing {
    CreateModelPricing {
        owner: PricingOwner::Global,
        provider: provider.to_string(),
        model: model.to_string(),
        input_per_1m_tokens: 1000,
        output_per_1m_tokens: 2000,
        per_image: None,
        per_request: None,
        cached_input_per_1m_tokens: None,
        cache_write_per_1m_tokens: None,
        reasoning_per_1m_tokens: None,
        per_second: None,
        per_1m_characters: None,
        source: PricingSource::Manual,
    }
}

fn create_org_pricing(org_id: Uuid, provider: &str, model: &str) -> CreateModelPricing {
    CreateModelPricing {
        owner: PricingOwner::Organization { org_id },
        provider: provider.to_string(),
        model: model.to_string(),
        input_per_1m_tokens: 1500,
        output_per_1m_tokens: 2500,
        per_image: Some(100),
        per_request: None,
        cached_input_per_1m_tokens: Some(500),
        cache_write_per_1m_tokens: None,
        reasoning_per_1m_tokens: None,
        per_second: None,
        per_1m_characters: None,
        source: PricingSource::Manual,
    }
}

fn create_project_pricing(project_id: Uuid, provider: &str, model: &str) -> CreateModelPricing {
    CreateModelPricing {
        owner: PricingOwner::Project { project_id },
        provider: provider.to_string(),
        model: model.to_string(),
        input_per_1m_tokens: 1800,
        output_per_1m_tokens: 2800,
        per_image: None,
        per_request: Some(50),
        cached_input_per_1m_tokens: None,
        cache_write_per_1m_tokens: Some(200),
        reasoning_per_1m_tokens: None,
        per_second: None,
        per_1m_characters: None,
        source: PricingSource::ProviderApi,
    }
}

fn create_user_pricing(user_id: Uuid, provider: &str, model: &str) -> CreateModelPricing {
    CreateModelPricing {
        owner: PricingOwner::User { user_id },
        provider: provider.to_string(),
        model: model.to_string(),
        input_per_1m_tokens: 2000,
        output_per_1m_tokens: 3000,
        per_image: Some(200),
        per_request: Some(75),
        cached_input_per_1m_tokens: Some(800),
        cache_write_per_1m_tokens: Some(300),
        reasoning_per_1m_tokens: Some(4000),
        per_second: None,
        per_1m_characters: None,
        source: PricingSource::Default,
    }
}

// ============================================================================
// Create Tests
// ============================================================================

pub async fn test_create_global_pricing(repo: &dyn ModelPricingRepo) {
    let input = create_global_pricing("openai", "gpt-4");
    let pricing = repo.create(input).await.expect("Failed to create pricing");

    assert!(!pricing.id.is_nil());
    assert!(matches!(pricing.owner, PricingOwner::Global));
    assert_eq!(pricing.provider, "openai");
    assert_eq!(pricing.model, "gpt-4");
    assert_eq!(pricing.input_per_1m_tokens, 1000);
    assert_eq!(pricing.output_per_1m_tokens, 2000);
    assert_eq!(pricing.source, PricingSource::Manual);
}

pub async fn test_create_org_pricing(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_pricing(org_id, "anthropic", "claude-3");
    let pricing = repo.create(input).await.expect("Failed to create pricing");

    assert!(matches!(pricing.owner, PricingOwner::Organization { org_id: id } if id == org_id));
    assert_eq!(pricing.provider, "anthropic");
    assert_eq!(pricing.per_image, Some(100));
    assert_eq!(pricing.cached_input_per_1m_tokens, Some(500));
}

pub async fn test_create_project_pricing(repo: &dyn ModelPricingRepo) {
    let project_id = Uuid::new_v4();
    let input = create_project_pricing(project_id, "google", "gemini-pro");
    let pricing = repo.create(input).await.expect("Failed to create pricing");

    assert!(matches!(pricing.owner, PricingOwner::Project { project_id: id } if id == project_id));
    assert_eq!(pricing.per_request, Some(50));
    assert_eq!(pricing.cache_write_per_1m_tokens, Some(200));
    assert_eq!(pricing.source, PricingSource::ProviderApi);
}

pub async fn test_create_user_pricing(repo: &dyn ModelPricingRepo) {
    let user_id = Uuid::new_v4();
    let input = create_user_pricing(user_id, "openai", "gpt-4-turbo");
    let pricing = repo.create(input).await.expect("Failed to create pricing");

    assert!(matches!(pricing.owner, PricingOwner::User { user_id: id } if id == user_id));
    assert_eq!(pricing.reasoning_per_1m_tokens, Some(4000));
    assert_eq!(pricing.source, PricingSource::Default);
}

pub async fn test_create_duplicate_pricing_fails(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();

    // Use org pricing because SQLite treats NULL as distinct in UNIQUE constraints
    let input = create_org_pricing(org_id, "openai", "gpt-4");
    repo.create(input)
        .await
        .expect("First create should succeed");

    let input2 = create_org_pricing(org_id, "openai", "gpt-4");
    let result = repo.create(input2).await;

    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_same_model_different_owners_allowed(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();

    // Create global pricing
    let input1 = create_global_pricing("openai", "gpt-4");
    repo.create(input1)
        .await
        .expect("Global create should succeed");

    // Create org pricing for same model
    let input2 = create_org_pricing(org_id, "openai", "gpt-4");
    repo.create(input2)
        .await
        .expect("Org create should succeed for same model");

    // Both should exist
    let global_pricing = repo
        .get_by_provider_model(&PricingOwner::Global, "openai", "gpt-4")
        .await
        .expect("Query should succeed")
        .expect("Global pricing should exist");
    assert!(matches!(global_pricing.owner, PricingOwner::Global));

    let org_pricing = repo
        .get_by_provider_model(&PricingOwner::Organization { org_id }, "openai", "gpt-4")
        .await
        .expect("Query should succeed")
        .expect("Org pricing should exist");
    assert!(matches!(
        org_pricing.owner,
        PricingOwner::Organization { .. }
    ));
}

// ============================================================================
// Get By ID Tests
// ============================================================================

pub async fn test_get_by_id_found(repo: &dyn ModelPricingRepo) {
    let input = create_global_pricing("openai", "gpt-4");
    let created = repo.create(input).await.expect("Failed to create pricing");

    let fetched = repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed")
        .expect("Pricing should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.provider, "openai");
    assert_eq!(fetched.model, "gpt-4");
}

pub async fn test_get_by_id_not_found(repo: &dyn ModelPricingRepo) {
    let result = repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

// ============================================================================
// Get By Provider Model Tests
// ============================================================================

pub async fn test_get_by_provider_model_global(repo: &dyn ModelPricingRepo) {
    let input = create_global_pricing("openai", "gpt-4");
    repo.create(input).await.expect("Failed to create pricing");

    let fetched = repo
        .get_by_provider_model(&PricingOwner::Global, "openai", "gpt-4")
        .await
        .expect("Query should succeed")
        .expect("Pricing should exist");

    assert!(matches!(fetched.owner, PricingOwner::Global));
    assert_eq!(fetched.provider, "openai");
}

pub async fn test_get_by_provider_model_org(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_pricing(org_id, "anthropic", "claude-3");
    repo.create(input).await.expect("Failed to create pricing");

    let fetched = repo
        .get_by_provider_model(
            &PricingOwner::Organization { org_id },
            "anthropic",
            "claude-3",
        )
        .await
        .expect("Query should succeed")
        .expect("Pricing should exist");

    assert!(matches!(fetched.owner, PricingOwner::Organization { org_id: id } if id == org_id));
}

pub async fn test_get_by_provider_model_not_found(repo: &dyn ModelPricingRepo) {
    let result = repo
        .get_by_provider_model(&PricingOwner::Global, "nonexistent", "model")
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

// ============================================================================
// Effective Pricing Tests (Hierarchical Lookup)
// ============================================================================

pub async fn test_get_effective_pricing_returns_user_level_first(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    // Create pricing at all levels
    repo.create(create_global_pricing("openai", "gpt-4"))
        .await
        .unwrap();
    repo.create(create_org_pricing(org_id, "openai", "gpt-4"))
        .await
        .unwrap();
    repo.create(create_project_pricing(project_id, "openai", "gpt-4"))
        .await
        .unwrap();
    repo.create(create_user_pricing(user_id, "openai", "gpt-4"))
        .await
        .unwrap();

    let effective = repo
        .get_effective_pricing(
            "openai",
            "gpt-4",
            Some(user_id),
            Some(project_id),
            Some(org_id),
        )
        .await
        .expect("Query should succeed")
        .expect("Should find pricing");

    // Should return user-level (highest priority)
    assert!(matches!(effective.owner, PricingOwner::User { .. }));
    assert_eq!(effective.input_per_1m_tokens, 2000); // User pricing values
}

pub async fn test_get_effective_pricing_returns_project_level_when_no_user(
    repo: &dyn ModelPricingRepo,
) {
    let org_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    // Create pricing at org, project, and global levels only
    repo.create(create_global_pricing("openai", "gpt-4"))
        .await
        .unwrap();
    repo.create(create_org_pricing(org_id, "openai", "gpt-4"))
        .await
        .unwrap();
    repo.create(create_project_pricing(project_id, "openai", "gpt-4"))
        .await
        .unwrap();

    let effective = repo
        .get_effective_pricing(
            "openai",
            "gpt-4",
            Some(user_id),
            Some(project_id),
            Some(org_id),
        )
        .await
        .expect("Query should succeed")
        .expect("Should find pricing");

    // Should return project-level
    assert!(matches!(effective.owner, PricingOwner::Project { .. }));
    assert_eq!(effective.input_per_1m_tokens, 1800); // Project pricing values
}

pub async fn test_get_effective_pricing_returns_org_level_when_no_project(
    repo: &dyn ModelPricingRepo,
) {
    let org_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    // Create pricing at org and global levels only
    repo.create(create_global_pricing("openai", "gpt-4"))
        .await
        .unwrap();
    repo.create(create_org_pricing(org_id, "openai", "gpt-4"))
        .await
        .unwrap();

    let effective = repo
        .get_effective_pricing(
            "openai",
            "gpt-4",
            Some(user_id),
            Some(project_id),
            Some(org_id),
        )
        .await
        .expect("Query should succeed")
        .expect("Should find pricing");

    // Should return org-level
    assert!(matches!(effective.owner, PricingOwner::Organization { .. }));
    assert_eq!(effective.input_per_1m_tokens, 1500); // Org pricing values
}

pub async fn test_get_effective_pricing_returns_global_as_fallback(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    // Create only global pricing
    repo.create(create_global_pricing("openai", "gpt-4"))
        .await
        .unwrap();

    let effective = repo
        .get_effective_pricing(
            "openai",
            "gpt-4",
            Some(user_id),
            Some(project_id),
            Some(org_id),
        )
        .await
        .expect("Query should succeed")
        .expect("Should find pricing");

    // Should return global
    assert!(matches!(effective.owner, PricingOwner::Global));
    assert_eq!(effective.input_per_1m_tokens, 1000); // Global pricing values
}

pub async fn test_get_effective_pricing_returns_none_when_no_pricing(repo: &dyn ModelPricingRepo) {
    let result = repo
        .get_effective_pricing(
            "openai",
            "gpt-4",
            Some(Uuid::new_v4()),
            Some(Uuid::new_v4()),
            Some(Uuid::new_v4()),
        )
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_effective_pricing_with_no_user_id(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();

    repo.create(create_project_pricing(project_id, "openai", "gpt-4"))
        .await
        .unwrap();

    let effective = repo
        .get_effective_pricing("openai", "gpt-4", None, Some(project_id), Some(org_id))
        .await
        .expect("Query should succeed")
        .expect("Should find pricing");

    assert!(matches!(effective.owner, PricingOwner::Project { .. }));
}

// ============================================================================
// List By Org Tests
// ============================================================================

pub async fn test_list_by_org(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();
    let other_org_id = Uuid::new_v4();

    repo.create(create_org_pricing(org_id, "openai", "gpt-4"))
        .await
        .unwrap();
    repo.create(create_org_pricing(org_id, "anthropic", "claude-3"))
        .await
        .unwrap();
    repo.create(create_org_pricing(other_org_id, "google", "gemini"))
        .await
        .unwrap();

    let result = repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Query should succeed");

    assert_eq!(result.items.len(), 2);
    // Sorted by created_at DESC (most recent first)
}

pub async fn test_list_by_org_empty(repo: &dyn ModelPricingRepo) {
    let result = repo
        .list_by_org(Uuid::new_v4(), ListParams::default())
        .await
        .expect("Query should succeed");

    assert!(result.items.is_empty());
}

// ============================================================================
// List By Project Tests
// ============================================================================

pub async fn test_list_by_project(repo: &dyn ModelPricingRepo) {
    let project_id = Uuid::new_v4();

    repo.create(create_project_pricing(project_id, "openai", "gpt-4"))
        .await
        .unwrap();
    repo.create(create_project_pricing(project_id, "openai", "gpt-3.5"))
        .await
        .unwrap();

    let result = repo
        .list_by_project(project_id, ListParams::default())
        .await
        .expect("Query should succeed");

    assert_eq!(result.items.len(), 2);
    // Sorted by created_at DESC (most recent first)
}

pub async fn test_list_by_project_empty(repo: &dyn ModelPricingRepo) {
    let result = repo
        .list_by_project(Uuid::new_v4(), ListParams::default())
        .await
        .expect("Query should succeed");

    assert!(result.items.is_empty());
}

// ============================================================================
// List By User Tests
// ============================================================================

pub async fn test_list_by_user(repo: &dyn ModelPricingRepo) {
    let user_id = Uuid::new_v4();

    repo.create(create_user_pricing(user_id, "openai", "gpt-4"))
        .await
        .unwrap();

    let result = repo
        .list_by_user(user_id, ListParams::default())
        .await
        .expect("Query should succeed");

    assert_eq!(result.items.len(), 1);
    assert!(matches!(result.items[0].owner, PricingOwner::User { .. }));
}

pub async fn test_list_by_user_empty(repo: &dyn ModelPricingRepo) {
    let result = repo
        .list_by_user(Uuid::new_v4(), ListParams::default())
        .await
        .expect("Query should succeed");

    assert!(result.items.is_empty());
}

// ============================================================================
// List Global Tests
// ============================================================================

pub async fn test_list_global(repo: &dyn ModelPricingRepo) {
    repo.create(create_global_pricing("openai", "gpt-4"))
        .await
        .unwrap();
    repo.create(create_global_pricing("anthropic", "claude-3"))
        .await
        .unwrap();
    repo.create(create_global_pricing("google", "gemini"))
        .await
        .unwrap();

    let result = repo
        .list_global(ListParams::default())
        .await
        .expect("Query should succeed");

    assert_eq!(result.items.len(), 3);
    // Sorted by created_at DESC (most recent first)
}

pub async fn test_list_global_empty(repo: &dyn ModelPricingRepo) {
    let result = repo
        .list_global(ListParams::default())
        .await
        .expect("Query should succeed");

    assert!(result.items.is_empty());
}

// ============================================================================
// List By Provider Tests
// ============================================================================

pub async fn test_list_by_provider(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();

    // Create pricing for same provider at different levels
    repo.create(create_global_pricing("openai", "gpt-4"))
        .await
        .unwrap();
    repo.create(create_global_pricing("openai", "gpt-3.5"))
        .await
        .unwrap();
    repo.create(create_org_pricing(org_id, "openai", "gpt-4-turbo"))
        .await
        .unwrap();
    repo.create(create_global_pricing("anthropic", "claude"))
        .await
        .unwrap();

    let result = repo
        .list_by_provider("openai", ListParams::default())
        .await
        .expect("Query should succeed");

    assert_eq!(result.items.len(), 3);
    // All should be openai
    assert!(result.items.iter().all(|p| p.provider == "openai"));
}

pub async fn test_list_by_provider_empty(repo: &dyn ModelPricingRepo) {
    let result = repo
        .list_by_provider("nonexistent", ListParams::default())
        .await
        .expect("Query should succeed");

    assert!(result.items.is_empty());
}

// ============================================================================
// Update Tests
// ============================================================================

pub async fn test_update_pricing(repo: &dyn ModelPricingRepo) {
    let input = create_global_pricing("openai", "gpt-4");
    let created = repo.create(input).await.expect("Failed to create pricing");

    let update = UpdateModelPricing {
        input_per_1m_tokens: Some(5000),
        output_per_1m_tokens: Some(10000),
        per_image: Some(500),
        per_request: None,
        cached_input_per_1m_tokens: Some(2500),
        cache_write_per_1m_tokens: None,
        reasoning_per_1m_tokens: Some(15000),
        per_second: None,
        per_1m_characters: None,
        source: Some(PricingSource::ProviderApi),
    };

    let updated = repo
        .update(created.id, update)
        .await
        .expect("Update should succeed");

    assert_eq!(updated.input_per_1m_tokens, 5000);
    assert_eq!(updated.output_per_1m_tokens, 10000);
    assert_eq!(updated.per_image, Some(500));
    assert_eq!(updated.cached_input_per_1m_tokens, Some(2500));
    assert_eq!(updated.reasoning_per_1m_tokens, Some(15000));
    assert_eq!(updated.source, PricingSource::ProviderApi);
    // Owner, provider, model should remain unchanged
    assert!(matches!(updated.owner, PricingOwner::Global));
    assert_eq!(updated.provider, "openai");
    assert_eq!(updated.model, "gpt-4");
}

pub async fn test_update_partial_fields(repo: &dyn ModelPricingRepo) {
    let input = create_global_pricing("openai", "gpt-4");
    let created = repo.create(input).await.expect("Failed to create pricing");

    // Only update input price
    let update = UpdateModelPricing {
        input_per_1m_tokens: Some(9999),
        output_per_1m_tokens: None,
        per_image: None,
        per_request: None,
        cached_input_per_1m_tokens: None,
        cache_write_per_1m_tokens: None,
        reasoning_per_1m_tokens: None,
        per_second: None,
        per_1m_characters: None,
        source: None,
    };

    let updated = repo
        .update(created.id, update)
        .await
        .expect("Update should succeed");

    assert_eq!(updated.input_per_1m_tokens, 9999);
    // Other fields should remain unchanged
    assert_eq!(updated.output_per_1m_tokens, 2000); // Original value
    assert_eq!(updated.source, PricingSource::Manual); // Original value
}

pub async fn test_update_not_found(repo: &dyn ModelPricingRepo) {
    let update = UpdateModelPricing {
        input_per_1m_tokens: Some(5000),
        output_per_1m_tokens: None,
        per_image: None,
        per_request: None,
        cached_input_per_1m_tokens: None,
        cache_write_per_1m_tokens: None,
        reasoning_per_1m_tokens: None,
        per_second: None,
        per_1m_characters: None,
        source: None,
    };

    let result = repo.update(Uuid::new_v4(), update).await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

// ============================================================================
// Delete Tests
// ============================================================================

pub async fn test_delete_pricing(repo: &dyn ModelPricingRepo) {
    let input = create_global_pricing("openai", "gpt-4");
    let created = repo.create(input).await.expect("Failed to create pricing");

    repo.delete(created.id)
        .await
        .expect("Delete should succeed");

    let result = repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed");
    assert!(result.is_none());
}

pub async fn test_delete_nonexistent_succeeds(repo: &dyn ModelPricingRepo) {
    // Delete of nonexistent ID should not fail
    let result = repo.delete(Uuid::new_v4()).await;
    assert!(result.is_ok());
}

// ============================================================================
// Upsert Tests
// ============================================================================

pub async fn test_upsert_creates_when_not_exists(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();
    let input = create_org_pricing(org_id, "openai", "gpt-4");
    let result = repo.upsert(input).await.expect("Upsert should succeed");

    assert_eq!(result.provider, "openai");
    assert_eq!(result.model, "gpt-4");
    assert_eq!(result.input_per_1m_tokens, 1500);
}

pub async fn test_upsert_updates_when_exists(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();

    // First create
    let input1 = create_org_pricing(org_id, "openai", "gpt-4");
    let created = repo
        .upsert(input1)
        .await
        .expect("First upsert should succeed");

    // Second upsert with different values
    let mut input2 = create_org_pricing(org_id, "openai", "gpt-4");
    input2.input_per_1m_tokens = 9999;
    input2.output_per_1m_tokens = 8888;

    let updated = repo
        .upsert(input2)
        .await
        .expect("Second upsert should succeed");

    // Should be same ID (updated, not created)
    assert_eq!(updated.id, created.id);
    // Values should be updated
    assert_eq!(updated.input_per_1m_tokens, 9999);
    assert_eq!(updated.output_per_1m_tokens, 8888);
}

pub async fn test_upsert_with_different_owner_creates_new(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();

    // Create org-level
    let org_pricing = repo
        .upsert(create_org_pricing(org_id, "openai", "gpt-4"))
        .await
        .expect("Upsert should succeed");

    // Upsert project-level (different owner)
    let project_pricing = repo
        .upsert(create_project_pricing(project_id, "openai", "gpt-4"))
        .await
        .expect("Upsert should succeed");

    // Should be different IDs
    assert_ne!(org_pricing.id, project_pricing.id);
    assert!(matches!(
        org_pricing.owner,
        PricingOwner::Organization { .. }
    ));
    assert!(matches!(
        project_pricing.owner,
        PricingOwner::Project { .. }
    ));
}

// ============================================================================
// Bulk Upsert Tests
// ============================================================================

pub async fn test_bulk_upsert_creates_all(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();

    let entries = vec![
        create_org_pricing(org_id, "openai", "gpt-4"),
        create_org_pricing(org_id, "anthropic", "claude-3"),
        create_org_pricing(org_id, "google", "gemini"),
    ];

    let count = repo
        .bulk_upsert(entries)
        .await
        .expect("Bulk upsert should succeed");

    assert_eq!(count, 3);

    let result = repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("List should succeed");
    assert_eq!(result.items.len(), 3);
}

pub async fn test_bulk_upsert_updates_existing(repo: &dyn ModelPricingRepo) {
    let org_id = Uuid::new_v4();

    // Create initial entry
    repo.create(create_org_pricing(org_id, "openai", "gpt-4"))
        .await
        .unwrap();

    // Bulk upsert with mix of new and existing
    let mut updated_entry = create_org_pricing(org_id, "openai", "gpt-4");
    updated_entry.input_per_1m_tokens = 9999;

    let entries = vec![
        updated_entry,
        create_org_pricing(org_id, "anthropic", "claude-3"),
    ];

    let count = repo
        .bulk_upsert(entries)
        .await
        .expect("Bulk upsert should succeed");

    assert_eq!(count, 2);

    // Verify update happened
    let openai = repo
        .get_by_provider_model(&PricingOwner::Organization { org_id }, "openai", "gpt-4")
        .await
        .expect("Query should succeed")
        .expect("Should exist");
    assert_eq!(openai.input_per_1m_tokens, 9999);
}

pub async fn test_bulk_upsert_empty(repo: &dyn ModelPricingRepo) {
    let count = repo
        .bulk_upsert(vec![])
        .await
        .expect("Bulk upsert should succeed");

    assert_eq!(count, 0);
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use crate::db::{
        sqlite::SqliteModelPricingRepo,
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repo() -> SqliteModelPricingRepo {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        SqliteModelPricingRepo::new(pool)
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
    sqlite_test!(test_create_global_pricing);
    sqlite_test!(test_create_org_pricing);
    sqlite_test!(test_create_project_pricing);
    sqlite_test!(test_create_user_pricing);
    sqlite_test!(test_create_duplicate_pricing_fails);
    sqlite_test!(test_same_model_different_owners_allowed);

    // Get by ID tests
    sqlite_test!(test_get_by_id_found);
    sqlite_test!(test_get_by_id_not_found);

    // Get by provider model tests
    sqlite_test!(test_get_by_provider_model_global);
    sqlite_test!(test_get_by_provider_model_org);
    sqlite_test!(test_get_by_provider_model_not_found);

    // Effective pricing tests
    sqlite_test!(test_get_effective_pricing_returns_user_level_first);
    sqlite_test!(test_get_effective_pricing_returns_project_level_when_no_user);
    sqlite_test!(test_get_effective_pricing_returns_org_level_when_no_project);
    sqlite_test!(test_get_effective_pricing_returns_global_as_fallback);
    sqlite_test!(test_get_effective_pricing_returns_none_when_no_pricing);
    sqlite_test!(test_get_effective_pricing_with_no_user_id);

    // List by org tests
    sqlite_test!(test_list_by_org);
    sqlite_test!(test_list_by_org_empty);

    // List by project tests
    sqlite_test!(test_list_by_project);
    sqlite_test!(test_list_by_project_empty);

    // List by user tests
    sqlite_test!(test_list_by_user);
    sqlite_test!(test_list_by_user_empty);

    // List global tests
    sqlite_test!(test_list_global);
    sqlite_test!(test_list_global_empty);

    // List by provider tests
    sqlite_test!(test_list_by_provider);
    sqlite_test!(test_list_by_provider_empty);

    // Update tests
    sqlite_test!(test_update_pricing);
    sqlite_test!(test_update_partial_fields);
    sqlite_test!(test_update_not_found);

    // Delete tests
    sqlite_test!(test_delete_pricing);
    sqlite_test!(test_delete_nonexistent_succeeds);

    // Upsert tests
    sqlite_test!(test_upsert_creates_when_not_exists);
    sqlite_test!(test_upsert_updates_when_exists);
    sqlite_test!(test_upsert_with_different_owner_creates_new);

    // Bulk upsert tests
    sqlite_test!(test_bulk_upsert_creates_all);
    sqlite_test!(test_bulk_upsert_updates_existing);
    sqlite_test!(test_bulk_upsert_empty);
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use crate::db::{
        postgres::PostgresModelPricingRepo,
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let pool = create_isolated_postgres_pool().await;
                run_postgres_migrations(&pool).await;
                let repo = PostgresModelPricingRepo::new(pool, None);
                super::$name(&repo).await;
            }
        };
    }

    // Create tests
    postgres_test!(test_create_global_pricing);
    postgres_test!(test_create_org_pricing);
    postgres_test!(test_create_project_pricing);
    postgres_test!(test_create_user_pricing);
    postgres_test!(test_create_duplicate_pricing_fails);
    postgres_test!(test_same_model_different_owners_allowed);

    // Get by ID tests
    postgres_test!(test_get_by_id_found);
    postgres_test!(test_get_by_id_not_found);

    // Get by provider model tests
    postgres_test!(test_get_by_provider_model_global);
    postgres_test!(test_get_by_provider_model_org);
    postgres_test!(test_get_by_provider_model_not_found);

    // Effective pricing tests
    postgres_test!(test_get_effective_pricing_returns_user_level_first);
    postgres_test!(test_get_effective_pricing_returns_project_level_when_no_user);
    postgres_test!(test_get_effective_pricing_returns_org_level_when_no_project);
    postgres_test!(test_get_effective_pricing_returns_global_as_fallback);
    postgres_test!(test_get_effective_pricing_returns_none_when_no_pricing);
    postgres_test!(test_get_effective_pricing_with_no_user_id);

    // List by org tests
    postgres_test!(test_list_by_org);
    postgres_test!(test_list_by_org_empty);

    // List by project tests
    postgres_test!(test_list_by_project);
    postgres_test!(test_list_by_project_empty);

    // List by user tests
    postgres_test!(test_list_by_user);
    postgres_test!(test_list_by_user_empty);

    // List global tests
    postgres_test!(test_list_global);
    postgres_test!(test_list_global_empty);

    // List by provider tests
    postgres_test!(test_list_by_provider);
    postgres_test!(test_list_by_provider_empty);

    // Update tests
    postgres_test!(test_update_pricing);
    postgres_test!(test_update_partial_fields);
    postgres_test!(test_update_not_found);

    // Delete tests
    postgres_test!(test_delete_pricing);
    postgres_test!(test_delete_nonexistent_succeeds);

    // Upsert tests
    postgres_test!(test_upsert_creates_when_not_exists);
    postgres_test!(test_upsert_updates_when_exists);
    postgres_test!(test_upsert_with_different_owner_creates_new);

    // Bulk upsert tests
    postgres_test!(test_bulk_upsert_creates_all);
    postgres_test!(test_bulk_upsert_updates_existing);
    postgres_test!(test_bulk_upsert_empty);
}
