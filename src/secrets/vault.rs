//! HashiCorp Vault / OpenBao secret manager implementation.
//!
//! Supports the KV v2 secrets engine for storing provider API keys.
//! Supports multiple authentication methods: Token, AppRole, and Kubernetes.

use async_trait::async_trait;
use vaultrs::{
    auth::{approle, kubernetes},
    client::{Client, VaultClient, VaultClientSettingsBuilder},
    kv2,
};

use super::{SecretError, SecretManager, SecretResult};

/// Authentication method for Vault.
#[derive(Debug, Clone)]
pub enum VaultAuthMethod {
    /// Direct token authentication.
    Token(String),
    /// AppRole authentication (recommended for production).
    AppRole {
        /// The mount path for AppRole auth (default: "approle")
        mount: String,
        /// AppRole role ID
        role_id: String,
        /// AppRole secret ID
        secret_id: String,
    },
    /// Kubernetes ServiceAccount authentication.
    Kubernetes {
        /// The mount path for Kubernetes auth (default: "kubernetes")
        mount: String,
        /// Vault role name configured for this ServiceAccount
        role: String,
        /// JWT token from the ServiceAccount
        jwt: String,
    },
}

/// Configuration for the Vault secret manager.
#[derive(Debug, Clone)]
pub struct VaultConfig {
    /// Vault server address (e.g., "https://vault.example.com:8200")
    pub address: String,
    /// Authentication method
    pub auth: VaultAuthMethod,
    /// KV v2 mount point (default: "secret")
    pub mount: String,
    /// Path prefix for all secrets (e.g., "gateway/providers")
    pub path_prefix: String,
}

impl VaultConfig {
    /// Create a new config with token authentication.
    pub fn new(address: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            auth: VaultAuthMethod::Token(token.into()),
            mount: "secret".to_string(),
            path_prefix: "hadrian".to_string(),
        }
    }

    /// Create a new config with AppRole authentication.
    pub fn with_approle(
        address: impl Into<String>,
        role_id: impl Into<String>,
        secret_id: impl Into<String>,
    ) -> Self {
        Self {
            address: address.into(),
            auth: VaultAuthMethod::AppRole {
                mount: "approle".to_string(),
                role_id: role_id.into(),
                secret_id: secret_id.into(),
            },
            mount: "secret".to_string(),
            path_prefix: "hadrian".to_string(),
        }
    }

    /// Create a new config with Kubernetes authentication.
    pub fn with_kubernetes(
        address: impl Into<String>,
        role: impl Into<String>,
        jwt: impl Into<String>,
    ) -> Self {
        Self {
            address: address.into(),
            auth: VaultAuthMethod::Kubernetes {
                mount: "kubernetes".to_string(),
                role: role.into(),
                jwt: jwt.into(),
            },
            mount: "secret".to_string(),
            path_prefix: "hadrian".to_string(),
        }
    }

    /// Set the auth mount path (for AppRole or Kubernetes).
    pub fn with_auth_mount(mut self, auth_mount: impl Into<String>) -> Self {
        let auth_mount = auth_mount.into();
        match &mut self.auth {
            VaultAuthMethod::Token(_) => {} // No mount path for token auth
            VaultAuthMethod::AppRole { mount, .. } => *mount = auth_mount,
            VaultAuthMethod::Kubernetes { mount, .. } => *mount = auth_mount,
        }
        self
    }

    pub fn with_mount(mut self, mount: impl Into<String>) -> Self {
        self.mount = mount.into();
        self
    }

    pub fn with_path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.path_prefix = prefix.into();
        self
    }
}

/// Vault/OpenBao secret manager using the KV v2 secrets engine.
pub struct VaultSecretManager {
    client: VaultClient,
    mount: String,
    path_prefix: String,
}

impl VaultSecretManager {
    /// Create a new Vault secret manager with the given configuration.
    ///
    /// For AppRole and Kubernetes auth, this will perform the login
    /// and obtain a token before returning.
    pub async fn new(config: VaultConfig) -> SecretResult<Self> {
        // For non-token auth, we need to create a client first, authenticate,
        // then update the client with the received token
        let (initial_token, needs_auth) = match &config.auth {
            VaultAuthMethod::Token(token) => (token.clone(), false),
            VaultAuthMethod::AppRole { .. } | VaultAuthMethod::Kubernetes { .. } => {
                // Use empty token initially; we'll authenticate and set the real token
                (String::new(), true)
            }
        };

        let settings = VaultClientSettingsBuilder::default()
            .address(&config.address)
            .token(&initial_token)
            .build()
            .map_err(|e| SecretError::Internal(format!("Failed to build Vault settings: {}", e)))?;

        let mut client = VaultClient::new(settings).map_err(|e| {
            SecretError::Connection(format!("Failed to create Vault client: {}", e))
        })?;

        // Perform authentication if needed
        if needs_auth {
            let token = match &config.auth {
                VaultAuthMethod::Token(_) => unreachable!(),
                VaultAuthMethod::AppRole {
                    mount,
                    role_id,
                    secret_id,
                } => {
                    tracing::debug!(mount = %mount, "Authenticating to Vault via AppRole");
                    let auth_info = approle::login(&client, mount, role_id, secret_id)
                        .await
                        .map_err(|e| {
                            SecretError::Auth(format!("AppRole authentication failed: {}", e))
                        })?;
                    auth_info.client_token
                }
                VaultAuthMethod::Kubernetes { mount, role, jwt } => {
                    tracing::debug!(mount = %mount, role = %role, "Authenticating to Vault via Kubernetes");
                    let auth_info =
                        kubernetes::login(&client, mount, role, jwt)
                            .await
                            .map_err(|e| {
                                SecretError::Auth(format!(
                                    "Kubernetes authentication failed: {}",
                                    e
                                ))
                            })?;
                    auth_info.client_token
                }
            };

            // Update the client with the received token
            client.set_token(&token);
            tracing::info!("Successfully authenticated to Vault");
        }

        Ok(Self {
            client,
            mount: config.mount,
            path_prefix: config.path_prefix,
        })
    }

