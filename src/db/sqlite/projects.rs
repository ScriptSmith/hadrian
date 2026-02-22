use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, ProjectRepo,
            cursor_from_row,
        },
    },
    models::{CreateProject, Project, UpdateProject},
};

pub struct SqliteProjectRepo {
    pool: SqlitePool,
}

impl SqliteProjectRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Helper method for cursor-based pagination.
    ///
    /// Uses keyset pagination with (created_at, id) tuple for efficient, consistent results.
    async fn list_with_cursor(
        &self,
        org_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<Project>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let deleted_filter = if params.include_deleted {
            ""
        } else {
            "AND deleted_at IS NULL"
        };

        let query = format!(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE org_id = ? AND (created_at, id) {} (?, ?)
            {}
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, deleted_filter, order, order
        );

        let rows = sqlx::query(&query)
            .bind(org_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<Project> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let team_id: Option<String> = row.get("team_id");
                Ok(Project {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    team_id: team_id.as_deref().map(parse_uuid).transpose()?,
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
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |proj| {
                cursor_from_row(proj.created_at, proj.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[async_trait]
impl ProjectRepo for SqliteProjectRepo {
    async fn create(&self, org_id: Uuid, input: CreateProject) -> DbResult<Project> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO projects (id, org_id, team_id, slug, name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(input.team_id.map(|id| id.to_string()))
        .bind(&input.slug)
        .bind(&input.name)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Project with slug '{}' already exists in this organization",
                    input.slug
                ))
            }
            _ => DbError::from(e),
        })?;

        Ok(Project {
            id,
            org_id,
            team_id: input.team_id,
            slug: input.slug,
            name: input.name,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<Project>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let team_id: Option<String> = row.get("team_id");
                Ok(Some(Project {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    team_id: team_id.as_deref().map(parse_uuid).transpose()?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_by_id_and_org(&self, id: Uuid, org_id: Uuid) -> DbResult<Option<Project>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE id = ? AND org_id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let team_id: Option<String> = row.get("team_id");
                Ok(Some(Project {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    team_id: team_id.as_deref().map(parse_uuid).transpose()?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<Project>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE org_id = ? AND slug = ? AND deleted_at IS NULL
            "#,
        )
        .bind(org_id.to_string())
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => {
                let team_id: Option<String> = row.get("team_id");
                Ok(Some(Project {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    team_id: team_id.as_deref().map(parse_uuid).transpose()?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_by_org(&self, org_id: Uuid, params: ListParams) -> DbResult<ListResult<Project>> {
        let limit = params.limit.unwrap_or(100);
        // Fetch one extra to determine if there are more items
        let fetch_limit = limit + 1;

        // Use cursor-based pagination
        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(org_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        // First page (no cursor provided)
        let query = if params.include_deleted {
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE org_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        } else {
            r#"
            SELECT id, org_id, team_id, slug, name, created_at, updated_at
            FROM projects
            WHERE org_id = ? AND deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#
        };

        let rows = sqlx::query(query)
            .bind(org_id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<Project> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                let team_id: Option<String> = row.get("team_id");
                Ok(Project {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    team_id: team_id.as_deref().map(parse_uuid).transpose()?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        // Generate cursors for pagination
        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |proj| {
                cursor_from_row(proj.created_at, proj.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_org(&self, org_id: Uuid, include_deleted: bool) -> DbResult<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) as count FROM projects WHERE org_id = ?"
        } else {
            "SELECT COUNT(*) as count FROM projects WHERE org_id = ? AND deleted_at IS NULL"
        };

        let row = sqlx::query(query)
            .bind(org_id.to_string())
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateProject) -> DbResult<Project> {
        let has_name_update = input.name.is_some();
        let has_team_update = input.team_id.is_some();

        if !has_name_update && !has_team_update {
            return self.get_by_id(id).await?.ok_or(DbError::NotFound);
        }

        let now = chrono::Utc::now();

        // Build dynamic update query
        let mut set_clauses = vec!["updated_at = ?"];
        if has_name_update {
            set_clauses.push("name = ?");
        }
        if has_team_update {
            set_clauses.push("team_id = ?");
        }

        let query = format!(
            "UPDATE projects SET {} WHERE id = ? AND deleted_at IS NULL",
            set_clauses.join(", ")
        );

        let mut query_builder = sqlx::query(&query).bind(now);

        if let Some(ref name) = input.name {
            query_builder = query_builder.bind(name);
        }
        if let Some(ref team_id_opt) = input.team_id {
            query_builder = query_builder.bind(team_id_opt.map(|id| id.to_string()));
        }

        let result = query_builder
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE projects
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
    use crate::db::repos::ProjectRepo;

    /// Create an in-memory SQLite database with the projects and organizations tables
    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create the organizations table (required for foreign key)
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

        // Create the projects table
        sqlx::query(
            r#"
            CREATE TABLE projects (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL,
                team_id TEXT,
                slug TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(org_id, slug)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create projects table");

        pool
    }

    /// Create a test organization and return its ID
    async fn create_test_org(pool: &SqlitePool, slug: &str) -> Uuid {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            r#"
            INSERT INTO organizations (id, slug, name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(slug)
        .bind(format!("Org {}", slug))
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("Failed to create test org");
        id
    }

    fn create_project_input(slug: &str, name: &str) -> CreateProject {
        CreateProject {
            slug: slug.to_string(),
            name: name.to_string(),
            team_id: None,
        }
    }

    #[tokio::test]
    async fn test_create_project() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let input = create_project_input("test-project", "Test Project");
        let project = repo
            .create(org_id, input)
            .await
            .expect("Failed to create project");

        assert_eq!(project.slug, "test-project");
        assert_eq!(project.name, "Test Project");
        assert_eq!(project.org_id, org_id);
        assert!(!project.id.is_nil());
    }

    #[tokio::test]
    async fn test_create_duplicate_slug_same_org_fails() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let input = create_project_input("duplicate", "First Project");
        repo.create(org_id, input)
            .await
            .expect("Failed to create first project");

        let input2 = create_project_input("duplicate", "Second Project");
        let result = repo.create(org_id, input2).await;

        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_create_same_slug_different_orgs_succeeds() {
        let pool = create_test_pool().await;
        let org1_id = create_test_org(&pool, "org-1").await;
        let org2_id = create_test_org(&pool, "org-2").await;
        let repo = SqliteProjectRepo::new(pool);

        let input1 = create_project_input("same-slug", "Project in Org 1");
        let project1 = repo
            .create(org1_id, input1)
            .await
            .expect("Failed to create project in org 1");

        let input2 = create_project_input("same-slug", "Project in Org 2");
        let project2 = repo
            .create(org2_id, input2)
            .await
            .expect("Failed to create project in org 2");

        assert_eq!(project1.slug, project2.slug);
        assert_ne!(project1.id, project2.id);
        assert_ne!(project1.org_id, project2.org_id);
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let input = create_project_input("get-test", "Get Test Project");
        let created = repo
            .create(org_id, input)
            .await
            .expect("Failed to create project");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get project")
            .expect("Project should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.org_id, org_id);
        assert_eq!(fetched.slug, "get-test");
        assert_eq!(fetched.name, "Get Test Project");
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteProjectRepo::new(pool);

        let result = repo
            .get_by_id(Uuid::new_v4())
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_slug() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let input = create_project_input("slug-test", "Slug Test Project");
        let created = repo
            .create(org_id, input)
            .await
            .expect("Failed to create project");

        let fetched = repo
            .get_by_slug(org_id, "slug-test")
            .await
            .expect("Failed to get project")
            .expect("Project should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.slug, "slug-test");
    }

    #[tokio::test]
    async fn test_get_by_slug_wrong_org() {
        let pool = create_test_pool().await;
        let org1_id = create_test_org(&pool, "org-1").await;
        let org2_id = create_test_org(&pool, "org-2").await;
        let repo = SqliteProjectRepo::new(pool);

        let input = create_project_input("project-slug", "Test Project");
        repo.create(org1_id, input)
            .await
            .expect("Failed to create project");

        // Try to get by slug with wrong org_id
        let result = repo
            .get_by_slug(org2_id, "project-slug")
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_by_slug_not_found() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let result = repo
            .get_by_slug(org_id, "nonexistent")
            .await
            .expect("Query should succeed");

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_by_org_empty() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list projects");

        assert!(result.items.is_empty());
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_by_org_with_projects() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        repo.create(org_id, create_project_input("proj-1", "Project 1"))
            .await
            .expect("Failed to create project 1");
        repo.create(org_id, create_project_input("proj-2", "Project 2"))
            .await
            .expect("Failed to create project 2");
        repo.create(org_id, create_project_input("proj-3", "Project 3"))
            .await
            .expect("Failed to create project 3");

        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list projects");

        assert_eq!(result.items.len(), 3);
        assert!(!result.has_more);
    }

    #[tokio::test]
    async fn test_list_by_org_filters_by_org() {
        let pool = create_test_pool().await;
        let org1_id = create_test_org(&pool, "org-1").await;
        let org2_id = create_test_org(&pool, "org-2").await;
        let repo = SqliteProjectRepo::new(pool);

        repo.create(org1_id, create_project_input("proj-1", "Org1 Project"))
            .await
            .expect("Failed to create project");
        repo.create(org2_id, create_project_input("proj-2", "Org2 Project"))
            .await
            .expect("Failed to create project");

        let org1_result = repo
            .list_by_org(org1_id, ListParams::default())
            .await
            .expect("Failed to list");
        let org2_result = repo
            .list_by_org(org2_id, ListParams::default())
            .await
            .expect("Failed to list");

        assert_eq!(org1_result.items.len(), 1);
        assert_eq!(org1_result.items[0].name, "Org1 Project");
        assert_eq!(org2_result.items.len(), 1);
        assert_eq!(org2_result.items[0].name, "Org2 Project");
    }

    #[tokio::test]
    async fn test_list_by_org_with_pagination() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        for i in 0..5 {
            repo.create(
                org_id,
                create_project_input(&format!("proj-{}", i), &format!("Project {}", i)),
            )
            .await
            .expect("Failed to create project");
        }

        // First page (no cursor)
        let page1 = repo
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

        // Second page (using cursor from first page)
        let page2 = repo
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

    #[tokio::test]
    async fn test_count_by_org_empty() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let count = repo
            .count_by_org(org_id, false)
            .await
            .expect("Failed to count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_count_by_org_with_projects() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        for i in 0..3 {
            repo.create(
                org_id,
                create_project_input(&format!("proj-{}", i), &format!("Project {}", i)),
            )
            .await
            .expect("Failed to create project");
        }

        let count = repo
            .count_by_org(org_id, false)
            .await
            .expect("Failed to count");
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_count_by_org_filters_by_org() {
        let pool = create_test_pool().await;
        let org1_id = create_test_org(&pool, "org-1").await;
        let org2_id = create_test_org(&pool, "org-2").await;
        let repo = SqliteProjectRepo::new(pool);

        repo.create(org1_id, create_project_input("proj-1", "Project 1"))
            .await
            .expect("Failed to create project");
        repo.create(org1_id, create_project_input("proj-2", "Project 2"))
            .await
            .expect("Failed to create project");
        repo.create(org2_id, create_project_input("proj-3", "Project 3"))
            .await
            .expect("Failed to create project");

        let org1_count = repo
            .count_by_org(org1_id, false)
            .await
            .expect("Failed to count");
        let org2_count = repo
            .count_by_org(org2_id, false)
            .await
            .expect("Failed to count");

        assert_eq!(org1_count, 2);
        assert_eq!(org2_count, 1);
    }

    #[tokio::test]
    async fn test_update_name() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let created = repo
            .create(org_id, create_project_input("update-test", "Original Name"))
            .await
            .expect("Failed to create project");

        let updated = repo
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

    #[tokio::test]
    async fn test_update_no_changes() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let created = repo
            .create(org_id, create_project_input("no-change", "Original"))
            .await
            .expect("Failed to create project");

        let result = repo
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

    #[tokio::test]
    async fn test_update_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteProjectRepo::new(pool);

        let result = repo
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

    #[tokio::test]
    async fn test_delete() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let created = repo
            .create(org_id, create_project_input("delete-test", "To Delete"))
            .await
            .expect("Failed to create project");

        repo.delete(created.id)
            .await
            .expect("Failed to delete project");

        // Should not be found by get_by_id (soft delete)
        let result = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed");
        assert!(result.is_none());

        // Should not be in list
        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list");
        assert!(result.items.is_empty());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteProjectRepo::new(pool);

        let result = repo.delete(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete_already_deleted() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let created = repo
            .create(
                org_id,
                create_project_input("double-delete", "Delete Twice"),
            )
            .await
            .expect("Failed to create project");

        repo.delete(created.id)
            .await
            .expect("First delete should succeed");
        let result = repo.delete(created.id).await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_count_excludes_deleted() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let proj1 = repo
            .create(org_id, create_project_input("proj-1", "Project 1"))
            .await
            .expect("Failed to create project 1");
        repo.create(org_id, create_project_input("proj-2", "Project 2"))
            .await
            .expect("Failed to create project 2");

        // Delete one
        repo.delete(proj1.id).await.expect("Failed to delete");

        let count = repo
            .count_by_org(org_id, false)
            .await
            .expect("Failed to count");
        assert_eq!(count, 1);

        let count_all = repo
            .count_by_org(org_id, true)
            .await
            .expect("Failed to count all");
        assert_eq!(count_all, 2);
    }

    #[tokio::test]
    async fn test_list_include_deleted() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let proj1 = repo
            .create(org_id, create_project_input("proj-1", "Project 1"))
            .await
            .expect("Failed to create project 1");
        repo.create(org_id, create_project_input("proj-2", "Project 2"))
            .await
            .expect("Failed to create project 2");

        repo.delete(proj1.id).await.expect("Failed to delete");

        let active = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list active");
        assert_eq!(active.items.len(), 1);

        let all = repo
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

    #[tokio::test]
    async fn test_get_by_slug_excludes_deleted() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let created = repo
            .create(
                org_id,
                create_project_input("deleted-slug", "Will Be Deleted"),
            )
            .await
            .expect("Failed to create project");

        repo.delete(created.id).await.expect("Failed to delete");

        let result = repo
            .get_by_slug(org_id, "deleted-slug")
            .await
            .expect("Query should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_update_deleted_project_fails() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        let created = repo
            .create(
                org_id,
                create_project_input("update-deleted", "Will Be Deleted"),
            )
            .await
            .expect("Failed to create project");

        repo.delete(created.id).await.expect("Failed to delete");

        let result = repo
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

    // ========================================================================
    // Cursor-based pagination tests
    // ========================================================================

    #[tokio::test]
    async fn test_cursor_pagination_forward() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        // Create 5 projects
        for i in 0..5 {
            repo.create(
                org_id,
                create_project_input(
                    &format!("cursor-proj-{}", i),
                    &format!("Cursor Project {}", i),
                ),
            )
            .await
            .expect("Failed to create project");
        }

        // Get first page
        let page1 = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 1");

        assert_eq!(page1.items.len(), 2);
        assert!(page1.has_more);
        assert!(page1.cursors.next.is_some());
        assert!(page1.cursors.prev.is_none()); // First page has no prev

        // Get second page using cursor
        let page2 = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(2),
                    cursor: page1.cursors.next,
                    direction: CursorDirection::Forward,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 2");

        assert_eq!(page2.items.len(), 2);
        assert!(page2.has_more);
        assert!(page2.cursors.next.is_some());
        assert!(page2.cursors.prev.is_some()); // Middle page has prev

        // Verify pages have different projects
        assert_ne!(page1.items[0].id, page2.items[0].id);
        assert_ne!(page1.items[1].id, page2.items[1].id);

        // Get third/last page
        let page3 = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(2),
                    cursor: page2.cursors.next,
                    direction: CursorDirection::Forward,
                    ..Default::default()
                },
            )
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
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        // Create 5 projects
        for i in 0..5 {
            repo.create(
                org_id,
                create_project_input(&format!("back-proj-{}", i), &format!("Back Project {}", i)),
            )
            .await
            .expect("Failed to create project");
        }

        // Get all projects to find middle cursor
        let all = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(100),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list all");

        assert_eq!(all.items.len(), 5);

        // Get the cursor from the 3rd item (index 2) and go backward
        let middle_cursor = cursor_from_row(all.items[2].created_at, all.items[2].id);

        let backward_page = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(2),
                    cursor: Some(middle_cursor),
                    direction: CursorDirection::Backward,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list backward");

        // Should get items before the cursor (items at index 0 and 1)
        assert_eq!(backward_page.items.len(), 2);
        // Items should be in descending order (newest first)
        assert!(backward_page.items[0].created_at >= backward_page.items[1].created_at);
    }

    #[tokio::test]
    async fn test_cursor_pagination_with_deleted_items() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        // Create 5 projects
        let mut projects = Vec::new();
        for i in 0..5 {
            let proj = repo
                .create(
                    org_id,
                    create_project_input(
                        &format!("del-cursor-proj-{}", i),
                        &format!("Del Cursor Project {}", i),
                    ),
                )
                .await
                .expect("Failed to create project");
            projects.push(proj);
        }

        // Delete project at index 2 (middle)
        repo.delete(projects[2].id)
            .await
            .expect("Failed to delete project");

        // Get all with cursor pagination (should skip deleted)
        let page1 = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(3),
                    include_deleted: false,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list");

        assert_eq!(page1.items.len(), 3);
        assert!(page1.has_more);

        // Ensure deleted project is not in results
        assert!(!page1.items.iter().any(|p| p.id == projects[2].id));

        // Get remaining with cursor
        let page2 = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(3),
                    cursor: page1.cursors.next,
                    direction: CursorDirection::Forward,
                    include_deleted: false,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list page 2");

        assert_eq!(page2.items.len(), 1); // 5 total - 1 deleted - 3 from page1 = 1
        assert!(!page2.has_more);
    }

    #[tokio::test]
    async fn test_offset_pagination_returns_cursors() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteProjectRepo::new(pool);

        // Create 3 projects
        for i in 0..3 {
            repo.create(
                org_id,
                create_project_input(
                    &format!("offset-cursor-proj-{}", i),
                    &format!("Offset Cursor Project {}", i),
                ),
            )
            .await
            .expect("Failed to create project");
        }

        // Use offset-based pagination
        let result = repo
            .list_by_org(
                org_id,
                ListParams {
                    limit: Some(2),
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to list");

        assert_eq!(result.items.len(), 2);
        assert!(result.has_more);
        // Should still have cursors for hybrid navigation
        assert!(result.cursors.next.is_some());
    }
}
