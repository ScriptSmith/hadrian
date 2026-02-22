//! Azure OpenAI provider.
//!
//! This provider implements the Azure OpenAI API, which is similar to the
//! standard OpenAI API but uses Azure-specific authentication and URL structure.
//!
//! Supports three authentication methods:
//! - API Key: Simple static key authentication
//! - Azure AD: Service principal with client secret (OAuth2 client credentials flow)
//! - Managed Identity: Automatic authentication using Azure VM or App Service identity

mod token;

use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::response::Response;
use token::AzureTokenSource;

use crate::{
    api_types::{
        CreateChatCompletionPayload, CreateCompletionPayload, CreateEmbeddingPayload,
        CreateResponsesPayload,
    },
    config::{AzureAuth, AzureOpenAiProviderConfig, CircuitBreakerConfig, RetryConfig},
    providers::{
        self, CircuitBreakerRegistry, ModelsResponse, Provider, ProviderError,
        circuit_breaker::CircuitBreaker, error::AzureOpenAiErrorParser, response::error_response,
        retry::with_circuit_breaker_and_retry,
    },
};

/// Authentication method for Azure OpenAI requests.
///
/// Uses `Arc<str>` for cached values to avoid heap allocations on every request.
/// Since auth headers are used for millions of requests with the same credentials,
/// cloning an `Arc` (atomic ref count increment) is much cheaper than cloning a `String`.
enum AzureAuthMethod {
    /// Static API key authentication (uses `api-key` header).
    ApiKey(Arc<str>),
    /// Token-based authentication using Azure AD or Managed Identity (uses `Authorization: Bearer` header).
    Token(Arc<AzureTokenSource>),
}

impl std::fmt::Debug for AzureAuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AzureAuthMethod::ApiKey(_) => write!(f, "ApiKey(****)"),
            AzureAuthMethod::Token(source) => write!(f, "Token({:?})", source),
        }
    }
}

/// Azure OpenAI provider with support for API key, Azure AD, and Managed Identity auth.
pub struct AzureOpenAIProvider {
    base_url: String,
    api_version: String,
    auth: AzureAuthMethod,
    deployments: HashMap<String, String>, // model -> deployment_id
    timeout: Duration,
    retry: RetryConfig,
    circuit_breaker_config: CircuitBreakerConfig,
    circuit_breaker: Option<Arc<CircuitBreaker>>,
}

impl AzureOpenAIProvider {
    /// Create a provider from configuration with a shared circuit breaker.
    pub fn from_config_with_registry(
        config: &AzureOpenAiProviderConfig,
        provider_name: &str,
        registry: &CircuitBreakerRegistry,
    ) -> Self {
        let base = Self::build_base(config);
        let circuit_breaker = registry.get_or_create(provider_name, &config.circuit_breaker);

        Self {
            base_url: base.0,
            api_version: base.1,
            auth: base.2,
            deployments: base.3,
            timeout: Duration::from_secs(config.timeout_secs),
            retry: config.retry.clone(),
            circuit_breaker_config: config.circuit_breaker.clone(),
            circuit_breaker,
        }
    }

    /// Build base configuration from provider config.
    fn build_base(
        config: &AzureOpenAiProviderConfig,
    ) -> (String, String, AzureAuthMethod, HashMap<String, String>) {
        let base_url = config.base_url();
        let api_version = config.api_version.clone();

        // Build model -> deployment mapping
        let mut deployments = HashMap::new();
        for (deployment_id, deployment) in &config.deployments {
            deployments.insert(deployment.model.clone(), deployment_id.clone());
        }

        let auth = match &config.auth {
            AzureAuth::ApiKey { api_key } => AzureAuthMethod::ApiKey(api_key.as_str().into()),
            auth @ (AzureAuth::AzureAd { .. } | AzureAuth::ManagedIdentity { .. }) => {
                match AzureTokenSource::from_config(auth) {
                    Ok(source) => {
                        tracing::info!("Azure OpenAI configured with {:?} authentication", source);
                        AzureAuthMethod::Token(Arc::new(source))
                    }
                    Err(e) => {
                        // This shouldn't happen with valid config, but handle gracefully
                        tracing::error!("Failed to initialize Azure token source: {}", e);
                        panic!("Failed to initialize Azure token source: {}", e);
                    }
                }
            }
        };

        (base_url, api_version, auth, deployments)
    }

