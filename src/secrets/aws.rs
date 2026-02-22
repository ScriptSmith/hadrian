//! AWS Secrets Manager implementation.
//!
//! Supports storing and retrieving secrets from AWS Secrets Manager.
//! Uses AWS SDK for Rust with standard credential chain (environment, instance profile, etc.)

use async_trait::async_trait;
use aws_sdk_secretsmanager::Client;

use super::{SecretError, SecretManager, SecretResult};

/// Configuration for AWS Secrets Manager.
#[derive(Debug, Clone)]
pub struct AwsSecretsManagerConfig {
    /// AWS region (e.g., "us-east-1")
    pub region: Option<String>,
    /// Optional prefix for all secret names (e.g., "gateway/")
    pub prefix: String,
    /// Optional endpoint URL for testing with localstack
    pub endpoint_url: Option<String>,
}

impl AwsSecretsManagerConfig {
    /// Create a new config with the given region.
    pub fn new(region: impl Into<String>) -> Self {
        Self {
            region: Some(region.into()),
            prefix: "gateway/".to_string(),
            endpoint_url: None,
        }
    }

    /// Create a new config using the default region from environment.
    pub fn from_env() -> Self {
        Self {
            region: None,
            prefix: "gateway/".to_string(),
            endpoint_url: None,
        }
    }

    /// Set the secret name prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Set a custom endpoint URL (useful for localstack testing).
    pub fn with_endpoint_url(mut self, url: impl Into<String>) -> Self {
        self.endpoint_url = Some(url.into());
        self
    }
}

/// AWS Secrets Manager secret manager.
pub struct AwsSecretsManager {
    client: Client,
    prefix: String,
}

impl AwsSecretsManager {
    /// Create a new AWS Secrets Manager client with the given configuration.
    pub async fn new(config: AwsSecretsManagerConfig) -> SecretResult<Self> {
        let mut aws_config = aws_config::from_env();

        if let Some(region) = &config.region {
            aws_config = aws_config.region(aws_config::Region::new(region.clone()));
        }

        let aws_config = aws_config.load().await;

        let mut sm_config = aws_sdk_secretsmanager::config::Builder::from(&aws_config);

        if let Some(endpoint_url) = &config.endpoint_url {
            sm_config = sm_config.endpoint_url(endpoint_url);
        }

        let client = Client::from_conf(sm_config.build());

        Ok(Self {
            client,
            prefix: config.prefix,
        })
    }

    /// Build the full secret name with prefix.
    fn full_name(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}{}", self.prefix, key)
        }
    }
}

#[async_trait]
impl SecretManager for AwsSecretsManager {
    async fn get(&self, key: &str) -> SecretResult<Option<String>> {
        let name = self.full_name(key);

        match self.client.get_secret_value().secret_id(&name).send().await {
            Ok(output) => {
                // AWS Secrets Manager can store secrets as string or binary
                if let Some(secret_string) = output.secret_string() {
                    // Try to parse as JSON and extract "value" or "api_key" field
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(secret_string) {
                        if let Some(value) = json.get("value").and_then(|v| v.as_str()) {
                            return Ok(Some(value.to_string()));
                        }
                        if let Some(value) = json.get("api_key").and_then(|v| v.as_str()) {
                            return Ok(Some(value.to_string()));
                        }
                    }
                    // If not JSON or no recognized field, return the raw string
                    Ok(Some(secret_string.to_string()))
                } else if let Some(secret_binary) = output.secret_binary() {
                    // Try to convert binary to string
                    String::from_utf8(secret_binary.clone().into_inner())
                        .map(Some)
                        .map_err(|e| {
                            SecretError::Internal(format!(
                                "Secret '{}' binary is not valid UTF-8: {}",
                                key, e
                            ))
                        })
                } else {
                    Ok(None)
                }
            }
            Err(err) => {
                let service_error = err.into_service_error();
                if service_error.is_resource_not_found_exception() {
                    Ok(None)
                } else {
                    Err(SecretError::Internal(format!(
                        "Failed to get secret '{}': {}",
                        key, service_error
                    )))
                }
            }
        }
    }

    async fn set(&self, key: &str, value: &str) -> SecretResult<()> {
        let name = self.full_name(key);

        // Store as JSON with "value" field for consistency with Vault
        let secret_string = serde_json::json!({ "value": value }).to_string();

        // Try to update first, create if not found
        match self
            .client
            .put_secret_value()
            .secret_id(&name)
            .secret_string(&secret_string)
            .send()
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                let service_error = err.into_service_error();
                if service_error.is_resource_not_found_exception() {
                    // Secret doesn't exist, create it
                    self.client
                        .create_secret()
                        .name(&name)
                        .secret_string(&secret_string)
                        .send()
                        .await
                        .map_err(|e| {
                            SecretError::Internal(format!(
                                "Failed to create secret '{}': {}",
                                key,
                                e.into_service_error()
                            ))
                        })?;
                    Ok(())
                } else {
                    Err(SecretError::Internal(format!(
                        "Failed to set secret '{}': {}",
                        key, service_error
                    )))
                }
            }
        }
    }

    async fn delete(&self, key: &str) -> SecretResult<()> {
        let name = self.full_name(key);

        // Delete with force (no recovery window) for simplicity
        match self
            .client
            .delete_secret()
            .secret_id(&name)
            .force_delete_without_recovery(true)
            .send()
            .await
        {
            Ok(_) => Ok(()),
            Err(err) => {
                let service_error = err.into_service_error();
                if service_error.is_resource_not_found_exception() {
                    // Already deleted, not an error
                    Ok(())
                } else {
                    Err(SecretError::Internal(format!(
                        "Failed to delete secret '{}': {}",
                        key, service_error
                    )))
                }
            }
        }
    }

    async fn health_check(&self) -> SecretResult<()> {
        // List secrets with max results of 1 to verify connectivity
        match self.client.list_secrets().max_results(1).send().await {
            Ok(_) => Ok(()),
            Err(err) => {
                let service_error = err.into_service_error();
                Err(SecretError::Connection(format!(
                    "AWS Secrets Manager health check failed: {}",
                    service_error
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
        let config = AwsSecretsManagerConfig::new("us-west-2")
            .with_prefix("myapp/")
            .with_endpoint_url("http://localhost:4566");

        assert_eq!(config.region, Some("us-west-2".to_string()));
        assert_eq!(config.prefix, "myapp/");
        assert_eq!(
            config.endpoint_url,
            Some("http://localhost:4566".to_string())
        );
    }

    #[test]
    fn test_config_from_env() {
        let config = AwsSecretsManagerConfig::from_env();
        assert_eq!(config.region, None);
        assert_eq!(config.prefix, "gateway/");
        assert_eq!(config.endpoint_url, None);
    }

    #[tokio::test]
    async fn test_full_name() {
        let config = AwsSecretsManagerConfig::from_env().with_prefix("gateway/providers/");
        let manager = AwsSecretsManager::new(config).await.unwrap();
        assert_eq!(manager.full_name("openai"), "gateway/providers/openai");
    }

    #[tokio::test]
    async fn test_full_name_empty_prefix() {
        let config = AwsSecretsManagerConfig::from_env().with_prefix("");
        let manager = AwsSecretsManager::new(config).await.unwrap();
        assert_eq!(manager.full_name("openai"), "openai");
    }
}
