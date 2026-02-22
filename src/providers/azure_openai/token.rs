//! Azure token management for Azure AD and Managed Identity authentication.
//!
//! This module provides token acquisition and caching for Azure OpenAI authentication
//! using either Azure AD (service principal with client secret) or Managed Identity.
//!
//! Tokens are cached using `Arc<str>` to avoid heap allocations on every request.
//! Since tokens are read-heavy (same token used for millions of requests over ~55 minutes),
//! cloning an `Arc` (atomic reference count increment) is much cheaper than cloning a `String`
//! (heap allocation + memcpy).

use std::sync::Arc;

use azure_core::credentials::{AccessToken, Secret, TokenCredential};
use azure_identity::{
    ClientSecretCredential, ManagedIdentityCredential, ManagedIdentityCredentialOptions,
    UserAssignedId,
};
use tokio::sync::RwLock;

use crate::config::AzureAuth;

/// The scope required for Azure OpenAI / Cognitive Services authentication.
pub const AZURE_COGNITIVE_SERVICES_SCOPE: &str = "https://cognitiveservices.azure.com/.default";

/// Buffer time before token expiry to trigger refresh (5 minutes).
/// Ensures tokens are refreshed before they actually expire.
const TOKEN_REFRESH_BUFFER_SECS: u64 = 300;

/// A cached access token with its expiration time.
///
/// Stores the pre-formatted "Bearer {token}" header value to avoid allocating
/// a new string on every request.
#[derive(Debug, Clone)]
struct CachedToken {
    /// Pre-formatted header value: "Bearer {token}"
    bearer_header: Arc<str>,
    /// Expiration time with safety margin applied (see `TOKEN_REFRESH_BUFFER_SECS`).
    expires_at: std::time::Instant,
}

impl CachedToken {
    fn is_expired(&self) -> bool {
        std::time::Instant::now() >= self.expires_at
    }
}

/// Token source that handles Azure AD and Managed Identity authentication.
pub struct AzureTokenSource {
    credential: Arc<dyn TokenCredential>,
    auth_type: &'static str,
    cached_token: RwLock<Option<CachedToken>>,
}

impl std::fmt::Debug for AzureTokenSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureTokenSource")
            .field("type", &self.auth_type)
            .finish()
    }
}

impl AzureTokenSource {
    /// Creates a new token source from Azure AD configuration (service principal with client secret).
    pub fn from_azure_ad(
        tenant_id: &str,
        client_id: &str,
        client_secret: &str,
    ) -> Result<Self, String> {
        let credential = ClientSecretCredential::new(
            tenant_id,
            client_id.to_string(),
            Secret::new(client_secret.to_string()),
            None, // Use default options (Azure Public Cloud)
        )
        .map_err(|e| format!("Failed to create client secret credential: {}", e))?;

        Ok(Self {
            credential,
            auth_type: "AzureAD",
            cached_token: RwLock::new(None),
        })
    }

    /// Creates a new token source from Managed Identity configuration.
    ///
    /// Uses `ManagedIdentityCredential` which authenticates using Azure Managed Identity
    /// from App Service or Virtual Machine environments.
    ///
    /// For user-assigned managed identity, pass the client_id of the managed identity.
    /// For system-assigned managed identity, pass None.
    pub fn from_managed_identity(client_id: Option<&str>) -> Result<Self, String> {
        let options = client_id.map(|id| {
            tracing::info!(
                "Using user-assigned managed identity with client_id: {}",
                id
            );
            ManagedIdentityCredentialOptions {
                user_assigned_id: Some(UserAssignedId::ClientId(id.to_string())),
                ..Default::default()
            }
        });

        let credential = ManagedIdentityCredential::new(options)
            .map_err(|e| format!("Failed to create managed identity credential: {}", e))?;

        Ok(Self {
            credential,
            auth_type: "ManagedIdentity",
            cached_token: RwLock::new(None),
        })
    }

