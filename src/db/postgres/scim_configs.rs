//! PostgreSQL implementation of the SCIM config repository.

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    db::{
        error::{DbError, DbResult},
        repos::OrgScimConfigRepo,
    },
    models::{CreateOrgScimConfig, OrgScimConfig, OrgScimConfigWithHash, UpdateOrgScimConfig},
};

pub struct PostgresOrgScimConfigRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresOrgScimConfigRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    /// Parse an OrgScimConfig from a database row.
    fn parse_config(row: &sqlx::postgres::PgRow) -> OrgScimConfig {
        OrgScimConfig {
            id: row.get("id"),
            org_id: row.get("org_id"),
            enabled: row.get("enabled"),
            token_prefix: row.get("token_prefix"),
            token_last_used_at: row.get("token_last_used_at"),
            create_users: row.get("create_users"),
            default_team_id: row.get("default_team_id"),
            default_org_role: row.get("default_org_role"),
            default_team_role: row.get("default_team_role"),
            sync_display_name: row.get("sync_display_name"),
            deactivate_deletes_user: row.get("deactivate_deletes_user"),
            revoke_api_keys_on_deactivate: row.get("revoke_api_keys_on_deactivate"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    /// Parse an OrgScimConfigWithHash from a database row.
    fn parse_config_with_hash(row: &sqlx::postgres::PgRow) -> OrgScimConfigWithHash {
        let config = Self::parse_config(row);
        let token_hash: String = row.get("token_hash");
        OrgScimConfigWithHash { config, token_hash }
    }
}

#[async_trait]
impl OrgScimConfigRepo for PostgresOrgScimConfigRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgScimConfig,
        token_hash: &str,
        token_prefix: &str,
    ) -> DbResult<OrgScimConfig> {
        let row = sqlx::query(
            r#"
            INSERT INTO org_scim_configs (
                id, org_id, enabled, token_hash, token_prefix,
                create_users, default_team_id, default_org_role, default_team_role,
                sync_display_name, deactivate_deletes_user, revoke_api_keys_on_deactivate
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id, org_id, enabled, token_hash, token_prefix, token_last_used_at,
                      create_users, default_team_id, default_org_role, default_team_role,
                      sync_display_name, deactivate_deletes_user, revoke_api_keys_on_deactivate,
                      created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
        .bind(input.enabled)
        .bind(token_hash)
        .bind(token_prefix)
        .bind(input.create_users)
        .bind(input.default_team_id)
        .bind(&input.default_org_role)
        .bind(&input.default_team_role)
        .bind(input.sync_display_name)
        .bind(input.deactivate_deletes_user)
        .bind(input.revoke_api_keys_on_deactivate)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("Organization already has a SCIM configuration".into())
            }
            _ => DbError::from(e),
        })?;

        Ok(Self::parse_config(&row))
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgScimConfig>> {
        let row = sqlx::query(
            r#"
            SELECT id, org_id, enabled, token_hash, token_prefix, token_last_used_at,
                   create_users, default_team_id, default_org_role, default_team_role,
                   sync_display_name, deactivate_deletes_user, revoke_api_keys_on_deactivate,
                   created_at, updated_at
            FROM org_scim_configs WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(row.map(|r| Self::parse_config(&r)))
    }

    async fn get_by_org_id(&self, org_id: Uuid) -> DbResult<Option<OrgScimConfig>> {
        let row = sqlx::query(
            r#"
            SELECT id, org_id, enabled, token_hash, token_prefix, token_last_used_at,
                   create_users, default_team_id, default_org_role, default_team_role,
                   sync_display_name, deactivate_deletes_user, revoke_api_keys_on_deactivate,
                   created_at, updated_at
            FROM org_scim_configs WHERE org_id = $1
            "#,
        )
        .bind(org_id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(row.map(|r| Self::parse_config(&r)))
    }

    async fn get_with_hash_by_org_id(
        &self,
        org_id: Uuid,
    ) -> DbResult<Option<OrgScimConfigWithHash>> {
        let row = sqlx::query("SELECT * FROM org_scim_configs WHERE org_id = $1")
            .bind(org_id)
            .fetch_optional(&self.read_pool)
            .await?;

        Ok(row.map(|r| Self::parse_config_with_hash(&r)))
    }

    async fn get_by_token_hash(&self, token_hash: &str) -> DbResult<Option<OrgScimConfigWithHash>> {
        let row = sqlx::query("SELECT * FROM org_scim_configs WHERE token_hash = $1")
            .bind(token_hash)
            .fetch_optional(&self.read_pool)
            .await?;

        Ok(row.map(|r| Self::parse_config_with_hash(&r)))
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

        let row = sqlx::query(
            r#"
            UPDATE org_scim_configs
            SET enabled = $1, create_users = $2, default_team_id = $3,
                default_org_role = $4, default_team_role = $5, sync_display_name = $6,
                deactivate_deletes_user = $7, revoke_api_keys_on_deactivate = $8,
                updated_at = NOW()
            WHERE id = $9
            RETURNING id, org_id, enabled, token_hash, token_prefix, token_last_used_at,
                      create_users, default_team_id, default_org_role, default_team_role,
                      sync_display_name, deactivate_deletes_user, revoke_api_keys_on_deactivate,
                      created_at, updated_at
            "#,
        )
        .bind(enabled)
        .bind(create_users)
        .bind(default_team_id)
        .bind(&default_org_role)
        .bind(&default_team_role)
        .bind(sync_display_name)
        .bind(deactivate_deletes_user)
        .bind(revoke_api_keys_on_deactivate)
        .bind(id)
        .fetch_one(&self.write_pool)
        .await?;

        Ok(Self::parse_config(&row))
    }

    async fn rotate_token(
        &self,
        id: Uuid,
        token_hash: &str,
        token_prefix: &str,
    ) -> DbResult<OrgScimConfig> {
        let row = sqlx::query(
            r#"
            UPDATE org_scim_configs
            SET token_hash = $1, token_prefix = $2, token_last_used_at = NULL, updated_at = NOW()
            WHERE id = $3
            RETURNING id, org_id, enabled, token_hash, token_prefix, token_last_used_at,
                      create_users, default_team_id, default_org_role, default_team_role,
                      sync_display_name, deactivate_deletes_user, revoke_api_keys_on_deactivate,
                      created_at, updated_at
            "#,
        )
        .bind(token_hash)
        .bind(token_prefix)
        .bind(id)
        .fetch_optional(&self.write_pool)
        .await?
        .ok_or_else(|| DbError::NotFound)?;

        Ok(Self::parse_config(&row))
    }

    async fn update_token_last_used(&self, id: Uuid) -> DbResult<()> {
        sqlx::query("UPDATE org_scim_configs SET token_last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;

        Ok(())
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM org_scim_configs WHERE id = $1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn list_enabled(&self) -> DbResult<Vec<OrgScimConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_id, enabled, token_hash, token_prefix, token_last_used_at,
                   create_users, default_team_id, default_org_role, default_team_role,
                   sync_display_name, deactivate_deletes_user, revoke_api_keys_on_deactivate,
                   created_at, updated_at
            FROM org_scim_configs WHERE enabled = true
            "#,
        )
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows.iter().map(Self::parse_config).collect())
    }
}
