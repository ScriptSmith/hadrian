//! AWS Bedrock provider.
//!
//! This provider implements the AWS Bedrock API for accessing foundation models
//! like Claude, Llama, and others. Uses AWS SigV4 for request signing.

mod convert;
mod stream;
mod types;

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use axum::response::Response;
use tokio::sync::RwLock;

use self::{
    convert::*,
    stream::{BedrockToOpenAIStream, BedrockToResponsesStream},
    types::{
        BedrockConverseRequest, BedrockInferenceConfig, BedrockToolConfig,
        ListFoundationModelsResponse, ListInferenceProfilesResponse, TitanEmbeddingsRequest,
        TitanEmbeddingsResponse,
    },
};
use crate::{
    api_types::{
        CreateChatCompletionPayload, CreateCompletionPayload, CreateEmbeddingPayload,
        CreateResponsesPayload,
        embeddings::{
            CreateEmbeddingResponse, EmbeddingData, EmbeddingInput, EmbeddingObjectType,
            EmbeddingResponseObjectType, EmbeddingUsage, EmbeddingVector,
        },
    },
    config::{BedrockProviderConfig, CircuitBreakerConfig, RetryConfig, StreamingBufferConfig},
    providers::{
        CircuitBreakerRegistry, ModelInfo, ModelsResponse, Provider, ProviderError,
        aws::AwsRequestSigner,
        circuit_breaker::CircuitBreaker,
        error::BedrockErrorParser,
        image::{ImageFetchConfig, preprocess_messages_for_images},
        response::{error_response, json_response, streaming_response},
    },
};

const SERVICE_NAME: &str = "bedrock";
/// How long to cache inference profiles before refreshing
const INFERENCE_PROFILE_CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour
/// How long to cache foundation models before refreshing
const FOUNDATION_MODELS_CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour

/// Cache for inference profile mappings (model_id -> inference_profile_id)
#[derive(Debug, Default)]
struct InferenceProfileCache {
    /// Maps model ARN suffix (e.g., "anthropic.claude-sonnet-4-5-20250929-v1:0")
    /// to inference profile ID (e.g., "us.anthropic.claude-sonnet-4-5-20250929-v1:0")
    model_to_profile: HashMap<String, String>,
    /// When the cache was last updated
    last_updated: Option<Instant>,
}

impl InferenceProfileCache {
    fn is_stale(&self) -> bool {
        self.last_updated
            .map(|t| t.elapsed() > INFERENCE_PROFILE_CACHE_TTL)
            .unwrap_or(true)
    }

    /// Extract model ID from a model ARN.
    /// e.g., "arn:aws:bedrock:us-east-1::foundation-model/anthropic.claude-sonnet-4-5-20250929-v1:0"
    /// -> "anthropic.claude-sonnet-4-5-20250929-v1:0"
    fn extract_model_id(model_arn: &str) -> Option<&str> {
        model_arn.rsplit('/').next()
    }
}

/// Cache for foundation models list
#[derive(Debug, Default)]
struct FoundationModelsCache {
    /// Cached list of models
    models: Vec<ModelInfo>,
    /// When the cache was last updated
    last_updated: Option<Instant>,
}

impl FoundationModelsCache {
    fn is_stale(&self) -> bool {
        self.last_updated
            .map(|t| t.elapsed() > FOUNDATION_MODELS_CACHE_TTL)
            .unwrap_or(true)
    }
}

pub struct BedrockProvider {
    /// Signer for Bedrock Runtime API (converse, invoke-model)
    runtime_signer: AwsRequestSigner,
    /// Signer for Bedrock Control Plane API (list-inference-profiles, list-foundation-models)
    control_plane_signer: AwsRequestSigner,
    region: String,
    timeout: Duration,
    retry: RetryConfig,
    circuit_breaker_config: CircuitBreakerConfig,
    circuit_breaker: Option<Arc<CircuitBreaker>>,
    streaming_buffer: StreamingBufferConfig,
    image_fetch_config: ImageFetchConfig,
    /// Custom Converse API base URL override.
    converse_base_url_override: Option<String>,
    /// Cached inference profiles
    inference_profile_cache: Arc<RwLock<InferenceProfileCache>>,
    /// Cached foundation models
    foundation_models_cache: Arc<RwLock<FoundationModelsCache>>,
}

