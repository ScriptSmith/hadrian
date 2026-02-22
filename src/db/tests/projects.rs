//! Shared tests for ProjectRepo implementations
//!
//! Tests are written as async functions that take a test context containing
//! both the project repo and utilities for creating test organizations.

use uuid::Uuid;

use crate::{
    db::{
        error::DbError,
        repos::{ListParams, OrganizationRepo, ProjectRepo},
    },
    models::{CreateOrganization, CreateProject, UpdateProject},
};

// ============================================================================
// Test Input Helpers
// ============================================================================

fn create_project_input(slug: &str, name: &str) -> CreateProject {
    CreateProject {
        slug: slug.to_string(),
        name: name.to_string(),
        team_id: None,
    }
}

fn create_org_input(slug: &str, name: &str) -> CreateOrganization {
    CreateOrganization {
        slug: slug.to_string(),
        name: name.to_string(),
    }
}

/// Test context containing repos needed for project tests
pub struct ProjectTestContext<'a> {
    pub project_repo: &'a dyn ProjectRepo,
    pub org_repo: &'a dyn OrganizationRepo,
}

impl<'a> ProjectTestContext<'a> {
    /// Create a test organization and return its ID
    pub async fn create_test_org(&self, slug: &str) -> Uuid {
        self.org_repo
            .create(create_org_input(slug, &format!("Org {}", slug)))
            .await
            .expect("Failed to create test org")
            .id
    }
}

// ============================================================================
// Shared Test Functions
// ============================================================================

pub async fn test_create_project(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_project_input("test-project", "Test Project");
    let project = ctx
        .project_repo
        .create(org_id, input)
        .await
        .expect("Failed to create project");

    assert_eq!(project.slug, "test-project");
    assert_eq!(project.name, "Test Project");
    assert_eq!(project.org_id, org_id);
    assert!(!project.id.is_nil());
}

pub async fn test_create_duplicate_slug_same_org_fails(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_project_input("duplicate", "First Project");
    ctx.project_repo
        .create(org_id, input)
        .await
        .expect("Failed to create first project");

    let input2 = create_project_input("duplicate", "Second Project");
    let result = ctx.project_repo.create(org_id, input2).await;

    assert!(matches!(result, Err(DbError::Conflict(_))));
}

pub async fn test_create_same_slug_different_orgs_succeeds(ctx: &ProjectTestContext<'_>) {
    let org1_id = ctx.create_test_org("org-1").await;
    let org2_id = ctx.create_test_org("org-2").await;

    let input1 = create_project_input("same-slug", "Project in Org 1");
    let project1 = ctx
        .project_repo
        .create(org1_id, input1)
        .await
        .expect("Failed to create project in org 1");

    let input2 = create_project_input("same-slug", "Project in Org 2");
    let project2 = ctx
        .project_repo
        .create(org2_id, input2)
        .await
        .expect("Failed to create project in org 2");

    assert_eq!(project1.slug, project2.slug);
    assert_ne!(project1.id, project2.id);
    assert_ne!(project1.org_id, project2.org_id);
}

