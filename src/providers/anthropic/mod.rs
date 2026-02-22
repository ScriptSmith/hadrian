//! Anthropic Claude API provider.
//!
//! This provider implements the Anthropic Messages API and converts
//! OpenAI-compatible requests to Anthropic format.

mod convert;
mod stream;
mod types;

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::response::Response;
use convert::{
    convert_anthropic_to_responses_response, convert_chat_completion_reasoning_config,
    convert_messages, convert_reasoning_config, convert_response,
    convert_responses_input_to_messages, convert_responses_tool_choice, convert_responses_tools,
    convert_stop, convert_tool_choice, convert_tools, supports_adaptive_thinking,
};
use serde::Deserialize;
use stream::{AnthropicToOpenAIStream, AnthropicToResponsesStream};
use types::{AnthropicMetadata, AnthropicRequest, AnthropicResponse};

use crate::{
    api_types::{
        CreateChatCompletionPayload, CreateCompletionPayload, CreateEmbeddingPayload,
        CreateResponsesPayload,
    },
    config::{AnthropicProviderConfig, CircuitBreakerConfig, RetryConfig, StreamingBufferConfig},
    providers::{
        CircuitBreakerRegistry, ModelInfo, ModelsResponse, Provider, ProviderError,
        circuit_breaker::CircuitBreaker,
        error::AnthropicErrorParser,
        image::{ImageFetchConfig, preprocess_messages_for_images},
        response::{error_response, json_response, streaming_response},
        retry::with_circuit_breaker_and_retry,
    },
};

/// Anthropic API version header value.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Default max tokens if not specified.
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Compute the `anthropic-beta` header value based on model and thinking config.
///
/// When thinking is enabled on models that support interleaved thinking (Opus 4.6+),
/// include the `interleaved-thinking-2025-05-14` beta flag.
fn compute_beta_header(
    model: &str,
    thinking: &Option<types::AnthropicThinkingConfig>,
) -> Option<String> {
    let thinking_enabled = matches!(
        thinking,
        Some(types::AnthropicThinkingConfig::Enabled { .. })
            | Some(types::AnthropicThinkingConfig::Adaptive)
    );
    if thinking_enabled && supports_adaptive_thinking(model) {
        Some("interleaved-thinking-2025-05-14".to_string())
    } else {
        None
    }
}

pub struct AnthropicProvider {
    api_key: String,
    base_url: String,
    default_model: Option<String>,
    default_max_tokens: Option<u32>,
    timeout: Duration,
    retry: RetryConfig,
    circuit_breaker_config: CircuitBreakerConfig,
    circuit_breaker: Option<Arc<CircuitBreaker>>,
    streaming_buffer: StreamingBufferConfig,
    image_fetch_config: ImageFetchConfig,
}