impl BedrockProvider {
    /// Create a provider from configuration with a shared circuit breaker.
    pub fn from_config_with_registry(
        config: &BedrockProviderConfig,
        provider_name: &str,
        registry: &CircuitBreakerRegistry,
    ) -> Self {
        Self::from_config_with_registry_and_image_config(
            config,
            provider_name,
            registry,
            ImageFetchConfig::default(),
        )
    }

    /// Create a provider from configuration with a shared circuit breaker and custom image fetch config.
    pub fn from_config_with_registry_and_image_config(
        config: &BedrockProviderConfig,
        provider_name: &str,
        registry: &CircuitBreakerRegistry,
        image_fetch_config: ImageFetchConfig,
    ) -> Self {
        let circuit_breaker = registry.get_or_create(provider_name, &config.circuit_breaker);

        Self {
            // Both runtime and control plane APIs use "bedrock" as the service name for signing
            runtime_signer: AwsRequestSigner::new(
                config.credentials.clone(),
                config.region.clone(),
                SERVICE_NAME,
            ),
            control_plane_signer: AwsRequestSigner::new(
                config.credentials.clone(),
                config.region.clone(),
                SERVICE_NAME,
            ),
            region: config.region.clone(),
            timeout: Duration::from_secs(config.timeout_secs),
            retry: config.retry.clone(),
            circuit_breaker_config: config.circuit_breaker.clone(),
            circuit_breaker,
            streaming_buffer: config.streaming_buffer.clone(),
            image_fetch_config,
            converse_base_url_override: config.converse_base_url.clone(),
            inference_profile_cache: Arc::new(RwLock::new(InferenceProfileCache::default())),
            foundation_models_cache: Arc::new(RwLock::new(FoundationModelsCache::default())),
        }
    }

