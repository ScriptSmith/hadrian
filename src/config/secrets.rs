//! Secrets manager configuration.

use serde::{Deserialize, Serialize};

/// Configuration for the secrets manager.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SecretsConfig {
    /// No secrets manager (secrets are not resolved from external sources)
    #[default]
    None,

    /// Environment variable-based secrets
    /// Keys are looked up directly as environment variable names.
    Env,

    /// HashiCorp Vault / OpenBao secrets manager. Requires the `vault` feature.
    #[cfg(feature = "vault")]
    Vault(VaultSecretsConfig),

    /// AWS Secrets Manager. Requires the `secrets-aws` feature.
    #[cfg(feature = "secrets-aws")]
    Aws(AwsSecretsConfig),

    /// Azure Key Vault. Requires the `secrets-azure` feature.
    #[cfg(feature = "secrets-azure")]
    Azure(AzureKeyVaultSecretsConfig),

    /// GCP Secret Manager. Requires the `secrets-gcp` feature.
    #[cfg(feature = "secrets-gcp")]
    Gcp(GcpSecretsConfig),
}

impl SecretsConfig {
    pub fn is_none(&self) -> bool {
        matches!(self, SecretsConfig::None)
    }
}

#[cfg(feature = "vault")]
/// Configuration for Vault/OpenBao secrets manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct VaultSecretsConfig {
    /// Vault server address (e.g., "https://vault.example.com:8200")
    pub address: String,

    /// Authentication method
    #[serde(flatten)]
    pub auth: VaultAuth,

    /// KV v2 mount point (default: "secret")
    #[serde(default = "default_vault_mount")]
    pub mount: String,

    /// Path prefix for all secrets (default: "hadrian")
    #[serde(default = "default_vault_path_prefix")]
    pub path_prefix: String,
}

#[cfg(feature = "vault")]
fn default_vault_mount() -> String {
    "secret".to_string()
}

#[cfg(feature = "vault")]
fn default_vault_path_prefix() -> String {
    "hadrian".to_string()
}

#[cfg(feature = "vault")]
/// Vault authentication methods.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "auth", rename_all = "snake_case")]
pub enum VaultAuth {
    /// Token-based authentication
    Token {
        /// The Vault token
        token: String,
    },

    /// AppRole authentication (recommended for production)
    AppRole {
        /// AppRole role ID
        role_id: String,
        /// AppRole secret ID
        secret_id: String,
        /// Auth mount path (default: "approle")
        #[serde(default = "default_approle_mount")]
        auth_mount: String,
    },

    /// Kubernetes authentication (for pods running in k8s)
    Kubernetes {
        /// Vault role name
        role: String,
        /// Path to the service account token (default: /var/run/secrets/kubernetes.io/serviceaccount/token)
        #[serde(default = "default_k8s_token_path")]
        token_path: String,
        /// Auth mount path (default: "kubernetes")
        #[serde(default = "default_k8s_mount")]
        auth_mount: String,
    },
}

#[cfg(feature = "vault")]
impl std::fmt::Debug for VaultAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultAuth::Token { .. } => f.debug_struct("Token").field("token", &"****").finish(),
            VaultAuth::AppRole {
                role_id,
                auth_mount,
                ..
            } => f
                .debug_struct("AppRole")
                .field("role_id", role_id)
                .field("secret_id", &"****")
                .field("auth_mount", auth_mount)
                .finish(),
            VaultAuth::Kubernetes {
                role,
                token_path,
                auth_mount,
            } => f
                .debug_struct("Kubernetes")
                .field("role", role)
                .field("token_path", token_path)
                .field("auth_mount", auth_mount)
                .finish(),
        }
    }
}

#[cfg(feature = "vault")]
fn default_approle_mount() -> String {
    "approle".to_string()
}

#[cfg(feature = "vault")]
fn default_k8s_mount() -> String {
    "kubernetes".to_string()
}

