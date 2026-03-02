//! Multi-authenticator registry for per-organization SSO.
//!
//! This module provides an `OidcAuthenticatorRegistry` that maps organization IDs
//! to their respective `OidcAuthenticator` instances, enabling per-organization
//! SSO configuration in multi-tenant deployments.
//!
//! # Usage
//!
//! The registry is initialized at startup from the `org_sso_configs` table:
//!
//! ```rust,ignore
//! let registry = OidcAuthenticatorRegistry::initialize_from_db(
//!     &org_sso_config_service,
//!     secret_manager.as_ref(),
//!     session_store,
//!     default_session_config,
//!     default_redirect_uri,
//! ).await?;
//!
//! // Look up authenticator for an organization
//! if let Some(auth) = registry.get(org_id) {
//!     let (auth_url, state) = auth.authorization_url(return_to).await?;
//! }
//! ```

use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;
use uuid::Uuid;

use super::{
    oidc::OidcAuthenticator,
    session_store::{AuthorizationState, SharedSessionStore},
};
use crate::{
    config::{OidcAuthConfig, ProvisioningConfig, SessionConfig},
    secrets::SecretManager,
    services::{OrgSsoConfigError, OrgSsoConfigService, OrgSsoConfigWithClientSecret},
};

/// Error type for registry operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Failed to load SSO configs: {0}")]
    LoadError(String),

    #[error("Failed to create authenticator for org {org_id}: {message}")]
    AuthenticatorCreation { org_id: Uuid, message: String },
}

impl From<OrgSsoConfigError> for RegistryError {
    fn from(e: OrgSsoConfigError) -> Self {
        RegistryError::LoadError(e.to_string())
    }
}

/// Registry of OIDC authenticators for per-organization SSO.
///
/// Each organization can have its own OIDC configuration (issuer, client credentials,
/// claim mappings, etc.) stored in the database. This registry manages the lifecycle
/// of authenticator instances for each organization.
///
/// All authenticators share the same session store to enable cross-org session
/// management and consistent session handling.
pub struct OidcAuthenticatorRegistry {
    /// Map of org_id -> OidcAuthenticator
    authenticators: Arc<RwLock<HashMap<Uuid, Arc<OidcAuthenticator>>>>,
    /// Shared session store used by all authenticators
    session_store: SharedSessionStore,
    /// Default session config for authenticators that don't specify one
    default_session_config: SessionConfig,
    /// Default redirect URI used when org config doesn't specify one
    default_redirect_uri: Option<String>,
}

impl OidcAuthenticatorRegistry {
    /// Create a new empty registry.
    pub fn new(
        session_store: SharedSessionStore,
        default_session_config: SessionConfig,
        default_redirect_uri: Option<String>,
    ) -> Self {
        Self {
            authenticators: Arc::new(RwLock::new(HashMap::new())),
            session_store,
            default_session_config,
            default_redirect_uri,
        }
    }

    /// Initialize the registry by loading all enabled SSO configs from the database.
    ///
    /// This is typically called at application startup.
    pub async fn initialize_from_db(
        service: &OrgSsoConfigService,
        secret_manager: &dyn SecretManager,
        session_store: SharedSessionStore,
        default_session_config: SessionConfig,
        default_redirect_uri: Option<String>,
    ) -> Result<Self, RegistryError> {
        let registry = Self::new(session_store, default_session_config, default_redirect_uri);

        // Load only OIDC SSO configs (not SAML — those use SamlAuthenticatorRegistry)
        let configs = service
            .list_enabled_with_secrets_by_type(secret_manager, crate::models::SsoProviderType::Oidc)
            .await?;

        for config in configs {
            let org_id = config.config.org_id;
            match registry.create_authenticator_from_config(&config) {
                Ok(auth) => {
                    registry.register(org_id, auth).await;
                    tracing::debug!(org_id = %org_id, "Registered SSO authenticator");
                }
                Err(e) => {
                    tracing::warn!(
                        org_id = %org_id,
                        error = %e,
                        "Failed to create authenticator for org, skipping"
                    );
                }
            }
        }

        Ok(registry)
    }

    /// Create an OidcAuthenticator from an org SSO config.
    fn create_authenticator_from_config(
        &self,
        config: &OrgSsoConfigWithClientSecret,
    ) -> Result<OidcAuthenticator, RegistryError> {
        let oidc_config = config.to_oidc_auth_config(
            self.default_redirect_uri.as_deref().unwrap_or(""),
            &self.default_session_config,
        );

        Ok(OidcAuthenticator::new(
            oidc_config,
            self.session_store.clone(),
        ))
    }

    /// Get the authenticator for an organization.
    pub async fn get(&self, org_id: Uuid) -> Option<Arc<OidcAuthenticator>> {
        let authenticators = self.authenticators.read().await;
        authenticators.get(&org_id).cloned()
    }

