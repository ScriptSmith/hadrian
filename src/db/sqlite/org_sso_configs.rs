use async_trait::async_trait;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::common::parse_uuid;
use crate::{
    db::{
        error::{DbError, DbResult},
        repos::OrgSsoConfigRepo,
    },
    models::{
        CreateOrgSsoConfig, OrgSsoConfig, OrgSsoConfigWithSecret, SsoEnforcementMode,
        SsoProviderType, UpdateOrgSsoConfig,
    },
};

pub struct SqliteOrgSsoConfigRepo {
    pool: SqlitePool,
}

impl SqliteOrgSsoConfigRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Parse an OrgSsoConfig from a database row.
    fn parse_config(row: &sqlx::sqlite::SqliteRow) -> DbResult<OrgSsoConfig> {
        let default_team_id: Option<String> = row.get("default_team_id");
        let default_team_id = default_team_id.map(|s| parse_uuid(&s)).transpose()?;

        let scopes_str: String = row.get("scopes");
        let scopes: Vec<String> = scopes_str.split_whitespace().map(String::from).collect();

        let allowed_domains_json: Option<String> = row.get("allowed_email_domains");
        let allowed_email_domains: Vec<String> = allowed_domains_json
            .map(|json| serde_json::from_str(&json).unwrap_or_default())
            .unwrap_or_default();

        let provider_type_str: String = row.get("provider_type");
        let provider_type = provider_type_str
            .parse::<SsoProviderType>()
            .unwrap_or_default();

        let enforcement_mode_str: String = row.get("enforcement_mode");
        let enforcement_mode = enforcement_mode_str
            .parse::<SsoEnforcementMode>()
            .unwrap_or_default();

        Ok(OrgSsoConfig {
            id: parse_uuid(&row.get::<String, _>("id"))?,
            org_id: parse_uuid(&row.get::<String, _>("org_id"))?,
            provider_type,
            // OIDC fields
            issuer: row.get("issuer"),
            discovery_url: row.get("discovery_url"),
            client_id: row.get("client_id"),
            redirect_uri: row.get("redirect_uri"),
            scopes,
            identity_claim: row.get("identity_claim"),
            org_claim: row.get("org_claim"),
            groups_claim: row.get("groups_claim"),
            // SAML fields
            saml_metadata_url: row.get("saml_metadata_url"),
            saml_idp_entity_id: row.get("saml_idp_entity_id"),
            saml_idp_sso_url: row.get("saml_idp_sso_url"),
            saml_idp_slo_url: row.get("saml_idp_slo_url"),
            saml_idp_certificate: row.get("saml_idp_certificate"),
            saml_sp_entity_id: row.get("saml_sp_entity_id"),
            saml_name_id_format: row.get("saml_name_id_format"),
            saml_sign_requests: row.get::<i32, _>("saml_sign_requests") != 0,
            saml_sp_certificate: row.get("saml_sp_certificate"),
            saml_force_authn: row.get::<i32, _>("saml_force_authn") != 0,
            saml_authn_context_class_ref: row.get("saml_authn_context_class_ref"),
            saml_identity_attribute: row.get("saml_identity_attribute"),
            saml_email_attribute: row.get("saml_email_attribute"),
            saml_name_attribute: row.get("saml_name_attribute"),
            saml_groups_attribute: row.get("saml_groups_attribute"),
            // JIT provisioning
            provisioning_enabled: row.get::<i32, _>("provisioning_enabled") != 0,
            create_users: row.get::<i32, _>("create_users") != 0,
            default_team_id,
            default_org_role: row.get("default_org_role"),
            default_team_role: row.get("default_team_role"),
            allowed_email_domains,
            sync_attributes_on_login: row.get::<i32, _>("sync_attributes_on_login") != 0,
            sync_memberships_on_login: row.get::<i32, _>("sync_memberships_on_login") != 0,
            enforcement_mode,
            enabled: row.get::<i32, _>("enabled") != 0,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Parse an OrgSsoConfigWithSecret from a database row.
    fn parse_config_with_secret(row: &sqlx::sqlite::SqliteRow) -> DbResult<OrgSsoConfigWithSecret> {
        let config = Self::parse_config(row)?;
        let client_secret_key: Option<String> = row.get("client_secret_key");
        let saml_sp_private_key_ref: Option<String> = row.get("saml_sp_private_key_ref");
        Ok(OrgSsoConfigWithSecret {
            config,
            client_secret_key,
            saml_sp_private_key_ref,
        })
    }
}

#[async_trait]
impl OrgSsoConfigRepo for SqliteOrgSsoConfigRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgSsoConfig,
        client_secret_key: Option<&str>,
        saml_sp_private_key_ref: Option<&str>,
    ) -> DbResult<OrgSsoConfig> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let scopes_str = input.scopes.join(" ");
        let allowed_domains_json = if input.allowed_email_domains.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&input.allowed_email_domains).unwrap_or_default())
        };

        sqlx::query(
            r#"
            INSERT INTO org_sso_configs (
                id, org_id, provider_type,
                -- OIDC fields
                issuer, discovery_url, client_id, client_secret_key,
                redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                -- SAML fields
                saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                saml_name_attribute, saml_groups_attribute,
                -- JIT provisioning
                provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                enforcement_mode, enabled, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(input.provider_type.to_string())
        // OIDC fields
        .bind(&input.issuer)
        .bind(&input.discovery_url)
        .bind(&input.client_id)
        .bind(client_secret_key)
        .bind(&input.redirect_uri)
        .bind(&scopes_str)
        .bind(&input.identity_claim)
        .bind(&input.org_claim)
        .bind(&input.groups_claim)
        // SAML fields
        .bind(&input.saml_metadata_url)
        .bind(&input.saml_idp_entity_id)
        .bind(&input.saml_idp_sso_url)
        .bind(&input.saml_idp_slo_url)
        .bind(&input.saml_idp_certificate)
        .bind(&input.saml_sp_entity_id)
        .bind(&input.saml_name_id_format)
        .bind(input.saml_sign_requests as i32)
        .bind(saml_sp_private_key_ref)
        .bind(&input.saml_sp_certificate)
        .bind(input.saml_force_authn as i32)
        .bind(&input.saml_authn_context_class_ref)
        .bind(&input.saml_identity_attribute)
        .bind(&input.saml_email_attribute)
        .bind(&input.saml_name_attribute)
        .bind(&input.saml_groups_attribute)
        // JIT provisioning
        .bind(input.provisioning_enabled as i32)
        .bind(input.create_users as i32)
        .bind(input.default_team_id.map(|id| id.to_string()))
        .bind(&input.default_org_role)
        .bind(&input.default_team_role)
        .bind(&allowed_domains_json)
        .bind(input.sync_attributes_on_login as i32)
        .bind(input.sync_memberships_on_login as i32)
        .bind(input.enforcement_mode.to_string())
        .bind(input.enabled as i32)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("Organization already has an SSO configuration".into())
            }
            _ => DbError::from(e),
        })?;

        Ok(OrgSsoConfig {
            id,
            org_id,
            provider_type: input.provider_type,
            // OIDC fields
            issuer: input.issuer,
            discovery_url: input.discovery_url,
            client_id: input.client_id,
            redirect_uri: input.redirect_uri,
            scopes: input.scopes,
            identity_claim: Some(input.identity_claim),
            org_claim: input.org_claim,
            groups_claim: input.groups_claim,
            // SAML fields
            saml_metadata_url: input.saml_metadata_url,
            saml_idp_entity_id: input.saml_idp_entity_id,
            saml_idp_sso_url: input.saml_idp_sso_url,
            saml_idp_slo_url: input.saml_idp_slo_url,
            saml_idp_certificate: input.saml_idp_certificate,
            saml_sp_entity_id: input.saml_sp_entity_id,
            saml_name_id_format: input.saml_name_id_format,
            saml_sign_requests: input.saml_sign_requests,
            saml_sp_certificate: input.saml_sp_certificate,
            saml_force_authn: input.saml_force_authn,
            saml_authn_context_class_ref: input.saml_authn_context_class_ref,
            saml_identity_attribute: input.saml_identity_attribute,
            saml_email_attribute: input.saml_email_attribute,
            saml_name_attribute: input.saml_name_attribute,
            saml_groups_attribute: input.saml_groups_attribute,
            // JIT provisioning
            provisioning_enabled: input.provisioning_enabled,
            create_users: input.create_users,
            default_team_id: input.default_team_id,
            default_org_role: input.default_org_role,
            default_team_role: input.default_team_role,
            allowed_email_domains: input.allowed_email_domains,
            sync_attributes_on_login: input.sync_attributes_on_login,
            sync_memberships_on_login: input.sync_memberships_on_login,
            enforcement_mode: input.enforcement_mode,
            enabled: input.enabled,
            created_at: now,
            updated_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgSsoConfig>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, provider_type,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_config(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_by_org_id(&self, org_id: Uuid) -> DbResult<Option<OrgSsoConfig>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, provider_type,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE org_id = ?
            "#,
        )
        .bind(org_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_config(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_with_secret(&self, id: Uuid) -> DbResult<Option<OrgSsoConfigWithSecret>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, provider_type,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_config_with_secret(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_with_secret_by_org_id(
        &self,
        org_id: Uuid,
    ) -> DbResult<Option<OrgSsoConfigWithSecret>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, provider_type,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE org_id = ?
            "#,
        )
        .bind(org_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_config_with_secret(&row)?)),
            None => Ok(None),
        }
    }

    async fn update(
        &self,
        id: Uuid,
        input: UpdateOrgSsoConfig,
        client_secret_key: Option<&str>,
        saml_sp_private_key_ref: Option<&str>,
    ) -> DbResult<OrgSsoConfig> {
        let now = chrono::Utc::now();

        // Fetch existing record to use as fallback for optional fields
        let existing = self.get_by_id(id).await?.ok_or(DbError::NotFound)?;
        let existing_with_secret = self.get_with_secret(id).await?.ok_or(DbError::NotFound)?;

        let scopes_str = input
            .scopes
            .as_ref()
            .map(|s| s.join(" "))
            .unwrap_or_else(|| existing.scopes.join(" "));

        let allowed_domains_json = input
            .allowed_email_domains
            .as_ref()
            .map(|domains| {
                if domains.is_empty() {
                    None
                } else {
                    Some(serde_json::to_string(domains).unwrap_or_default())
                }
            })
            .unwrap_or_else(|| {
                if existing.allowed_email_domains.is_empty() {
                    None
                } else {
                    Some(serde_json::to_string(&existing.allowed_email_domains).unwrap_or_default())
                }
            });

        sqlx::query(
            r#"
            UPDATE org_sso_configs SET
                provider_type = ?, issuer = ?, discovery_url = ?, client_id = ?, client_secret_key = ?,
                redirect_uri = ?, scopes = ?, identity_claim = ?, org_claim = ?, groups_claim = ?,
                -- SAML fields
                saml_metadata_url = ?, saml_idp_entity_id = ?, saml_idp_sso_url = ?, saml_idp_slo_url = ?,
                saml_idp_certificate = ?, saml_sp_entity_id = ?, saml_name_id_format = ?,
                saml_sign_requests = ?, saml_sp_private_key_ref = ?, saml_sp_certificate = ?, saml_force_authn = ?,
                saml_authn_context_class_ref = ?, saml_identity_attribute = ?, saml_email_attribute = ?,
                saml_name_attribute = ?, saml_groups_attribute = ?,
                -- JIT provisioning
                provisioning_enabled = ?, create_users = ?, default_team_id = ?,
                default_org_role = ?, default_team_role = ?, allowed_email_domains = ?,
                sync_attributes_on_login = ?, sync_memberships_on_login = ?,
                enforcement_mode = ?, enabled = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(
            input
                .provider_type
                .unwrap_or(existing.provider_type)
                .to_string(),
        )
        .bind(input.issuer.or(existing.issuer))
        .bind(
            input
                .discovery_url
                .unwrap_or(existing.discovery_url.clone()),
        )
        .bind(input.client_id.or(existing.client_id))
        .bind(
            client_secret_key
                .map(String::from)
                .or(existing_with_secret.client_secret_key),
        )
        .bind(input.redirect_uri.unwrap_or(existing.redirect_uri.clone()))
        .bind(&scopes_str)
        .bind(input.identity_claim.or(existing.identity_claim))
        .bind(input.org_claim.unwrap_or(existing.org_claim.clone()))
        .bind(input.groups_claim.unwrap_or(existing.groups_claim.clone()))
        // SAML fields
        .bind(input.saml_metadata_url.unwrap_or(existing.saml_metadata_url.clone()))
        .bind(input.saml_idp_entity_id.unwrap_or(existing.saml_idp_entity_id.clone()))
        .bind(input.saml_idp_sso_url.unwrap_or(existing.saml_idp_sso_url.clone()))
        .bind(input.saml_idp_slo_url.unwrap_or(existing.saml_idp_slo_url.clone()))
        .bind(input.saml_idp_certificate.unwrap_or(existing.saml_idp_certificate.clone()))
        .bind(input.saml_sp_entity_id.unwrap_or(existing.saml_sp_entity_id.clone()))
        .bind(input.saml_name_id_format.unwrap_or(existing.saml_name_id_format.clone()))
        .bind(input.saml_sign_requests.unwrap_or(existing.saml_sign_requests) as i32)
        .bind(
            saml_sp_private_key_ref
                .map(String::from)
                .or(existing_with_secret.saml_sp_private_key_ref),
        )
        .bind(input.saml_sp_certificate.unwrap_or(existing.saml_sp_certificate.clone()))
        .bind(input.saml_force_authn.unwrap_or(existing.saml_force_authn) as i32)
        .bind(input.saml_authn_context_class_ref.unwrap_or(existing.saml_authn_context_class_ref.clone()))
        .bind(input.saml_identity_attribute.unwrap_or(existing.saml_identity_attribute.clone()))
        .bind(input.saml_email_attribute.unwrap_or(existing.saml_email_attribute.clone()))
        .bind(input.saml_name_attribute.unwrap_or(existing.saml_name_attribute.clone()))
        .bind(input.saml_groups_attribute.unwrap_or(existing.saml_groups_attribute.clone()))
        // JIT provisioning
        .bind(input.provisioning_enabled.unwrap_or(existing.provisioning_enabled) as i32)
        .bind(input.create_users.unwrap_or(existing.create_users) as i32)
        .bind(
            input
                .default_team_id
                .unwrap_or(existing.default_team_id)
                .map(|id| id.to_string()),
        )
        .bind(input.default_org_role.unwrap_or(existing.default_org_role))
        .bind(input.default_team_role.unwrap_or(existing.default_team_role))
        .bind(&allowed_domains_json)
        .bind(input.sync_attributes_on_login.unwrap_or(existing.sync_attributes_on_login) as i32)
        .bind(input.sync_memberships_on_login.unwrap_or(existing.sync_memberships_on_login) as i32)
        .bind(
            input
                .enforcement_mode
                .unwrap_or(existing.enforcement_mode)
                .to_string(),
        )
        .bind(input.enabled.unwrap_or(existing.enabled) as i32)
        .bind(now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;

        self.get_by_id(id).await?.ok_or(DbError::NotFound)
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM org_sso_configs WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn find_enabled_oidc_by_issuer(&self, issuer: &str) -> DbResult<Vec<OrgSsoConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_id, provider_type,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE enabled = 1 AND provider_type = 'oidc' AND issuer = ?
            "#,
        )
        .bind(issuer)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(Self::parse_config).collect()
    }

    async fn find_by_email_domain(&self, domain: &str) -> DbResult<Option<OrgSsoConfig>> {
        // Search for configs where the domain is in the allowed_email_domains JSON array
        // SQLite uses json_each to search JSON arrays
        let result = sqlx::query(
            r#"
            SELECT c.id, c.org_id, c.provider_type,
                   c.issuer, c.discovery_url, c.client_id, c.client_secret_key,
                   c.redirect_uri, c.scopes, c.identity_claim, c.org_claim, c.groups_claim,
                   c.saml_metadata_url, c.saml_idp_entity_id, c.saml_idp_sso_url, c.saml_idp_slo_url,
                   c.saml_idp_certificate, c.saml_sp_entity_id, c.saml_name_id_format,
                   c.saml_sign_requests, c.saml_sp_private_key_ref, c.saml_sp_certificate, c.saml_force_authn,
                   c.saml_authn_context_class_ref, c.saml_identity_attribute, c.saml_email_attribute,
                   c.saml_name_attribute, c.saml_groups_attribute,
                   c.provisioning_enabled, c.create_users, c.default_team_id, c.default_org_role, c.default_team_role,
                   c.allowed_email_domains, c.sync_attributes_on_login, c.sync_memberships_on_login,
                   c.enforcement_mode, c.enabled, c.created_at, c.updated_at
            FROM org_sso_configs c, json_each(c.allowed_email_domains) as d
            WHERE c.enabled = 1 AND d.value = ?
            LIMIT 1
            "#,
        )
        .bind(domain)
        .fetch_optional(&self.pool)
        .await?;

        match result {
            Some(row) => Ok(Some(Self::parse_config(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_enabled(&self) -> DbResult<Vec<OrgSsoConfigWithSecret>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_id, provider_type,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE enabled = 1
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter()
            .map(Self::parse_config_with_secret)
            .collect::<DbResult<Vec<_>>>()
    }

    async fn any_enabled(&self) -> DbResult<bool> {
        let result: (i32,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM org_sso_configs WHERE enabled = 1)")
                .fetch_one(&self.pool)
                .await?;
        Ok(result.0 != 0)
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

        // Create org_sso_configs table
        sqlx::query(
            r#"
            CREATE TABLE org_sso_configs (
                id TEXT PRIMARY KEY NOT NULL,
                org_id TEXT NOT NULL UNIQUE REFERENCES organizations(id) ON DELETE CASCADE,
                provider_type TEXT NOT NULL DEFAULT 'oidc',
                -- OIDC fields
                issuer TEXT NOT NULL,
                discovery_url TEXT,
                client_id TEXT NOT NULL,
                client_secret_key TEXT NOT NULL,
                redirect_uri TEXT,
                scopes TEXT NOT NULL DEFAULT 'openid email profile',
                identity_claim TEXT NOT NULL DEFAULT 'sub',
                org_claim TEXT,
                groups_claim TEXT,
                -- SAML fields
                saml_metadata_url TEXT,
                saml_idp_entity_id TEXT,
                saml_idp_sso_url TEXT,
                saml_idp_slo_url TEXT,
                saml_idp_certificate TEXT,
                saml_sp_entity_id TEXT,
                saml_name_id_format TEXT,
                saml_sign_requests INTEGER NOT NULL DEFAULT 0,
                saml_sp_private_key_ref TEXT,
                saml_sp_certificate TEXT,
                saml_force_authn INTEGER NOT NULL DEFAULT 0,
                saml_authn_context_class_ref TEXT,
                saml_identity_attribute TEXT,
                saml_email_attribute TEXT,
                saml_name_attribute TEXT,
                saml_groups_attribute TEXT,
                -- JIT provisioning
                provisioning_enabled INTEGER NOT NULL DEFAULT 1,
                create_users INTEGER NOT NULL DEFAULT 1,
                default_team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
                default_org_role TEXT NOT NULL DEFAULT 'member',
                default_team_role TEXT NOT NULL DEFAULT 'member',
                allowed_email_domains TEXT,
                sync_attributes_on_login INTEGER NOT NULL DEFAULT 0,
                sync_memberships_on_login INTEGER NOT NULL DEFAULT 1,
                enforcement_mode TEXT NOT NULL DEFAULT 'optional',
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create org_sso_configs table");

        // Match production indexes
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_org_sso_configs_issuer_enabled
              ON org_sso_configs(issuer, provider_type, enabled) WHERE enabled = 1 AND provider_type = 'oidc'
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create issuer index");

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

    fn make_test_input() -> CreateOrgSsoConfig {
        CreateOrgSsoConfig {
            provider_type: SsoProviderType::Oidc,
            issuer: Some("https://idp.example.com".to_string()),
            client_id: Some("test-client-id".to_string()),
            client_secret: Some("test-secret".to_string()),
            scopes: vec!["openid".to_string(), "email".to_string()],
            identity_claim: "sub".to_string(),
            groups_claim: Some("groups".to_string()),
            allowed_email_domains: vec!["example.com".to_string()],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_create_sso_config() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let input = make_test_input();
        let config = repo
            .create(org_id, input, Some("secret-key-ref"), None)
            .await
            .expect("Failed to create SSO config");

        assert_eq!(config.org_id, org_id);
        assert_eq!(config.issuer, Some("https://idp.example.com".to_string()));
        assert_eq!(config.client_id, Some("test-client-id".to_string()));
        assert_eq!(config.scopes, vec!["openid", "email"]);
        assert_eq!(config.groups_claim, Some("groups".to_string()));
        assert_eq!(config.allowed_email_domains, vec!["example.com"]);
        assert!(config.enabled);
    }

    #[tokio::test]
    async fn test_create_duplicate_fails() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let input = make_test_input();
        repo.create(org_id, input.clone(), Some("key1"), None)
            .await
            .expect("First create should succeed");

        let result = repo.create(org_id, input, Some("key2"), None).await;
        assert!(matches!(result, Err(DbError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let input = make_test_input();
        let created = repo
            .create(org_id, input, Some("key"), None)
            .await
            .expect("Failed to create");

        let fetched = repo
            .get_by_id(created.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.issuer, created.issuer);
    }

    #[tokio::test]
    async fn test_get_by_org_id() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let input = make_test_input();
        let created = repo
            .create(org_id, input, Some("key"), None)
            .await
            .expect("Failed to create");

        let fetched = repo
            .get_by_org_id(org_id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.id, created.id);
    }

    #[tokio::test]
    async fn test_get_with_secret() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let input = make_test_input();
        let created = repo
            .create(org_id, input, Some("my-secret-key"), None)
            .await
            .expect("Failed to create");

        let fetched = repo
            .get_with_secret(created.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");

        assert_eq!(fetched.client_secret_key, Some("my-secret-key".to_string()));
        assert_eq!(fetched.config.id, created.id);
    }

    #[tokio::test]
    async fn test_update_config() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let team_id = create_test_team(&pool, org_id, "new-team").await;
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let input = make_test_input();
        let created = repo
            .create(org_id, input, Some("old-key"), None)
            .await
            .expect("Failed to create");

        let update = UpdateOrgSsoConfig {
            issuer: Some("https://new-idp.example.com".to_string()),
            default_team_id: Some(Some(team_id)),
            enabled: Some(false),
            ..Default::default()
        };

        let updated = repo
            .update(created.id, update, Some("new-key"), None)
            .await
            .expect("Failed to update");

        assert_eq!(
            updated.issuer,
            Some("https://new-idp.example.com".to_string())
        );
        assert_eq!(updated.default_team_id, Some(team_id));
        assert!(!updated.enabled);

        // Verify secret key was updated
        let with_secret = repo
            .get_with_secret(created.id)
            .await
            .expect("Failed to get")
            .expect("Should exist");
        assert_eq!(with_secret.client_secret_key, Some("new-key".to_string()));
    }

    #[tokio::test]
    async fn test_delete_config() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let input = make_test_input();
        let created = repo
            .create(org_id, input, Some("key"), None)
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
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let result = repo.delete(Uuid::new_v4()).await;
        assert!(matches!(result, Err(DbError::NotFound)));
    }

    #[tokio::test]
    async fn test_find_by_email_domain() {
        let pool = create_test_pool().await;
        let org_id = create_test_org(&pool, "test-org").await;
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let mut input = make_test_input();
        input.allowed_email_domains = vec!["acme.com".to_string(), "acme.io".to_string()];

        repo.create(org_id, input, Some("key"), None)
            .await
            .expect("Failed to create");

        // Should find by domain
        let found = repo
            .find_by_email_domain("acme.com")
            .await
            .expect("Failed to search")
            .expect("Should find");
        assert_eq!(found.org_id, org_id);

        // Should also find by alternate domain
        let found = repo
            .find_by_email_domain("acme.io")
            .await
            .expect("Failed to search")
            .expect("Should find");
        assert_eq!(found.org_id, org_id);

        // Should not find unknown domain
        let not_found = repo
            .find_by_email_domain("other.com")
            .await
            .expect("Failed to search");
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_list_enabled() {
        let pool = create_test_pool().await;
        let org1_id = create_test_org(&pool, "org-1").await;
        let org2_id = create_test_org(&pool, "org-2").await;
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let mut input = make_test_input();
        input.enabled = true;
        repo.create(org1_id, input.clone(), Some("key1"), None)
            .await
            .expect("Failed to create");

        input.enabled = false;
        repo.create(org2_id, input, Some("key2"), None)
            .await
            .expect("Failed to create");

        let enabled = repo.list_enabled().await.expect("Failed to list enabled");
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].config.org_id, org1_id);
    }

    #[tokio::test]
    async fn test_find_enabled_oidc_by_issuer() {
        let pool = create_test_pool().await;
        let org1_id = create_test_org(&pool, "org-1").await;
        let org2_id = create_test_org(&pool, "org-2").await;
        let org3_id = create_test_org(&pool, "org-3").await;
        let repo = SqliteOrgSsoConfigRepo::new(pool);

        let issuer = "https://idp.acme.com";

        // Org1: enabled OIDC with matching issuer
        let mut input = make_test_input();
        input.issuer = Some(issuer.to_string());
        input.enabled = true;
        repo.create(org1_id, input, Some("key1"), None)
            .await
            .expect("Failed to create");

        // Org2: enabled OIDC with different issuer
        let mut input2 = make_test_input();
        input2.issuer = Some("https://idp.other.com".to_string());
        input2.enabled = true;
        repo.create(org2_id, input2, Some("key2"), None)
            .await
            .expect("Failed to create");

        // Org3: disabled OIDC with matching issuer
        let mut input3 = make_test_input();
        input3.issuer = Some(issuer.to_string());
        input3.enabled = false;
        repo.create(org3_id, input3, Some("key3"), None)
            .await
            .expect("Failed to create");

        // Should only find org1 (enabled + matching issuer)
        let found = repo
            .find_enabled_oidc_by_issuer(issuer)
            .await
            .expect("Failed to search");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].org_id, org1_id);

        // Different issuer should find org2
        let found = repo
            .find_enabled_oidc_by_issuer("https://idp.other.com")
            .await
            .expect("Failed to search");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].org_id, org2_id);

        // Unknown issuer returns empty
        let found = repo
            .find_enabled_oidc_by_issuer("https://unknown.com")
            .await
            .expect("Failed to search");
        assert!(found.is_empty());
    }
}