impl AnthropicProvider {
    /// Create a provider from configuration with a shared circuit breaker.
    pub fn from_config_with_registry(
        config: &AnthropicProviderConfig,
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
        config: &AnthropicProviderConfig,
        provider_name: &str,
        registry: &CircuitBreakerRegistry,
        image_fetch_config: ImageFetchConfig,
    ) -> Self {
        let circuit_breaker = registry.get_or_create(provider_name, &config.circuit_breaker);

        Self {
            api_key: config.api_key.clone(),
            base_url: config.base_url.trim_end_matches('/').to_string(),
            default_model: config.default_model.clone(),
            default_max_tokens: config.default_max_tokens,
            timeout: Duration::from_secs(config.timeout_secs),
            retry: config.retry.clone(),
            circuit_breaker_config: config.circuit_breaker.clone(),
            circuit_breaker,
            streaming_buffer: config.streaming_buffer.clone(),
            image_fetch_config,
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn default_health_check_model(&self) -> Option<&str> {
        Some("claude-haiku-4-5-20251001")
    }

    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "anthropic",
            operation = "chat_completion",
            model = %payload.model.as_deref().or(self.default_model.as_deref()).unwrap_or("claude-sonnet-4-20250514"),
            stream = payload.stream
        )
    )]
    async fn create_chat_completion(
        &self,
        client: &reqwest::Client,
        payload: CreateChatCompletionPayload,
    ) -> Result<Response, ProviderError> {
        let model = payload
            .model
            .clone()
            .or_else(|| self.default_model.clone())
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

        let max_tokens = payload
            .max_tokens
            .map(|v| v as u32)
            .or(self.default_max_tokens)
            .unwrap_or(DEFAULT_MAX_TOKENS);

        // Preprocess messages to convert HTTP image URLs to data URLs
        // Anthropic only supports base64 images, so we fetch HTTP URLs and convert them
        let mut messages_to_convert = payload.messages;
        preprocess_messages_for_images(
            client,
            &mut messages_to_convert,
            Some(&self.image_fetch_config),
        )
        .await;

        let (system, messages) = convert_messages(messages_to_convert);
        let stream = payload.stream;

        // Convert tools and tool_choice
        let tools = convert_tools(payload.tools);
        let tool_choice = if tools.is_some() {
            convert_tool_choice(payload.tool_choice)
        } else {
            None
        };

        // Convert reasoning config to thinking config (model-aware for adaptive thinking)
        let (thinking, output_config) =
            convert_chat_completion_reasoning_config(payload.reasoning.as_ref(), &model);

        // Note: When thinking is enabled, temperature must be 1.0 per Anthropic API requirements
        let temperature = if thinking.is_some() {
            None // Anthropic requires temperature=1 when thinking is enabled, so we don't send it
        } else {
            payload.temperature
        };

        // Build metadata if user is provided
        let metadata = payload.user.map(|user_id| AnthropicMetadata {
            user_id: Some(user_id),
        });

        let anthropic_request = AnthropicRequest {
            model,
            messages,
            max_tokens,
            system,
            temperature,
            top_p: payload.top_p,
            top_k: None, // Not supported in chat completions payload
            stop_sequences: convert_stop(payload.stop),
            stream,
            tools,
            tool_choice,
            thinking,
            output_config,
            metadata,
        };

        // Pre-serialize request body before retry loop to avoid repeated serialization
        let beta_header =
            compute_beta_header(&anthropic_request.model, &anthropic_request.thinking);
        let body = serde_json::to_vec(&anthropic_request).unwrap_or_default();

        let url = format!("{}/v1/messages", self.base_url);
        let api_key = self.api_key.clone();
        let timeout = self.timeout;

        let response = with_circuit_breaker_and_retry(
            self.circuit_breaker.as_deref(),
            &self.circuit_breaker_config,
            &self.retry,
            "anthropic",
            "chat_completion",
            || async {
                let mut req = client
                    .post(&url)
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .header("content-type", "application/json")
                    .timeout(timeout);
                if let Some(beta) = &beta_header {
                    req = req.header("anthropic-beta", beta.as_str());
                }
                req.body(body.clone()).send().await
            },
        )
        .await?;

        let status = response.status();
        if !status.is_success() {
            return error_response::<AnthropicErrorParser>(response).await;
        }

        if stream {
            // Transform Anthropic SSE events to OpenAI-compatible format
            use futures_util::StreamExt;

            let byte_stream =
                response
                    .bytes_stream()
                    .map(|result| -> Result<bytes::Bytes, std::io::Error> {
                        result.map_err(std::io::Error::other)
                    });
            let transformed_stream =
                AnthropicToOpenAIStream::new(byte_stream, &self.streaming_buffer);

            streaming_response(status, transformed_stream)
        } else {
            let anthropic_response: AnthropicResponse = response.json().await?;
            let openai_response = convert_response(anthropic_response);
            json_response(status, &openai_response)
        }
    }

    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "anthropic",
            operation = "responses",
            model = %payload.model.as_deref().or(self.default_model.as_deref()).unwrap_or("claude-sonnet-4-20250514"),
            stream = payload.stream
        )
    )]
    async fn create_responses(
        &self,
        client: &reqwest::Client,
        payload: CreateResponsesPayload,
    ) -> Result<Response, ProviderError> {
        let model = payload
            .model
            .clone()
            .or_else(|| self.default_model.clone())
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

        let max_tokens = payload
            .max_output_tokens
            .map(|v| v as u32)
            .or(self.default_max_tokens)
            .unwrap_or(DEFAULT_MAX_TOKENS);

        // Convert Responses API input to Anthropic messages format
        let (system, messages) =
            convert_responses_input_to_messages(payload.input, payload.instructions.clone());

        // Convert tools and tool_choice
        let tools = convert_responses_tools(payload.tools);
        let tool_choice = if tools.is_some() {
            convert_responses_tool_choice(payload.tool_choice)
        } else {
            None
        };

        // Convert reasoning config to thinking config (model-aware for adaptive thinking)
        let (thinking, output_config) =
            convert_reasoning_config(payload.reasoning.as_ref(), &model);

        // Note: When thinking is enabled, temperature must be 1.0 per Anthropic API requirements
        let temperature = if thinking.is_some() {
            None // Anthropic requires temperature=1 when thinking is enabled, so we don't send it
        } else {
            payload.temperature
        };

        let stream = payload.stream;

        // Build metadata if user is provided
        let metadata = payload.user.clone().map(|user_id| AnthropicMetadata {
            user_id: Some(user_id),
        });

        let anthropic_request = AnthropicRequest {
            model,
            messages,
            max_tokens,
            system,
            temperature,
            top_p: payload.top_p,
            top_k: None,
            stop_sequences: None, // Responses API doesn't have stop sequences
            stream,
            tools,
            tool_choice,
            thinking,
            output_config,
            metadata,
        };

        // Pre-serialize request body before retry loop to avoid repeated serialization
        let beta_header =
            compute_beta_header(&anthropic_request.model, &anthropic_request.thinking);
        let body = serde_json::to_vec(&anthropic_request).unwrap_or_default();

        let url = format!("{}/v1/messages", self.base_url);
        let api_key = self.api_key.clone();
        let timeout = self.timeout;

        let response = with_circuit_breaker_and_retry(
            self.circuit_breaker.as_deref(),
            &self.circuit_breaker_config,
            &self.retry,
            "anthropic",
            "responses",
            || async {
                let mut req = client
                    .post(&url)
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .header("content-type", "application/json")
                    .timeout(timeout);
                if let Some(beta) = &beta_header {
                    req = req.header("anthropic-beta", beta.as_str());
                }
                req.body(body.clone()).send().await
            },
        )
        .await?;

        let status = response.status();
        if !status.is_success() {
            return error_response::<AnthropicErrorParser>(response).await;
        }

        if stream {
            // Transform Anthropic SSE events to OpenAI Responses API format
            use futures_util::StreamExt;

            let byte_stream =
                response
                    .bytes_stream()
                    .map(|result| -> Result<bytes::Bytes, std::io::Error> {
                        result.map_err(std::io::Error::other)
                    });
            let transformed_stream =
                AnthropicToResponsesStream::new(byte_stream, &self.streaming_buffer);

            streaming_response(status, transformed_stream)
        } else {
            let anthropic_response: AnthropicResponse = response.json().await?;
            let responses_response = convert_anthropic_to_responses_response(
                anthropic_response,
                payload.reasoning.as_ref(),
                payload.user,
            );
            json_response(status, &responses_response)
        }
    }

    async fn create_completion(
        &self,
        _client: &reqwest::Client,
        _payload: CreateCompletionPayload,
    ) -> Result<Response, ProviderError> {
        Err(ProviderError::Internal(
            "Anthropic does not support legacy completions API".to_string(),
        ))
    }

    async fn create_embedding(
        &self,
        _client: &reqwest::Client,
        _payload: CreateEmbeddingPayload,
    ) -> Result<Response, ProviderError> {
        Err(ProviderError::Internal(
            "Anthropic does not support embeddings API".to_string(),
        ))
    }

    #[tracing::instrument(
        skip(self, client),
        fields(provider = "anthropic", operation = "list_models")
    )]
    async fn list_models(&self, client: &reqwest::Client) -> Result<ModelsResponse, ProviderError> {
        #[derive(Deserialize)]
        struct Page {
            data: Vec<ModelInfo>,
            has_more: bool,
            last_id: Option<String>,
        }

        let mut all_models = Vec::new();
        let mut after_id: Option<String> = None;

        loop {
            let mut url = format!("{}/v1/models?limit=1000", self.base_url);
            if let Some(ref cursor) = after_id {
                url.push_str("&after_id=");
                url.push_str(cursor);
            }

            let api_key = self.api_key.clone();
            let timeout = self.timeout;

            let response = with_circuit_breaker_and_retry(
                self.circuit_breaker.as_deref(),
                &self.circuit_breaker_config,
                &self.retry.for_read_only(),
                "anthropic",
                "list_models",
                || async {
                    client
                        .get(&url)
                        .header("x-api-key", &api_key)
                        .header("anthropic-version", ANTHROPIC_VERSION)
                        .timeout(timeout)
                        .send()
                        .await
                },
            )
            .await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(
                    status = %status,
                    body = %body,
                    "Failed to list models from Anthropic API"
                );
                return Err(ProviderError::Internal(format!(
                    "Anthropic models API error: {status} - {body}"
                )));
            }

            let page: Page = response.json().await?;
            all_models.extend(page.data);

            if !page.has_more {
                break;
            }
            after_id = page.last_id;
        }

        Ok(ModelsResponse { data: all_models })
    }
}
