//! Secrets management for provider API keys and other sensitive data.
//!
//! Supports multiple backends:
//! - Environment variables (default for local development)
//! - In-memory (for testing)
//! - HashiCorp Vault / OpenBao (for production) - requires `vault` feature
//! - AWS Secrets Manager - requires `secrets-aws` feature
//! - Azure Key Vault - requires `secrets-azure` feature
//! - GCP Secret Manager - requires `secrets-gcp` feature

#[cfg(feature = "secrets-aws")]
mod aws;
#[cfg(feature = "secrets-azure")]
mod azure;
#[cfg(feature = "secrets-gcp")]
mod gcp;
#[cfg(feature = "vault")]
mod vault;

use async_trait::async_trait;
#[cfg(feature = "secrets-aws")]
pub use aws::{AwsSecretsManager, AwsSecretsManagerConfig};
#[cfg(feature = "secrets-azure")]
pub use azure::{AzureKeyVaultConfig, AzureKeyVaultManager};
#[cfg(feature = "secrets-gcp")]
pub use gcp::{GcpSecretManager, GcpSecretManagerConfig};
use thiserror::Error;
#[cfg(feature = "vault")]
pub use vault::{VaultConfig, VaultSecretManager};

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("Secret not found: {0}")]
    NotFound(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type SecretResult<T> = Result<T, SecretError>;

/// Trait for managing secrets (provider API keys, etc.)
#[async_trait]
pub trait SecretManager: Send + Sync {
    /// Get a secret by key. Returns None if not found.
    async fn get(&self, key: &str) -> SecretResult<Option<String>>;

    /// Set a secret. Not all backends support this.
    async fn set(&self, key: &str, value: &str) -> SecretResult<()>;

    /// Delete a secret. Not all backends support this.
    async fn delete(&self, key: &str) -> SecretResult<()>;

    /// Check if the secret manager is healthy/connected.
    async fn health_check(&self) -> SecretResult<()> {
        Ok(())
    }
}

/// In-memory secret manager (for testing only)
pub struct MemorySecretManager {
    secrets: std::sync::Arc<dashmap::DashMap<String, String>>,
}

impl MemorySecretManager {
    pub fn new() -> Self {
        Self {
            secrets: std::sync::Arc::new(dashmap::DashMap::new()),
        }
    }
}

impl Default for MemorySecretManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecretManager for MemorySecretManager {
    async fn get(&self, key: &str) -> SecretResult<Option<String>> {
        Ok(self.secrets.get(key).map(|v| v.value().clone()))
    }

    async fn set(&self, key: &str, value: &str) -> SecretResult<()> {
        self.secrets.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn delete(&self, key: &str) -> SecretResult<()> {
        self.secrets.remove(key);
        Ok(())
    }
}

/// Environment-based secret manager (reads from env vars)
pub struct EnvSecretManager;

impl EnvSecretManager {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnvSecretManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecretManager for EnvSecretManager {
    async fn get(&self, key: &str) -> SecretResult<Option<String>> {
        Ok(std::env::var(key).ok())
    }

    async fn set(&self, _key: &str, _value: &str) -> SecretResult<()> {
        Err(SecretError::Internal(
            "Cannot set secrets in environment manager".to_string(),
        ))
    }

    async fn delete(&self, _key: &str) -> SecretResult<()> {
        Err(SecretError::Internal(
            "Cannot delete secrets from environment manager".to_string(),
        ))
    }
}
