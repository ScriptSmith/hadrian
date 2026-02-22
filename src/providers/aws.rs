//! Shared AWS utilities for Bedrock and other AWS providers.
//!
//! This module provides shared credential handling and SigV4 request signing
//! functionality used by both the Bedrock LLM provider and Bedrock Guardrails provider.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use aws_credential_types::Credentials;
use aws_sigv4::{
    http_request::{SignableBody, SignableRequest, SigningSettings},
    sign::v4::SigningParams,
};
use tokio::sync::{Notify, RwLock};

use crate::config::AwsCredentials;

/// Buffer time before credential expiry to trigger refresh (5 minutes).
/// This ensures credentials are refreshed before they actually expire,
/// preventing request failures during the refresh window.
const CREDENTIAL_REFRESH_BUFFER_SECS: u64 = 300;

/// Error type for AWS credential operations.
#[derive(Debug, thiserror::Error)]
pub enum AwsError {
    #[error("No credentials provider available")]
    NoCredentialsProvider,

    #[error("Failed to get credentials: {0}")]
    CredentialsFailed(String),

    #[error("Failed to build signing params: {0}")]
    SigningParamsBuild(String),

    #[error("Failed to create signable request: {0}")]
    SignableRequestFailed(String),

    #[error("Failed to sign request: {0}")]
    SigningFailed(String),
}

/// Shared AWS credential cache with automatic refresh.
///
/// This struct provides credential caching with automatic refresh when credentials
/// are about to expire (5 minute buffer). It uses an atomic flag to prevent the
/// "thundering herd" problem where multiple concurrent requests could all trigger
/// credential refresh operations simultaneously.
#[derive(Clone)]
pub struct AwsCredentialCache {
    credentials: Arc<RwLock<Option<Credentials>>>,
    credential_source: AwsCredentials,
    /// Atomic flag to prevent concurrent refresh operations (thundering herd).
    refreshing: Arc<AtomicBool>,
    /// Notification for waiters when refresh completes.
    refresh_notify: Arc<Notify>,
}

impl AwsCredentialCache {
    /// Creates a new credential cache with the given credential source.
    pub fn new(credential_source: AwsCredentials) -> Self {
        Self {
            credentials: Arc::new(RwLock::new(None)),
            credential_source,
            refreshing: Arc::new(AtomicBool::new(false)),
            refresh_notify: Arc::new(Notify::new()),
        }
    }

    /// Get AWS credentials, refreshing if necessary.
    ///
    /// Credentials are cached and refreshed when they expire or are within
    /// 5 minutes of expiry. Uses an atomic flag to prevent the "thundering herd"
    /// problem where multiple concurrent requests could all trigger refresh.
    pub async fn get_credentials(&self) -> Result<Credentials, AwsError> {
        loop {
            // Fast path: check cache with read lock
            {
                let cache = self.credentials.read().await;
                if let Some(creds) = cache.as_ref()
                    && Self::credentials_valid(creds)
                {
                    return Ok(creds.clone());
                }
            }

            // Credentials need refresh. Try to acquire the refresh lock.
            // Only one task will succeed; others will wait for notification.
            if self
                .refreshing
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                // We acquired the refresh lock - perform the refresh
                let result = self.fetch_credentials().await;

                // Update cache and release lock
                match &result {
                    Ok(credentials) => {
                        let mut cache = self.credentials.write().await;
                        *cache = Some(credentials.clone());
                    }
                    Err(_) => {
                        // On error, don't update cache - let next caller retry
                    }
                }

                // Release refresh lock and notify waiters
                self.refreshing.store(false, Ordering::SeqCst);
                self.refresh_notify.notify_waiters();

                return result;
            }

            // Another task is refreshing. Wait for notification then retry.
            self.refresh_notify.notified().await;
        }
    }

    /// Check if credentials are still valid (with refresh buffer).
    fn credentials_valid(creds: &Credentials) -> bool {
        match creds.expiry() {
            Some(expiry) => {
                let now = std::time::SystemTime::now();
                let buffer = std::time::Duration::from_secs(CREDENTIAL_REFRESH_BUFFER_SECS);
                expiry > now + buffer
            }
            // No expiry means static credentials - always valid
            None => true,
        }
    }

    /// Fetch fresh credentials from the configured source.
    async fn fetch_credentials(&self) -> Result<Credentials, AwsError> {
        match &self.credential_source {
            AwsCredentials::Static {
                access_key_id,
                secret_access_key,
                session_token,
            } => Ok(Credentials::new(
                access_key_id.clone(),
                secret_access_key.clone(),
                session_token.clone(),
                None,
                "static",
            )),
            AwsCredentials::Default
            | AwsCredentials::Profile { .. }
            | AwsCredentials::AssumeRole { .. } => {
                // Use the default credential chain
                let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
                let provider = config
                    .credentials_provider()
                    .ok_or(AwsError::NoCredentialsProvider)?;

                use aws_credential_types::provider::ProvideCredentials;
                provider
                    .provide_credentials()
                    .await
                    .map_err(|e| AwsError::CredentialsFailed(e.to_string()))
            }
        }
    }
}

