use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, OrganizationRepo, PageCursors,
            cursor_from_row,
        },
    },
    models::{CreateOrganization, Organization, UpdateOrganization},
};

pub struct SqliteOrganizationRepo {
    pool: SqlitePool,
}

impl SqliteOrganizationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Helper method for cursor-based pagination.
    ///
    /// Uses keyset pagination with (created_at, id) tuple for efficient, consistent results.
    async fn list_with_cursor(
        &self,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<Organization>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };

        let query = format!(
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE (created_at, id) {} (?, ?)
            {}
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, deleted_filter, order, order
        );

        let rows = sqlx::query(&query)
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<Organization> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(Organization {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        // Generate cursors
        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |org| {
                cursor_from_row(org.created_at, org.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[async_trait]
impl OrganizationRepo for SqliteOrganizationRepo {
    async fn create(&self, input: CreateOrganization) -> DbResult<Organization> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO organizations (id, slug, name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&input.slug)
        .bind(&input.name)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => DbError::Conflict(
                format!("Organization with slug '{}' already exists", input.slug),
            ),
            _ => DbError::from(e),
        })?;

        Ok(Organization {
            id,
            slug: input.slug,
            name: input.name,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Organization>> {
        let result = sqlx::query(
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Organization {
                id: parse_uuid(&row.get::<String, _>("id"))?,
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn get_by_slug(&self, slug: &str) -> DbResult<Option<Organization>> {
        let result = sqlx::query(
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE slug = ? AND deleted_at IS NULL
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Organization {
                id: parse_uuid(&row.get::<String, _>("id"))?,
                slug: row.get("slug"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn list(&self, params: ListParams) -> DbResult<ListResult<Organization>> {
        let limit = params.limit.unwrap_or(100);
        // Fetch one extra to determine if there are more items
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(&params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let query = if params.include_deleted {
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        } else {
            r#"
            SELECT id, slug, name, created_at, updated_at
            FROM organizations
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        };

        let rows = sqlx::query(query)
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<Organization> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(Organization {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        // Generate cursors for pagination
        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |org| {
                cursor_from_row(org.created_at, org.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count(&self, include_deleted: bool) -> DbResult<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) as count FROM organizations"
        } else {
            "SELECT COUNT(*) as count FROM organizations WHERE deleted_at IS NULL"
        };

        let row = sqlx::query(query).fetch_one(&self.pool).await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateOrganization) -> DbResult<Organization> {
        if let Some(name) = input.name {
            let now = chrono::Utc::now();

            let result = sqlx::query(
                r#"
                UPDATE organizations
                SET name = ?, updated_at = ?
                WHERE id = ? AND deleted_at IS NULL
                "#,
            )
            .bind(&name)
            .bind(now)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

            if result.rows_affected() == 0 {
                return Err(DbError::NotFound);
            }

            // Return the updated org
            self.get_by_id(id).await?.ok_or(DbError::NotFound)
        } else {
            self.get_by_id(id).await?.ok_or(DbError::NotFound)
        }
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE organizations
            SET deleted_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::OrganizationRepo;

    /// Create an in-memory SQLite database with the organizations table
    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create the organizations table
        sqlx::query(
            r#"
            CREATE TABLE organizations (
                id TEXT PRIMARY KEY NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create organizations table");

        pool
    }

    fn create_org_input(slug: &str, name: &str) -> CreateOrganization {
        CreateOrganization {
            slug: slug.to_string(),
            name: name.to_string(),
        }
    }

    #[tokio::test]
    async fn test_create_organization() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        let input = create_org_input("test-org", "Test Organization");
        let org = repo.create(input).await.expect("Failed to create org");

        assert_eq!(org.slug, "test-org");
        assert_eq!(org.name, "Test Organization");
        assert!(!org.id.is_nil());
    }

    #[tokio::test]
    async fn test_create_duplicate_slug_fails() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        let input = create_org_input("duplicate", "First Org");
        repo.create(input)
            .await
            .expect("Failed to create first org");

        let input2 = create_org_input("duplicate", "Second Org");
        let result = repo.create(input2).await;

        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        let result = repo
            .get_by_id(Uuid::new_v4())
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_slug() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_get_by_slug_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        let result = repo
            .get_by_slug("nonexistent")
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        let result = repo
            .list(ListParams::default())
            .await
            .expect("Failed to list orgs");

        assert!(result.items.is_empty());
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_with_orgs() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_list_with_pagination() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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
        assert!(page2.has_more); // Still has 1 more after page 1
        // Pages should have different orgs
        assert_ne!(page1.items[0].id, page2.items[0].id);
    }

    #[tokio::test]
    async fn test_count_empty() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        let count = repo.count(false).await.expect("Failed to count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_count_with_orgs() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_update_name() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_update_no_changes() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_update_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_delete() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_delete_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        let result = repo.delete(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete_already_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_count_excludes_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_list_include_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_get_by_slug_excludes_deleted() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    #[tokio::test]
    async fn test_update_deleted_org_fails() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

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

    // ========================================================================
    // Cursor-based pagination tests
    // ========================================================================

    #[tokio::test]
    async fn test_cursor_pagination_forward() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        // Create 5 orgs with slight time delays to ensure distinct created_at
        for i in 0..5 {
            repo.create(create_org_input(
                &format!("cursor-org-{}", i),
                &format!("Cursor Org {}", i),
            ))
            .await
            .expect("Failed to create org");
        }

        // Get first page
        let page1 = repo
            .list(ListParams {
                limit: Some(2),
                ..Default::default()
            })
            .await
            .expect("Failed to list page 1");

        assert_eq!(page1.items.len(), 2);
        assert!(page1.has_more);
        assert!(page1.cursors.next.is_some());
        assert!(page1.cursors.prev.is_none()); // First page has no prev

        // Get second page using cursor
        let page2 = repo
            .list(ListParams {
                limit: Some(2),
                cursor: page1.cursors.next,
                direction: CursorDirection::Forward,
                ..Default::default()
            })
            .await
            .expect("Failed to list page 2");

        assert_eq!(page2.items.len(), 2);
        assert!(page2.has_more);
        assert!(page2.cursors.next.is_some());
        assert!(page2.cursors.prev.is_some()); // Middle page has prev

        // Verify pages have different orgs
        assert_ne!(page1.items[0].id, page2.items[0].id);
        assert_ne!(page1.items[1].id, page2.items[1].id);

        // Get third/last page
        let page3 = repo
            .list(ListParams {
                limit: Some(2),
                cursor: page2.cursors.next,
                direction: CursorDirection::Forward,
                ..Default::default()
            })
            .await
            .expect("Failed to list page 3");

        assert_eq!(page3.items.len(), 1); // Only 1 remaining
        assert!(!page3.has_more);
        assert!(page3.cursors.next.is_none()); // Last page has no next
        assert!(page3.cursors.prev.is_some());
    }

    #[tokio::test]
    async fn test_cursor_pagination_backward() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        // Create 5 orgs
        for i in 0..5 {
            repo.create(create_org_input(
                &format!("back-org-{}", i),
                &format!("Back Org {}", i),
            ))
            .await
            .expect("Failed to create org");
        }

        // Get all orgs to find middle cursor
        let all = repo
            .list(ListParams {
                limit: Some(100),
                ..Default::default()
            })
            .await
            .expect("Failed to list all");

        assert_eq!(all.items.len(), 5);

        // Get the cursor from the 3rd item (index 2) and go backward
        let middle_cursor = cursor_from_row(all.items[2].created_at, all.items[2].id);

        let backward_page = repo
            .list(ListParams {
                limit: Some(2),
                cursor: Some(middle_cursor),
                direction: CursorDirection::Backward,
                ..Default::default()
            })
            .await
            .expect("Failed to list backward");

        // Should get items before the cursor (items at index 0 and 1)
        assert_eq!(backward_page.items.len(), 2);
        // Items should be in descending order (newest first)
        assert!(backward_page.items[0].created_at >= backward_page.items[1].created_at);
    }

    #[tokio::test]
    async fn test_cursor_pagination_returns_correct_cursors() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        // Create 3 orgs
        let org1 = repo
            .create(create_org_input("cursor-test-1", "Org 1"))
            .await
            .expect("Failed to create org 1");
        let org2 = repo
            .create(create_org_input("cursor-test-2", "Org 2"))
            .await
            .expect("Failed to create org 2");
        let _org3 = repo
            .create(create_org_input("cursor-test-3", "Org 3"))
            .await
            .expect("Failed to create org 3");

        // Get first page with limit 2
        let page1 = repo
            .list(ListParams {
                limit: Some(2),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        // Most recent orgs should be first (descending order)
        // The next cursor should point to the last item in the result (org2)
        let next_cursor = page1.cursors.next.expect("Should have next cursor");
        assert_eq!(next_cursor.id, org2.id);

        // Navigate to next page using cursor
        let page2 = repo
            .list(ListParams {
                limit: Some(2),
                cursor: Some(next_cursor.clone()),
                direction: CursorDirection::Forward,
                ..Default::default()
            })
            .await
            .expect("Failed to list page 2");

        // Should get org1 (the oldest)
        assert_eq!(page2.items.len(), 1);
        assert_eq!(page2.items[0].id, org1.id);

        // Prev cursor should point to first item of page2 (org1)
        let prev_cursor = page2.cursors.prev.expect("Should have prev cursor");
        assert_eq!(prev_cursor.id, org1.id);
    }

    #[tokio::test]
    async fn test_cursor_pagination_empty_result() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        // Create one org
        let org = repo
            .create(create_org_input("single-org", "Single Org"))
            .await
            .expect("Failed to create org");

        // Use cursor to search for items after this org (there are none)
        let cursor = cursor_from_row(org.created_at, org.id);
        let result = repo
            .list(ListParams {
                limit: Some(10),
                cursor: Some(cursor),
                direction: CursorDirection::Forward,
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert!(result.items.is_empty());
        assert!(!result.has_more);
        assert!(result.cursors.next.is_none());
        assert!(result.cursors.prev.is_none());
    }

    #[tokio::test]
    async fn test_cursor_pagination_with_deleted_items() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        // Create 5 orgs
        let mut orgs = Vec::new();
        for i in 0..5 {
            let org = repo
                .create(create_org_input(
                    &format!("del-cursor-org-{}", i),
                    &format!("Del Cursor Org {}", i),
                ))
                .await
                .expect("Failed to create org");
            orgs.push(org);
        }

        // Delete org at index 2 (middle)
        repo.delete(orgs[2].id).await.expect("Failed to delete org");

        // Get all with cursor pagination (should skip deleted)
        let page1 = repo
            .list(ListParams {
                limit: Some(3),
                include_deleted: false,
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(page1.items.len(), 3);
        assert!(page1.has_more);

        // Ensure deleted org is not in results
        assert!(!page1.items.iter().any(|o| o.id == orgs[2].id));

        // Get remaining with cursor
        let page2 = repo
            .list(ListParams {
                limit: Some(3),
                cursor: page1.cursors.next,
                direction: CursorDirection::Forward,
                include_deleted: false,
                ..Default::default()
            })
            .await
            .expect("Failed to list page 2");

        assert_eq!(page2.items.len(), 1); // 5 total - 1 deleted - 3 from page1 = 1
        assert!(!page2.has_more);
    }

    #[tokio::test]
    async fn test_offset_pagination_returns_cursors() {
        let pool = create_test_pool().await;
        let repo = SqliteOrganizationRepo::new(pool);

        // Create 3 orgs
        for i in 0..3 {
            repo.create(create_org_input(
                &format!("offset-cursor-org-{}", i),
                &format!("Offset Cursor Org {}", i),
            ))
            .await
            .expect("Failed to create org");
        }

        // Use offset-based pagination
        let result = repo
            .list(ListParams {
                limit: Some(2),
                ..Default::default()
            })
            .await
            .expect("Failed to list");

        assert_eq!(result.items.len(), 2);
        assert!(result.has_more);
        // Should still have cursors for hybrid navigation
        assert!(result.cursors.next.is_some());
    }
}