    /// Creates a token source from the Azure auth configuration.
    pub fn from_config(auth: &AzureAuth) -> Result<Self, String> {
        match auth {
            AzureAuth::ApiKey { .. } => {
                Err("Cannot create token source from API key auth".to_string())
            }
            AzureAuth::AzureAd {
                tenant_id,
                client_id,
                client_secret,
            } => Self::from_azure_ad(tenant_id, client_id, client_secret),
            AzureAuth::ManagedIdentity { client_id } => {
                Self::from_managed_identity(client_id.as_deref())
            }
        }
    }

    /// Gets a valid access token as a pre-formatted "Bearer {token}" header value.
    ///
    /// Returns an `Arc<str>` to avoid heap allocations on every request. Since tokens
    /// are cached for ~55 minutes and used for millions of requests, this is a significant
    /// optimization over returning `String`.
    ///
    /// This method is safe to call concurrently - it uses a read-write lock
    /// to minimize contention while ensuring thread-safety.
    pub async fn get_bearer_header(&self) -> Result<Arc<str>, String> {
        // Fast path: check if we have a valid cached token
        {
            let cache = self.cached_token.read().await;
            if let Some(ref cached) = *cache
                && !cached.is_expired()
            {
                return Ok(cached.bearer_header.clone());
            }
        }

        // Slow path: need to refresh the token
        let mut cache = self.cached_token.write().await;

        // Double-check after acquiring write lock (another thread may have refreshed)
        if let Some(ref cached) = *cache
            && !cached.is_expired()
        {
            return Ok(cached.bearer_header.clone());
        }

        // Fetch a new token
        let scopes = &[AZURE_COGNITIVE_SERVICES_SCOPE];
        let access_token: AccessToken = self
            .credential
            .get_token(scopes, None)
            .await
            .map_err(|e| format!("Failed to get Azure token: {}", e))?;

        // Calculate expiration with safety margin to refresh before actual expiry
        let now = time::OffsetDateTime::now_utc();
        let expires_in = access_token.expires_on - now;
        let expires_in_secs = expires_in.whole_seconds().max(0) as u64;
        let safety_margin = std::time::Duration::from_secs(TOKEN_REFRESH_BUFFER_SECS);
        let expires_at = std::time::Instant::now()
            + std::time::Duration::from_secs(expires_in_secs).saturating_sub(safety_margin);

        // Pre-format the Bearer header value to avoid allocating on every request
        let bearer_header: Arc<str> = format!("Bearer {}", access_token.token.secret()).into();

        *cache = Some(CachedToken {
            bearer_header: bearer_header.clone(),
            expires_at,
        });

        tracing::debug!(
            "Acquired new Azure {} token, expires in {} seconds",
            self.auth_type,
            expires_in_secs
        );

        Ok(bearer_header)
    }

    /// Clears the cached token, forcing a refresh on the next call.
    #[allow(dead_code)] // Public API for token management
    pub async fn clear_cache(&self) {
        let mut cache = self.cached_token.write().await;
        *cache = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_token_expiry() {
        let token = CachedToken {
            bearer_header: "Bearer test".into(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(3600),
        };
        assert!(!token.is_expired());

        let expired_token = CachedToken {
            bearer_header: "Bearer test".into(),
            expires_at: std::time::Instant::now() - std::time::Duration::from_secs(1),
        };
        assert!(expired_token.is_expired());
    }

    #[test]
    fn test_arc_str_clone_is_cheap() {
        // Verify that Arc<str> clone is just a reference count increment
        let bearer: Arc<str> = "Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...".into();
        let clone1 = bearer.clone();
        let clone2 = bearer.clone();

        // All clones point to the same allocation
        assert!(std::ptr::eq(bearer.as_ptr(), clone1.as_ptr()));
        assert!(std::ptr::eq(bearer.as_ptr(), clone2.as_ptr()));

        // Strong count should be 3
        assert_eq!(Arc::strong_count(&bearer), 3);
    }

    #[test]
    fn test_from_config_api_key_error() {
        let auth = AzureAuth::ApiKey {
            api_key: "test".to_string(),
        };
        let result = AzureTokenSource::from_config(&auth);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Cannot create token source from API key")
        );
    }
}