    /// Build the full path for a secret key.
    fn full_path(&self, key: &str) -> String {
        if self.path_prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.path_prefix, key)
        }
    }
}

#[async_trait]
impl SecretManager for VaultSecretManager {
    async fn get(&self, key: &str) -> SecretResult<Option<String>> {
        let path = self.full_path(key);

        match kv2::read::<serde_json::Value>(&self.client, &self.mount, &path).await {
            Ok(secret) => {
                // Extract the "value" field from the secret data
                if let Some(value) = secret.get("value").and_then(|v| v.as_str()) {
                    Ok(Some(value.to_string()))
                } else {
                    // Try to get "api_key" field as an alternative
                    if let Some(value) = secret.get("api_key").and_then(|v| v.as_str()) {
                        Ok(Some(value.to_string()))
                    } else {
                        Ok(None)
                    }
                }
            }
            Err(vaultrs::error::ClientError::APIError { code: 404, .. }) => Ok(None),
            Err(e) => Err(SecretError::Internal(format!(
                "Failed to read secret '{}': {}",
                key, e
            ))),
        }
    }

    async fn set(&self, key: &str, value: &str) -> SecretResult<()> {
        let path = self.full_path(key);

        let data = serde_json::json!({
            "value": value
        });

        kv2::set(&self.client, &self.mount, &path, &data)
            .await
            .map_err(|e| SecretError::Internal(format!("Failed to set secret '{}': {}", key, e)))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> SecretResult<()> {
        let path = self.full_path(key);

        kv2::delete_latest(&self.client, &self.mount, &path)
            .await
            .map_err(|e| {
                SecretError::Internal(format!("Failed to delete secret '{}': {}", key, e))
            })?;

        Ok(())
    }

    async fn health_check(&self) -> SecretResult<()> {
        // Try to read a non-existent key to verify connectivity
        // The error type will tell us if we can connect or not
        match kv2::read::<serde_json::Value>(&self.client, &self.mount, "__health_check__").await {
            Ok(_) => Ok(()),
            Err(vaultrs::error::ClientError::APIError { code: 404, .. }) => Ok(()), // Expected
            Err(vaultrs::error::ClientError::APIError { code: 403, .. }) => {
                Err(SecretError::Auth("Permission denied".to_string()))
            }
            Err(e) => Err(SecretError::Connection(format!(
                "Health check failed: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_config_token_auth() {
        let config = VaultConfig::new("http://localhost:8200", "test-token")
            .with_mount("kv")
            .with_path_prefix("myapp/secrets");

        assert_eq!(config.address, "http://localhost:8200");
        assert!(matches!(
            config.auth,
            VaultAuthMethod::Token(ref t) if t == "test-token"
        ));
        assert_eq!(config.mount, "kv");
        assert_eq!(config.path_prefix, "myapp/secrets");
    }

    #[test]
    fn test_vault_config_approle_auth() {
        let config =
            VaultConfig::with_approle("http://localhost:8200", "my-role-id", "my-secret-id")
                .with_auth_mount("custom-approle")
                .with_mount("kv")
                .with_path_prefix("myapp/secrets");

        assert_eq!(config.address, "http://localhost:8200");
        assert!(matches!(
            config.auth,
            VaultAuthMethod::AppRole {
                ref mount,
                ref role_id,
                ref secret_id
            } if mount == "custom-approle"
                && role_id == "my-role-id"
                && secret_id == "my-secret-id"
        ));
        assert_eq!(config.mount, "kv");
    }

    #[test]
    fn test_vault_config_kubernetes_auth() {
        let config = VaultConfig::with_kubernetes(
            "http://localhost:8200",
            "my-role",
            "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...",
        )
        .with_auth_mount("custom-k8s")
        .with_mount("kv");

        assert_eq!(config.address, "http://localhost:8200");
        assert!(matches!(
            config.auth,
            VaultAuthMethod::Kubernetes {
                ref mount,
                ref role,
                ref jwt
            } if mount == "custom-k8s"
                && role == "my-role"
                && jwt.starts_with("eyJ")
        ));
    }

    #[tokio::test]
    async fn test_full_path() {
        let config = VaultConfig::new("http://localhost:8200", "token")
            .with_path_prefix("gateway/providers");

        let manager = VaultSecretManager::new(config).await.unwrap();
        assert_eq!(
            manager.full_path("openai-key"),
            "gateway/providers/openai-key"
        );
    }

    #[tokio::test]
    async fn test_full_path_empty_prefix() {
        let config =
            VaultConfig::new("http://localhost:8200", "token").with_path_prefix(String::new());

        let manager = VaultSecretManager::new(config).await.unwrap();
        assert_eq!(manager.full_path("openai-key"), "openai-key");
    }
}
