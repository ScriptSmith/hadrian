//! SAML authenticator registry for per-organization SSO.
//!
//! This module provides a `SamlAuthenticatorRegistry` that maps organization IDs
//! to their respective `SamlAuthenticator` instances, enabling per-organization
//! SAML 2.0 SSO configuration in multi-tenant deployments.
//!
//! # Usage
//!
//! The registry is initialized at startup from the `org_sso_configs` table:
//!
//! ```rust,ignore
//! let registry = SamlAuthenticatorRegistry::initialize_from_db(
//!     &org_sso_config_service,
//!     secret_manager.as_ref(),
//!     session_store,
//!     default_session_config,
//!     default_acs_url,
//! ).await?;
//!
//! // Look up authenticator for an organization
//! if let Some(auth) = registry.get(org_id) {
//!     let (auth_url, state) = auth.authorization_url(return_to).await?;
//! }
//! ```

use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{
    registry::RegistryError,
    saml::{SamlAuthConfig, SamlAuthenticator, derive_acs_url_from_entity_id},
    session_store::{AuthorizationState, OidcSession, SharedSessionStore},
};
use crate::{
    config::SessionConfig,
    models::SsoProviderType,
    secrets::SecretManager,
    services::{OrgSsoConfigService, OrgSsoConfigWithClientSecret},
};

/// Registry of SAML authenticators for per-organization SSO.
///
/// Each organization can have its own SAML configuration (IdP entity ID, certificates,
/// attribute mappings, etc.) stored in the database. This registry manages the lifecycle
/// of authenticator instances for each organization.
///
/// All authenticators share the same session store to enable cross-org session
/// management and consistent session handling.
pub struct SamlAuthenticatorRegistry {
    /// Map of org_id -> SamlAuthenticator
    authenticators: Arc<RwLock<HashMap<Uuid, Arc<SamlAuthenticator>>>>,
    /// Shared session store used by all authenticators
    session_store: SharedSessionStore,
    /// Default session config for authenticators that don't specify one
    default_session_config: SessionConfig,
    /// Default ACS URL used when org config doesn't specify one
    default_acs_url: String,
}

impl SamlAuthenticatorRegistry {
    /// Create a new empty registry.
    pub fn new(
        session_store: SharedSessionStore,
        default_session_config: SessionConfig,
        default_acs_url: String,
    ) -> Self {
        Self {
            authenticators: Arc::new(RwLock::new(HashMap::new())),
            session_store,
            default_session_config,
            default_acs_url,
        }
    }

    /// Initialize the registry by loading all enabled SAML SSO configs from the database.
    ///
    /// This is typically called at application startup.
    pub async fn initialize_from_db(
        service: &OrgSsoConfigService,
        secret_manager: &dyn SecretManager,
        session_store: SharedSessionStore,
        default_session_config: SessionConfig,
        default_acs_url: String,
    ) -> Result<Self, RegistryError> {
        let registry = Self::new(session_store, default_session_config, default_acs_url);

        // Load all enabled SAML SSO configs with their secrets
        let configs = service
            .list_enabled_with_secrets_by_type(secret_manager, SsoProviderType::Saml)
            .await?;

        for config in configs {
            let org_id = config.config.org_id;
            match registry.create_authenticator_from_config(&config) {
                Ok(auth) => {
                    registry.register(org_id, auth).await;
                    tracing::debug!(org_id = %org_id, "Registered SAML SSO authenticator");
                }
                Err(e) => {
                    tracing::warn!(
                        org_id = %org_id,
                        error = %e,
                        "Failed to create SAML authenticator for org, skipping"
                    );
                }
            }
        }

        Ok(registry)
    }

