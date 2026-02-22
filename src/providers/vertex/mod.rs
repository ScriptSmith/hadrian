//! Google Vertex AI provider.
//!
//! This provider implements the Vertex AI API for accessing Google's Gemini
//! models and other foundation models.
//!
//! Supports two authentication modes:
//! - **API Key**: Simple authentication using `?key=` query parameter
//! - **OAuth/ADC**: Full Vertex AI authentication with service accounts or ADC

mod convert;
mod stream;
mod types;

use std::{path::Path, sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::response::Response;
use convert::{
    convert_chat_completion_reasoning_to_thinking_config, convert_messages,
    convert_reasoning_to_thinking_config, convert_response, convert_responses_input_to_vertex,
    convert_responses_tool_choice_to_vertex, convert_responses_tools_to_vertex, convert_stop,
    convert_tool_choice, convert_tools, convert_vertex_to_responses_response,
};
use google_cloud_token::TokenSourceProvider;
#[cfg(test)]
use stream::StreamState;
pub use stream::{VertexToOpenAIStream, VertexToResponsesStream};
use tokio::sync::RwLock;
use types::*;

use crate::{
    api_types::{
        CreateChatCompletionPayload, CreateCompletionPayload, CreateEmbeddingPayload,
        CreateResponsesPayload,
        embeddings::{
            CreateEmbeddingResponse, EmbeddingData, EmbeddingInput, EmbeddingObjectType,
            EmbeddingResponseObjectType, EmbeddingUsage, EmbeddingVector,
        },
    },
    config::{
        CircuitBreakerConfig, GcpCredentials, RetryConfig, StreamingBufferConfig,
        VertexProviderConfig,
    },
    providers::{
        CircuitBreakerRegistry, ModelInfo, ModelsResponse, Provider, ProviderError,
        circuit_breaker::CircuitBreaker,
        error::VertexErrorParser,
        image::{ImageFetchConfig, preprocess_messages_for_images},
        response::{error_response, json_response, streaming_response},
        retry::with_circuit_breaker_and_retry,
    },
};

const VERTEX_AI_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

/// Buffer time before token expiry to trigger refresh (5 minutes).
/// Ensures tokens are refreshed before they actually expire.
const TOKEN_REFRESH_BUFFER_SECS: u64 = 300;

/// Default token cache duration (1 hour).
/// Most Google OAuth tokens have a 1-hour lifetime.
const TOKEN_CACHE_DURATION_SECS: u64 = 3600;

/// Authentication mode for the Vertex provider.
#[derive(Clone)]
enum AuthMode {
    /// API key authentication (simple Gemini API).
    /// Uses `?key=` query parameter.
    ApiKey(String),
    /// OAuth/ADC authentication (full Vertex AI).
    /// Uses Bearer token from service account or ADC.
    OAuth {
        project: String,
        region: String,
        credentials: GcpCredentials,
    },
}

pub struct VertexProvider {
    auth_mode: AuthMode,
    publisher: String,
    base_url_override: Option<String>,
    token_cache: Arc<RwLock<Option<CachedToken>>>,
    timeout: Duration,
    retry: RetryConfig,
    circuit_breaker_config: CircuitBreakerConfig,
    circuit_breaker: Option<Arc<CircuitBreaker>>,
    streaming_buffer: StreamingBufferConfig,
    image_fetch_config: ImageFetchConfig,
}

struct CachedToken {
    token: String,
    expires_at: std::time::Instant,
}

impl VertexProvider {
    /// Create a provider from configuration with a shared circuit breaker.
    pub fn from_config_with_registry(
        config: &VertexProviderConfig,
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
        config: &VertexProviderConfig,
        provider_name: &str,
        registry: &CircuitBreakerRegistry,
        image_fetch_config: ImageFetchConfig,
    ) -> Self {
        let circuit_breaker = registry.get_or_create(provider_name, &config.circuit_breaker);

        let auth_mode = if let Some(api_key) = &config.api_key {
            AuthMode::ApiKey(api_key.clone())
        } else {
            AuthMode::OAuth {
                project: config.project.clone().unwrap_or_default(),
                region: config.region.clone().unwrap_or_default(),
                credentials: config.credentials.clone(),
            }
        };

        Self {
            auth_mode,
            publisher: config.publisher.clone(),
            base_url_override: config.base_url.clone(),
            token_cache: Arc::new(RwLock::new(None)),
            timeout: Duration::from_secs(config.timeout_secs),
            retry: config.retry.clone(),
            circuit_breaker_config: config.circuit_breaker.clone(),
            circuit_breaker,
            streaming_buffer: config.streaming_buffer.clone(),
            image_fetch_config,
        }
    }

    /// Get the base URL for API requests (without the model path).
    fn base_url(&self) -> String {
        if let Some(override_url) = &self.base_url_override {
            return override_url.clone();
        }

        match &self.auth_mode {
            AuthMode::ApiKey(_) => {
                // API key mode: global endpoint
                format!(
                    "https://aiplatform.googleapis.com/v1/publishers/{}/models",
                    self.publisher
                )
            }
            AuthMode::OAuth {
                project, region, ..
            } => {
                // OAuth mode: regional endpoint with project path
                format!(
                    "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/{}/models",
                    region, project, region, self.publisher
                )
            }
        }
    }

    /// Build the full URL for a model endpoint.
    fn model_url(&self, model: &str, endpoint: &str, stream: bool) -> String {
        let base = self.base_url();
        let mut url = format!("{}/{}:{}", base, model, endpoint);

        match &self.auth_mode {
            AuthMode::ApiKey(api_key) => {
                // Add API key as query parameter
                if stream {
                    url.push_str("?alt=sse&key=");
                } else {
                    url.push_str("?key=");
                }
                url.push_str(api_key);
            }
            AuthMode::OAuth { .. } => {
                // OAuth uses header auth, just add SSE param if streaming
                if stream {
                    url.push_str("?alt=sse");
                }
            }
        }
        url
    }

    /// Get an access token for OAuth mode, refreshing if necessary.
    /// Returns None for API key mode (no token needed).
    async fn get_token(&self) -> Result<Option<String>, ProviderError> {
        let credentials = match &self.auth_mode {
            AuthMode::ApiKey(_) => return Ok(None),
            AuthMode::OAuth { credentials, .. } => credentials,
        };

        // Check cache first
        {
            let cache = self.token_cache.read().await;
            if let Some(cached) = cache.as_ref() {
                // Return cached token if not expired (with refresh buffer)
                if cached.expires_at
                    > std::time::Instant::now()
                        + std::time::Duration::from_secs(TOKEN_REFRESH_BUFFER_SECS)
                {
                    return Ok(Some(cached.token.clone()));
                }
            }
        }

        // Get token based on credential type
        let token = match credentials {
            GcpCredentials::Default => {
                // Use Application Default Credentials
                let config =
                    google_cloud_auth::project::Config::default().with_scopes(&[VERTEX_AI_SCOPE]);

                let ts = google_cloud_auth::token::DefaultTokenSourceProvider::new(config)
                    .await
                    .map_err(|e| {
                        ProviderError::Internal(format!("Failed to create token source: {}", e))
                    })?;

                ts.token_source()
                    .token()
                    .await
                    .map_err(|e| ProviderError::Internal(format!("Failed to get token: {}", e)))?
            }
            GcpCredentials::ServiceAccount { key_path } => {
                // Load service account key from file
                self.get_token_from_service_account_file(Path::new(key_path))
                    .await?
            }
            GcpCredentials::ServiceAccountJson { json } => {
                // Parse service account key from JSON string
                self.get_token_from_service_account_json(json).await?
            }
        };

        // Cache token (assume standard expiry for Google tokens)
        {
            let mut cache = self.token_cache.write().await;
            *cache = Some(CachedToken {
                token: token.clone(),
                expires_at: std::time::Instant::now()
                    + std::time::Duration::from_secs(TOKEN_CACHE_DURATION_SECS),
            });
        }

        Ok(Some(token))
    }

    /// Get token from a service account key file.
    async fn get_token_from_service_account_file(
        &self,
        key_path: &Path,
    ) -> Result<String, ProviderError> {
        let key_json = tokio::fs::read_to_string(key_path).await.map_err(|e| {
            ProviderError::Internal(format!(
                "Failed to read service account key file '{}': {}",
                key_path.display(),
                e
            ))
        })?;

        self.get_token_from_service_account_json(&key_json).await
    }

    /// Get token from a service account key JSON string.
    async fn get_token_from_service_account_json(
        &self,
        json: &str,
    ) -> Result<String, ProviderError> {
        use google_cloud_auth::credentials::CredentialsFile;

        let creds: CredentialsFile = serde_json::from_str(json).map_err(|e| {
            ProviderError::Internal(format!("Failed to parse service account JSON: {}", e))
        })?;

        let config = google_cloud_auth::project::Config::default().with_scopes(&[VERTEX_AI_SCOPE]);

        let ts = google_cloud_auth::token::DefaultTokenSourceProvider::new_with_credentials(
            config,
            Box::new(creds),
        )
        .await
        .map_err(|e| {
            ProviderError::Internal(format!(
                "Failed to create token source from service account: {}",
                e
            ))
        })?;

        ts.token_source()
            .token()
            .await
            .map_err(|e| ProviderError::Internal(format!("Failed to get token: {}", e)))
    }

    /// Build a request with appropriate authentication.
    fn build_request(
        &self,
        client: &reqwest::Client,
        url: &str,
        token: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let mut req = client
            .post(url)
            .header("Content-Type", "application/json")
            .timeout(self.timeout);

        if let Some(token) = token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        req
    }
}

#[async_trait]
impl Provider for VertexProvider {
    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "vertex",
            operation = "chat_completion",
            model = %payload.model.as_deref().unwrap_or("gemini-1.5-pro"),
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
            .unwrap_or_else(|| "gemini-1.5-pro".to_string());

        // Preprocess messages to convert HTTP image URLs to data URLs
        let mut messages = payload.messages;
        preprocess_messages_for_images(client, &mut messages, Some(&self.image_fetch_config)).await;

        // HashMap to track tool_call_id -> function_name mappings for Tool messages
        let mut tool_call_names = std::collections::HashMap::new();
        let (system_instruction, contents) = convert_messages(messages, &mut tool_call_names);
        let stream = payload.stream;

        // Convert tools and tool_choice
        let tools = convert_tools(payload.tools);
        let tool_config = if tools.is_some() {
            convert_tool_choice(payload.tool_choice)
        } else {
            None
        };

        // Convert reasoning config to thinking config
        let thinking_config = convert_chat_completion_reasoning_to_thinking_config(
            payload.reasoning.as_ref(),
            &model,
        );

        let vertex_request = VertexGenerateContentRequest {
            contents,
            system_instruction,
            generation_config: Some(VertexGenerationConfig {
                max_output_tokens: payload.max_tokens,
                temperature: payload.temperature,
                top_p: payload.top_p,
                stop_sequences: convert_stop(payload.stop),
                thinking_config,
            }),
            tools,
            tool_config,
        };

        // Pre-serialize request body before retry loop to avoid repeated serialization
        let body = serde_json::to_vec(&vertex_request).unwrap_or_default();

        let token = self.get_token().await?;
        let endpoint = if stream {
            "streamGenerateContent"
        } else {
            "generateContent"
        };
        let url = self.model_url(&model, endpoint, stream);

        let response = with_circuit_breaker_and_retry(
            self.circuit_breaker.as_deref(),
            &self.circuit_breaker_config,
            &self.retry,
            "vertex",
            "chat_completion",
            || async {
                self.build_request(client, &url, token.as_deref())
                    .header("content-type", "application/json")
                    .body(body.clone())
                    .send()
                    .await
            },
        )
        .await?;

        if stream {
            // Transform Vertex SSE events to OpenAI-compatible format
            use futures_util::StreamExt;

            let status = response.status();
            if !status.is_success() {
                return error_response::<VertexErrorParser>(response).await;
            }

            // Wrap the response stream with our transformer
            let byte_stream =
                response
                    .bytes_stream()
                    .map(|result| -> Result<bytes::Bytes, std::io::Error> {
                        result.map_err(std::io::Error::other)
                    });
            let transformed_stream =
                VertexToOpenAIStream::new(byte_stream, model, &self.streaming_buffer);

            streaming_response(status, transformed_stream)
        } else {
            let status = response.status();

            if !status.is_success() {
                return error_response::<VertexErrorParser>(response).await;
            }

            let vertex_response: VertexGenerateContentResponse = response.json().await?;
            let openai_response = convert_response(vertex_response, &model);
            json_response(status, &openai_response)
        }
    }

    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "vertex",
            operation = "responses",
            model = %payload.model.as_deref().unwrap_or("gemini-2.0-flash"),
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
            .unwrap_or_else(|| "gemini-2.0-flash".to_string());

        let stream = payload.stream;

        // Convert Responses API input to Vertex format
        let (system_instruction, contents) =
            convert_responses_input_to_vertex(payload.input, payload.instructions.clone());

        // Convert tools and tool_choice
        let tools = convert_responses_tools_to_vertex(payload.tools.clone());
        let tool_config = if tools.is_some() {
            convert_responses_tool_choice_to_vertex(payload.tool_choice)
        } else {
            None
        };

        // Convert reasoning config to thinking config
        let thinking_config =
            convert_reasoning_to_thinking_config(payload.reasoning.as_ref(), &model);

        // Build generation config
        let generation_config = Some(VertexGenerationConfig {
            max_output_tokens: payload.max_output_tokens.map(|v| v as u64),
            temperature: payload.temperature,
            top_p: payload.top_p,
            stop_sequences: None,
            thinking_config,
        });

        let vertex_request = VertexGenerateContentRequest {
            contents,
            system_instruction,
            generation_config,
            tools,
            tool_config,
        };

        // Pre-serialize request body before retry loop to avoid repeated serialization
        let body = serde_json::to_vec(&vertex_request).unwrap_or_default();

        let token = self.get_token().await?;
        let endpoint = if stream {
            "streamGenerateContent"
        } else {
            "generateContent"
        };
        let url = self.model_url(&model, endpoint, stream);

        let response = with_circuit_breaker_and_retry(
            self.circuit_breaker.as_deref(),
            &self.circuit_breaker_config,
            &self.retry,
            "vertex",
            "responses",
            || async {
                self.build_request(client, &url, token.as_deref())
                    .header("content-type", "application/json")
                    .body(body.clone())
                    .send()
                    .await
            },
        )
        .await?;

        if stream {
            use futures_util::StreamExt;

            let status = response.status();
            if !status.is_success() {
                return error_response::<VertexErrorParser>(response).await;
            }

            // Transform Vertex SSE to Responses API SSE format
            let byte_stream = response
                .bytes_stream()
                .map(|result| result.map_err(std::io::Error::other));
            let transformed_stream =
                VertexToResponsesStream::new(byte_stream, model, &self.streaming_buffer);

            return streaming_response(status, transformed_stream);
        }

        let status = response.status();

        if !status.is_success() {
            return error_response::<VertexErrorParser>(response).await;
        }

        let vertex_response: VertexGenerateContentResponse = response.json().await?;
        let responses_response = convert_vertex_to_responses_response(
            vertex_response,
            &model,
            payload.reasoning.as_ref(),
            payload.user,
        );
        json_response(status, &responses_response)
    }

    #[tracing::instrument(
        skip(self, _client, _payload),
        fields(provider = "vertex", operation = "completion")
    )]
    async fn create_completion(
        &self,
        _client: &reqwest::Client,
        _payload: CreateCompletionPayload,
    ) -> Result<Response, ProviderError> {
        Ok(Response::builder()
            .status(http::StatusCode::NOT_IMPLEMENTED)
            .body(axum::body::Body::from(
                r#"{"error": "Legacy completions API not supported for Vertex AI provider"}"#,
            ))?)
    }

    #[tracing::instrument(
        skip(self, client, payload),
        fields(
            provider = "vertex",
            operation = "embedding",
            model = %payload.model
        )
    )]
    async fn create_embedding(
        &self,
        client: &reqwest::Client,
        payload: CreateEmbeddingPayload,
    ) -> Result<Response, ProviderError> {
        let model = payload.model.clone();

        // Convert OpenAI input format to Vertex instances
        let instances: Vec<VertexEmbeddingInstance> = match &payload.input {
            EmbeddingInput::Text(text) => vec![VertexEmbeddingInstance {
                content: text.clone(),
                task_type: payload.input_type.clone(),
            }],
            EmbeddingInput::TextArray(texts) => texts
                .iter()
                .map(|text| VertexEmbeddingInstance {
                    content: text.clone(),
                    task_type: payload.input_type.clone(),
                })
                .collect(),
            EmbeddingInput::Tokens(_) | EmbeddingInput::TokenArrays(_) => {
                return Ok(Response::builder()
                    .status(http::StatusCode::BAD_REQUEST)
                    .body(axum::body::Body::from(
                        r#"{"error": "Token input not supported for Vertex AI embeddings - use text input"}"#,
                    ))?);
            }
            EmbeddingInput::Multimodal(_) => {
                return Ok(Response::builder()
                    .status(http::StatusCode::BAD_REQUEST)
                    .body(axum::body::Body::from(
                        r#"{"error": "Multimodal embeddings not yet supported for Vertex AI - use text input"}"#,
                    ))?);
            }
        };

        let parameters = if payload.dimensions.is_some() {
            Some(VertexEmbeddingParameters {
                auto_truncate: Some(true),
                output_dimensionality: payload.dimensions,
            })
        } else {
            None
        };

        let vertex_request = VertexEmbeddingsRequest {
            instances,
            parameters,
        };

        // Pre-serialize request body before retry loop to avoid repeated serialization
        let body = serde_json::to_vec(&vertex_request).unwrap_or_default();

        let token = self.get_token().await?;
        let url = self.model_url(&model, "predict", false);

        let response = with_circuit_breaker_and_retry(
            self.circuit_breaker.as_deref(),
            &self.circuit_breaker_config,
            &self.retry.for_embedding(),
            "vertex",
            "embedding",
            || async {
                self.build_request(client, &url, token.as_deref())
                    .header("content-type", "application/json")
                    .body(body.clone())
                    .send()
                    .await
            },
        )
        .await?;

        let status = response.status();

        if !status.is_success() {
            return error_response::<VertexErrorParser>(response).await;
        }

        let vertex_response: VertexEmbeddingsResponse = response.json().await?;

        // Convert to OpenAI format
        let mut total_tokens = 0i64;
        let data: Vec<EmbeddingData> = vertex_response
            .predictions
            .into_iter()
            .enumerate()
            .map(|(index, prediction)| {
                if let Some(stats) = &prediction.embeddings.statistics {
                    total_tokens += stats.token_count;
                }
                EmbeddingData {
                    object: EmbeddingObjectType::Embedding,
                    embedding: EmbeddingVector::Float(prediction.embeddings.values),
                    index: Some(index as f64),
                }
            })
            .collect();

        let openai_response = CreateEmbeddingResponse {
            id: Some(format!("vertex-emb-{}", uuid::Uuid::new_v4())),
            object: EmbeddingResponseObjectType::List,
            data,
            model,
            usage: Some(EmbeddingUsage {
                prompt_tokens: total_tokens as f64,
                total_tokens: total_tokens as f64,
                cost: None,
            }),
        };

        json_response(status, &openai_response)
    }

    #[tracing::instrument(
        skip(self, _client),
        fields(provider = "vertex", operation = "list_models")
    )]
    async fn list_models(
        &self,
        _client: &reqwest::Client,
    ) -> Result<ModelsResponse, ProviderError> {
        // Return known Gemini models
        Ok(ModelsResponse {
            data: vec![
                // Gemini 3.0 (preview)
                ModelInfo {
                    id: "gemini-3-pro-preview".to_string(),
                    extra: serde_json::json!({"owned_by": "google", "provider": "vertex", "description": "Latest reasoning model"}),
                },
                // Gemini 2.5 (GA)
                ModelInfo {
                    id: "gemini-2.5-pro".to_string(),
                    extra: serde_json::json!({"owned_by": "google", "provider": "vertex"}),
                },
                ModelInfo {
                    id: "gemini-2.5-flash".to_string(),
                    extra: serde_json::json!({"owned_by": "google", "provider": "vertex"}),
                },
                ModelInfo {
                    id: "gemini-2.5-flash-lite".to_string(),
                    extra: serde_json::json!({"owned_by": "google", "provider": "vertex"}),
                },
                // Gemini 2.0 (GA)
                ModelInfo {
                    id: "gemini-2.0-flash".to_string(),
                    extra: serde_json::json!({"owned_by": "google", "provider": "vertex"}),
                },
                ModelInfo {
                    id: "gemini-2.0-flash-lite".to_string(),
                    extra: serde_json::json!({"owned_by": "google", "provider": "vertex"}),
                },
                // Embedding models
                ModelInfo {
                    id: "gemini-embedding-001".to_string(),
                    extra: serde_json::json!({"owned_by": "google", "provider": "vertex", "type": "embedding", "dimensions": 3072}),
                },
                ModelInfo {
                    id: "text-embedding-005".to_string(),
                    extra: serde_json::json!({"owned_by": "google", "provider": "vertex", "type": "embedding", "dimensions": 768}),
                },
                ModelInfo {
                    id: "text-multilingual-embedding-002".to_string(),
                    extra: serde_json::json!({"owned_by": "google", "provider": "vertex", "type": "embedding", "dimensions": 768}),
                },
            ],
        })
    }
}