    /// Register an authenticator for an organization.
    ///
    /// If an authenticator already exists for this org, it will be replaced.
    pub async fn register(&self, org_id: Uuid, authenticator: OidcAuthenticator) {
        let mut authenticators = self.authenticators.write().await;
        authenticators.insert(org_id, Arc::new(authenticator));
    }

    /// Remove the authenticator for an organization.
    ///
    /// Returns the removed authenticator if one existed.
    pub async fn remove(&self, org_id: Uuid) -> Option<Arc<OidcAuthenticator>> {
        let mut authenticators = self.authenticators.write().await;
        authenticators.remove(&org_id)
    }

    /// Add or update an authenticator from an org SSO config.
    ///
    /// This is useful when an org SSO config is created or updated at runtime.
    pub async fn register_from_config(
        &self,
        config: &OrgSsoConfigWithClientSecret,
    ) -> Result<(), RegistryError> {
        let org_id = config.config.org_id;
        let authenticator = self.create_authenticator_from_config(config)?;
        self.register(org_id, authenticator).await;
        Ok(())
    }

    /// List all registered organization IDs.
    pub async fn list_orgs(&self) -> Vec<Uuid> {
        let authenticators = self.authenticators.read().await;
        authenticators.keys().copied().collect()
    }

    /// Get the number of registered authenticators.
    pub async fn len(&self) -> usize {
        let authenticators = self.authenticators.read().await;
        authenticators.len()
    }

    /// Check if the registry is empty.
    pub async fn is_empty(&self) -> bool {
        let authenticators = self.authenticators.read().await;
        authenticators.is_empty()
    }

    /// Get the shared session store.
    pub fn session_store(&self) -> &SharedSessionStore {
        &self.session_store
    }

