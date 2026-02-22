//! SQLite implementation of the SCIM config repository.

use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::OrgScimConfigRepo,
    },
    models::{CreateOrgScimConfig, OrgScimConfig, OrgScimConfigWithHash, UpdateOrgScimConfig},
};

pub struct SqliteOrgScimConfigRepo {
    pool: SqlitePool,
}

impl SqliteOrgScimConfigRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Parse an OrgScimConfig from a database row.
    fn parse_config(row: &sqlx::sqlite::SqliteRow) -> DbResult<OrgScimConfig> {
        let default_team_id: Option<String> = row.get("default_team_id");
        let default_team_id = default_team_id.map(|s| parse_uuid(&s)).transpose()?;

        Ok(OrgScimConfig {
            id: parse_uuid(&row.get::<String, _>("id"))?,
            org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
            enabled: row.get::<i32, _>("enabled") != 0,
            token_prefix: row.get("token_prefix"),
            token_last_used_at: row.get("token_last_used_at"),
            create_users: row.get::<i32, _>("create_users") != 0,
            default_team_id,
            default_org_role: row.get("default_org_role"),
            default_team_role: row.get("default_team_role"),
            sync_display_name: row.get::<i32, _>("sync_display_name") != 0,
            deactivate_deletes_user: row.get::<i32, _>("deactivate_deletes_user") != 0,
            revoke_api_keys_on_deactivate: row.get::<i32, _>("revoke_api_keys_on_deactivate") != 0,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Parse an OrgScimConfigWithHash from a database row.
    fn parse_config_with_hash(row: &sqlx::sqlite::SqliteRow) -> DbResult<OrgScimConfigWithHash> {
        let config = Self::parse_config(row)?;
        let token_hash: String = row.get("token_hash");
        Ok(OrgScimConfigWithHash { config, token_hash })
    }
}

#[async_trait]
impl OrgScimConfigRepo for SqliteOrgScimConfigRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgScimConfig,
        token_hash: &str,
        token_prefix: &str,
    ) -> DbResult<OrgScimConfig> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            INSERT INTO org_scim_configs (
                id, org_id, enabled, token_hash, token_prefix,
                create_users, default_team_id, default_org_role, default_team_role,
                sync_display_name, deactivate_deletes_user, revoke_api_keys_on_deactivate,
                created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(input.enabled as i32)
        .bind(token_hash)
        .bind(token_prefix)
        .bind(input.create_users as i32)
        .bind(input.default_team_id.map(|id| id.to_string()))
        .bind(&input.default_org_role)
        .bind(&input.default_team_role)
        .bind(input.sync_display_name as i32)
        .bind(input.deactivate_deletes_user as i32)
        .bind(input.revoke_api_keys_on_deactivate as i32)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("Organization already has a SCIM configuration".into())
            }
            _ => DbError::from(e),
        })?;

        Ok(OrgScimConfig {
            id,
            org_id,
            enabled: input.enabled,
            token_prefix: token_prefix.to_string(),
            token_last_used_at: None,
            create_users: input.create_users,
            default_team_id: input.default_team_id,
            default_org_role: input.default_org_role,
            default_team_role: input.default_team_role,
            sync_display_name: input.sync_display_name,
            deactivate_deletes_user: input.deactivate_deletes_user,
            revoke_api_keys_on_deactivate: input.revoke_api_keys_on_deactivate,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgScimConfig>> {
        let row = sqlx::query("SELECT * FROM org_scim_configs WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        row.map(|r| Self::parse_config(&r)).transpose()
    }

    async fn get_by_org_id(&self, org_id: Uuid) -> DbResult<Option<OrgScimConfig>> {
        let row = sqlx::query("SELECT * FROM org_scim_configs WHERE org_id = ?")
            .bind(org_id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        row.map(|r| Self::parse_config(&r)).transpose()
    }

    async fn get_with_hash_by_org_id(
        &self,
        org_id: Uuid,
    ) -> DbResult<Option<OrgScimConfigWithHash>> {
        let row = sqlx::query("SELECT * FROM org_scim_configs WHERE org_id = ?")
            .bind(org_id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        row.map(|r| Self::parse_config_with_hash(&r)).transpose()
    }

    async fn get_by_token_hash(&self, token_hash: &str) -> DbResult<Option<OrgScimConfigWithHash>> {
        let row = sqlx::query("SELECT * FROM org_scim_configs WHERE token_hash = ?")
            .bind(token_hash)
            .fetch_optional(&self.pool)
            .await?;

        row.map(|r| Self::parse_config_with_hash(&r)).transpose()
    }

    async fn update(&self, id: Uuid, input: UpdateOrgScimConfig) -> DbResult<OrgScimConfig> {
        // Get the current config to apply updates
        let current = self.get_by_id(id).await?.ok_or_else(|| DbError::NotFound)?;

        let enabled = input.enabled.unwrap_or(current.enabled);
        let create_users = input.create_users.unwrap_or(current.create_users);
        let default_team_id = match input.default_team_id {
            Some(v) => v, // Some(Some(uuid)) or Some(None)
            None => current.default_team_id,
        };
        let default_org_role = input
            .default_org_role
            .unwrap_or(current.default_org_role.clone());
        let default_team_role = input
            .default_team_role
            .unwrap_or(current.default_team_role.clone());
        let sync_display_name = input.sync_display_name.unwrap_or(current.sync_display_name);
        let deactivate_deletes_user = input
            .deactivate_deletes_user
            .unwrap_or(current.deactivate_deletes_user);
        let revoke_api_keys_on_deactivate = input
            .revoke_api_keys_on_deactivate
            .unwrap_or(current.revoke_api_keys_on_deactivate);

        let now = chrono::Utc::now();

        sqlx::query(
            r#"
            UPDATE org_scim_configs
            SET enabled = ?, create_users = ?, default_team_id = ?,
                default_org_role = ?, default_team_role = ?, sync_display_name = ?,
                deactivate_deletes_user = ?, revoke_api_keys_on_deactivate = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(enabled as i32)
        .bind(create_users as i32)
        .bind(default_team_id.map(|id| id.to_string()))
        .bind(&default_org_role)
        .bind(&default_team_role)
        .bind(sync_display_name as i32)
        .bind(deactivate_deletes_user as i32)
        .bind(revoke_api_keys_on_deactivate as i32)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(OrgScimConfig {
            id: current.id,
            org_id: current.org_id,
            enabled,
            token_prefix: current.token_prefix,
            token_last_used_at: current.token_last_used_at,
            create_users,
            default_team_id,
            default_org_role,
            default_team_role,
            sync_display_name,
            deactivate_deletes_user,
            revoke_api_keys_on_deactivate,
            created_at: current.created_at,
            updated_at: now,
        })
    }

    async fn rotate_token(
        &self,
        id: Uuid,
        token_hash: &str,
        token_prefix: &str,
    ) -> DbResult<OrgScimConfig> {
        let now = chrono::Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE org_scim_configs
            SET token_hash = ?, token_prefix = ?, token_last_used_at = NULL, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(token_hash)
        .bind(token_prefix)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        self.get_by_id(id).await?.ok_or_else(|| DbError::NotFound)
    }

    async fn update_token_last_used(&self, id: Uuid) -> DbResult<()> {
        let now = chrono::Utc::now();

        sqlx::query("UPDATE org_scim_configs SET token_last_used_at = ? WHERE id = ?")
            .bind(now)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM org_scim_configs WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn list_enabled(&self) -> DbResult<Vec<OrgScimConfig>> {
        let rows = sqlx::query("SELECT * FROM org_scim_configs WHERE enabled = 1")
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(Self::parse_config).collect()
    }
}
