use std::sync::Arc;

use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult},
    models::{CreateOrgSsoConfig, OrgSsoConfig, SsoProviderType, UpdateOrgSsoConfig},
    secrets::SecretManager,
};

/// Service layer for organization SSO configuration operations.
///
/// This service handles CRUD operations for per-organization SSO configurations,
/// including secure storage of client secrets via the configured secret manager.
#[derive(Clone)]
pub struct OrgSsoConfigService {
    db: Arc<DbPool>,
}

impl OrgSsoConfigService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new SSO configuration for an organization.
    ///
    /// Secrets (OIDC client secret, SAML SP private key) are stored in the
    /// provided secret manager and only key references are stored in the database.
    ///
    /// # Arguments
    /// * `org_id` - The organization this SSO config belongs to
    /// * `input` - The SSO configuration details (including secrets)
    /// * `secret_manager` - Secret manager for storing secrets
    pub async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgSsoConfig,
        secret_manager: &dyn SecretManager,
    ) -> Result<OrgSsoConfig, OrgSsoConfigError> {
        // Store OIDC client secret if provided (for OIDC provider type)
        let client_secret_key = if let Some(ref client_secret) = input.client_secret {
            let key = format!("org-sso/{}/client-secret", org_id);
            secret_manager
                .set(&key, client_secret)
                .await
                .map_err(|e| OrgSsoConfigError::SecretStorage(e.to_string()))?;
            Some(key)
        } else {
            None
        };

        // Store SAML SP private key if provided (for SAML provider type)
        let saml_private_key_ref = if let Some(ref private_key) = input.saml_sp_private_key {
            let key = format!("org-sso/{}/saml-sp-private-key", org_id);
            if let Err(e) = secret_manager.set(&key, private_key).await {
                // Rollback: delete the client secret we just stored
                if let Some(ref client_key) = client_secret_key
                    && let Err(cleanup_err) = secret_manager.delete(client_key).await
                {
                    tracing::warn!(
                        "Failed to clean up orphaned client secret at {} after SAML key storage error: {}",
                        client_key,
                        cleanup_err
                    );
                }
                return Err(OrgSsoConfigError::SecretStorage(e.to_string()));
            }
            Some(key)
        } else {
            None
        };

        // Create the config in the database with the secret key references
        let config = match self
            .db
            .org_sso_configs()
            .create(
                org_id,
                input,
                client_secret_key.as_deref(),
                saml_private_key_ref.as_deref(),
            )
            .await
        {
            Ok(config) => config,
            Err(e) => {
                // Rollback: delete the orphaned secrets
                if let Some(ref client_key) = client_secret_key
                    && let Err(cleanup_err) = secret_manager.delete(client_key).await
                {
                    tracing::warn!(
                        "Failed to clean up orphaned client secret at {} after database error: {}",
                        client_key,
                        cleanup_err
                    );
                }
                if let Some(ref saml_key) = saml_private_key_ref
                    && let Err(cleanup_err) = secret_manager.delete(saml_key).await
                {
                    tracing::warn!(
                        "Failed to clean up orphaned SAML private key at {} after database error: {}",
                        saml_key,
                        cleanup_err
                    );
                }
                return Err(OrgSsoConfigError::Database(e));
            }
        };

        Ok(config)
    }

    /// Get an SSO configuration by its ID.
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgSsoConfig>> {
        self.db.org_sso_configs().get_by_id(id).await
    }

    /// Get an SSO configuration by organization ID.
    pub async fn get_by_org_id(&self, org_id: Uuid) -> DbResult<Option<OrgSsoConfig>> {
        self.db.org_sso_configs().get_by_org_id(org_id).await
    }

    /// Get an SSO configuration with its secrets.
    ///
    /// The OIDC client secret and SAML SP private key (if configured) are
    /// retrieved from the secret manager.
    ///
    /// # Returns
    /// The config with decrypted secrets, or None if not found.
    pub async fn get_with_secret(
        &self,
        id: Uuid,
        secret_manager: &dyn SecretManager,
    ) -> Result<Option<OrgSsoConfigWithClientSecret>, OrgSsoConfigError> {
        let config_with_key = match self.db.org_sso_configs().get_with_secret(id).await? {
            Some(c) => c,
            None => return Ok(None),
        };

        // Retrieve OIDC client secret if reference exists (for OIDC configs)
        let client_secret = if let Some(ref secret_key) = config_with_key.client_secret_key {
            Some(
                secret_manager
                    .get(secret_key)
                    .await
                    .map_err(|e| OrgSsoConfigError::SecretRetrieval(e.to_string()))?
                    .ok_or_else(|| {
                        OrgSsoConfigError::SecretRetrieval(format!(
                            "Client secret not found at key: {}",
                            secret_key
                        ))
                    })?,
            )
        } else {
            None
        };

        // Retrieve SAML SP private key if reference exists (for SAML configs)
        let saml_sp_private_key = if let Some(ref key_ref) = config_with_key.saml_sp_private_key_ref
        {
            Some(
                secret_manager
                    .get(key_ref)
                    .await
                    .map_err(|e| OrgSsoConfigError::SecretRetrieval(e.to_string()))?
                    .ok_or_else(|| {
                        OrgSsoConfigError::SecretRetrieval(format!(
                            "SAML SP private key not found at key: {}",
                            key_ref
                        ))
                    })?,
            )
        } else {
            None
        };

        Ok(Some(OrgSsoConfigWithClientSecret {
            config: config_with_key.config,
            client_secret,
            saml_sp_private_key,
        }))
    }

    /// Get an SSO configuration with its secrets by organization ID.
    pub async fn get_with_secret_by_org_id(
        &self,
        org_id: Uuid,
        secret_manager: &dyn SecretManager,
    ) -> Result<Option<OrgSsoConfigWithClientSecret>, OrgSsoConfigError> {
        let config_with_key = match self
            .db
            .org_sso_configs()
            .get_with_secret_by_org_id(org_id)
            .await?
        {
            Some(c) => c,
            None => return Ok(None),
        };

        // Retrieve OIDC client secret if reference exists (for OIDC configs)
        let client_secret = if let Some(ref secret_key) = config_with_key.client_secret_key {
            Some(
                secret_manager
                    .get(secret_key)
                    .await
                    .map_err(|e| OrgSsoConfigError::SecretRetrieval(e.to_string()))?
                    .ok_or_else(|| {
                        OrgSsoConfigError::SecretRetrieval(format!(
                            "Client secret not found at key: {}",
                            secret_key
                        ))
                    })?,
            )
        } else {
            None
        };

        // Retrieve SAML SP private key if reference exists (for SAML configs)
        let saml_sp_private_key = if let Some(ref key_ref) = config_with_key.saml_sp_private_key_ref
        {
            Some(
                secret_manager
                    .get(key_ref)
                    .await
                    .map_err(|e| OrgSsoConfigError::SecretRetrieval(e.to_string()))?
                    .ok_or_else(|| {
                        OrgSsoConfigError::SecretRetrieval(format!(
                            "SAML SP private key not found at key: {}",
                            key_ref
                        ))
                    })?,
            )
        } else {
            None
        };

        Ok(Some(OrgSsoConfigWithClientSecret {
            config: config_with_key.config,
            client_secret,
            saml_sp_private_key,
        }))
    }

    /// Update an SSO configuration.
    ///
    /// If new secrets (client_secret, saml_sp_private_key) are provided, they
    /// will be stored in the secret manager and the references updated.
    pub async fn update(
        &self,
        id: Uuid,
        input: UpdateOrgSsoConfig,
        secret_manager: &dyn SecretManager,
    ) -> Result<OrgSsoConfig, OrgSsoConfigError> {
        // We need the org_id for generating secret keys, so fetch it once if any secrets need updating
        let org_id = if input.client_secret.is_some() || input.saml_sp_private_key.is_some() {
            let existing = self
                .db
                .org_sso_configs()
                .get_by_id(id)
                .await?
                .ok_or(OrgSsoConfigError::NotFound)?;
            Some(existing.org_id)
        } else {
            None
        };

        // Update OIDC client secret if provided
        let new_client_secret_key = if let Some(ref new_secret) = input.client_secret {
            let secret_key = format!("org-sso/{}/client-secret", org_id.unwrap());
            secret_manager
                .set(&secret_key, new_secret)
                .await
                .map_err(|e| OrgSsoConfigError::SecretStorage(e.to_string()))?;
            Some(secret_key)
        } else {
            None
        };

        // Update SAML SP private key if provided
        let new_saml_key_ref = if let Some(ref new_key) = input.saml_sp_private_key {
            let key = format!("org-sso/{}/saml-sp-private-key", org_id.unwrap());
            secret_manager
                .set(&key, new_key)
                .await
                .map_err(|e| OrgSsoConfigError::SecretStorage(e.to_string()))?;
            Some(key)
        } else {
            None
        };

        let config = match self
            .db
            .org_sso_configs()
            .update(
                id,
                input,
                new_client_secret_key.as_deref(),
                new_saml_key_ref.as_deref(),
            )
            .await
        {
            Ok(config) => config,
            Err(e) => {
                // For update, we overwrote existing secrets, so we can't easily rollback
                // (we'd need to have saved the old values first). Log a warning about potential
                // inconsistent state - the secrets have new values but the DB update failed.
                if new_client_secret_key.is_some() || new_saml_key_ref.is_some() {
                    tracing::warn!(
                        "Database update failed after secrets were updated for config {}. \
                         Secrets may be inconsistent until next successful update: {}",
                        id,
                        e
                    );
                }
                return Err(OrgSsoConfigError::Database(e));
            }
        };

        Ok(config)
    }

    /// Delete an SSO configuration.
    ///
    /// Also deletes the associated secrets (client secret, SAML SP private key)
    /// from the secret manager.
    pub async fn delete(
        &self,
        id: Uuid,
        secret_manager: &dyn SecretManager,
    ) -> Result<(), OrgSsoConfigError> {
        // Get the secret keys before deleting
        let config_with_key = self
            .db
            .org_sso_configs()
            .get_with_secret(id)
            .await?
            .ok_or(OrgSsoConfigError::NotFound)?;

        // Delete from database first
        self.db.org_sso_configs().delete(id).await?;

        // Then try to clean up secrets (best effort)
        // Clean up OIDC client secret if it exists
        if let Some(ref client_key) = config_with_key.client_secret_key
            && let Err(e) = secret_manager.delete(client_key).await
        {
            tracing::warn!("Failed to delete client secret at {}: {}", client_key, e);
        }

        // Clean up SAML SP private key if it exists
        if let Some(ref saml_key) = config_with_key.saml_sp_private_key_ref
            && let Err(e) = secret_manager.delete(saml_key).await
        {
            tracing::warn!(
                "Failed to delete SAML SP private key at {}: {}",
                saml_key,
                e
            );
        }

        Ok(())
    }

    /// Find SSO configuration by email domain.
    ///
    /// Used for IdP discovery during login.
    pub async fn find_by_email_domain(&self, domain: &str) -> DbResult<Option<OrgSsoConfig>> {
        self.db.org_sso_configs().find_by_email_domain(domain).await
    }

    /// Check if any enabled SSO configurations exist.
    ///
    /// Used to determine if email discovery should be shown on the login page.
    pub async fn any_enabled(&self) -> DbResult<bool> {
        self.db.org_sso_configs().any_enabled().await
    }

    /// List all enabled SSO configurations with their secrets.
    ///
    /// Used for initializing the authenticator registry on startup.
    pub async fn list_enabled_with_secrets(
        &self,
        secret_manager: &dyn SecretManager,
    ) -> Result<Vec<OrgSsoConfigWithClientSecret>, OrgSsoConfigError> {
        let configs = self.db.org_sso_configs().list_enabled().await?;

        let mut results = Vec::with_capacity(configs.len());
        for config_with_key in configs {
            // Retrieve OIDC client secret if reference exists (for OIDC configs)
            let client_secret = if let Some(ref secret_key) = config_with_key.client_secret_key {
                Some(
                    secret_manager
                        .get(secret_key)
                        .await
                        .map_err(|e| OrgSsoConfigError::SecretRetrieval(e.to_string()))?
                        .ok_or_else(|| {
                            OrgSsoConfigError::SecretRetrieval(format!(
                                "Client secret not found at key: {}",
                                secret_key
                            ))
                        })?,
                )
            } else {
                None
            };

            // Retrieve SAML SP private key if reference exists (for SAML configs)
            let saml_sp_private_key =
                if let Some(ref key_ref) = config_with_key.saml_sp_private_key_ref {
                    Some(
                        secret_manager
                            .get(key_ref)
                            .await
                            .map_err(|e| OrgSsoConfigError::SecretRetrieval(e.to_string()))?
                            .ok_or_else(|| {
                                OrgSsoConfigError::SecretRetrieval(format!(
                                    "SAML SP private key not found at key: {}",
                                    key_ref
                                ))
                            })?,
                    )
                } else {
                    None
                };

            results.push(OrgSsoConfigWithClientSecret {
                config: config_with_key.config,
                client_secret,
                saml_sp_private_key,
            });
        }

        Ok(results)
    }

    /// List all enabled SSO configurations of a specific provider type with their secrets.
    ///
    /// Used for initializing the SAML or OIDC authenticator registries on startup.
    pub async fn list_enabled_with_secrets_by_type(
        &self,
        secret_manager: &dyn SecretManager,
        provider_type: SsoProviderType,
    ) -> Result<Vec<OrgSsoConfigWithClientSecret>, OrgSsoConfigError> {
        let all_configs = self.list_enabled_with_secrets(secret_manager).await?;
        Ok(all_configs
            .into_iter()
            .filter(|c| c.config.provider_type == provider_type)
            .collect())
    }
}

/// SSO configuration with decrypted secrets.
///
/// This is returned by service methods that need to provide the actual secrets
/// (e.g., for OIDC client initialization or SAML request signing).
#[derive(Debug, Clone)]
pub struct OrgSsoConfigWithClientSecret {
    /// The SSO configuration
    pub config: OrgSsoConfig,
    /// The decrypted OIDC client secret (for OIDC configs)
    pub client_secret: Option<String>,
    /// The decrypted SAML SP private key (PEM format, for SAML configs)
    pub saml_sp_private_key: Option<String>,
}

/// Errors that can occur in OrgSsoConfigService operations.
#[derive(Debug, thiserror::Error)]
pub enum OrgSsoConfigError {
    #[error("Database error: {0}")]
    Database(#[from] crate::db::DbError),

    #[error("Failed to store client secret: {0}")]
    SecretStorage(String),

    #[error("Failed to retrieve client secret: {0}")]
    SecretRetrieval(String),

    #[error("SSO configuration not found")]
    NotFound,
}

// Implement conversion from SecretResult for convenience
impl From<crate::secrets::SecretError> for OrgSsoConfigError {
    fn from(e: crate::secrets::SecretError) -> Self {
        OrgSsoConfigError::SecretRetrieval(e.to_string())
    }
}