    /// Sign a request for the Bedrock Runtime API using AWS SigV4.
    async fn sign_runtime_request(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<Vec<(String, String)>, ProviderError> {
        self.runtime_signer
            .sign_request(method, url, headers, body)
            .await
            .map_err(|e| ProviderError::Internal(e.to_string()))
    }

    /// Sign a request for the Bedrock Control Plane API using AWS SigV4.
    async fn sign_control_plane_request(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<Vec<(String, String)>, ProviderError> {
        self.control_plane_signer
            .sign_request(method, url, headers, body)
            .await
            .map_err(|e| ProviderError::Internal(e.to_string()))
    }

    fn runtime_base_url(&self) -> String {
        self.converse_base_url_override
            .clone()
            .unwrap_or_else(|| format!("https://bedrock-runtime.{}.amazonaws.com", self.region))
    }

    fn control_plane_base_url(&self) -> String {
        format!("https://bedrock.{}.amazonaws.com", self.region)
    }

    /// Fetch inference profiles from the Bedrock API and update the cache.
    async fn refresh_inference_profiles(
        &self,
        client: &reqwest::Client,
    ) -> Result<(), ProviderError> {
        let url = format!(
            "{}/inference-profiles?maxResults=1000&type=SYSTEM_DEFINED",
            self.control_plane_base_url()
        );

        let headers: [(&str, &str); 0] = [];
        let signed_headers = self
            .sign_control_plane_request("GET", &url, &headers, &[])
            .await?;

        let mut request = client.get(&url).timeout(self.timeout);
        for (name, value) in signed_headers {
            request = request.header(name, value);
        }

        let response = request.send().await.map_err(ProviderError::Request)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::warn!(
                status = %status,
                body = %body,
                "Failed to fetch inference profiles, using fallback pattern matching"
            );
            return Ok(()); // Don't fail, just use fallback
        }

        let profiles_response: ListInferenceProfilesResponse =
            response.json().await.map_err(ProviderError::Request)?;

        let mut cache = self.inference_profile_cache.write().await;
        cache.model_to_profile.clear();

        for profile in profiles_response.inference_profile_summaries {
            // Extract model IDs from the model ARNs and map to inference profile ID
            for model in &profile.models {
                if let Some(model_id) = InferenceProfileCache::extract_model_id(&model.model_arn) {
                    cache
                        .model_to_profile
                        .insert(model_id.to_string(), profile.inference_profile_id.clone());
                }
            }
        }

        cache.last_updated = Some(Instant::now());
        tracing::debug!(
            profiles_count = cache.model_to_profile.len(),
            "Cached inference profiles"
        );

        Ok(())
    }

    /// Get the inference ID to use for a model, fetching profiles if needed.
    async fn get_inference_id(
        &self,
        client: &reqwest::Client,
        model: &str,
    ) -> Result<String, ProviderError> {
        // If the model already has an inference profile prefix or is an ARN, use as-is
        if let Some(prefix) = model.split('.').next()
            && (prefix == "global"
                || prefix == "us"
                || prefix == "eu"
                || prefix == "ap"
                || prefix.starts_with("arn:"))
        {
            return Ok(model.to_string());
        }

        // Check cache (read lock)
        {
            let cache = self.inference_profile_cache.read().await;
            if !cache.is_stale() {
                if let Some(profile_id) = cache.model_to_profile.get(model) {
                    return Ok(profile_id.clone());
                }
                // Model not in cache, use as-is
                return Ok(model.to_string());
            }
        }

        // Cache is stale, refresh it
        self.refresh_inference_profiles(client).await?;

        // Check again after refresh
        let cache = self.inference_profile_cache.read().await;
        if let Some(profile_id) = cache.model_to_profile.get(model) {
            return Ok(profile_id.clone());
        }

        // Model not found in inference profiles, use the model ID directly
        Ok(model.to_string())
    }

    /// Fetch foundation models from the Bedrock API and update the cache.
    async fn refresh_foundation_models(
        &self,
        client: &reqwest::Client,
    ) -> Result<(), ProviderError> {
        // Fetch models that support ON_DEMAND inference (serverless)
        let url = format!(
            "{}/foundation-models?byInferenceType=ON_DEMAND",
            self.control_plane_base_url()
        );

        let headers: [(&str, &str); 0] = [];
        let signed_headers = self
            .sign_control_plane_request("GET", &url, &headers, &[])
            .await?;

        let mut request = client.get(&url).timeout(self.timeout);
        for (name, value) in signed_headers {
            request = request.header(name, value);
        }

        let response = request.send().await.map_err(ProviderError::Request)?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::warn!(
                status = %status,
                body = %body,
                "Failed to fetch foundation models from Bedrock API"
            );
            return Err(ProviderError::Internal(format!(
                "Failed to fetch foundation models: {} - {}",
                status, body
            )));
        }

        let models_response: ListFoundationModelsResponse =
            response.json().await.map_err(ProviderError::Request)?;

        let models: Vec<ModelInfo> = models_response
            .model_summaries
            .into_iter()
            .map(|summary| {
                let mut extra = serde_json::json!({
                    "provider": "bedrock",
                });

                if let Some(provider) = summary.provider_name {
                    extra["owned_by"] = serde_json::json!(provider.to_lowercase());
                }

                if !summary.input_modalities.is_empty() {
                    extra["input_modalities"] = serde_json::json!(summary.input_modalities);
                }

                if !summary.output_modalities.is_empty() {
                    extra["output_modalities"] = serde_json::json!(summary.output_modalities);
                }

                if summary.response_streaming_supported == Some(true) {
                    extra["streaming"] = serde_json::json!(true);
                }

                if let Some(lifecycle) = summary.model_lifecycle {
                    extra["lifecycle_status"] = serde_json::json!(lifecycle.status);
                }

                ModelInfo {
                    id: summary.model_id,
                    extra,
                }
            })
            .collect();

        let mut cache = self.foundation_models_cache.write().await;
        cache.models = models;
        cache.last_updated = Some(Instant::now());

        tracing::debug!(
            models_count = cache.models.len(),
            "Cached foundation models from Bedrock API"
        );

        Ok(())
    }