    /// Get the deployment ID for a model, falling back to using the model name as deployment.
    fn deployment_for_model<'a>(&'a self, model: &'a str) -> &'a str {
        match self.deployments.get(model) {
            Some(deployment) => deployment.as_str(),
            None => {
                if !self.deployments.is_empty() {
                    tracing::warn!(
                        model = model,
                        configured_models = ?self.deployments.keys().collect::<Vec<_>>(),
                        "Model not found in Azure deployment mappings, using model name as deployment ID. \
                         This may cause 404 errors if the deployment doesn't exist."
                    );
                }
                model
            }
        }
    }

    /// Get the authentication header name and value for a request.
    ///
    /// Returns `Arc<str>` for the value to avoid heap allocations on every request.
    /// The header name is a static string since it never changes.
    async fn get_auth_header(&self) -> Result<(&'static str, Arc<str>), ProviderError> {
        match &self.auth {
            AzureAuthMethod::ApiKey(key) => Ok(("api-key", key.clone())),
            AzureAuthMethod::Token(source) => {
                let bearer_header = source
                    .get_bearer_header()
                    .await
                    .map_err(ProviderError::Internal)?;
                Ok(("Authorization", bearer_header))
            }
        }
    }
}

#[async_trait]
impl Provider for AzureOpenAIProvider {
    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "azure_openai",
            operation = "chat_completion",
            model = %payload.model.as_deref().unwrap_or("gpt-4"),
            stream = payload.stream
        )
    )]
    async fn create_chat_completion(
        &self,
        client: &reqwest::Client,
        payload: CreateChatCompletionPayload,
    ) -> Result<Response, ProviderError> {
        let (header_name, header_value) = self.get_auth_header().await?;
        let timeout = self.timeout;

        let model = payload.model.as_deref().unwrap_or("gpt-4");
        let deployment = self.deployment_for_model(model);
        let stream = payload.stream;

        // Pre-serialize request body before retry loop to avoid repeated serialization
        let body = serde_json::to_vec(&payload).unwrap_or_default();

        let url = format!(
            "{}/deployments/{}/chat/completions?api-version={}",
            self.base_url, deployment, self.api_version
        );

        let response = with_circuit_breaker_and_retry(
            self.circuit_breaker.as_deref(),
            &self.circuit_breaker_config,
            &self.retry,
            "azure_openai",
            "chat_completion",
            || async {
                client
                    .post(&url)
                    .header(header_name, &*header_value)
                    .header("content-type", "application/json")
                    .timeout(timeout)
                    .body(body.clone())
                    .send()
                    .await
            },
        )
        .await?;

        if !response.status().is_success() {
            return error_response::<AzureOpenAiErrorParser>(response).await;
        }
        providers::build_response(response, stream).await
    }

    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "azure_openai",
            operation = "responses",
            model = %payload.model.as_deref().unwrap_or("gpt-4o"),
            stream = payload.stream
        )
    )]
    async fn create_responses(
        &self,
        client: &reqwest::Client,
        payload: CreateResponsesPayload,
    ) -> Result<Response, ProviderError> {
        let (header_name, header_value) = self.get_auth_header().await?;
        let timeout = self.timeout;
        let stream = payload.stream;

        // Pre-serialize request body before retry loop to avoid repeated serialization
        let body = serde_json::to_vec(&payload).unwrap_or_default();

        // Azure OpenAI v1 API uses /openai/v1/responses endpoint
        // This is the new unified API that doesn't require deployment names in the path
        // base_url already includes /openai, so we just append /v1/responses
        let url = format!("{}/v1/responses", self.base_url);

        let response = with_circuit_breaker_and_retry(
            self.circuit_breaker.as_deref(),
            &self.circuit_breaker_config,
            &self.retry,
            "azure_openai",
            "responses",
            || async {
                client
                    .post(&url)
                    .header(header_name, &*header_value)
                    .header("content-type", "application/json")
                    .timeout(timeout)
                    .body(body.clone())
                    .send()
                    .await
            },
        )
        .await?;

        if !response.status().is_success() {
            return error_response::<AzureOpenAiErrorParser>(response).await;
        }
        providers::build_response(response, stream).await
    }

    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "azure_openai",
            operation = "completion",
            model = %payload.model.as_deref().unwrap_or("gpt-35-turbo-instruct"),
            stream = payload.stream
        )
    )]
    async fn create_completion(
        &self,
        client: &reqwest::Client,
        payload: CreateCompletionPayload,
    ) -> Result<Response, ProviderError> {
        let (header_name, header_value) = self.get_auth_header().await?;
        let timeout = self.timeout;

        let model = payload.model.as_deref().unwrap_or("gpt-35-turbo-instruct");
        let deployment = self.deployment_for_model(model);
        let stream = payload.stream;

        // Pre-serialize request body before retry loop to avoid repeated serialization
        let body = serde_json::to_vec(&payload).unwrap_or_default();

        let url = format!(
            "{}/deployments/{}/completions?api-version={}",
            self.base_url, deployment, self.api_version
        );

        let response = with_circuit_breaker_and_retry(
            self.circuit_breaker.as_deref(),
            &self.circuit_breaker_config,
            &self.retry,
            "azure_openai",
            "completion",
            || async {
                client
                    .post(&url)
                    .header(header_name, &*header_value)
                    .header("content-type", "application/json")
                    .timeout(timeout)
                    .body(body.clone())
                    .send()
                    .await
            },
        )
        .await?;

        if !response.status().is_success() {
            return error_response::<AzureOpenAiErrorParser>(response).await;
        }
        providers::build_response(response, stream).await
    }

    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "azure_openai",
            operation = "embedding",
            model = %payload.model
        )
    )]
    async fn create_embedding(
        &self,
        client: &reqwest::Client,
        payload: CreateEmbeddingPayload,
    ) -> Result<Response, ProviderError> {
        let (header_name, header_value) = self.get_auth_header().await?;
        let timeout = self.timeout;

        let deployment = self.deployment_for_model(&payload.model);

        // Pre-serialize request body before retry loop to avoid repeated serialization
        let body = serde_json::to_vec(&payload).unwrap_or_default();

        let url = format!(
            "{}/deployments/{}/embeddings?api-version={}",
            self.base_url, deployment, self.api_version
        );

        let response = with_circuit_breaker_and_retry(
            self.circuit_breaker.as_deref(),
            &self.circuit_breaker_config,
            &self.retry.for_embedding(),
            "azure_openai",
            "embedding",
            || async {
                client
                    .post(&url)
                    .header(header_name, &*header_value)
                    .header("content-type", "application/json")
                    .timeout(timeout)
                    .body(body.clone())
                    .send()
                    .await
            },
        )
        .await?;

        if !response.status().is_success() {
            return error_response::<AzureOpenAiErrorParser>(response).await;
        }
        providers::build_response(response, false).await
    }

    #[tracing::instrument(
        skip(self, client),
        fields(provider = "azure_openai", operation = "list_models")
    )]
    async fn list_models(&self, client: &reqwest::Client) -> Result<ModelsResponse, ProviderError> {
        let (header_name, header_value) = self.get_auth_header().await?;
        let timeout = self.timeout;

        // Azure OpenAI v1 API uses /openai/v1/models endpoint
        // base_url already includes /openai, so we just append /v1/models
        let url = format!("{}/v1/models", self.base_url);

        let response = with_circuit_breaker_and_retry(
            self.circuit_breaker.as_deref(),
            &self.circuit_breaker_config,
            &self.retry,
            "azure_openai",
            "list_models",
            || async {
                client
                    .get(&url)
                    .header(header_name, &*header_value)
                    .timeout(timeout)
                    .send()
                    .await
            },
        )
        .await?;

        let models: ModelsResponse = response.json().await?;
        Ok(models)
    }
}