pub async fn test_get_by_id(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_project_input("get-test", "Get Test Project");
    let created = ctx
        .project_repo
        .create(org_id, input)
        .await
        .expect("Failed to create project");

    let fetched = ctx
        .project_repo
        .get_by_id(created.id)
        .await
        .expect("Failed to get project")
        .expect("Project should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.org_id, org_id);
    assert_eq!(fetched.slug, "get-test");
    assert_eq!(fetched.name, "Get Test Project");
}

pub async fn test_get_by_id_not_found(ctx: &ProjectTestContext<'_>) {
    let result = ctx
        .project_repo
        .get_by_id(Uuid::new_v4())
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_slug(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let input = create_project_input("slug-test", "Slug Test Project");
    let created = ctx
        .project_repo
        .create(org_id, input)
        .await
        .expect("Failed to create project");

    let fetched = ctx
        .project_repo
        .get_by_slug(org_id, "slug-test")
        .await
        .expect("Failed to get project")
        .expect("Project should exist");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.slug, "slug-test");
}

pub async fn test_get_by_slug_wrong_org(ctx: &ProjectTestContext<'_>) {
    let org1_id = ctx.create_test_org("org-1").await;
    let org2_id = ctx.create_test_org("org-2").await;

    let input = create_project_input("project-slug", "Test Project");
    ctx.project_repo
        .create(org1_id, input)
        .await
        .expect("Failed to create project");

    // Try to get by slug with wrong org_id
    let result = ctx
        .project_repo
        .get_by_slug(org2_id, "project-slug")
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_get_by_slug_not_found(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let result = ctx
        .project_repo
        .get_by_slug(org_id, "nonexistent")
        .await
        .expect("Query should succeed");

    assert!(result.is_none());
}

pub async fn test_list_by_org_empty(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let result = ctx
        .project_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list projects");

    assert!(result.items.is_empty());
    assert!(!result.has_more);
}

pub async fn test_list_by_org_with_projects(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    ctx.project_repo
        .create(org_id, create_project_input("proj-1", "Project 1"))
        .await
        .expect("Failed to create project 1");
    ctx.project_repo
        .create(org_id, create_project_input("proj-2", "Project 2"))
        .await
        .expect("Failed to create project 2");
    ctx.project_repo
        .create(org_id, create_project_input("proj-3", "Project 3"))
        .await
        .expect("Failed to create project 3");

    let result = ctx
        .project_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list projects");

    assert_eq!(result.items.len(), 3);
    assert!(!result.has_more);
}

pub async fn test_list_by_org_filters_by_org(ctx: &ProjectTestContext<'_>) {
    let org1_id = ctx.create_test_org("org-1").await;
    let org2_id = ctx.create_test_org("org-2").await;

    ctx.project_repo
        .create(org1_id, create_project_input("proj-1", "Project 1"))
        .await
        .expect("Failed to create project");
    ctx.project_repo
        .create(org1_id, create_project_input("proj-2", "Project 2"))
        .await
        .expect("Failed to create project");
    ctx.project_repo
        .create(org2_id, create_project_input("proj-3", "Project 3"))
        .await
        .expect("Failed to create project");

    let org1_result = ctx
        .project_repo
        .list_by_org(org1_id, ListParams::default())
        .await
        .expect("Failed to list projects");
    let org2_result = ctx
        .project_repo
        .list_by_org(org2_id, ListParams::default())
        .await
        .expect("Failed to list projects");

    assert_eq!(org1_result.items.len(), 2);
    assert_eq!(org2_result.items.len(), 1);
}

pub async fn test_list_by_org_with_pagination(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..5 {
        ctx.project_repo
            .create(
                org_id,
                create_project_input(&format!("proj-{}", i), &format!("Project {}", i)),
            )
            .await
            .expect("Failed to create project");
    }

    let page1 = ctx
        .project_repo
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
        .project_repo
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
    // Pages should have different projects
    assert_ne!(page1.items[0].id, page2.items[0].id);
}

pub async fn test_count_by_org_empty(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let count = ctx
        .project_repo
        .count_by_org(org_id, false)
        .await
        .expect("Failed to count");
    assert_eq!(count, 0);
}

pub async fn test_count_by_org_with_projects(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    for i in 0..3 {
        ctx.project_repo
            .create(
                org_id,
                create_project_input(&format!("proj-{}", i), &format!("Project {}", i)),
            )
            .await
            .expect("Failed to create project");
    }

    let count = ctx
        .project_repo
        .count_by_org(org_id, false)
        .await
        .expect("Failed to count");
    assert_eq!(count, 3);
}

pub async fn test_update_name(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .project_repo
        .create(org_id, create_project_input("update-test", "Original Name"))
        .await
        .expect("Failed to create project");

    let updated = ctx
        .project_repo
        .update(
            created.id,
            UpdateProject {
                name: Some("Updated Name".to_string()),
                team_id: None,
            },
        )
        .await
        .expect("Failed to update project");

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.slug, "update-test"); // slug unchanged
    assert_eq!(updated.name, "Updated Name");
    assert!(updated.updated_at >= created.updated_at);
}

pub async fn test_update_no_changes(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .project_repo
        .create(org_id, create_project_input("no-change", "Original"))
        .await
        .expect("Failed to create project");

    let result = ctx
        .project_repo
        .update(
            created.id,
            UpdateProject {
                name: None,
                team_id: None,
            },
        )
        .await
        .expect("Failed to update project");

    assert_eq!(result.name, "Original");
}

pub async fn test_update_not_found(ctx: &ProjectTestContext<'_>) {
    let result = ctx
        .project_repo
        .update(
            Uuid::new_v4(),
            UpdateProject {
                name: Some("New Name".to_string()),
                team_id: None,
            },
        )
        .await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_delete(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .project_repo
        .create(org_id, create_project_input("delete-test", "To Delete"))
        .await
        .expect("Failed to create project");

    ctx.project_repo
        .delete(created.id)
        .await
        .expect("Failed to delete project");

    // Should not be found by get_by_id (soft delete)
    let result = ctx
        .project_repo
        .get_by_id(created.id)
        .await
        .expect("Query should succeed");
    assert!(result.is_none());

    // Should not be in list
    let result = ctx
        .project_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list");
    assert!(result.items.is_empty());
}

pub async fn test_delete_not_found(ctx: &ProjectTestContext<'_>) {
    let result = ctx.project_repo.delete(Uuid::new_v4()).await;
    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_delete_already_deleted(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .project_repo
        .create(
            org_id,
            create_project_input("double-delete", "Delete Twice"),
        )
        .await
        .expect("Failed to create project");

    ctx.project_repo
        .delete(created.id)
        .await
        .expect("First delete should succeed");
    let result = ctx.project_repo.delete(created.id).await;

    assert!(matches!(result, Err(DbError::NotFound)));
}

pub async fn test_count_excludes_deleted(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let proj1 = ctx
        .project_repo
        .create(org_id, create_project_input("proj-1", "Project 1"))
        .await
        .expect("Failed to create project 1");
    ctx.project_repo
        .create(org_id, create_project_input("proj-2", "Project 2"))
        .await
        .expect("Failed to create project 2");

    ctx.project_repo
        .delete(proj1.id)
        .await
        .expect("Failed to delete");

    let count = ctx
        .project_repo
        .count_by_org(org_id, false)
        .await
        .expect("Failed to count");
    assert_eq!(count, 1);

    let count_all = ctx
        .project_repo
        .count_by_org(org_id, true)
        .await
        .expect("Failed to count all");
    assert_eq!(count_all, 2);
}

pub async fn test_list_include_deleted(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let proj1 = ctx
        .project_repo
        .create(org_id, create_project_input("proj-1", "Project 1"))
        .await
        .expect("Failed to create project 1");
    ctx.project_repo
        .create(org_id, create_project_input("proj-2", "Project 2"))
        .await
        .expect("Failed to create project 2");

    ctx.project_repo
        .delete(proj1.id)
        .await
        .expect("Failed to delete");

    let active = ctx
        .project_repo
        .list_by_org(org_id, ListParams::default())
        .await
        .expect("Failed to list active");
    assert_eq!(active.items.len(), 1);

    let all = ctx
        .project_repo
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

pub async fn test_get_by_slug_excludes_deleted(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .project_repo
        .create(
            org_id,
            create_project_input("deleted-slug", "Will Be Deleted"),
        )
        .await
        .expect("Failed to create project");

    ctx.project_repo
        .delete(created.id)
        .await
        .expect("Failed to delete");

    let result = ctx
        .project_repo
        .get_by_slug(org_id, "deleted-slug")
        .await
        .expect("Query should succeed");
    assert!(result.is_none());
}

pub async fn test_update_deleted_project_fails(ctx: &ProjectTestContext<'_>) {
    let org_id = ctx.create_test_org("test-org").await;

    let created = ctx
        .project_repo
        .create(
            org_id,
            create_project_input("update-deleted", "Will Be Deleted"),
        )
        .await
        .expect("Failed to create project");

    ctx.project_repo
        .delete(created.id)
        .await
        .expect("Failed to delete");

    let result = ctx
        .project_repo
        .update(
            created.id,
            UpdateProject {
                name: Some("New Name".to_string()),
                team_id: None,
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
        sqlite::{SqliteOrganizationRepo, SqliteProjectRepo},
        tests::harness::{create_sqlite_pool, run_sqlite_migrations},
    };

    async fn create_repos() -> (SqliteProjectRepo, SqliteOrganizationRepo) {
        let pool = create_sqlite_pool().await;
        run_sqlite_migrations(&pool).await;
        (
            SqliteProjectRepo::new(pool.clone()),
            SqliteOrganizationRepo::new(pool),
        )
    }

    macro_rules! sqlite_test {
        ($name:ident) => {
            #[tokio::test]
            async fn $name() {
                let (project_repo, org_repo) = create_repos().await;
                let ctx = ProjectTestContext {
                    project_repo: &project_repo,
                    org_repo: &org_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    sqlite_test!(test_create_project);
    sqlite_test!(test_create_duplicate_slug_same_org_fails);
    sqlite_test!(test_create_same_slug_different_orgs_succeeds);
    sqlite_test!(test_get_by_id);
    sqlite_test!(test_get_by_id_not_found);
    sqlite_test!(test_get_by_slug);
    sqlite_test!(test_get_by_slug_wrong_org);
    sqlite_test!(test_get_by_slug_not_found);
    sqlite_test!(test_list_by_org_empty);
    sqlite_test!(test_list_by_org_with_projects);
    sqlite_test!(test_list_by_org_filters_by_org);
    sqlite_test!(test_list_by_org_with_pagination);
    sqlite_test!(test_count_by_org_empty);
    sqlite_test!(test_count_by_org_with_projects);
    sqlite_test!(test_update_name);
    sqlite_test!(test_update_no_changes);
    sqlite_test!(test_update_not_found);
    sqlite_test!(test_delete);
    sqlite_test!(test_delete_not_found);
    sqlite_test!(test_delete_already_deleted);
    sqlite_test!(test_count_excludes_deleted);
    sqlite_test!(test_list_include_deleted);
    sqlite_test!(test_get_by_slug_excludes_deleted);
    sqlite_test!(test_update_deleted_project_fails);
}

// ============================================================================
// PostgreSQL Tests - Require Docker, run with `cargo test -- --ignored`
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
mod postgres_tests {
    use super::*;
    use crate::db::{
        postgres::{PostgresOrganizationRepo, PostgresProjectRepo},
        tests::harness::postgres::{create_isolated_postgres_pool, run_postgres_migrations},
    };

    macro_rules! postgres_test {
        ($name:ident) => {
            #[tokio::test]
            #[ignore = "Requires Docker - run with `cargo test -- --ignored`"]
            async fn $name() {
                let pool = create_isolated_postgres_pool().await;
                run_postgres_migrations(&pool).await;
                let project_repo = PostgresProjectRepo::new(pool.clone(), None);
                let org_repo = PostgresOrganizationRepo::new(pool, None);
                let ctx = ProjectTestContext {
                    project_repo: &project_repo,
                    org_repo: &org_repo,
                };
                super::$name(&ctx).await;
            }
        };
    }

    postgres_test!(test_create_project);
    postgres_test!(test_create_duplicate_slug_same_org_fails);
    postgres_test!(test_create_same_slug_different_orgs_succeeds);
    postgres_test!(test_get_by_id);
    postgres_test!(test_get_by_id_not_found);
    postgres_test!(test_get_by_slug);
    postgres_test!(test_get_by_slug_wrong_org);
    postgres_test!(test_get_by_slug_not_found);
    postgres_test!(test_list_by_org_empty);
    postgres_test!(test_list_by_org_with_projects);
    postgres_test!(test_list_by_org_filters_by_org);
    postgres_test!(test_list_by_org_with_pagination);
    postgres_test!(test_count_by_org_empty);
    postgres_test!(test_count_by_org_with_projects);
    postgres_test!(test_update_name);
    postgres_test!(test_update_no_changes);
    postgres_test!(test_update_not_found);
    postgres_test!(test_delete);
    postgres_test!(test_delete_not_found);
    postgres_test!(test_delete_already_deleted);
    postgres_test!(test_count_excludes_deleted);
    postgres_test!(test_list_include_deleted);
    postgres_test!(test_get_by_slug_excludes_deleted);
    postgres_test!(test_update_deleted_project_fails);
}
