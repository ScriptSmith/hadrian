//! Shared tests for OrganizationRepo implementations
//!
//! Tests are written as async functions that take `&dyn OrganizationRepo`,
//! allowing the same test logic to run against both SQLite and PostgreSQL.

use uuid::Uuid;

use crate::{
    db::{
        error::DbError,
        repos::{ListParams, OrganizationRepo},
    },
    models::{CreateOrganization, UpdateOrganization},
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

// ============================================================================
// Shared Test Functions
// These are called by both SQLite and PostgreSQL test implementations
// ============================================================================

pub async fn test_create_organization(repo: &dyn OrganizationRepo) {
    let input = create_org_input("test-org", "Test Organization");
    let org = repo.create(input).await.expect("Failed to create org");

    assert_eq!(org.slug, "test-org");
    assert_eq!(org.name, "Test Organization");
    assert!(!org.id.is_nil());
}

pub async fn test_create_duplicate_slug_fails(repo: &dyn OrganizationRepo) {
    let input = create_org_input("duplicate", "First Org");
    repo.create(input)
        .await
        .expect("Failed to create first org");

    let input2 = create_org_input("duplicate", "Second Org");
    let result = repo.create(input2).await;

    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_get_by_id(repo: &dyn OrganizationRepo) {
    let input = create_org_input("get-test", "Get Test Org");
    let created = repo.create(input).await.expect("Failed to create org");

    let fetched = repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get org")
        .expect("Org should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.slug, "get-test");
    assert_eq!(fetched.name, "Get Test Org");
}

pub async fn test_get_by_id_not_found(repo: &dyn OrganizationRepo) {
    let result = repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_slug(repo: &dyn OrganizationRepo) {
    let input = create_org_input("slug-test", "Slug Test Org");
    let created = repo.create(input).await.expect("Failed to create org");

    let fetched = repo
        .get_by_slug("slug-test")
        .await
        .expect("Failed to get org")
        .expect("Org should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.slug, "slug-test");
}

pub async fn test_get_by_slug_not_found(repo: &dyn OrganizationRepo) {
    let result = repo
        .get_by_slug("nonexistent")
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_list_empty(repo: &dyn OrganizationRepo) {
    let result = repo
        .list(ListParams::default())
        .await
        .expect("Failed to list orgs");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_with_orgs(repo: &dyn OrganizationRepo) {
    repo.create(create_org_input("org-1", "Org 1"))
        .await
        .expect("Failed to create org 1");
    repo.create(create_org_input("org-2", "Org 2"))
        .await
        .expect("Failed to create org 2");
    repo.create(create_org_input("org-3", "Org 3"))
        .await
        .expect("Failed to create org 3");

    let result = repo
        .list(ListParams::default())
        .await
        .expect("Failed to list orgs");

    assert_eq!(result.items.len(), 3);
    assert!(!result.has_more);
}

pub async fn test_list_with_pagination(repo: &dyn OrganizationRepo) {
    for i in 0..5 {
        repo.create(create_org_input(
            &format!("org-{}", i),
            &format!("Org {}", i),
        ))
        .await
        .expect("Failed to create org");
    }

    // First page (no cursor)
    let page1 = repo
        .list(ListParams {
            limit: Some(2),
            include_deleted: false,
            ..Default::default()
        })
        .await
        .expect("Failed to list page 1");

    // Second page (using cursor from first page)
    let page2 = repo
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
    assert!(page1.has_more); // 5 total, page 1 has 2, more available
    assert!(page2.has_more); // Still has 1 more after offset 2 + limit 2
    // Pages should have different orgs
    assert_ne!(page1.items[0].id, page2.items[0].id);
}

pub async fn test_count_empty(repo: &dyn OrganizationRepo) {
    let count = repo.count(false).await.expect("Failed to count");
    assert_eq!(count, 0);
}

pub async fn test_count_with_orgs(repo: &dyn OrganizationRepo) {
    for i in 0..3 {
        repo.create(create_org_input(
            &format!("org-{}", i),
            &format!("Org {}", i),
        ))
        .await
        .expect("Failed to create org");
    }

    let count = repo.count(false).await.expect("Failed to count");
    assert_eq!(count, 3);
}

pub async fn test_update_name(repo: &dyn OrganizationRepo) {
    let created = repo
        .create(create_org_input("update-test", "Original Name"))
        .await
        .expect("Failed to create org");

    let updated = repo
        .update(
            created.id,
            UpdateOrganization {
                name: Some("Updated Name".to_string()),
            },
        )
        .await
        .expect("Failed to update org");

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.slug, "update-test"); // slug unchanged
    assert_eq!(updated.name, "Updated Name");
    assert!(updated.updated_at >= created.updated_at);
}

pub async fn test_update_no_changes(repo: &dyn OrganizationRepo) {
    let created = repo
        .create(create_org_input("no-change", "Original"))
        .await
        .expect("Failed to create org");

    let result = repo
        .update(created.id, UpdateOrganization { name: None })
        .await
        .expect("Failed to update org");

    assert_eq!(result.name, "Original");
}

pub async fn test_update_not_found(repo: &dyn OrganizationRepo) {
    let result = repo
        .update(
            Uuid::new_v4(),
            UpdateOrganization {
                name: Some("New Name".to_string()),
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_delete(repo: &dyn OrganizationRepo) {
    let created = repo
        .create(create_org_input("delete-test", "To Delete"))
        .await
        .expect("Failed to create org");

    repo.delete(created.id).await.expect("Failed to delete org");

    // Should not be found by get_by_id (soft delete)
    let result = repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed");
    assert!(result.is_none());

    // Should not be in list
    let result = repo
        .list(ListParams::default())
        .await
        .expect("Failed to list");
    assert!(result.items.is_empty());
}

pub async fn test_delete_not_found(repo: &dyn OrganizationRepo) {
    let result = repo.delete(Uuid::new_v4()).await;
    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_delete_already_deleted(repo: &dyn OrganizationRepo) {
    let created = repo
        .create(create_org_input("double-delete", "Delete Twice"))
        .await
        .expect("Failed to create org");

    repo.delete(created.id)
        .await
        .expect("First delete should succeed");
    let result = repo.delete(created.id).await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_count_excludes_deleted(repo: &dyn OrganizationRepo) {
    let org1 = repo
        .create(create_org_input("org-1", "Org 1"))
        .await
        .expect("Failed to create org 1");
    repo.create(create_org_input("org-2", "Org 2"))
        .await
        .expect("Failed to create org 2");

    // Delete one
    repo.delete(org1.id).await.expect("Failed to delete");

    let count = repo.count(false).await.expect("Failed to count");
    assert_eq!(count, 1);

    let count_all = repo.count(true).await.expect("Failed to count all");
    assert_eq!(count_all, 2);
}

pub async fn test_list_include_deleted(repo: &dyn OrganizationRepo) {
    let org1 = repo
        .create(create_org_input("org-1", "Org 1"))
        .await
        .expect("Failed to create org 1");
    repo.create(create_org_input("org-2", "Org 2"))
        .await
        .expect("Failed to create org 2");

    repo.delete(org1.id).await.expect("Failed to delete");

    let active = repo
        .list(ListParams::default())
        .await
        .expect("Failed to list active");
    assert_eq!(active.items.len(), 1);

    let all = repo
        .list(ListParams {
            include_deleted: true,
            ..Default::default()
        })
        .await
        .expect("Failed to list all");
    assert_eq!(all.items.len(), 2);
}

pub async fn test_get_by_slug_excludes_deleted(repo: &dyn OrganizationRepo) {
    let created = repo
        .create(create_org_input("deleted-slug", "Will Be Deleted"))
        .await
        .expect("Failed to create org");

    repo.delete(created.id).await.expect("Failed to delete");

    let result = repo
        .get_by_slug("deleted-slug")
        .await
        .expect("Query should succeed");
    assert!(result.is_none());
}

pub async fn test_update_deleted_org_fails(repo: &dyn OrganizationRepo) {
    let created = repo
        .create(create_org_input("update-deleted", "Will Be Deleted"))
        .await
        .expect("Failed to create org");

    repo.delete(created.id).await.expect("Failed to delete");

    let result = repo
        .update(
            created.id,
            UpdateOrganization {
                name: Some("New Name".to_string()),
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

// ============================================================================
// SQLite Tests - Fast, in-memory
// ============================================================================

#[cfg(all(test, feature = "database-sqlite"))]
mod sqlite_tests {
    use super::*;
    use crate::db::{
        sqlite::SqliteOrganizationRepo,
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repo() -> SqliteOrganizationRepo {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        SqliteOrganizationRepo::new(pool)
    }

    #[tokio::test]
    async fn sqlite_create_organization() {
        let repo = create_repo().await;
        test_create_organization(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_create_duplicate_slug_fails() {
        let repo = create_repo().await;
        test_create_duplicate_slug_fails(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_get_by_id() {
        let repo = create_repo().await;
        test_get_by_id(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_get_by_id_not_found() {
        let repo = create_repo().await;
        test_get_by_id_not_found(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_get_by_slug() {
        let repo = create_repo().await;
        test_get_by_slug(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_get_by_slug_not_found() {
        let repo = create_repo().await;
        test_get_by_slug_not_found(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_list_empty() {
        let repo = create_repo().await;
        test_list_empty(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_list_with_orgs() {
        let repo = create_repo().await;
        test_list_with_orgs(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_list_with_pagination() {
        let repo = create_repo().await;
        test_list_with_pagination(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_count_empty() {
        let repo = create_repo().await;
        test_count_empty(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_count_with_orgs() {
        let repo = create_repo().await;
        test_count_with_orgs(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_update_name() {
        let repo = create_repo().await;
        test_update_name(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_update_no_changes() {
        let repo = create_repo().await;
        test_update_no_changes(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_update_not_found() {
        let repo = create_repo().await;
        test_update_not_found(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_delete() {
        let repo = create_repo().await;
        test_delete(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_delete_not_found() {
        let repo = create_repo().await;
        test_delete_not_found(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_delete_already_deleted() {
        let repo = create_repo().await;
        test_delete_already_deleted(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_count_excludes_deleted() {
        let repo = create_repo().await;
        test_count_excludes_deleted(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_list_include_deleted() {
        let repo = create_repo().await;
        test_list_include_deleted(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_get_by_slug_excludes_deleted() {
        let repo = create_repo().await;
        test_get_by_slug_excludes_deleted(&repo).await;
    }

    #[tokio::test]
    async fn sqlite_update_deleted_org_fails() {
        let repo = create_repo().await;
        test_update_deleted_org_fails(&repo).await;
    }
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use crate::db::{
        postgres::PostgresOrganizationRepo,
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let pool = create_isolated_postgres_pool().await;
                run_postgres_migrations(&pool).await;
                let repo = PostgresOrganizationRepo::new(pool, None);
                super::$name(&repo).await;
            }
        };
    }

    postgres_test!(test_create_organization);
    postgres_test!(test_create_duplicate_slug_fails);
    postgres_test!(test_get_by_id);
    postgres_test!(test_get_by_id_not_found);
    postgres_test!(test_get_by_slug);
    postgres_test!(test_get_by_slug_not_found);
    postgres_test!(test_list_empty);
    postgres_test!(test_list_with_orgs);
    postgres_test!(test_list_with_pagination);
    postgres_test!(test_count_empty);
    postgres_test!(test_count_with_orgs);
    postgres_test!(test_update_name);
    postgres_test!(test_update_no_changes);
    postgres_test!(test_update_not_found);
    postgres_test!(test_delete);
    postgres_test!(test_delete_not_found);
    postgres_test!(test_delete_already_deleted);
    postgres_test!(test_count_excludes_deleted);
    postgres_test!(test_list_include_deleted);
    postgres_test!(test_get_by_slug_excludes_deleted);
    postgres_test!(test_update_deleted_org_fails);
}