#[cfg(feature = "vault")]
fn default_k8s_token_path() -> String {
    "/var/run/secrets/kubernetes.io/serviceaccount/token".to_string()
}

#[cfg(feature = "secrets-aws")]
/// Configuration for AWS Secrets Manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct AwsSecretsConfig {
    /// AWS region (e.g., "us-east-1"). If not set, uses AWS_REGION environment variable.
    #[serde(default)]
    pub region: Option<String>,

    /// Prefix for all secret names (default: "gateway/")
    #[serde(default = "default_aws_prefix")]
    pub prefix: String,

    /// Custom endpoint URL (for localstack or other AWS-compatible services)
    #[serde(default)]
    pub endpoint_url: Option<String>,
}

#[cfg(feature = "secrets-aws")]
fn default_aws_prefix() -> String {
    "gateway/".to_string()
}

#[cfg(feature = "secrets-azure")]
/// Configuration for Azure Key Vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct AzureKeyVaultSecretsConfig {
    /// Key Vault URL (e.g., "https://myvault.vault.azure.net")
    pub vault_url: String,

    /// Prefix for all secret names (default: "gateway-")
    /// Note: Azure Key Vault only allows alphanumeric characters and hyphens in secret names.
    #[serde(default = "default_azure_prefix")]
    pub prefix: String,
}

#[cfg(feature = "secrets-azure")]
fn default_azure_prefix() -> String {
    "gateway-".to_string()
}

#[cfg(feature = "secrets-gcp")]
/// Configuration for GCP Secret Manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct GcpSecretsConfig {
    /// GCP project ID
    pub project_id: String,

    /// Prefix for all secret names (default: "gateway-")
    #[serde(default = "default_gcp_prefix")]
    pub prefix: String,
}

#[cfg(feature = "secrets-gcp")]
fn default_gcp_prefix() -> String {
    "gateway-".to_string()
}

#[cfg(all(test, feature = "vault"))]
mod tests {
    use super::*;

    #[test]
    fn test_vault_auth_token_debug_redacts_token() {
        let auth = VaultAuth::Token {
            token: "hvs.super-secret-vault-token".to_string(),
        };

        let debug_output = format!("{:?}", auth);
        assert!(
            debug_output.contains("****"),
            "Debug output should contain redacted marker"
        );
        assert!(
            !debug_output.contains("hvs.super-secret-vault-token"),
            "Debug output must NOT contain actual token"
        );
    }

    #[test]
    fn test_vault_auth_approle_debug_redacts_secret_id() {
        let auth = VaultAuth::AppRole {
            role_id: "role-id-visible".to_string(),
            secret_id: "secret-id-super-secret".to_string(),
            auth_mount: "approle".to_string(),
        };

        let debug_output = format!("{:?}", auth);
        assert!(
            debug_output.contains("****"),
            "Debug output should contain redacted marker"
        );
        assert!(
            !debug_output.contains("secret-id-super-secret"),
            "Debug output must NOT contain secret_id"
        );
        // Non-sensitive fields should still be visible
        assert!(
            debug_output.contains("role-id-visible"),
            "Role ID should be visible"
        );
        assert!(
            debug_output.contains("approle"),
            "Auth mount should be visible"
        );
    }

    #[test]
    fn test_vault_auth_kubernetes_not_redacted() {
        // Kubernetes auth has no inline secrets (token is read from a file)
        let auth = VaultAuth::Kubernetes {
            role: "my-k8s-role".to_string(),
            token_path: "/var/run/secrets/kubernetes.io/serviceaccount/token".to_string(),
            auth_mount: "kubernetes".to_string(),
        };

        let debug_output = format!("{:?}", auth);
        assert!(
            debug_output.contains("my-k8s-role"),
            "Role should be visible"
        );
        assert!(
            debug_output.contains("/var/run/secrets"),
            "Token path should be visible"
        );
    }
}
