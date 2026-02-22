//! Service layer for organization SCIM configuration operations.
//!
//! This service handles CRUD operations for per-organization SCIM configurations,
//! including secure token generation and hashing for SCIM API authentication.

use std::sync::Arc;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    db::{DbPool, DbResult},
    models::{
        CreateOrgScimConfig, CreatedOrgScimConfig, OrgScimConfig, OrgScimConfigWithHash,
        UpdateOrgScimConfig,
    },
};

/// Service layer for organization SCIM configuration operations.
///
/// SCIM tokens are hashed (like API keys) before storage. Unlike SSO client
/// secrets, we don't use the SecretManager because SCIM tokens need fast
/// lookup for every provisioning request.
#[derive(Clone)]
pub struct OrgScimConfigService {
    db: Arc<DbPool>,
}

impl OrgScimConfigService {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self { db }
    }

    /// Create a new SCIM configuration for an organization.
    ///
    /// Generates a secure bearer token that is returned only once.
    /// The token is hashed before storage.
    ///
    /// # Arguments
    /// * `org_id` - The organization this SCIM config belongs to
    /// * `input` - The SCIM configuration settings
    ///
    /// # Returns
    /// The created config along with the raw token (shown only once)
    pub async fn create(
        &self,
        org_id: Uuid,
        input: CreateOrgScimConfig,
    ) -> Result<CreatedOrgScimConfig, OrgScimConfigError> {
        // Generate a secure token
        let (raw_token, token_hash, token_prefix) = generate_scim_token();

        // Create the config in the database
        let config = self
            .db
            .scim_configs()
            .create(org_id, input, &token_hash, &token_prefix)
            .await
            .map_err(OrgScimConfigError::Database)?;

        Ok(CreatedOrgScimConfig {
            config,
            token: raw_token,
        })
    }

    /// Get a SCIM configuration by its ID.
    pub async fn get_by_id(&self, id: Uuid) -> DbResult<Option<OrgScimConfig>> {
        self.db.scim_configs().get_by_id(id).await
    }

    /// Get a SCIM configuration by organization ID.
    pub async fn get_by_org_id(&self, org_id: Uuid) -> DbResult<Option<OrgScimConfig>> {
        self.db.scim_configs().get_by_org_id(org_id).await
    }

    /// Authenticate a SCIM bearer token and return the associated config.
    ///
    /// This is the hot path for SCIM API authentication.
    /// Updates the `token_last_used_at` timestamp on successful auth.
    ///
    /// # Arguments
    /// * `token` - The raw bearer token from the Authorization header
    ///
    /// # Returns
    /// The config with hash if valid, None if token is invalid
    pub async fn authenticate_token(
        &self,
        token: &str,
    ) -> Result<Option<OrgScimConfigWithHash>, OrgScimConfigError> {
        // Hash the incoming token
        let token_hash = hash_token(token);

        // Look up by hash
        let config = self
            .db
            .scim_configs()
            .get_by_token_hash(&token_hash)
            .await
            .map_err(OrgScimConfigError::Database)?;

        // Update last used timestamp (fire and forget)
        if let Some(ref c) = config
            && let Err(e) = self
                .db
                .scim_configs()
                .update_token_last_used(c.config.id)
                .await
        {
            tracing::warn!("Failed to update SCIM token last_used_at: {}", e);
        }

        Ok(config)
    }

    /// Update a SCIM configuration.
    pub async fn update(&self, id: Uuid, input: UpdateOrgScimConfig) -> DbResult<OrgScimConfig> {
        self.db.scim_configs().update(id, input).await
    }

    /// Rotate the SCIM token for a configuration.
    ///
    /// Generates a new token and invalidates the old one.
    ///
    /// # Returns
    /// The updated config along with the new raw token (shown only once)
    pub async fn rotate_token(&self, id: Uuid) -> Result<CreatedOrgScimConfig, OrgScimConfigError> {
        // Generate a new secure token
        let (raw_token, token_hash, token_prefix) = generate_scim_token();

        // Update the token in the database
        let config = self
            .db
            .scim_configs()
            .rotate_token(id, &token_hash, &token_prefix)
            .await
            .map_err(OrgScimConfigError::Database)?;

        Ok(CreatedOrgScimConfig {
            config,
            token: raw_token,
        })
    }

    /// Delete a SCIM configuration.
    pub async fn delete(&self, id: Uuid) -> DbResult<()> {
        self.db.scim_configs().delete(id).await
    }

    /// List all enabled SCIM configurations.
    pub async fn list_enabled(&self) -> DbResult<Vec<OrgScimConfig>> {
        self.db.scim_configs().list_enabled().await
    }
}

/// Error types for SCIM config operations.
#[derive(Debug, thiserror::Error)]
pub enum OrgScimConfigError {
    #[error("Database error: {0}")]
    Database(#[from] crate::db::DbError),
}

/// Generate a new SCIM bearer token.
///
/// Returns (raw_token, token_hash, token_prefix).
///
/// Token format: `scim_<32 bytes base64url>` (approximately 48 characters)
fn generate_scim_token() -> (String, String, String) {
    use base64::Engine;
    use rand::RngCore;

    // Generate 32 random bytes
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);

    // Base64url encode
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);

    // Construct the full token
    let raw_token = format!("scim_{}", encoded);

    // Hash for storage
    let token_hash = hash_token(&raw_token);

    // Prefix for identification (first 8 chars of the random part)
    let token_prefix = format!("scim_{}", &encoded[..4]);

    (raw_token, token_hash, token_prefix)
}

/// Hash a token using SHA-256.
fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}
