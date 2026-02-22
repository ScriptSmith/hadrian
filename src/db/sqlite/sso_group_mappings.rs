use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::{
            Cursor, CursorDirection, ListParams, ListResult, PageCursors, SsoGroupMappingRepo,
            cursor_from_row,
        },
    },
    models::{CreateSsoGroupMapping, SsoGroupMapping, UpdateSsoGroupMapping},
};

pub struct SqliteSsoGroupMappingRepo {
    pool: SqlitePool,
}

impl SqliteSsoGroupMappingRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Parse an SsoGroupMapping from a database row.
    fn parse_mapping(row: &sqlx::sqlite::SqliteRow) -> DbResult<SsoGroupMapping> {
        let team_id: Option<String> = row.get("team_id");
        let team_id = team_id.map(|s| parse_uuid(&s)).transpose()?;

        Ok(SsoGroupMapping {
            id: parse_uuid(&row.get::<String, _>("id"))?,
            sso_connection_name: row.get("sso_connection_name"),
            idp_group: row.get("idp_group"),
            org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
            team_id,
            role: row.get("role"),
            priority: row.get("priority"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Helper method for cursor-based pagination.
    async fn list_with_cursor(
        &self,
        org_id: Uuid,
        sso_connection_name: Option<&str>,
        params: &ListParams,
        cursor: &Cursor,
        fetch_limit: i64,
        limit: i64,
    ) -> DbResult<ListResult<SsoGroupMapping>> {
        let (comparison, order, should_reverse) =
            params.sort_order.cursor_query_params(params.direction);

        let connection_filter = if sso_connection_name.is_some() {
            "AND sso_connection_name = ?"
        } else {
            ""
        };

        let query = format!(
            r#"
            SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            FROM sso_group_mappings
            WHERE org_id = ? AND (created_at, id) {} (?, ?)
            {}
            ORDER BY created_at {}, id {}
            LIMIT ?
            "#,
            comparison, connection_filter, order, order
        );

        let mut query_builder = sqlx::query(&query)
            .bind(org_id.to_string())
            .bind(cursor.created_at)
            .bind(cursor.id.to_string());

        if let Some(name) = sso_connection_name {
            query_builder = query_builder.bind(name);
        }

        let rows = query_builder
            .bind(fetch_limit)
            .fetch_all(&self.pool)
            .await?;

        let has_more = rows.len() as i64 > limit;
        let mut items: Vec<SsoGroupMapping> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_mapping)
            .collect::<DbResult<Vec<_>>>()?;

        if should_reverse {
            items.reverse();
        }

        let cursors =
            PageCursors::from_items(&items, has_more, params.direction, Some(cursor), |m| {
                cursor_from_row(m.created_at, m.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }
}

#[async_trait]
impl SsoGroupMappingRepo for SqliteSsoGroupMappingRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateSsoGroupMapping,
    ) -> DbResult<SsoGroupMapping> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO sso_group_mappings (id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&input.sso_connection_name)
        .bind(&input.idp_group)
        .bind(org_id.to_string())
        .bind(input.team_id.map(|id| id.to_string()))
        .bind(&input.role)
        .bind(input.priority)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict(format!(
                    "Mapping for IdP group '{}' already exists for this connection/org/team combination",
                    input.idp_group
                ))
            }
            _ => DbError::from(e),
        })?;

        Ok(SsoGroupMapping {
            id,
            sso_connection_name: input.sso_connection_name,
            idp_group: input.idp_group,
            org_id,
            team_id: input.team_id,
            role: input.role,
            priority: input.priority,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<SsoGroupMapping>> {
        let result = sqlx::query(
            r#"
            SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            FROM sso_group_mappings
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_mapping(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_by_org(
        &self,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<SsoGroupMapping>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(org_id, None, &params, cursor, fetch_limit, limit)
                .await;
        }

        let rows = sqlx::query(
            r#"
            SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            FROM sso_group_mappings
            WHERE org_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(org_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<SsoGroupMapping> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_mapping)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |m| {
                cursor_from_row(m.created_at, m.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn list_by_connection(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        params: ListParams,
    ) -> DbResult<ListResult<SsoGroupMapping>> {
        let limit = params.limit.unwrap_or(100);
        let fetch_limit = limit + 1;

        if let Some(ref cursor) = params.cursor {
            return self
                .list_with_cursor(
                    org_id,
                    Some(sso_connection_name),
                    &params,
                    cursor,
                    fetch_limit,
                    limit,
                )
                .await;
        }

        let rows = sqlx::query(
            r#"
            SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            FROM sso_group_mappings
            WHERE sso_connection_name = ? AND org_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(sso_connection_name)
        .bind(org_id.to_string())
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let has_more = rows.len() as i64 > limit;
        let items: Vec<SsoGroupMapping> = rows
            .iter()
            .take(limit as usize)
            .map(Self::parse_mapping)
            .collect::<DbResult<Vec<_>>>()?;

        let cursors =
            PageCursors::from_items(&items, has_more, CursorDirection::Forward, None, |m| {
                cursor_from_row(m.created_at, m.id)
            });

        Ok(ListResult::new(items, has_more, cursors))
    }

    async fn find_mappings_for_groups(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        idp_groups: &[String],
    ) -> DbResult<Vec<SsoGroupMapping>> {
        if idp_groups.is_empty() {
            return Ok(Vec::new());
        }

        // Build placeholders for IN clause
        let placeholders: Vec<&str> = idp_groups.iter().map(|_| "?").collect();
        let in_clause = placeholders.join(", ");

        let query = format!(
            r#"
            SELECT id, sso_connection_name, idp_group, org_id, team_id, role, priority, created_at, updated_at
            FROM sso_group_mappings
            WHERE sso_connection_name = ? AND org_id = ? AND idp_group IN ({})
            ORDER BY priority DESC, idp_group, created_at
            "#,
            in_clause
        );

        let mut query_builder = sqlx::query(&query)
            .bind(sso_connection_name)
            .bind(org_id.to_string());

        for group in idp_groups {
            query_builder = query_builder.bind(group);
        }

        let rows = query_builder.fetch_all(&self.pool).await?;

        rows.iter().map(Self::parse_mapping).collect()
    }

    async fn count_by_org(&self, org_id: Uuid) -> DbResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM sso_group_mappings WHERE org_id = ?")
            .bind(org_id.to_string())
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get::<i64, _>("count"))
    }

    async fn update(&self, id: Uuid, input: UpdateSsoGroupMapping) -> DbResult<SsoGroupMapping> {
        let has_idp_group = input.idp_group.is_some();
        let has_team_id = input.team_id.is_some();
        let has_role = input.role.is_some();
        let has_priority = input.priority.is_some();

        if !has_idp_group && !has_team_id && !has_role && !has_priority {
            return self.get_by_id(id).await?.ok_or(DbError::NotFound);
        }

        let now = chrono::Utc::now();

        let mut set_clauses = vec!["updated_at = ?"];
        if has_idp_group {
            set_clauses.push("idp_group = ?");
        }
        if has_team_id {
            set_clauses.push("team_id = ?");
        }
        if has_role {
            set_clauses.push("role = ?");
        }
        if has_priority {
            set_clauses.push("priority = ?");
        }

        let query = format!(
            "UPDATE sso_group_mappings SET {} WHERE id = ?",
            set_clauses.join(", ")
        );

        let mut query_builder = sqlx::query(&query).bind(now);

        if let Some(ref idp_group) = input.idp_group {
            query_builder = query_builder.bind(idp_group);
        }
        if let Some(ref team_id_opt) = input.team_id {
            query_builder = query_builder.bind(team_id_opt.map(|id| id.to_string()));
        }
        if let Some(ref role_opt) = input.role {
            query_builder = query_builder.bind(role_opt.as_ref());
        }
        if let Some(priority) = input.priority {
            query_builder = query_builder.bind(priority);
        }

        let result = query_builder
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                    DbError::Conflict("Mapping with this combination already exists".into())
                }
                _ => DbError::from(e),
            })?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM sso_group_mappings WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn delete_by_idp_group(
        &self,
        sso_connection_name: &str,
        org_id: Uuid,
        idp_group: &str,
    ) -> DbResult<u64> {
        let result = sqlx::query(
            "DELETE FROM sso_group_mappings WHERE sso_connection_name = ? AND org_id = ? AND idp_group = ?",
        )
        .bind(sso_connection_name)
        .bind(org_id.to_string())
        .bind(idp_group)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create organizations table (required for FK)
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

        // Create teams table (required for FK)
        sqlx::query(
            r#"
            CREATE TABLE teams (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
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
        .expect("Failed to create teams table");

        // Create sso_group_mappings table
        sqlx::query(
            r#"
            CREATE TABLE sso_group_mappings (
                id TEXT PRIMARY KEY NOT NULL,
                sso_connection_name TEXT NOT NULL DEFAULT 'default',
                idp_group TEXT NOT NULL,
                org_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
                team_id TEXT REFERENCES teams(id) ON DELETE CASCADE,
                role TEXT,
                priority INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create sso_group_mappings table");

        // Create unique indexes
        sqlx::query(
            r#"
            CREATE UNIQUE INDEX idx_sso_group_mappings_unique_with_team
            ON sso_group_mappings(sso_connection_name, idp_group, org_id, team_id) WHERE team_id IS NOT NULL
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create unique index with team");

        sqlx::query(
            r#"
            CREATE UNIQUE INDEX idx_sso_group_mappings_unique_without_team
            ON sso_group_mappings(sso_connection_name, idp_group, org_id) WHERE team_id IS NULL
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create unique index without team");

        pool
    }

    async fn create_test_org(pool: &SqlitePool, slug: &str) -> Uuid {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO organizations (id, slug, name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
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

    async fn create_test_team(pool: &SqlitePool, org_id: Uuid, slug: &str) -> Uuid {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO teams (id, org_id, slug, name, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(slug)
        .bind(format!("Team {}", slug))
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .expect("Failed to create test team");
        id
    }

    // ==================== Create Tests ====================

    #[tokio::test]
    async fn test_create_mapping_without_team() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let input = CreateSsoGroupMapping {
            sso_connection_name: "default".to_string(),
            idp_group: "Engineering".to_string(),
            team_id: None,
            role: Some("developer".to_string()),
            priority: 0,
        };

        let mapping = repo
            .create(org_id, input)
            .await
            .expect("Failed to create mapping");

        assert_eq!(mapping.sso_connection_name, "default");
        assert_eq!(mapping.idp_group, "Engineering");
        assert_eq!(mapping.org_id, org_id);
        assert!(mapping.team_id.is_none());
        assert_eq!(mapping.role, Some("developer".to_string()));
        assert_eq!(mapping.priority, 0);
    }

    #[tokio::test]
    async fn test_create_mapping_with_team() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let team_id = create_test_team(&pool, org_id, "engineering").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let input = CreateSsoGroupMapping {
            sso_connection_name: "okta".to_string(),
            idp_group: "Engineering".to_string(),
            team_id: Some(team_id),
            role: Some("member".to_string()),
            priority: 0,
        };

        let mapping = repo
            .create(org_id, input)
            .await
            .expect("Failed to create mapping");

        assert_eq!(mapping.sso_connection_name, "okta");
        assert_eq!(mapping.team_id, Some(team_id));
    }

    #[tokio::test]
    async fn test_create_duplicate_mapping_fails() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let input = CreateSsoGroupMapping {
            sso_connection_name: "default".to_string(),
            idp_group: "Admins".to_string(),
            team_id: None,
            role: Some("admin".to_string()),
            priority: 0,
        };

        repo.create(org_id, input.clone())
            .await
            .expect("First create should succeed");
        let result = repo.create(org_id, input).await;

        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_same_group_different_teams_allowed() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let team1_id = create_test_team(&pool, org_id, "team-1").await;
        let team2_id = create_test_team(&pool, org_id, "team-2").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        // Same IdP group can map to multiple teams
        let input1 = CreateSsoGroupMapping {
            sso_connection_name: "default".to_string(),
            idp_group: "Engineering".to_string(),
            team_id: Some(team1_id),
            role: Some("member".to_string()),
            priority: 0,
        };

        let input2 = CreateSsoGroupMapping {
            sso_connection_name: "default".to_string(),
            idp_group: "Engineering".to_string(),
            team_id: Some(team2_id),
            role: Some("member".to_string()),
            priority: 0,
        };

        repo.create(org_id, input1)
            .await
            .expect("First mapping should succeed");
        repo.create(org_id, input2)
            .await
            .expect("Second mapping to different team should succeed");
    }

    // ==================== Get/List Tests ====================

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let input = CreateSsoGroupMapping {
            sso_connection_name: "default".to_string(),
            idp_group: "Test".to_string(),
            team_id: None,
            role: None,
            priority: 0,
        };

        let created = repo.create(org_id, input).await.expect("Failed to create");
        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.idp_group, "Test");
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let result = repo
            .get_by_id(Uuid::new_v4())
            .await
            .expect("Query should succeed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_by_org() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        for group in ["Group1", "Group2", "Group3"] {
            let input = CreateSsoGroupMapping {
                sso_connection_name: "default".to_string(),
                idp_group: group.to_string(),
                team_id: None,
                role: None,
                priority: 0,
            };
            repo.create(org_id, input).await.expect("Failed to create");
        }

        let result = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list");

        assert_eq!(result.items.len(), 3);
    }

    #[tokio::test]
    async fn test_list_by_connection() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        // Create mappings for different connections
        for (conn, group) in [
            ("okta", "OktaGroup"),
            ("azure", "AzureGroup"),
            ("okta", "OktaGroup2"),
        ] {
            let input = CreateSsoGroupMapping {
                sso_connection_name: conn.to_string(),
                idp_group: group.to_string(),
                team_id: None,
                role: None,
                priority: 0,
            };
            repo.create(org_id, input).await.expect("Failed to create");
        }

        let okta_result = repo
            .list_by_connection("okta", org_id, ListParams::default())
            .await
            .expect("Failed to list");
        let azure_result = repo
            .list_by_connection("azure", org_id, ListParams::default())
            .await
            .expect("Failed to list");

        assert_eq!(okta_result.items.len(), 2);
        assert_eq!(azure_result.items.len(), 1);
    }

    // ==================== Find Mappings for Groups Tests ====================

    #[tokio::test]
    async fn test_find_mappings_for_groups() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let team_id = create_test_team(&pool, org_id, "engineering").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        // Create several mappings
        for (group, team, role) in [
            ("Engineering", Some(team_id), Some("developer")),
            ("Admins", None, Some("admin")),
            ("Support", None, Some("support")),
        ] {
            let input = CreateSsoGroupMapping {
                sso_connection_name: "default".to_string(),
                idp_group: group.to_string(),
                team_id: team,
                role: role.map(String::from),
                priority: 0,
            };
            repo.create(org_id, input).await.expect("Failed to create");
        }

        // Find mappings for a subset of groups
        let user_groups = vec!["Engineering".to_string(), "Admins".to_string()];
        let mappings = repo
            .find_mappings_for_groups("default", org_id, &user_groups)
            .await
            .expect("Failed to find mappings");

        assert_eq!(mappings.len(), 2);
        let group_names: Vec<&str> = mappings.iter().map(|m| m.idp_group.as_str()).collect();
        assert!(group_names.contains(&"Engineering"));
        assert!(group_names.contains(&"Admins"));
        assert!(!group_names.contains(&"Support"));
    }

    #[tokio::test]
    async fn test_find_mappings_empty_groups() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let mappings = repo
            .find_mappings_for_groups("default", org_id, &[])
            .await
            .expect("Failed to find mappings");

        assert!(mappings.is_empty());
    }

    #[tokio::test]
    async fn test_find_mappings_no_matches() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let input = CreateSsoGroupMapping {
            sso_connection_name: "default".to_string(),
            idp_group: "Engineering".to_string(),
            team_id: None,
            role: None,
            priority: 0,
        };
        repo.create(org_id, input).await.expect("Failed to create");

        let mappings = repo
            .find_mappings_for_groups("default", org_id, &["NonExistent".to_string()])
            .await
            .expect("Failed to find mappings");

        assert!(mappings.is_empty());
    }

    #[tokio::test]
    async fn test_find_mappings_ordered_by_priority() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        // Create different teams for each mapping to satisfy unique constraint
        let team_low = create_test_team(&pool, org_id, "team-low").await;
        let team_medium = create_test_team(&pool, org_id, "team-medium").await;
        let team_high = create_test_team(&pool, org_id, "team-high").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        // Create mappings with different priorities for the same group
        // Insert in non-priority order to verify sorting works
        for (priority, role, team_id) in [
            (0, "low", team_low),
            (10, "high", team_high),
            (5, "medium", team_medium),
        ] {
            let input = CreateSsoGroupMapping {
                sso_connection_name: "default".to_string(),
                idp_group: "Engineering".to_string(),
                team_id: Some(team_id),
                role: Some(role.to_string()),
                priority,
            };
            repo.create(org_id, input).await.expect("Failed to create");
        }

        let mappings = repo
            .find_mappings_for_groups("default", org_id, &["Engineering".to_string()])
            .await
            .expect("Failed to find mappings");

        assert_eq!(mappings.len(), 3);
        // Should be ordered by priority DESC
        assert_eq!(mappings[0].priority, 10);
        assert_eq!(mappings[0].role, Some("high".to_string()));
        assert_eq!(mappings[1].priority, 5);
        assert_eq!(mappings[1].role, Some("medium".to_string()));
        assert_eq!(mappings[2].priority, 0);
        assert_eq!(mappings[2].role, Some("low".to_string()));
    }

    // ==================== Update Tests ====================

    #[tokio::test]
    async fn test_update_mapping() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let team_id = create_test_team(&pool, org_id, "new-team").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let input = CreateSsoGroupMapping {
            sso_connection_name: "default".to_string(),
            idp_group: "OldGroup".to_string(),
            team_id: None,
            role: Some("old-role".to_string()),
            priority: 0,
        };
        let created = repo.create(org_id, input).await.expect("Failed to create");

        let updated = repo
            .update(
                created.id,
                UpdateSsoGroupMapping {
                    idp_group: Some("NewGroup".to_string()),
                    team_id: Some(Some(team_id)),
                    role: Some(Some("new-role".to_string())),
                    priority: None,
                },
            )
            .await
            .expect("Failed to update");

        assert_eq!(updated.idp_group, "NewGroup");
        assert_eq!(updated.team_id, Some(team_id));
        assert_eq!(updated.role, Some("new-role".to_string()));
    }

    #[tokio::test]
    async fn test_update_clear_optional_fields() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let team_id = create_test_team(&pool, org_id, "team").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let input = CreateSsoGroupMapping {
            sso_connection_name: "default".to_string(),
            idp_group: "Group".to_string(),
            team_id: Some(team_id),
            role: Some("role".to_string()),
            priority: 0,
        };
        let created = repo.create(org_id, input).await.expect("Failed to create");

        // Clear team_id and role by setting to None
        let updated = repo
            .update(
                created.id,
                UpdateSsoGroupMapping {
                    idp_group: None,
                    team_id: Some(None), // Clear team_id
                    role: Some(None),    // Clear role
                    priority: None,
                },
            )
            .await
            .expect("Failed to update");

        assert!(updated.team_id.is_none());
        assert!(updated.role.is_none());
    }

    #[tokio::test]
    async fn test_update_not_found() {
        let pool = create_test_pool().await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let result = repo
            .update(
                Uuid::new_v4(),
                UpdateSsoGroupMapping {
                    idp_group: Some("New".to_string()),
                    team_id: None,
                    role: None,
                    priority: None,
                },
            )
            .await;

        assert!(matches!(result, Err(DbError::NotFound)));
    }

    // ==================== Delete Tests ====================

    #[tokio::test]
    async fn test_delete_mapping() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let input = CreateSsoGroupMapping {
            sso_connection_name: "default".to_string(),
            idp_group: "ToDelete".to_string(),
            team_id: None,
            role: None,
            priority: 0,
        };
        let created = repo.create(org_id, input).await.expect("Failed to create");

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
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        let result = repo.delete(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_delete_by_idp_group() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let team1_id = create_test_team(&pool, org_id, "team-1").await;
        let team2_id = create_test_team(&pool, org_id, "team-2").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        // Create multiple mappings for same IdP group (different teams)
        for team_id in [team1_id, team2_id] {
            let input = CreateSsoGroupMapping {
                sso_connection_name: "default".to_string(),
                idp_group: "Engineering".to_string(),
                team_id: Some(team_id),
                role: None,
                priority: 0,
            };
            repo.create(org_id, input).await.expect("Failed to create");
        }

        // Also create a mapping for a different group
        let input = CreateSsoGroupMapping {
            sso_connection_name: "default".to_string(),
            idp_group: "Other".to_string(),
            team_id: None,
            role: None,
            priority: 0,
        };
        repo.create(org_id, input).await.expect("Failed to create");

        // Delete all Engineering mappings
        let deleted_count = repo
            .delete_by_idp_group("default", org_id, "Engineering")
            .await
            .expect("Failed to delete");

        assert_eq!(deleted_count, 2);

        // Verify only "Other" remains
        let remaining = repo
            .list_by_org(org_id, ListParams::default())
            .await
            .expect("Failed to list");
        assert_eq!(remaining.items.len(), 1);
        assert_eq!(remaining.items[0].idp_group, "Other");
    }

    // ==================== Count Tests ====================

    #[tokio::test]
    async fn test_count_by_org() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteSsoGroupMappingRepo::new(pool);

        assert_eq!(repo.count_by_org(org_id).await.expect("Failed to count"), 0);

        for i in 0..3 {
            let input = CreateSsoGroupMapping {
                sso_connection_name: "default".to_string(),
                idp_group: format!("Group{}", i),
                team_id: None,
                role: None,
                priority: 0,
            };
            repo.create(org_id, input).await.expect("Failed to create");
        }

        assert_eq!(repo.count_by_org(org_id).await.expect("Failed to count"), 3);
    }
}