    /// Create a SamlAuthenticator from an org SSO config.
    fn create_authenticator_from_config(
        &self,
        config: &OrgSsoConfigWithClientSecret,
    ) -> Result<SamlAuthenticator, RegistryError> {
        let saml_config = config
            .to_saml_auth_config(
                &self.default_acs_url,
                self.default_session_config.clone(),
                config.saml_sp_private_key.clone(),
            )
            .map_err(|e| RegistryError::AuthenticatorCreation {
                org_id: config.config.org_id,
                message: e,
            })?;

        Ok(SamlAuthenticator::new(
            saml_config,
            self.session_store.clone(),
        ))
    }

    /// Get the authenticator for an organization.
    pub async fn get(&self, org_id: Uuid) -> Option<Arc<SamlAuthenticator>> {
        let authenticators = self.authenticators.read().await;
        authenticators.get(&org_id).cloned()
    }

    /// Register an authenticator for an organization.
    ///
    /// If an authenticator already exists for this org, it will be replaced.
    pub async fn register(&self, org_id: Uuid, authenticator: SamlAuthenticator) {
        let mut authenticators = self.authenticators.write().await;
        authenticators.insert(org_id, Arc::new(authenticator));
    }

    /// Remove the authenticator for an organization.
    ///
    /// Returns the removed authenticator if one existed.
    pub async fn remove(&self, org_id: Uuid) -> Option<Arc<SamlAuthenticator>> {
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

    /// Get a session by ID from the shared session store.
    ///
    /// This method performs the following checks:
    /// 1. Verifies the session exists
    /// 2. Checks absolute expiration (`expires_at`)
    /// 3. Checks inactivity timeout (if enhanced sessions are enabled)
    /// 4. Updates `last_activity` timestamp (if enhanced sessions are enabled)
    ///
    /// Returns `Ok(Some(session))` if found and valid, `Ok(None)` if not found or expired,
    /// or `Err` if there was a storage error.
    pub async fn get_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<OidcSession>, RegistryError> {
        let mut session = match self.session_store.get_session(session_id).await {
            Ok(Some(s)) => s,
            Ok(None) => return Ok(None),
            Err(e) => {
                return Err(RegistryError::LoadError(format!(
                    "Failed to get session: {}",
                    e
                )));
            }
        };

        // Check absolute expiration
        if session.is_expired() {
            let _ = self.session_store.delete_session(session_id).await;
            return Ok(None);
        }

        // Check inactivity timeout (Phase 2)
        let enhanced = &self.default_session_config.enhanced;
        if enhanced.enabled && session.is_inactive(enhanced.inactivity_timeout_secs) {
            let _ = self.session_store.delete_session(session_id).await;
            tracing::info!(
                session_id = %session_id,
                external_id = %session.external_id,
                last_activity = ?session.last_activity,
                timeout_secs = enhanced.inactivity_timeout_secs,
                "SAML registry session invalidated due to inactivity"
            );
            return Ok(None);
        }

        // Update last_activity if enhanced sessions enabled
        if enhanced.enabled {
            session.last_activity = Some(Utc::now());
            if let Err(e) = self.session_store.update_session(session.clone()).await {
                // Non-fatal: log but don't fail the request
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to update SAML registry session last_activity"
                );
            }
        }

        Ok(Some(session))
    }