    /// Peek at pending authorization state to get the org_id.
    ///
    /// This is used in the callback to determine which authenticator to use
    /// before the state is consumed.
    pub async fn peek_auth_state(
        &self,
        state: &str,
    ) -> Result<Option<AuthorizationState>, RegistryError> {
        self.session_store
            .peek_auth_state(state)
            .await
            .map_err(|e| RegistryError::LoadError(format!("Failed to peek auth state: {}", e)))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversion helper
// ─────────────────────────────────────────────────────────────────────────────

impl OrgSsoConfigWithClientSecret {
    /// Convert an org SSO config to an OidcAuthConfig.
    ///
    /// This maps the database model fields to the config structure expected
    /// by `OidcAuthenticator`.
    pub fn to_oidc_auth_config(
        &self,
        default_redirect_uri: &str,
        default_session_config: &SessionConfig,
    ) -> OidcAuthConfig {
        OidcAuthConfig {
            // These fields are required for OIDC configs - unwrap them
            // (should always be present when provider_type == oidc)
            issuer: self.config.issuer.clone().unwrap_or_default(),
            discovery_url: self.config.discovery_url.clone(),
            client_id: self.config.client_id.clone().unwrap_or_default(),
            client_secret: self.client_secret.clone().unwrap_or_default(),
            redirect_uri: self
                .config
                .redirect_uri
                .clone()
                .unwrap_or_else(|| default_redirect_uri.to_string()),
            scopes: self.config.scopes.clone(),
            identity_claim: self
                .config
                .identity_claim
                .clone()
                .unwrap_or_else(|| "sub".to_string()),
            org_claim: self.config.org_claim.clone(),
            groups_claim: self.config.groups_claim.clone(),
            session: default_session_config.clone(),
            provisioning: ProvisioningConfig {
                enabled: self.config.provisioning_enabled,
                create_users: self.config.create_users,
                // Bind to this specific organization
                organization_id: Some(self.config.org_id.to_string()),
                default_team_id: self.config.default_team_id.map(|t| t.to_string()),
                default_org_role: self.config.default_org_role.clone(),
                default_team_role: self.config.default_team_role.clone(),
                allowed_email_domains: self.config.allowed_email_domains.clone(),
                sync_attributes_on_login: self.config.sync_attributes_on_login,
                sync_memberships_on_login: self.config.sync_memberships_on_login,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::{
        auth::session_store::MemorySessionStore,
        models::{OrgSsoConfig, SsoEnforcementMode, SsoProviderType},
    };

    fn create_test_session_store() -> SharedSessionStore {
        Arc::new(MemorySessionStore::new())
    }

    fn create_test_config(org_id: Uuid) -> OrgSsoConfigWithClientSecret {
        OrgSsoConfigWithClientSecret {
            config: OrgSsoConfig {
                id: Uuid::new_v4(),
                org_id,
                provider_type: SsoProviderType::Oidc,
                // OIDC fields
                issuer: Some("https://auth.example.com".to_string()),
                discovery_url: None,
                client_id: Some("test-client-id".to_string()),
                redirect_uri: Some("https://gateway.example.com/auth/callback".to_string()),
                scopes: vec!["openid".to_string(), "email".to_string()],
                identity_claim: Some("sub".to_string()),
                org_claim: None,
                groups_claim: Some("groups".to_string()),
                // SAML fields (not used for OIDC)
                saml_metadata_url: None,
                saml_idp_entity_id: None,
                saml_idp_sso_url: None,
                saml_idp_slo_url: None,
                saml_idp_certificate: None,
                saml_sp_entity_id: None,
                saml_name_id_format: None,
                saml_sign_requests: false,
                saml_sp_certificate: None,
                saml_force_authn: false,
                saml_authn_context_class_ref: None,
                saml_identity_attribute: None,
                saml_email_attribute: None,
                saml_name_attribute: None,
                saml_groups_attribute: None,
                // JIT provisioning
                provisioning_enabled: true,
                create_users: true,
                default_team_id: None,
                default_org_role: "member".to_string(),
                default_team_role: "member".to_string(),
                allowed_email_domains: vec![],
                sync_attributes_on_login: false,
                sync_memberships_on_login: true,
                enforcement_mode: SsoEnforcementMode::Optional,
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            client_secret: Some("test-client-secret".to_string()),
            saml_sp_private_key: None,
        }
    }

    #[test]
    fn test_to_oidc_auth_config() {
        let org_id = Uuid::new_v4();
        let config = create_test_config(org_id);
        let default_session = SessionConfig::default();

        let oidc_config =
            config.to_oidc_auth_config("https://default.example.com/callback", &default_session);

        assert_eq!(oidc_config.issuer, "https://auth.example.com");
        assert_eq!(oidc_config.client_id, "test-client-id");
        assert_eq!(oidc_config.client_secret, "test-client-secret");
        assert_eq!(
            oidc_config.redirect_uri,
            "https://gateway.example.com/auth/callback"
        );
        assert_eq!(oidc_config.identity_claim, "sub");
        assert_eq!(oidc_config.groups_claim, Some("groups".to_string()));
        assert!(oidc_config.provisioning.enabled);
        assert!(oidc_config.provisioning.create_users);
        assert_eq!(
            oidc_config.provisioning.organization_id,
            Some(org_id.to_string())
        );
    }

    #[test]
    fn test_to_oidc_auth_config_uses_default_redirect() {
        let org_id = Uuid::new_v4();
        let mut config = create_test_config(org_id);
        config.config.redirect_uri = None; // Clear the redirect URI
        let default_session = SessionConfig::default();

        let oidc_config =
            config.to_oidc_auth_config("https://default.example.com/callback", &default_session);

        assert_eq!(
            oidc_config.redirect_uri,
            "https://default.example.com/callback"
        );
    }

    #[tokio::test]
    async fn test_registry_register_and_get() {
        let session_store = create_test_session_store();
        let registry =
            OidcAuthenticatorRegistry::new(session_store.clone(), SessionConfig::default(), None);

        let org_id = Uuid::new_v4();
        let config = create_test_config(org_id);

        // Register from config
        registry.register_from_config(&config).await.unwrap();

        // Should be able to get it
        let auth = registry.get(org_id).await;
        assert!(auth.is_some());

        // Different org should return None
        let other_org = Uuid::new_v4();
        assert!(registry.get(other_org).await.is_none());
    }

    #[tokio::test]
    async fn test_registry_remove() {
        let session_store = create_test_session_store();
        let registry =
            OidcAuthenticatorRegistry::new(session_store.clone(), SessionConfig::default(), None);

        let org_id = Uuid::new_v4();
        let config = create_test_config(org_id);

        registry.register_from_config(&config).await.unwrap();
        assert!(registry.get(org_id).await.is_some());

        // Remove it
        let removed = registry.remove(org_id).await;
        assert!(removed.is_some());

        // Should be gone now
        assert!(registry.get(org_id).await.is_none());
    }

    #[tokio::test]
    async fn test_registry_list_orgs() {
        let session_store = create_test_session_store();
        let registry =
            OidcAuthenticatorRegistry::new(session_store.clone(), SessionConfig::default(), None);

        let org1 = Uuid::new_v4();
        let org2 = Uuid::new_v4();

        registry
            .register_from_config(&create_test_config(org1))
            .await
            .unwrap();
        registry
            .register_from_config(&create_test_config(org2))
            .await
            .unwrap();

        let orgs = registry.list_orgs().await;
        assert_eq!(orgs.len(), 2);
        assert!(orgs.contains(&org1));
        assert!(orgs.contains(&org2));
    }

    #[tokio::test]
    async fn test_registry_len_and_is_empty() {
        let session_store = create_test_session_store();
        let registry =
            OidcAuthenticatorRegistry::new(session_store.clone(), SessionConfig::default(), None);

        assert!(registry.is_empty().await);
        assert_eq!(registry.len().await, 0);

        let org_id = Uuid::new_v4();
        registry
            .register_from_config(&create_test_config(org_id))
            .await
            .unwrap();

        assert!(!registry.is_empty().await);
        assert_eq!(registry.len().await, 1);
    }
}