/// Signs an HTTP request using AWS SigV4.
///
/// # Arguments
///
/// * `credentials` - AWS credentials to use for signing
/// * `region` - AWS region
/// * `service` - AWS service name (e.g., "bedrock")
/// * `method` - HTTP method (e.g., "POST")
/// * `url` - Full request URL
/// * `headers` - Request headers as (name, value) pairs
/// * `body` - Request body bytes
///
/// # Returns
///
/// A vector of (header_name, header_value) pairs to add to the request.
pub fn sign_request(
    credentials: &Credentials,
    region: &str,
    service: &str,
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Vec<(String, String)>, AwsError> {
    let identity = credentials.clone().into();

    let signing_settings = SigningSettings::default();
    let signing_params = SigningParams::builder()
        .identity(&identity)
        .region(region)
        .name(service)
        .time(std::time::SystemTime::now())
        .settings(signing_settings)
        .build()
        .map_err(|e| AwsError::SigningParamsBuild(e.to_string()))?;

    let signable_request = SignableRequest::new(
        method,
        url,
        headers.iter().copied(),
        SignableBody::Bytes(body),
    )
    .map_err(|e| AwsError::SignableRequestFailed(e.to_string()))?;

    let (signing_instructions, _signature) =
        aws_sigv4::http_request::sign(signable_request, &signing_params.into())
            .map_err(|e| AwsError::SigningFailed(e.to_string()))?
            .into_parts();

    // Extract headers from signing instructions
    let mut signed_headers = Vec::new();
    for (name, value) in signing_instructions.headers() {
        signed_headers.push((name.to_string(), value.to_string()));
    }

    Ok(signed_headers)
}

/// Helper struct for AWS request signing that combines credential cache with region/service.
///
/// This provides a convenient interface for providers that need to sign multiple requests.
pub struct AwsRequestSigner {
    credential_cache: AwsCredentialCache,
    region: String,
    service: String,
}

impl AwsRequestSigner {
    /// Creates a new request signer.
    pub fn new(
        credential_source: AwsCredentials,
        region: impl Into<String>,
        service: impl Into<String>,
    ) -> Self {
        Self {
            credential_cache: AwsCredentialCache::new(credential_source),
            region: region.into(),
            service: service.into(),
        }
    }

    /// Returns the AWS region.
    pub fn region(&self) -> &str {
        &self.region
    }

    /// Signs an HTTP request using AWS SigV4.
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method (e.g., "POST")
    /// * `url` - Full request URL
    /// * `headers` - Request headers as (name, value) pairs
    /// * `body` - Request body bytes
    ///
    /// # Returns
    ///
    /// A vector of (header_name, header_value) pairs to add to the request.
    pub async fn sign_request(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<Vec<(String, String)>, AwsError> {
        let credentials = self.credential_cache.get_credentials().await?;
        sign_request(
            &credentials,
            &self.region,
            &self.service,
            method,
            url,
            headers,
            body,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_cache_creation() {
        let cache = AwsCredentialCache::new(AwsCredentials::Default);
        // Just verify it can be created (drop ensures it was valid)
        drop(cache);
    }

    #[test]
    fn test_request_signer_creation() {
        let signer = AwsRequestSigner::new(AwsCredentials::Default, "us-east-1", "bedrock");
        assert_eq!(signer.region(), "us-east-1");
    }

    #[tokio::test]
    async fn test_static_credentials() {
        let cache = AwsCredentialCache::new(AwsCredentials::Static {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
        });

        let creds = cache.get_credentials().await.unwrap();
        assert_eq!(creds.access_key_id(), "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(
            creds.secret_access_key(),
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
        );
    }

    #[tokio::test]
    async fn test_credentials_caching() {
        let cache = AwsCredentialCache::new(AwsCredentials::Static {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
        });

        // Get credentials twice - should return cached version
        let creds1 = cache.get_credentials().await.unwrap();
        let creds2 = cache.get_credentials().await.unwrap();

        assert_eq!(creds1.access_key_id(), creds2.access_key_id());
    }

    #[test]
    fn test_sign_request_with_static_credentials() {
        let credentials = Credentials::new(
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            None,
            None,
            "test",
        );

        let result = sign_request(
            &credentials,
            "us-east-1",
            "bedrock",
            "POST",
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/test/converse",
            &[("content-type", "application/json")],
            b"{}",
        );

        assert!(result.is_ok());
        let headers = result.unwrap();

        // Should have authorization and other signing headers
        assert!(
            headers
                .iter()
                .any(|(name, _)| name.to_lowercase() == "authorization")
        );
        assert!(
            headers
                .iter()
                .any(|(name, _)| name.to_lowercase() == "x-amz-date")
        );
    }

    #[tokio::test]
    async fn test_concurrent_credential_access() {
        // Test that concurrent access doesn't cause thundering herd
        let cache = AwsCredentialCache::new(AwsCredentials::Static {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
        });

        // Spawn 100 concurrent tasks all requesting credentials
        let mut handles = Vec::new();
        for _ in 0..100 {
            let cache_clone = cache.clone();
            handles.push(tokio::spawn(
                async move { cache_clone.get_credentials().await },
            ));
        }

        // All should succeed with the same credentials
        let mut results = Vec::new();
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            results.push(result.unwrap());
        }

        // Verify all got the same credentials
        let first = &results[0];
        for creds in &results[1..] {
            assert_eq!(first.access_key_id(), creds.access_key_id());
            assert_eq!(first.secret_access_key(), creds.secret_access_key());
        }
    }

    #[test]
    fn test_credentials_valid_with_expiry() {
        use std::time::{Duration, SystemTime};

        // Credentials expiring in 10 minutes should be valid
        let future_expiry = SystemTime::now() + Duration::from_secs(600);
        let creds = Credentials::new("key", "secret", None, Some(future_expiry), "test");
        assert!(AwsCredentialCache::credentials_valid(&creds));

        // Credentials expiring in 4 minutes should NOT be valid (within 5 min buffer)
        let near_expiry = SystemTime::now() + Duration::from_secs(240);
        let creds = Credentials::new("key", "secret", None, Some(near_expiry), "test");
        assert!(!AwsCredentialCache::credentials_valid(&creds));

        // Credentials already expired should NOT be valid
        let past_expiry = SystemTime::now() - Duration::from_secs(60);
        let creds = Credentials::new("key", "secret", None, Some(past_expiry), "test");
        assert!(!AwsCredentialCache::credentials_valid(&creds));
    }

    #[test]
    fn test_credentials_valid_without_expiry() {
        // Static credentials (no expiry) should always be valid
        let creds = Credentials::new("key", "secret", None, None, "test");
        assert!(AwsCredentialCache::credentials_valid(&creds));
    }
}
