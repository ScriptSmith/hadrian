//! GCP Secret Manager implementation.
//!
//! Supports storing and retrieving secrets from Google Cloud Secret Manager.
//! Uses google-cloud-secretmanager-v1 with Application Default Credentials.

use async_trait::async_trait;
use google_cloud_secretmanager_v1::{
    client::SecretManagerService,
    model::{Replication, Secret, SecretPayload, replication::Automatic},
};

use super::{SecretError, SecretManager, SecretResult};

/// Configuration for GCP Secret Manager.
#[derive(Debug, Clone)]
pub struct GcpSecretManagerConfig {
    /// GCP project ID
    pub project_id: String,
    /// Optional prefix for all secret names (e.g., "gateway-")
    pub prefix: String,
}

impl GcpSecretManagerConfig {
    /// Create a new config with the given project ID.
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            prefix: "gateway-".to_string(),
        }
    }

    /// Set the secret name prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }
}

/// GCP Secret Manager secret manager.
pub struct GcpSecretManager {
    client: SecretManagerService,
    project_id: String,
    prefix: String,
}

impl GcpSecretManager {
    /// Create a new GCP Secret Manager client with the given configuration.
    ///
    /// Uses Application Default Credentials which tries:
    /// - GOOGLE_APPLICATION_CREDENTIALS environment variable
    /// - gcloud CLI credentials
    /// - Metadata server (when running in GCP)
    pub async fn new(config: GcpSecretManagerConfig) -> SecretResult<Self> {
        let client = SecretManagerService::builder()
            .build()
            .await
            .map_err(|e| SecretError::Connection(format!("Failed to create GCP client: {}", e)))?;

        Ok(Self {
            client,
            project_id: config.project_id,
            prefix: config.prefix,
        })
    }

    /// Build the full secret name with prefix.
    fn full_name(&self, key: &str) -> String {
        let sanitized_key: String = key
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
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

    /// Build the parent resource name (project).
    fn project_name(&self) -> String {
        format!("projects/{}", self.project_id)
    }

    /// Build the full secret resource name.
    fn secret_name(&self, key: &str) -> String {
        format!(
            "projects/{}/secrets/{}",
            self.project_id,
            self.full_name(key)
        )
    }

    /// Build the full secret version resource name.
    fn secret_version_name(&self, key: &str) -> String {
        format!("{}/versions/latest", self.secret_name(key))
    }

    /// Check if an error is a NOT_FOUND gRPC error.
    fn is_not_found(err: &google_cloud_secretmanager_v1::Error) -> bool {
        // The error message typically contains the gRPC status code
        let err_str = err.to_string();
        err_str.contains("NOT_FOUND") || err_str.contains("status: NotFound")
    }

    /// Check if an error is a PERMISSION_DENIED gRPC error.
    fn is_permission_denied(err: &google_cloud_secretmanager_v1::Error) -> bool {
        let err_str = err.to_string();
        err_str.contains("PERMISSION_DENIED") || err_str.contains("status: PermissionDenied")
    }
}

#[async_trait]
impl SecretManager for GcpSecretManager {
    async fn get(&self, key: &str) -> SecretResult<Option<String>> {
        let name = self.secret_version_name(key);

        match self
            .client
            .access_secret_version()
            .set_name(&name)
            .send()
            .await
        {
            Ok(response) => {
                if let Some(payload) = response.payload {
                    let data = payload.data;
                    if !data.is_empty() {
                        let value = String::from_utf8(data.to_vec()).map_err(|e| {
                            SecretError::Internal(format!(
                                "Secret '{}' is not valid UTF-8: {}",
                                key, e
                            ))
                        })?;

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
        let secret_name = self.secret_name(key);
        let secret_value = serde_json::json!({ "value": value }).to_string();

        // Try to add a new version first (if secret exists)
        let add_result = self
            .client
            .add_secret_version()
            .set_parent(&secret_name)
            .set_payload(SecretPayload::default().set_data(secret_value.clone().into_bytes()))
            .send()
            .await;

        match add_result {
            Ok(_) => return Ok(()),
            Err(err) => {
                if Self::is_not_found(&err) {
                    // Create the secret first with automatic replication
                    self.client
                        .create_secret()
                        .set_parent(self.project_name())
                        .set_secret_id(self.full_name(key))
                        .set_secret(Secret::default().set_replication(
                            Replication::default().set_automatic(Box::new(Automatic::default())),
                        ))
                        .send()
                        .await
                        .map_err(|e| {
                            SecretError::Internal(format!(
                                "Failed to create secret '{}': {}",
                                key, e
                            ))
                        })?;

                    // Now add the version
                    self.client
                        .add_secret_version()
                        .set_parent(&secret_name)
                        .set_payload(SecretPayload::default().set_data(secret_value.into_bytes()))
                        .send()
                        .await
                        .map_err(|e| {
                            SecretError::Internal(format!(
                                "Failed to add secret version '{}': {}",
                                key, e
                            ))
                        })?;

                    return Ok(());
                }
                return Err(SecretError::Internal(format!(
                    "Failed to set secret '{}': {}",
                    key, err
                )));
            }
        }
    }

    async fn delete(&self, key: &str) -> SecretResult<()> {
        let name = self.secret_name(key);

        match self.client.delete_secret().set_name(&name).send().await {
            Ok(_) => Ok(()),
            Err(err) => {
                // Not found is okay (already deleted)
                if Self::is_not_found(&err) {
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
        match self
            .client
            .list_secrets()
            .set_parent(self.project_name())
            .set_page_size(1)
            .send()
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                // Permission denied is okay for health check (we can connect)
                if Self::is_permission_denied(&err) {
                    return Ok(());
                }
                Err(SecretError::Connection(format!(
                    "GCP Secret Manager health check failed: {}",
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
        let config = GcpSecretManagerConfig::new("my-project").with_prefix("myapp-");

        assert_eq!(config.project_id, "my-project");
        assert_eq!(config.prefix, "myapp-");
    }

    #[test]
    fn test_config_default_prefix() {
        let config = GcpSecretManagerConfig::new("my-project");
        assert_eq!(config.prefix, "gateway-");
    }
}
