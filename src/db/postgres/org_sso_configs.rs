use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

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

pub struct PostgresOrgSsoConfigRepo {
    write_pool: PgPool,
    read_pool: PgPool,
}

impl PostgresOrgSsoConfigRepo {
    pub fn new(write_pool: PgPool, read_pool: Option<PgPool>) -> Self {
        let read_pool = read_pool.unwrap_or_else(|| write_pool.clone());
        Self {
            write_pool,
            read_pool,
        }
    }

    /// Parse an OrgSsoConfig from a database row.
    fn parse_config(row: &sqlx::postgres::PgRow) -> OrgSsoConfig {
        let scopes_str: String = row.get("scopes");
        let scopes: Vec<String> = scopes_str.split_whitespace().map(String::from).collect();

        let allowed_domains_json: Option<serde_json::Value> = row.get("allowed_email_domains");
        let allowed_email_domains: Vec<String> = allowed_domains_json
            .map(|json| serde_json::from_value(json).unwrap_or_default())
            .unwrap_or_default();

        let provider_type_str: String = row.get("provider_type");
        let provider_type = provider_type_str
            .parse::<SsoProviderType>()
            .unwrap_or_default();

        let enforcement_mode_str: String = row.get("enforcement_mode");
        let enforcement_mode = enforcement_mode_str
            .parse::<SsoEnforcementMode>()
            .unwrap_or_default();

        OrgSsoConfig {
            id: row.get("id"),
            org_id: row.get("org_id"),
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
            saml_sign_requests: row.get("saml_sign_requests"),
            saml_sp_certificate: row.get("saml_sp_certificate"),
            saml_force_authn: row.get("saml_force_authn"),
            saml_authn_context_class_ref: row.get("saml_authn_context_class_ref"),
            saml_identity_attribute: row.get("saml_identity_attribute"),
            saml_email_attribute: row.get("saml_email_attribute"),
            saml_name_attribute: row.get("saml_name_attribute"),
            saml_groups_attribute: row.get("saml_groups_attribute"),
            // JIT provisioning
            provisioning_enabled: row.get("provisioning_enabled"),
            create_users: row.get("create_users"),
            default_team_id: row.get("default_team_id"),
            default_org_role: row.get("default_org_role"),
            default_team_role: row.get("default_team_role"),
            allowed_email_domains,
            sync_attributes_on_login: row.get("sync_attributes_on_login"),
            sync_memberships_on_login: row.get("sync_memberships_on_login"),
            enforcement_mode,
            enabled: row.get("enabled"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    /// Parse an OrgSsoConfigWithSecret from a database row.
    fn parse_config_with_secret(row: &sqlx::postgres::PgRow) -> OrgSsoConfigWithSecret {
        let config = Self::parse_config(row);
        let client_secret_key: Option<String> = row.get("client_secret_key");
        let saml_sp_private_key_ref: Option<String> = row.get("saml_sp_private_key_ref");
        OrgSsoConfigWithSecret {
            config,
            client_secret_key,
            saml_sp_private_key_ref,
        }
    }
}

#[async_trait]
impl OrgSsoConfigRepo for PostgresOrgSsoConfigRepo {
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgSsoConfig,
        client_secret_key: Option<&str>,
        saml_sp_private_key_ref: Option<&str>,
    ) -> DbResult<OrgSsoConfig> {
        let scopes_str = input.scopes.join(" ");
        let allowed_domains_json: Option<serde_json::Value> =
            if input.allowed_email_domains.is_empty() {
                None
            } else {
                Some(serde_json::to_value(&input.allowed_email_domains).unwrap_or_default())
            };

        let row = sqlx::query(
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
                enforcement_mode, enabled
            )
            VALUES ($1, $2, $3::sso_provider_type, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37::sso_enforcement_mode, $38)
            RETURNING id, org_id, provider_type::text,
                      issuer, discovery_url, client_id, client_secret_key,
                      redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                      saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                      saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                      saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                      saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                      saml_name_attribute, saml_groups_attribute,
                      provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                      allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                      enforcement_mode::text, enabled, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(org_id)
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
        .bind(input.saml_sign_requests)
        .bind(saml_sp_private_key_ref)
        .bind(&input.saml_sp_certificate)
        .bind(input.saml_force_authn)
        .bind(&input.saml_authn_context_class_ref)
        .bind(&input.saml_identity_attribute)
        .bind(&input.saml_email_attribute)
        .bind(&input.saml_name_attribute)
        .bind(&input.saml_groups_attribute)
        // JIT provisioning
        .bind(input.provisioning_enabled)
        .bind(input.create_users)
        .bind(input.default_team_id)
        .bind(&input.default_org_role)
        .bind(&input.default_team_role)
        .bind(&allowed_domains_json)
        .bind(input.sync_attributes_on_login)
        .bind(input.sync_memberships_on_login)
        .bind(input.enforcement_mode.to_string())
        .bind(input.enabled)
        .fetch_one(&self.write_pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                DbError::Conflict("Organization already has an SSO configuration".into())
            }
            _ => DbError::from(e),
        })?;

        Ok(Self::parse_config(&row))
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgSsoConfig>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, provider_type::text,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode::text, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Self::parse_config(&row)))
    }

    async fn get_by_org_id(&self, org_id: Uuid) -> DbResult<Option<OrgSsoConfig>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, provider_type::text,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode::text, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE org_id = $1
            "#,
        )
        .bind(org_id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Self::parse_config(&row)))
    }

    async fn get_with_secret(&self, id: Uuid) -> DbResult<Option<OrgSsoConfigWithSecret>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, provider_type::text,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode::text, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Self::parse_config_with_secret(&row)))
    }

    async fn get_with_secret_by_org_id(
        &self,
        org_id: Uuid,
    ) -> DbResult<Option<OrgSsoConfigWithSecret>> {
        let result = sqlx::query(
            r#"
            SELECT id, org_id, provider_type::text,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode::text, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE org_id = $1
            "#,
        )
        .bind(org_id)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Self::parse_config_with_secret(&row)))
    }

    async fn update(
        &self,
        id: Uuid,
        input: UpdateOrgSsoConfig,
        client_secret_key: Option<&str>,
        saml_sp_private_key_ref: Option<&str>,
    ) -> DbResult<OrgSsoConfig> {
        // Fetch existing record to fill in missing fields
        let existing = self.get_by_id(id).await?.ok_or(DbError::NotFound)?;
        let existing_with_secret = self.get_with_secret(id).await?.ok_or(DbError::NotFound)?;

        let scopes_str = input
            .scopes
            .as_ref()
            .map(|s| s.join(" "))
            .unwrap_or_else(|| existing.scopes.join(" "));

        let allowed_domains_json: Option<serde_json::Value> = input
            .allowed_email_domains
            .as_ref()
            .map(|domains| {
                if domains.is_empty() {
                    None
                } else {
                    Some(serde_json::to_value(domains).unwrap_or_default())
                }
            })
            .unwrap_or_else(|| {
                if existing.allowed_email_domains.is_empty() {
                    None
                } else {
                    Some(serde_json::to_value(&existing.allowed_email_domains).unwrap_or_default())
                }
            });

        let row = sqlx::query(
            r#"
            UPDATE org_sso_configs SET
                provider_type = $1::sso_provider_type, issuer = $2, discovery_url = $3, client_id = $4, client_secret_key = $5,
                redirect_uri = $6, scopes = $7, identity_claim = $8, org_claim = $9, groups_claim = $10,
                -- SAML fields
                saml_metadata_url = $11, saml_idp_entity_id = $12, saml_idp_sso_url = $13, saml_idp_slo_url = $14,
                saml_idp_certificate = $15, saml_sp_entity_id = $16, saml_name_id_format = $17,
                saml_sign_requests = $18, saml_sp_private_key_ref = $19, saml_sp_certificate = $20, saml_force_authn = $21,
                saml_authn_context_class_ref = $22, saml_identity_attribute = $23, saml_email_attribute = $24,
                saml_name_attribute = $25, saml_groups_attribute = $26,
                -- JIT provisioning
                provisioning_enabled = $27, create_users = $28, default_team_id = $29,
                default_org_role = $30, default_team_role = $31, allowed_email_domains = $32,
                sync_attributes_on_login = $33, sync_memberships_on_login = $34,
                enforcement_mode = $35::sso_enforcement_mode, enabled = $36, updated_at = NOW()
            WHERE id = $37
            RETURNING id, org_id, provider_type::text,
                      issuer, discovery_url, client_id, client_secret_key,
                      redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                      saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                      saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                      saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                      saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                      saml_name_attribute, saml_groups_attribute,
                      provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                      allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                      enforcement_mode::text, enabled, created_at, updated_at
            "#,
        )
        .bind(input.provider_type.unwrap_or(existing.provider_type).to_string())
        .bind(input.issuer.or(existing.issuer))
        .bind(input.discovery_url.unwrap_or(existing.discovery_url.clone()))
        .bind(input.client_id.or(existing.client_id))
        .bind(client_secret_key.map(String::from).or(existing_with_secret.client_secret_key))
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
        .bind(input.saml_sign_requests.unwrap_or(existing.saml_sign_requests))
        .bind(
            saml_sp_private_key_ref
                .map(String::from)
                .or(existing_with_secret.saml_sp_private_key_ref),
        )
        .bind(input.saml_sp_certificate.unwrap_or(existing.saml_sp_certificate.clone()))
        .bind(input.saml_force_authn.unwrap_or(existing.saml_force_authn))
        .bind(input.saml_authn_context_class_ref.unwrap_or(existing.saml_authn_context_class_ref.clone()))
        .bind(input.saml_identity_attribute.unwrap_or(existing.saml_identity_attribute.clone()))
        .bind(input.saml_email_attribute.unwrap_or(existing.saml_email_attribute.clone()))
        .bind(input.saml_name_attribute.unwrap_or(existing.saml_name_attribute.clone()))
        .bind(input.saml_groups_attribute.unwrap_or(existing.saml_groups_attribute.clone()))
        // JIT provisioning
        .bind(input.provisioning_enabled.unwrap_or(existing.provisioning_enabled))
        .bind(input.create_users.unwrap_or(existing.create_users))
        .bind(input.default_team_id.unwrap_or(existing.default_team_id))
        .bind(input.default_org_role.unwrap_or(existing.default_org_role))
        .bind(input.default_team_role.unwrap_or(existing.default_team_role))
        .bind(&allowed_domains_json)
        .bind(input.sync_attributes_on_login.unwrap_or(existing.sync_attributes_on_login))
        .bind(input.sync_memberships_on_login.unwrap_or(existing.sync_memberships_on_login))
        .bind(input.enforcement_mode.unwrap_or(existing.enforcement_mode).to_string())
        .bind(input.enabled.unwrap_or(existing.enabled))
        .bind(id)
        .fetch_one(&self.write_pool)
        .await?;

        Ok(Self::parse_config(&row))
    }

    async fn delete(&self, id: Uuid) -> DbResult<()> {
        let result = sqlx::query("DELETE FROM org_sso_configs WHERE id = $1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }

        Ok(())
    }

    async fn find_enabled_oidc_by_issuer(&self, issuer: &str) -> DbResult<Vec<OrgSsoConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_id, provider_type::text,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode::text, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE enabled = TRUE AND provider_type = 'oidc'::sso_provider_type AND issuer = $1
            "#,
        )
        .bind(issuer)
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows.iter().map(Self::parse_config).collect())
    }

    async fn find_by_email_domain(&self, domain: &str) -> DbResult<Option<OrgSsoConfig>> {
        // PostgreSQL uses jsonb ? operator or jsonb_array_elements to search JSON arrays
        let result = sqlx::query(
            r#"
            SELECT id, org_id, provider_type::text,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode::text, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE enabled = TRUE AND allowed_email_domains ? $1
            LIMIT 1
            "#,
        )
        .bind(domain)
        .fetch_optional(&self.read_pool)
        .await?;

        Ok(result.map(|row| Self::parse_config(&row)))
    }

    async fn list_enabled(&self) -> DbResult<Vec<OrgSsoConfigWithSecret>> {
        let rows = sqlx::query(
            r#"
            SELECT id, org_id, provider_type::text,
                   issuer, discovery_url, client_id, client_secret_key,
                   redirect_uri, scopes, identity_claim, org_claim, groups_claim,
                   saml_metadata_url, saml_idp_entity_id, saml_idp_sso_url, saml_idp_slo_url,
                   saml_idp_certificate, saml_sp_entity_id, saml_name_id_format,
                   saml_sign_requests, saml_sp_private_key_ref, saml_sp_certificate, saml_force_authn,
                   saml_authn_context_class_ref, saml_identity_attribute, saml_email_attribute,
                   saml_name_attribute, saml_groups_attribute,
                   provisioning_enabled, create_users, default_team_id, default_org_role, default_team_role,
                   allowed_email_domains, sync_attributes_on_login, sync_memberships_on_login,
                   enforcement_mode::text, enabled, created_at, updated_at
            FROM org_sso_configs
            WHERE enabled = TRUE
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.read_pool)
        .await?;

        Ok(rows.iter().map(Self::parse_config_with_secret).collect())
    }

    async fn any_enabled(&self) -> DbResult<bool> {
        let result: (bool,) =
            sqlx::query_as("SELECT EXISTS(SELECT 1 FROM org_sso_configs WHERE enabled = true)")
                .fetch_one(&self.read_pool)
                .await?;
        Ok(result.0)
    }
}
