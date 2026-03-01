use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    db::error::DbResult,
    models::{CreateOrgSsoConfig, OrgSsoConfig, OrgSsoConfigWithSecret, UpdateOrgSsoConfig},
};

/// Repository for organization SSO configurations.
///
/// Organization SSO configs enable multi-tenant SSO where each organization
/// can configure their own identity provider. This repository handles
/// CRUD operations for these configurations.
///
/// Note: Client secrets are stored separately in a secret manager.
/// The `client_secret_key` field contains a reference to the secret.
#[async_trait]
pub trait OrgSsoConfigRepo: Send + Sync {
    /// Create a new SSO configuration for an organization.
    ///
    /// # Arguments
    /// * `org_id` - The organization this SSO config belongs to
    /// * `input` - The SSO configuration details
    /// * `client_secret_key` - Key reference for the OIDC client secret in the secret manager (for OIDC)
    /// * `saml_sp_private_key_ref` - Key reference for the SAML SP private key (for SAML)
    ///
    /// # Errors
    /// Returns an error if the organization already has an SSO config (one per org).
    async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgSsoConfig,
        client_secret_key: Option<&str>,
        saml_sp_private_key_ref: Option<&str>,
    ) -> DbResult<OrgSsoConfig>;

    /// Get an SSO configuration by its ID.
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgSsoConfig>>;

    /// Get an SSO configuration by organization ID.
    ///
    /// This is the primary lookup method - each organization has at most one SSO config.
    async fn get_by_org_id(&self, org_id: Uuid) -> DbResult<Option<OrgSsoConfig>>;

    /// Get an SSO configuration with its secret key reference.
    ///
    /// Used internally when the client secret needs to be retrieved from the secret manager.
    async fn get_with_secret(&self, id: Uuid) -> DbResult<Option<OrgSsoConfigWithSecret>>;

    /// Get an SSO configuration with its secret key reference by organization ID.
    async fn get_with_secret_by_org_id(
        &self,
        org_id: Uuid,
    ) -> DbResult<Option<OrgSsoConfigWithSecret>>;

    /// Update an SSO configuration.
    ///
    /// # Arguments
    /// * `id` - The SSO config ID
    /// * `input` - The fields to update
    /// * `client_secret_key` - New OIDC secret key reference (if client_secret was updated)
    /// * `saml_sp_private_key_ref` - New SAML SP private key reference (if updated)
    async fn update(
        &self,
        id: Uuid,
        input: UpdateOrgSsoConfig,
        client_secret_key: Option<&str>,
        saml_sp_private_key_ref: Option<&str>,
    ) -> DbResult<OrgSsoConfig>;

    /// Delete an SSO configuration (hard delete).
    async fn delete(&self, id: Uuid) -> DbResult<()>;

    /// Find SSO config by email domain.
    ///
    /// Used for IdP discovery: when a user enters their email, we look up
    /// which organization's SSO config handles that email domain.
    ///
    /// # Arguments
    /// * `domain` - The email domain to search for (e.g., "acme.com")
    ///
    /// # Returns
    /// The SSO config if found and enabled, None otherwise.
    async fn find_by_email_domain(&self, domain: &str) -> DbResult<Option<OrgSsoConfig>>;

    /// Find enabled OIDC SSO configurations by issuer URL.
    ///
    /// Used by the gateway JWT registry to route tokens to the correct per-org
    /// validator based on the `iss` claim.
    async fn find_enabled_oidc_by_issuer(&self, issuer: &str) -> DbResult<Vec<OrgSsoConfig>>;

    /// List all enabled SSO configurations.
    ///
    /// Used for building the authenticator registry on startup.
    async fn list_enabled(&self) -> DbResult<Vec<OrgSsoConfigWithSecret>>;

    /// Check if any enabled SSO configurations exist.
    ///
    /// Used to determine if email discovery should be shown on the login page.
    async fn any_enabled(&self) -> DbResult<bool>;
}