    /// Get the list of foundation models, fetching from API if cache is stale.
    async fn get_foundation_models(
        &self,
        client: &reqwest::Client,
    ) -> Result<Vec<ModelInfo>, ProviderError> {
        // Check cache (read lock)
        {
            let cache = self.foundation_models_cache.read().await;
            if !cache.is_stale() {
                return Ok(cache.models.clone());
            }
        }

        // Cache is stale, refresh it
        self.refresh_foundation_models(client).await?;

        // Return the refreshed cache
        let cache = self.foundation_models_cache.read().await;
        Ok(cache.models.clone())
    }

    /// Execute a signed request with retry logic.
    ///
    /// AWS SigV4 signing must be redone per attempt since signatures are time-based.
    /// On exhaustion or non-retryable failure, records a circuit breaker failure.
    async fn retry_signed_request(
        &self,
        client: &reqwest::Client,
        url: &str,
        body: &[u8],
        operation: &str,
    ) -> Result<reqwest::Response, ProviderError> {
        let max_attempts = if self.retry.enabled {
            self.retry.max_retries + 1
        } else {
            1
        };

        let mut last_error: Option<ProviderError> = None;

        for attempt in 0..max_attempts {
            let headers = [("content-type", "application/json")];
            let signed_headers = match self.sign_runtime_request("POST", url, &headers, body).await
            {
                Ok(h) => h,
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_attempts - 1 {
                        let delay = self.retry.delay_for_attempt(attempt);
                        tracing::warn!(
                            provider = "bedrock",
                            operation,
                            attempt = attempt + 1,
                            max_attempts,
                            delay_ms = delay.as_millis(),
                            "Signing failed, will retry after delay"
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    break;
                }
            };

            let mut request = client
                .post(url)
                .header("content-type", "application/json")
                .timeout(self.timeout);
            for (name, value) in signed_headers {
                request = request.header(name, value);
            }

            match request.body(body.to_vec()).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if self.retry.should_retry_status(status.as_u16()) && attempt < max_attempts - 1
                    {
                        let delay = self.retry.delay_for_attempt(attempt);
                        tracing::warn!(
                            provider = "bedrock",
                            operation,
                            status = %status,
                            attempt = attempt + 1,
                            max_attempts,
                            delay_ms = delay.as_millis(),
                            "Retryable status code, will retry after delay"
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    if crate::providers::retry::is_retryable_error(&e) && attempt < max_attempts - 1
                    {
                        let delay = self.retry.delay_for_attempt(attempt);
                        tracing::warn!(
                            provider = "bedrock",
                            operation,
                            error = %e,
                            attempt = attempt + 1,
                            max_attempts,
                            delay_ms = delay.as_millis(),
                            "Retryable error, will retry after delay"
                        );
                        tokio::time::sleep(delay).await;
                        last_error = Some(ProviderError::Request(e));
                        continue;
                    }
                    if let Some(cb) = &self.circuit_breaker {
                        cb.record_failure();
                    }
                    return Err(ProviderError::Request(e));
                }
            }
        }

        if let Some(cb) = &self.circuit_breaker {
            cb.record_failure();
        }
        Err(last_error
            .unwrap_or_else(|| ProviderError::Internal("All retry attempts exhausted".to_string())))
    }
}

#[async_trait]
impl Provider for BedrockProvider {
    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "bedrock",
            operation = "chat_completion",
            model = %payload.model.as_deref().unwrap_or("anthropic.claude-3-sonnet-20240229-v1:0"),
            stream = payload.stream
        )
    )]
    async fn create_chat_completion(
        &self,
        client: &reqwest::Client,
        payload: CreateChatCompletionPayload,
    ) -> Result<Response, ProviderError> {
        // Check circuit breaker before attempting request
        if let Some(cb) = &self.circuit_breaker {
            cb.check()?;
        }

        let model = payload
            .model
            .clone()
            .unwrap_or_else(|| "anthropic.claude-3-sonnet-20240229-v1:0".to_string());

        // Preprocess messages to convert HTTP image URLs to data URLs
        // Bedrock only supports base64 images, so we fetch HTTP URLs and convert them
        let mut messages_to_convert = payload.messages;
        preprocess_messages_for_images(
            client,
            &mut messages_to_convert,
            Some(&self.image_fetch_config),
        )
        .await;

        let (system, messages) = convert_messages(messages_to_convert);

        // Convert tools and tool_choice
        let tools = convert_tools(payload.tools);
        let tool_choice = if tools.is_some() {
            convert_tool_choice(payload.tool_choice)
        } else {
            None
        };

        let tool_config = tools.map(|tools| BedrockToolConfig { tools, tool_choice });

        // Bedrock Claude models don't allow both temperature and top_p
        // Prefer temperature if set, otherwise use top_p
        let (temperature, top_p) = if payload.temperature.is_some() {
            (payload.temperature, None)
        } else {
            (None, payload.top_p)
        };

        // Convert reasoning config based on model type
        let additional_model_request_fields = if is_claude_model(&model) {
            convert_chat_completion_reasoning_to_bedrock_claude(payload.reasoning.as_ref(), &model)
        } else if is_nova_model(&model) {
            convert_chat_completion_reasoning_to_bedrock_nova(payload.reasoning.as_ref())
        } else {
            None
        };

        let bedrock_request = BedrockConverseRequest {
            messages,
            system,
            inference_config: Some(BedrockInferenceConfig {
                max_tokens: payload.max_tokens,
                temperature,
                top_p,
                stop_sequences: convert_stop(payload.stop),
            }),
            tool_config,
            additional_model_request_fields,
        };

        let body = serde_json::to_vec(&bedrock_request).unwrap_or_default();

        // Get inference profile ID if required (fetches from API and caches)
        let inference_id = self.get_inference_id(client, &model).await?;

        if payload.stream {
            let stream_url = format!(
                "{}/model/{}/converse-stream",
                self.runtime_base_url(),
                inference_id
            );

            let response = self
                .retry_signed_request(client, &stream_url, &body, "chat_completion_stream")
                .await?;
            let status = response.status();

            if !status.is_success() {
                if let Some(cb) = &self.circuit_breaker {
                    cb.record_failure();
                }
                return error_response::<BedrockErrorParser>(response).await;
            }

            if let Some(cb) = &self.circuit_breaker {
                cb.record_success();
            }

            let byte_stream = response.bytes_stream();
            let transformed_stream =
                BedrockToOpenAIStream::new(byte_stream, model, &self.streaming_buffer);

            return streaming_response(status, transformed_stream);
        }

        let url = format!(
            "{}/model/{}/converse",
            self.runtime_base_url(),
            inference_id
        );

        let response = self
            .retry_signed_request(client, &url, &body, "chat_completion")
            .await?;
        let status = response.status();

        // Record result to circuit breaker
        if let Some(cb) = &self.circuit_breaker {
            if self
                .circuit_breaker_config
                .is_failure_status(status.as_u16())
            {
                cb.record_failure();
            } else {
                cb.record_success();
            }
        }

        if !status.is_success() {
            return error_response::<BedrockErrorParser>(response).await;
        }

        let bedrock_response: types::BedrockConverseResponse = response.json().await?;
        let openai_response = convert_response(bedrock_response, &model);
        json_response(status, &openai_response)
    }

    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "bedrock",
            operation = "responses",
            model = %payload.model.as_deref().unwrap_or("anthropic.claude-3-sonnet-20240229-v1:0"),
            stream = payload.stream
        )
    )]
    async fn create_responses(
        &self,
        client: &reqwest::Client,
        payload: CreateResponsesPayload,
    ) -> Result<Response, ProviderError> {
        // Check circuit breaker before attempting request
        if let Some(cb) = &self.circuit_breaker {
            cb.check()?;
        }

        let model = payload
            .model
            .clone()
            .unwrap_or_else(|| "anthropic.claude-3-sonnet-20240229-v1:0".to_string());

        // Convert Responses API input to Bedrock Converse format
        let (system, messages) =
            convert_responses_input_to_bedrock_messages(payload.input, payload.instructions);

        // Convert tools and tool_choice
        let tools = convert_responses_tools_to_bedrock(payload.tools);
        let tool_choice = if tools.is_some() {
            convert_responses_tool_choice_to_bedrock(payload.tool_choice)
        } else {
            None
        };

        let tool_config = tools.map(|tools| BedrockToolConfig { tools, tool_choice });

        // Bedrock Claude models don't allow both temperature and top_p
        // Prefer temperature if set, otherwise use top_p
        let (temperature, top_p) = if payload.temperature.is_some() {
            (payload.temperature, None)
        } else {
            (None, payload.top_p)
        };

        // Convert reasoning config based on model type
        let additional_model_request_fields = if is_claude_model(&model) {
            convert_responses_reasoning_to_bedrock_claude(payload.reasoning.as_ref(), &model)
        } else if is_nova_model(&model) {
            convert_responses_reasoning_to_bedrock_nova(payload.reasoning.as_ref())
        } else {
            None
        };

        let bedrock_request = BedrockConverseRequest {
            messages,
            system,
            inference_config: Some(BedrockInferenceConfig {
                max_tokens: payload.max_output_tokens.map(|t| t as u64),
                temperature,
                top_p,
                stop_sequences: None,
            }),
            tool_config,
            additional_model_request_fields,
        };

        let body = serde_json::to_vec(&bedrock_request).unwrap_or_default();

        // Get inference profile ID if required (fetches from API and caches)
        let inference_id = self.get_inference_id(client, &model).await?;

        if payload.stream {
            let stream_url = format!(
                "{}/model/{}/converse-stream",
                self.runtime_base_url(),
                inference_id
            );

            let response = self
                .retry_signed_request(client, &stream_url, &body, "responses_stream")
                .await?;
            let status = response.status();

            if !status.is_success() {
                if let Some(cb) = &self.circuit_breaker {
                    cb.record_failure();
                }
                return error_response::<BedrockErrorParser>(response).await;
            }

            if let Some(cb) = &self.circuit_breaker {
                cb.record_success();
            }

            let byte_stream = response.bytes_stream();
            let transformed_stream =
                BedrockToResponsesStream::new(byte_stream, model, &self.streaming_buffer);

            return streaming_response(status, transformed_stream);
        }

        let url = format!(
            "{}/model/{}/converse",
            self.runtime_base_url(),
            inference_id
        );

        let response = self
            .retry_signed_request(client, &url, &body, "responses")
            .await?;
        let status = response.status();

        // Record result to circuit breaker
        if let Some(cb) = &self.circuit_breaker {
            if self
                .circuit_breaker_config
                .is_failure_status(status.as_u16())
            {
                cb.record_failure();
            } else {
                cb.record_success();
            }
        }

        if !status.is_success() {
            return error_response::<BedrockErrorParser>(response).await;
        }

        let bedrock_response: types::BedrockConverseResponse = response.json().await?;
        let responses_response = convert_bedrock_to_responses_response(
            bedrock_response,
            &model,
            payload.reasoning.as_ref(),
            payload.user,
        );
        json_response(status, &responses_response)
    }

    #[tracing::instrument(
        skip(self, _client, _payload),
        fields(provider = "bedrock", operation = "completion")
    )]
    async fn create_completion(
        &self,
        _client: &reqwest::Client,
        _payload: CreateCompletionPayload,
    ) -> Result<Response, ProviderError> {
        Ok(Response::builder()
            .status(http::StatusCode::NOT_IMPLEMENTED)
            .body(axum::body::Body::from(
                r#"{"error": "Legacy completions API not supported for Bedrock provider"}"#,
            ))?)
    }

    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "bedrock",
            operation = "embedding",
            model = %payload.model
        )
    )]
    async fn create_embedding(
        &self,
        client: &reqwest::Client,
        payload: CreateEmbeddingPayload,
    ) -> Result<Response, ProviderError> {
        // Check circuit breaker before attempting request
        if let Some(cb) = &self.circuit_breaker {
            cb.check()?;
        }

        let model = payload.model.clone();

        // Convert OpenAI input format to list of texts
        let texts: Vec<String> = match &payload.input {
            EmbeddingInput::Text(text) => vec![text.clone()],
            EmbeddingInput::TextArray(texts) => texts.clone(),
            EmbeddingInput::Tokens(_) | EmbeddingInput::TokenArrays(_) => {
                return Ok(Response::builder()
                    .status(http::StatusCode::BAD_REQUEST)
                    .body(axum::body::Body::from(
                        r#"{"error": "Token input not supported for Bedrock embeddings - use text input"}"#,
                    ))?);
            }
            EmbeddingInput::Multimodal(_) => {
                return Ok(Response::builder()
                    .status(http::StatusCode::BAD_REQUEST)
                    .body(axum::body::Body::from(
                        r#"{"error": "Multimodal embeddings not supported for Bedrock Titan - use text input"}"#,
                    ))?);
            }
        };

        // Titan embeddings only support one text per request, so process each text
        let mut embeddings_data: Vec<EmbeddingData> = Vec::with_capacity(texts.len());
        let mut total_tokens = 0i64;

        for (index, text) in texts.into_iter().enumerate() {
            let titan_request = TitanEmbeddingsRequest {
                input_text: text,
                dimensions: payload.dimensions,
                normalize: Some(true),
            };

            let body = serde_json::to_vec(&titan_request).unwrap_or_default();

            // Titan embeddings use invoke-model endpoint
            let url = format!("{}/model/{}/invoke", self.runtime_base_url(), model);

            // Sign the request
            let headers = [("content-type", "application/json")];
            let signed_headers = self
                .sign_runtime_request("POST", &url, &headers, &body)
                .await?;

            let mut request = client
                .post(&url)
                .header("content-type", "application/json")
                .timeout(self.timeout);
            for (name, value) in signed_headers {
                request = request.header(name, value);
            }

            let response = request.body(body).send().await?;
            let status = response.status();

            if !status.is_success() {
                // Return error response from the first failed request
                return error_response::<BedrockErrorParser>(response).await;
            }

            let titan_response: TitanEmbeddingsResponse = response.json().await?;
            total_tokens += titan_response.input_text_token_count;

            embeddings_data.push(EmbeddingData {
                object: EmbeddingObjectType::Embedding,
                embedding: EmbeddingVector::Float(titan_response.embedding),
                index: Some(index as f64),
            });

            // Record circuit breaker success
            if let Some(cb) = &self.circuit_breaker {
                cb.record_success();
            }
        }

        let openai_response = CreateEmbeddingResponse {
            id: Some(format!("bedrock-emb-{}", uuid::Uuid::new_v4())),
            object: EmbeddingResponseObjectType::List,
            data: embeddings_data,
            model,
            usage: Some(EmbeddingUsage {
                prompt_tokens: total_tokens as f64,
                total_tokens: total_tokens as f64,
                cost: None,
            }),
        };

        json_response(http::StatusCode::OK, &openai_response)
    }

    #[tracing::instrument(
        skip(self, client),
        fields(provider = "bedrock", operation = "list_models")
    )]
    async fn list_models(&self, client: &reqwest::Client) -> Result<ModelsResponse, ProviderError> {
        let models = self.get_foundation_models(client).await?;
        Ok(ModelsResponse { data: models })
    }
}

