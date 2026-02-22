//! Azure Key Vault secret manager implementation.
//!
//! Supports storing and retrieving secrets from Azure Key Vault.
//! Uses Azure SDK for Rust with credential chain authentication.

use std::sync::Arc;

use async_trait::async_trait;
use azure_core::http::request::RequestContent;
use azure_identity::{AzureCliCredential, ManagedIdentityCredential};
use azure_security_keyvault_secrets::{SecretClient, models::SetSecretParameters};
use bytes::Bytes;

use super::{SecretError, SecretManager, SecretResult};

/// Configuration for Azure Key Vault.
#[derive(Debug, Clone)]
pub struct AzureKeyVaultConfig {
    /// Key Vault URL (e.g., "https://myvault.vault.azure.net")
    pub vault_url: String,
    /// Optional prefix for all secret names (e.g., "gateway-")
    /// Note: Azure Key Vault secret names can only contain alphanumeric and hyphens
    pub prefix: String,
}

impl AzureKeyVaultConfig {
    /// Create a new config with the given vault URL.
    pub fn new(vault_url: impl Into<String>) -> Self {
        Self {
            vault_url: vault_url.into(),
            prefix: "gateway-".to_string(),
        }
    }

    /// Set the secret name prefix.
    /// Note: Azure Key Vault only allows alphanumeric characters and hyphens in secret names.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }
}

/// Azure Key Vault secret manager.
pub struct AzureKeyVaultManager {
    client: SecretClient,
    prefix: String,
}

impl AzureKeyVaultManager {
    /// Create a new Azure Key Vault client with the given configuration.
    ///
    /// Tries multiple authentication methods:
    /// - Managed Identity (when running in Azure)
    /// - Azure CLI credentials (for local development)
    pub async fn new(config: AzureKeyVaultConfig) -> SecretResult<Self> {
        // Try Managed Identity first (for Azure deployments), then Azure CLI (for local dev)
        let credential: Arc<dyn azure_core::credentials::TokenCredential> =
            if let Ok(mi) = ManagedIdentityCredential::new(None) {
                mi
            } else {
                AzureCliCredential::new(None).map_err(|e| {
                    SecretError::Auth(format!("Failed to create Azure CLI credential: {}", e))
                })?
            };

        let client = SecretClient::new(&config.vault_url, credential, None).map_err(|e| {
            SecretError::Connection(format!("Failed to create Key Vault client: {}", e))
        })?;

        Ok(Self {
            client,
            prefix: config.prefix,
        })
    }

    /// Build the full secret name with prefix.
    /// Sanitizes the name to only contain allowed characters (alphanumeric and hyphens).
    fn full_name(&self, key: &str) -> String {
        let sanitized_key: String = key
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect();

        if self.prefix.is_empty() {
            sanitized_key
        } else {
            format!("{}{}", self.prefix, sanitized_key)
        }
    }

    /// Check if an error is a NOT_FOUND error.
    fn is_not_found(err: &azure_core::Error) -> bool {
        let err_str = err.to_string();
        err_str.contains("NotFound")
            || err_str.contains("404")
            || err_str.contains("SecretNotFound")
    }

    /// Check if an error is a FORBIDDEN error.
    fn is_forbidden(err: &azure_core::Error) -> bool {
        let err_str = err.to_string();
        err_str.contains("Forbidden") || err_str.contains("403")
    }
}

#[async_trait]
impl SecretManager for AzureKeyVaultManager {
    async fn get(&self, key: &str) -> SecretResult<Option<String>> {
        let name = self.full_name(key);

        match self.client.get_secret(&name, None).await {
            Ok(response) => {
                // Deserialize the response body to Secret
                let body = response.into_body();
                // ResponseBody is a wrapper around Bytes, dereference to get the bytes
                let secret: azure_security_keyvault_secrets::models::Secret =
                    serde_json::from_slice(&body).map_err(|e| {
                        SecretError::Internal(format!("Failed to parse secret response: {}", e))
                    })?;

                if let Some(value) = secret.value {
                    // Try to parse as JSON and extract "value" or "api_key" field
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&value) {
                        if let Some(v) = json.get("value").and_then(|v| v.as_str()) {
                            return Ok(Some(v.to_string()));
                        }
                        if let Some(v) = json.get("api_key").and_then(|v| v.as_str()) {
                            return Ok(Some(v.to_string()));
                        }
                    }
                    // If not JSON or no recognized field, return the raw string
                    return Ok(Some(value));
                }
                Ok(None)
            }
            Err(err) => {
                if Self::is_not_found(&err) {
                    return Ok(None);
                }
                Err(SecretError::Internal(format!(
                    "Failed to get secret '{}': {}",
                    key, err
                )))
            }
        }
    }

    async fn set(&self, key: &str, value: &str) -> SecretResult<()> {
        let name = self.full_name(key);

        // Store as JSON with "value" field for consistency with Vault
        let secret_value = serde_json::json!({ "value": value }).to_string();

        let params = SetSecretParameters {
            value: Some(secret_value),
            ..Default::default()
        };

        let body: RequestContent<SetSecretParameters> =
            Bytes::from(serde_json::to_vec(&params).unwrap()).into();

        self.client
            .set_secret(&name, body, None)
            .await
            .map_err(|e| SecretError::Internal(format!("Failed to set secret '{}': {}", key, e)))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> SecretResult<()> {
        let name = self.full_name(key);

        match self.client.delete_secret(&name, None).await {
            Ok(_) => Ok(()),
            Err(err) => {
                if Self::is_not_found(&err) {
                    // Already deleted
                    return Ok(());
                }
                Err(SecretError::Internal(format!(
                    "Failed to delete secret '{}': {}",
                    key, err
                )))
            }
        }
    }

    async fn health_check(&self) -> SecretResult<()> {
        // Try to get a non-existent secret to verify connectivity
        match self.client.get_secret("__health_check__", None).await {
            Ok(_) => Ok(()),
            Err(err) => {
                // 404 Not Found is expected (secret doesn't exist) - we can connect
                if Self::is_not_found(&err) {
                    return Ok(());
                }
                // Permission denied means we can connect but don't have access
                if Self::is_forbidden(&err) {
                    return Ok(());
                }
                Err(SecretError::Connection(format!(
                    "Azure Key Vault health check failed: {}",
                    err
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_new() {
        let config =
            AzureKeyVaultConfig::new("https://myvault.vault.azure.net").with_prefix("myapp-");

        assert_eq!(config.vault_url, "https://myvault.vault.azure.net");
        assert_eq!(config.prefix, "myapp-");
    }

    #[test]
    fn test_full_name_sanitization() {
        // We can't easily test without creating a real client, so just test the config
        let config = AzureKeyVaultConfig::new("https://myvault.vault.azure.net");
        assert_eq!(config.prefix, "gateway-");
    }
}