#[cfg(test)]
mod streaming_tests {
    use std::io;

    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_parse_vertex_text_response() {
        let json = r#"{"candidates":[{"content":{"parts":[{"text":"Hello"}],"role":"model"},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5,"totalTokenCount":15}}"#;
        let response: VertexGenerateContentResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.candidates.len(), 1);
        assert_eq!(
            response.candidates[0].content.parts[0].text,
            Some("Hello".to_string())
        );
        assert_eq!(
            response.candidates[0].finish_reason,
            Some("STOP".to_string())
        );
        assert!(response.usage_metadata.is_some());
        let usage = response.usage_metadata.unwrap();
        assert_eq!(usage.prompt_token_count, 10);
        assert_eq!(usage.candidates_token_count, 5);
        assert_eq!(usage.total_token_count, 15);
    }

    #[test]
    fn test_parse_vertex_function_call_response() {
        let json = r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"get_weather","args":{"location":"Seattle"}}}],"role":"model"},"finishReason":"STOP"}]}"#;
        let response: VertexGenerateContentResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.candidates.len(), 1);
        let fc = response.candidates[0].content.parts[0]
            .function_call
            .as_ref()
            .unwrap();
        assert_eq!(fc.name, "get_weather");
        assert_eq!(fc.args, serde_json::json!({"location": "Seattle"}));
    }

    #[test]
    fn test_stream_state_initial_values() {
        let state = StreamState::default();
        assert!(state.message_id.starts_with("vertex-"));
        assert!(state.model.is_empty());
        assert_eq!(state.input_tokens, 0);
        assert_eq!(state.output_tokens, 0);
        assert!(!state.sent_role);
        assert_eq!(state.tool_call_count, 0);
    }

    #[test]
    fn test_handle_response_sends_role_first() {
        use futures_util::stream;
        let empty_stream = stream::empty::<Result<Bytes, io::Error>>();
        let mut transformer = VertexToOpenAIStream::new(
            empty_stream,
            "gemini-2.0-flash".into(),
            &StreamingBufferConfig::default(),
        );

        assert!(!transformer.state.sent_role);

        // Simulate receiving a response
        let response = VertexGenerateContentResponse {
            candidates: vec![VertexCandidate {
                content: VertexResponseContent {
                    parts: vec![VertexResponsePart {
                        text: Some("Hello".to_string()),
                        function_call: None,
                        thought: false,
                    }],
                },
                finish_reason: None,
            }],
            usage_metadata: None,
        };

        transformer.handle_response(response);

        assert!(transformer.state.sent_role);
        // Should have at least 2 chunks: role chunk and content delta
        assert!(transformer.output_buffer.len() >= 2);

        // First chunk should contain role
        let first_chunk = std::str::from_utf8(&transformer.output_buffer[0]).unwrap();
        assert!(first_chunk.contains(r#""role":"assistant""#));
    }

    #[test]
    fn test_handle_response_with_finish_reason() {
        use futures_util::stream;
        let empty_stream = stream::empty::<Result<Bytes, io::Error>>();
        let mut transformer = VertexToOpenAIStream::new(
            empty_stream,
            "gemini-2.0-flash".into(),
            &StreamingBufferConfig::default(),
        );

        let response = VertexGenerateContentResponse {
            candidates: vec![VertexCandidate {
                content: VertexResponseContent {
                    parts: vec![VertexResponsePart {
                        text: Some("Done".to_string()),
                        function_call: None,
                        thought: false,
                    }],
                },
                finish_reason: Some("STOP".to_string()),
            }],
            usage_metadata: Some(VertexUsageMetadata {
                prompt_token_count: 10,
                candidates_token_count: 5,
                total_token_count: 15,
                thoughts_token_count: 0,
            }),
        };

        transformer.handle_response(response);

        // Should emit [DONE] at the end
        let last_chunk = std::str::from_utf8(transformer.output_buffer.last().unwrap()).unwrap();
        assert_eq!(last_chunk, "data: [DONE]\n\n");

        // Should have usage in second-to-last chunk
        let chunks: Vec<&str> = transformer
            .output_buffer
            .iter()
            .map(|b| std::str::from_utf8(b).unwrap())
            .collect();

        let has_usage = chunks.iter().any(|c| c.contains(r#""prompt_tokens":10"#));
        assert!(has_usage, "Should include usage data");
    }

    #[test]
    fn test_finish_reason_conversion() {
        use futures_util::stream;

        // Test STOP -> stop
        let empty_stream = stream::empty::<Result<Bytes, io::Error>>();
        let mut transformer = VertexToOpenAIStream::new(
            empty_stream,
            "gemini-2.0-flash".into(),
            &StreamingBufferConfig::default(),
        );

        let response = VertexGenerateContentResponse {
            candidates: vec![VertexCandidate {
                content: VertexResponseContent { parts: vec![] },
                finish_reason: Some("STOP".to_string()),
            }],
            usage_metadata: None,
        };

        transformer.handle_response(response);

        let chunks: Vec<String> = transformer
            .output_buffer
            .iter()
            .map(|b| String::from_utf8_lossy(b).to_string())
            .collect();

        assert!(
            chunks
                .iter()
                .any(|c| c.contains(r#""finish_reason":"stop""#))
        );
    }

    #[test]
    fn test_finish_reason_with_tool_calls() {
        use futures_util::stream;
        let empty_stream = stream::empty::<Result<Bytes, io::Error>>();
        let mut transformer = VertexToOpenAIStream::new(
            empty_stream,
            "gemini-2.0-flash".into(),
            &StreamingBufferConfig::default(),
        );

        let response = VertexGenerateContentResponse {
            candidates: vec![VertexCandidate {
                content: VertexResponseContent {
                    parts: vec![VertexResponsePart {
                        text: None,
                        function_call: Some(VertexFunctionCall {
                            name: "get_weather".to_string(),
                            args: serde_json::json!({"city": "Seattle"}),
                        }),
                        thought: false,
                    }],
                },
                finish_reason: Some("STOP".to_string()),
            }],
            usage_metadata: None,
        };

        transformer.handle_response(response);

        let chunks: Vec<String> = transformer
            .output_buffer
            .iter()
            .map(|b| String::from_utf8_lossy(b).to_string())
            .collect();

        // With tool calls, STOP should be converted to "tool_calls"
        assert!(
            chunks
                .iter()
                .any(|c| c.contains(r#""finish_reason":"tool_calls""#))
        );
    }

    #[test]
    fn test_process_sse_line() {
        use futures_util::stream;
        let empty_stream = stream::empty::<Result<Bytes, io::Error>>();
        let mut transformer = VertexToOpenAIStream::new(
            empty_stream,
            "gemini-2.0-flash".into(),
            &StreamingBufferConfig::default(),
        );

        let sse_line = r#"data: {"candidates":[{"content":{"parts":[{"text":"Hello world"}],"role":"model"}}]}"#;
        transformer.process_sse_line(sse_line);

        assert!(transformer.output_buffer.len() >= 2);
        let content_chunk = transformer
            .output_buffer
            .iter()
            .find(|b| {
                let s = std::str::from_utf8(b).unwrap_or("");
                s.contains("Hello world")
            })
            .expect("Should have content chunk");
        let content_str = std::str::from_utf8(content_chunk).unwrap();
        assert!(content_str.contains(r#""content":"Hello world""#));
    }
}