#[cfg(test)]
mod url_tests {
    use std::collections::HashMap;

    use super::*;
    use crate::{
        config::{
            AwsCredentials, BedrockProviderConfig, CircuitBreakerConfig, RetryConfig,
            StreamingBufferConfig,
        },
        providers::CircuitBreakerRegistry,
    };

    fn create_test_provider(region: &str) -> BedrockProvider {
        create_test_provider_with_converse_url(region, None)
    }

    fn create_test_provider_with_converse_url(
        region: &str,
        converse_base_url: Option<String>,
    ) -> BedrockProvider {
        let config = BedrockProviderConfig {
            region: region.to_string(),
            credentials: AwsCredentials::Default,
            timeout_secs: 300,
            allowed_models: Vec::new(),
            model_aliases: HashMap::new(),
            inference_profile_arn: None,
            models: HashMap::new(),
            retry: RetryConfig::default(),
            circuit_breaker: CircuitBreakerConfig::default(),
            streaming_buffer: StreamingBufferConfig::default(),
            fallback_providers: Vec::new(),
            model_fallbacks: HashMap::new(),
            converse_base_url,
            health_check: Default::default(),
            catalog_provider: None,
        };
        let registry = CircuitBreakerRegistry::default();
        BedrockProvider::from_config_with_registry(&config, "test", &registry)
    }

