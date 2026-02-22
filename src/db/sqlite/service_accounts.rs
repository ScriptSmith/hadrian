use async_trait::async_trait;
use chrono::SubsecRound;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, ServiceAccountRepo,
            cursor_from_row,
        },
    },
    models::{CreateServiceAccount, ServiceAccount, UpdateServiceAccount},
};

pub struct SqliteServiceAccountRepo {
    pool: SqlitePool,
}

impl SqliteServiceAccountRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Parse roles from JSON string to Vec<String>
    fn parse_roles(roles_json: &str) -> Vec<String> {
        serde_json::from_str(roles_json).unwrap_or_default()
    }

    /// Serialize roles to JSON string
    fn serialize_roles(roles: &[String]) -> String {
        serde_json::to_string(roles).unwrap_or_else(|_| "[]".to_string())
    }

    /// Helper method for cursor-based pagination.
    async fn list_with_cursor(
        &self,
        org_id: Uuid,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<ServiceAccount>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let query = format!(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE org_id = ? AND deleted_at IS NULL AND (created_at, id) {} (?, ?)
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, order, order
        );

        let rows = sqlx::query(&query)
            .bind(org_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string())
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<ServiceAccount> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(ServiceAccount {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    description: row.get("description"),
                    roles: Self::parse_roles(&row.get::<String, _>("roles")),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |sa| {
                cursor_from_row(sa.created_at, sa.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[async_trait]
impl ServiceAccountRepo for SqliteServiceAccountRepo {
    async fn create(&self, org_id: Uuid, input: CreateServiceAccount) -> DbResult<ServiceAccount> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().trunc_subsecs(3);
        let roles_json = Self::serialize_roles(&input.roles);

        sqlx::query(
            r#"
            INSERT INTO service_accounts (id, org_id, slug, name, description, roles, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(&input.slug)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&roles_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Service account with slug '{}' already exists in this organization",
                    input.slug
                ))
            }
            _ => DbError::from(e),
        })?;

        Ok(ServiceAccount {
            id,
            org_id,
            slug: input.slug,
            name: input.name,
            description: input.description,
            roles: input.roles,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ServiceAccount>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(ServiceAccount {
                id: parse_uuid(&row.get::<String, _>("id"))?,
                org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                slug: row.get("slug"),
                name: row.get("name"),
                description: row.get("description"),
                roles: Self::parse_roles(&row.get::<String, _>("roles")),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn get_by_slug(&self, org_id: Uuid, slug: &str) -> DbResult<Option<ServiceAccount>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE org_id = ? AND slug = ? AND deleted_at IS NULL
            "#,
        )
        .bind(org_id.to_string())
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(ServiceAccount {
                id: parse_uuid(&row.get::<String, _>("id"))?,
                org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                slug: row.get("slug"),
                name: row.get("name"),
                description: row.get("description"),
                roles: Self::parse_roles(&row.get::<String, _>("roles")),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<ServiceAccount>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(org_id, &params, cursor, fetch_limit, limit)
                .await;
        }

        let rows = sqlx::query(
            r#"
            SELECT id, org_id, slug, name, description, roles, created_at, updated_at
            FROM service_accounts
            WHERE org_id = ? AND deleted_at IS NULL
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(org_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<ServiceAccount> = rows
            .into_iter()
            .take(limit as usize)
            .map(|row| {
                Ok(ServiceAccount {
                    id: parse_uuid(&row.get::<String, _>("id"))?,
                    org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
                    slug: row.get("slug"),
                    name: row.get("name"),
                    description: row.get("description"),
                    roles: Self::parse_roles(&row.get::<String, _>("roles")),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                })
            })
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |sa| {
                cursor_from_row(sa.created_at, sa.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM service_accounts WHERE org_id = ? AND deleted_at IS NULL",
        )
        .bind(org_id.to_string())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateServiceAccount) -> DbResult<ServiceAccount> {
        // First check if the service account exists
        let existing = self.get_by_id(id).await?.ok_or(DbError::NotFound)?;

        let now = chrono::Utc::now().trunc_subsecs(3);
        let new_name = input.name.unwrap_or(existing.name);
        let new_description = input.description.or(existing.description);
        let new_roles = input.roles.unwrap_or(existing.roles);
        let roles_json = Self::serialize_roles(&new_roles);

        let result = sqlx::query(
            r#"
            UPDATE service_accounts
            SET name = ?, description = ?, roles = ?, updated_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&new_name)
        .bind(&new_description)
        .bind(&roles_json)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        // Record was deleted between get_by_id and UPDATE
        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(ServiceAccount {
            id,
            org_id: existing.org_id,
            slug: existing.slug,
            name: new_name,
            description: new_description,
            roles: new_roles,
            created_at: existing.created_at,
            updated_at: now,
        })
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now().trunc_subsecs(3);

        let result = sqlx::query(
            r#"
            UPDATE service_accounts
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

    async fn delete_with_api_key_revocation(&self, id: Uuid) -> DbResult<Vec<Uuid>> {
        // SQLite IMMEDIATE transactions provide write locking, preventing race conditions
        // where new API keys could be created between deleting the SA and revoking its keys.
        // Note: sqlx uses IMMEDIATE mode for write transactions on SQLite by default.
        let mut tx = self.pool.begin().await?;
        let now = chrono::Utc::now().trunc_subsecs(3);

        // 1. Check if the service account exists (locks the row in SQLite's transaction)
        let exists = sqlx::query(
            r#"
            SELECT id FROM service_accounts
            WHERE id = ? AND deleted_at IS NULL
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&mut *tx)
        .await?;

        if exists.is_none() {
            return Err(DbError::NotFound);
        }

        // 2. Get API key IDs before revoking (SQLite RETURNING is available since 3.35)
        let revoked_ids: Vec<String> = sqlx::query_scalar(
            r#"
            UPDATE api_keys
            SET revoked_at = ?, updated_at = ?
            WHERE owner_type = 'service_account' AND owner_id = ? AND revoked_at IS NULL
            RETURNING id
            "#,
        )
        .bind(now)
        .bind(now)
        .bind(id.to_string())
        .fetch_all(&mut *tx)
        .await?;

        // 3. Soft-delete the service account
        sqlx::query(
            r#"
            UPDATE service_accounts
            SET deleted_at = ?
            WHERE id = ?
            "#,
        )
        .bind(now)
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        // Convert string IDs to UUIDs
        let revoked_uuids: Vec<Uuid> = revoked_ids
            .into_iter()
            .filter_map(|s| parse_uuid(&s).ok())
            .collect();

        Ok(revoked_uuids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::ServiceAccountRepo;

    /// Create an in-memory SQLite database with the required tables
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

        // Create the service_accounts table
        sqlx::query(
            r#"
            CREATE TABLE service_accounts (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                slug TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                roles TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                deleted_at TEXT,
                UNIQUE(org_id, slug)
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create service_accounts table");

        pool
    }

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

    fn create_sa_input(slug: &str, name: &str) -> CreateServiceAccount {
        CreateServiceAccount {
            slug: slug.to_string(),
            name: name.to_string(),
            description: None,
            roles: vec![],
        }
    }

    #[tokio::test]
    async fn test_create_service_account() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteServiceAccountRepo::new(pool);

        let input = create_sa_input("ci-bot", "CI Bot");
        let sa = repo
            .create(org_id, input)
            .await
            .expect("Failed to create service account");

        assert_eq!(sa.slug, "ci-bot");
        assert_eq!(sa.name, "CI Bot");
        assert_eq!(sa.org_id, org_id);
        assert!(sa.roles.is_empty());
        assert!(!sa.id.is_nil());
    }

    #[tokio::test]
    async fn test_create_with_roles() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteServiceAccountRepo::new(pool);

        let input = CreateServiceAccount {
            slug: "admin-bot".to_string(),
            name: "Admin Bot".to_string(),
            description: Some("Automated admin tasks".to_string()),
            roles: vec!["admin".to_string(), "deployer".to_string()],
        };
        let sa = repo
            .create(org_id, input)
            .await
            .expect("Failed to create service account");

        assert_eq!(sa.roles, vec!["admin", "deployer"]);
        assert_eq!(sa.description, Some("Automated admin tasks".to_string()));
    }

    #[tokio::test]
    async fn test_create_duplicate_slug_fails() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteServiceAccountRepo::new(pool);

        let input = create_sa_input("duplicate", "First");
        repo.create(org_id, input)
            .await
            .expect("Failed to create first");

        let input2 = create_sa_input("duplicate", "Second");
        let result = repo.create(org_id, input2).await;

        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteServiceAccountRepo::new(pool);

        let input = create_sa_input("test-sa", "Test SA");
        let created = repo.create(org_id, input).await.expect("Failed to create");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.slug, "test-sa");
    }

    #[tokio::test]
    async fn test_get_by_slug() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteServiceAccountRepo::new(pool);

        let input = create_sa_input("slug-test", "Slug Test");
        let created = repo.create(org_id, input).await.expect("Failed to create");

        let fetched = repo
            .get_by_slug(org_id, "slug-test")
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.id, created.id);
    }

    #[tokio::test]
    async fn test_list_by_org() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteServiceAccountRepo::new(pool);

        for i in 0..3 {
            repo.create(
                org_id,
                create_sa_input(&format!("sa-{}", i), &format!("SA {}", i)),
            )
            .await
            .expect("Failed to create");
        }

        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list");

        assert_eq!(result.items.len(), 3);
    }

    #[tokio::test]
    async fn test_count_by_org() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteServiceAccountRepo::new(pool);

        for i in 0..3 {
            repo.create(
                org_id,
                create_sa_input(&format!("sa-{}", i), &format!("SA {}", i)),
            )
            .await
            .expect("Failed to create");
        }

        let count = repo.count_by_org(org_id).await.expect("Failed to count");
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_update() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteServiceAccountRepo::new(pool);

        let created = repo
            .create(org_id, create_sa_input("update-test", "Original"))
            .await
            .expect("Failed to create");

        let updated = repo
            .update(
                created.id,
                UpdateServiceAccount {
                    name: Some("Updated".to_string()),
                    description: Some("New description".to_string()),
                    roles: Some(vec!["admin".to_string()]),
                },
            )
            .await
            .expect("Failed to update");

        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.description, Some("New description".to_string()));
        assert_eq!(updated.roles, vec!["admin"]);
        assert!(updated.updated_at >= created.updated_at);
    }

    #[tokio::test]
    async fn test_delete() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteServiceAccountRepo::new(pool);

        let created = repo
            .create(org_id, create_sa_input("delete-test", "To Delete"))
            .await
            .expect("Failed to create");

        repo.delete(created.id).await.expect("Failed to delete");

        let result = repo
            .get_by_id(created.id)
            .await
            .expect("Query should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteServiceAccountRepo::new(pool);

        let result = repo.delete(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }
}