    /// Get the default session config.
    pub fn default_session_config(&self) -> &SessionConfig {
        &self.default_session_config
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
    /// Convert an org SSO config to a SamlAuthConfig.
    ///
    /// This maps the database model fields to the config structure expected
    /// by `SamlAuthenticator`.
    ///
    /// # Arguments
    /// * `default_acs_url` - Default ACS URL if not specified in config
    /// * `session_config` - Session configuration for cookies/timeouts
    /// * `sp_private_key` - The SP private key (already retrieved from secret manager)
    ///
    /// # Returns
    /// * `Ok(SamlAuthConfig)` if all required SAML fields are present
    /// * `Err(String)` if required fields are missing
    pub fn to_saml_auth_config(
        &self,
        default_acs_url: &str,
        session_config: SessionConfig,
        sp_private_key: Option<String>,
    ) -> Result<SamlAuthConfig, String> {
        // Validate required SAML fields
        let idp_entity_id = self
            .config
            .saml_idp_entity_id
            .clone()
            .ok_or("SAML IdP entity ID is required")?;

        let idp_sso_url = self
            .config
            .saml_idp_sso_url
            .clone()
            .ok_or("SAML IdP SSO URL is required")?;

        let idp_certificate = self
            .config
            .saml_idp_certificate
            .clone()
            .ok_or("SAML IdP certificate is required")?;

        let sp_entity_id = self
            .config
            .saml_sp_entity_id
            .clone()
            .ok_or("SAML SP entity ID is required")?;

        // Derive SP ACS URL from SP entity ID
        // If SP entity ID is "http://example.com/saml" or "http://example.com",
        // derive the ACS URL as "http://example.com/auth/saml/acs"
        let sp_acs_url = derive_acs_url_from_entity_id(&sp_entity_id)
            .unwrap_or_else(|| default_acs_url.to_string());

        Ok(SamlAuthConfig {
            idp_entity_id,
            idp_sso_url,
            idp_slo_url: self.config.saml_idp_slo_url.clone(),
            idp_certificate,
            sp_entity_id,
            sp_acs_url,
            name_id_format: self.config.saml_name_id_format.clone(),
            sign_requests: self.config.saml_sign_requests,
            force_authn: self.config.saml_force_authn,
            authn_context_class_ref: self.config.saml_authn_context_class_ref.clone(),
            identity_attribute: self.config.saml_identity_attribute.clone(),
            email_attribute: self.config.saml_email_attribute.clone(),
            name_attribute: self.config.saml_name_attribute.clone(),
            groups_attribute: self.config.saml_groups_attribute.clone(),
            sp_private_key,
            sp_certificate: self.config.saml_sp_certificate.clone(),
            session: session_config,
            metadata_url: self.config.saml_metadata_url.clone(),
        })
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

    fn create_test_saml_config(org_id: Uuid) -> OrgSsoConfigWithClientSecret {
        OrgSsoConfigWithClientSecret {
            config: OrgSsoConfig {
                id: Uuid::new_v4(),
                org_id,
                provider_type: SsoProviderType::Saml,
                // OIDC fields (not used for SAML)
                issuer: None,
                discovery_url: None,
                client_id: None,
                redirect_uri: None,
                scopes: vec![],
                identity_claim: None,
                org_claim: None,
                groups_claim: None,
                // SAML fields
                saml_metadata_url: None,
                saml_idp_entity_id: Some("https://idp.example.com".to_string()),
                saml_idp_sso_url: Some("https://idp.example.com/sso".to_string()),
                saml_idp_slo_url: Some("https://idp.example.com/slo".to_string()),
                saml_idp_certificate: Some(
                    r#"-----BEGIN CERTIFICATE-----
MIICpDCCAYwCCQCqhQ5lgj5e6TANBgkqhkiG9w0BAQsFADAUMRIwEAYDVQQDDAls
b2NhbGhvc3QwHhcNMjEwMTAxMDAwMDAwWhcNMzEwMTAxMDAwMDAwWjAUMRIwEAYD
VQQDDAlsb2NhbGhvc3QwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC7
o5e7hP0eGqMGJDQ3lGBBsY7h0aTzUCNMGd7FmBb0QPsLKbCrL6Wv1Gj7WPV7ht4p
3CwXSQPK1bHDk1L6TRHwkbPqK6VVb8PvL3LHQ7yPvJ7vR3Z4H8BQAA0X3D6L2t8j
5mP7AqNWL3N7kZ8P3R7NZQ3o0E5K9P6c4X8V7kL3T4Z7R5VhJ6L7P3Q0Z6T3R7N5
P6c4X8V7kL3T4Z7R5VhJ6L7P3Q0Z6T3R7N5P6c4X8V7kL3T4Z7R5VhJ6L7P3Q0Z6
-----END CERTIFICATE-----"#
                        .to_string(),
                ),
                saml_sp_entity_id: Some("https://gateway.example.com".to_string()),
                saml_name_id_format: Some(
                    "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string(),
                ),
                saml_sign_requests: false,
                saml_sp_certificate: None,
                saml_force_authn: false,
                saml_authn_context_class_ref: None,
                saml_identity_attribute: None,
                saml_email_attribute: Some("email".to_string()),
                saml_name_attribute: Some("displayName".to_string()),
                saml_groups_attribute: Some("groups".to_string()),
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
            client_secret: None, // Not used for SAML
            saml_sp_private_key: None,
        }
    }

    #[test]
    fn test_to_saml_auth_config() {
        let org_id = Uuid::new_v4();
        let config = create_test_saml_config(org_id);
        let default_session = SessionConfig::default();

        let saml_config = config.to_saml_auth_config(
            "https://gateway.example.com/auth/saml/acs",
            default_session,
            None,
        );

        assert!(saml_config.is_ok());
        let saml_config = saml_config.unwrap();

        assert_eq!(saml_config.idp_entity_id, "https://idp.example.com");
        assert_eq!(saml_config.idp_sso_url, "https://idp.example.com/sso");
        assert_eq!(
            saml_config.idp_slo_url,
            Some("https://idp.example.com/slo".to_string())
        );
        assert_eq!(saml_config.sp_entity_id, "https://gateway.example.com");
        assert_eq!(
            saml_config.sp_acs_url,
            "https://gateway.example.com/auth/saml/acs"
        );
        assert_eq!(
            saml_config.name_id_format,
            Some("urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress".to_string())
        );
        assert_eq!(saml_config.email_attribute, Some("email".to_string()));
        assert_eq!(saml_config.groups_attribute, Some("groups".to_string()));
    }

    #[test]
    fn test_to_saml_auth_config_missing_required_fields() {
        let org_id = Uuid::new_v4();
        let mut config = create_test_saml_config(org_id);
        config.config.saml_idp_entity_id = None; // Remove required field

        let result = config.to_saml_auth_config(
            "https://gateway.example.com/auth/saml/acs",
            SessionConfig::default(),
            None,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("IdP entity ID"));
    }

    #[tokio::test]
    async fn test_registry_register_and_get() {
        let session_store = create_test_session_store();
        let registry = SamlAuthenticatorRegistry::new(
            session_store.clone(),
            SessionConfig::default(),
            "https://gateway.example.com/auth/saml/acs".to_string(),
        );

        let org_id = Uuid::new_v4();
        let config = create_test_saml_config(org_id);

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
        let registry = SamlAuthenticatorRegistry::new(
            session_store.clone(),
            SessionConfig::default(),
            "https://gateway.example.com/auth/saml/acs".to_string(),
        );

        let org_id = Uuid::new_v4();
        let config = create_test_saml_config(org_id);

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
        let registry = SamlAuthenticatorRegistry::new(
            session_store.clone(),
            SessionConfig::default(),
            "https://gateway.example.com/auth/saml/acs".to_string(),
        );

        let org1 = Uuid::new_v4();
        let org2 = Uuid::new_v4();

        registry
            .register_from_config(&create_test_saml_config(org1))
            .await
            .unwrap();
        registry
            .register_from_config(&create_test_saml_config(org2))
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
        let registry = SamlAuthenticatorRegistry::new(
            session_store.clone(),
            SessionConfig::default(),
            "https://gateway.example.com/auth/saml/acs".to_string(),
        );

        assert!(registry.is_empty().await);
        assert_eq!(registry.len().await, 0);

        let org_id = Uuid::new_v4();
        registry
            .register_from_config(&create_test_saml_config(org_id))
            .await
            .unwrap();

        assert!(!registry.is_empty().await);
        assert_eq!(registry.len().await, 1);
    }
}