    #[test]
    fn test_runtime_base_url() {
        let provider = create_test_provider("us-east-1");
        assert_eq!(
            provider.runtime_base_url(),
            "https://bedrock-runtime.us-east-1.amazonaws.com"
        );

        let provider = create_test_provider("eu-west-1");
        assert_eq!(
            provider.runtime_base_url(),
            "https://bedrock-runtime.eu-west-1.amazonaws.com"
        );

        let provider = create_test_provider("ap-northeast-1");
        assert_eq!(
            provider.runtime_base_url(),
            "https://bedrock-runtime.ap-northeast-1.amazonaws.com"
        );
    }

    #[test]
    fn test_converse_base_url_custom_override() {
        // Test custom URL override for VPC endpoints or testing
        let custom_url = "http://localhost:8080";
        let provider =
            create_test_provider_with_converse_url("us-east-1", Some(custom_url.to_string()));

        assert_eq!(provider.runtime_base_url(), custom_url);
    }

    #[test]
    fn test_control_plane_base_url() {
        let provider = create_test_provider("us-east-1");
        assert_eq!(
            provider.control_plane_base_url(),
            "https://bedrock.us-east-1.amazonaws.com"
        );
    }

    #[test]
    fn test_extract_model_id_from_arn() {
        // Test extracting model ID from full ARN
        assert_eq!(
            InferenceProfileCache::extract_model_id(
                "arn:aws:bedrock:us-east-1::foundation-model/anthropic.claude-sonnet-4-5-20250929-v1:0"
            ),
            Some("anthropic.claude-sonnet-4-5-20250929-v1:0")
        );

        // Test with just model ID (no slash)
        assert_eq!(
            InferenceProfileCache::extract_model_id("anthropic.claude-sonnet-4-5-20250929-v1:0"),
            Some("anthropic.claude-sonnet-4-5-20250929-v1:0")
        );
    }

    #[test]
    fn test_inference_profile_cache_is_stale() {
        let cache = InferenceProfileCache::default();
        // New cache should be stale
        assert!(cache.is_stale());
    }
}
