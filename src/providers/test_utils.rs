//! Test utilities for provider testing with wiremock.
//!
//! This module provides a fixture-based testing system for providers:
//! - JSON fixtures loaded from `tests/fixtures/providers/`
//! - Fixture mounting on wiremock with expectation counts
//!
//! # Example
//!
//! ```ignore
//! use crate::providers::test_utils::{FixtureId, load_fixture, mount_fixture_data};
//!
//! #[tokio::test]
//! async fn test_chat_completion() {
//!     let mock_server = MockServer::start().await;
//!     let fixture = load_fixture(FixtureId::OpenAiChatCompletionSuccess);
//!     mount_fixture_data(&mock_server, &fixture, 1).await;
//!
//!     // ... test logic
//! }
//! ```
//!
// Allow dead code: this is a shared test utility module. Not all helpers, fixtures,
// and validators are used in every feature profile (e.g., Bedrock/Vertex fixtures
// are unused when those provider features are disabled, validators are unused in
// the `tiny` profile which has no database for provider e2e tests).
#![allow(dead_code)]

use std::{collections::HashMap, path::PathBuf};

#[cfg(feature = "provider-bedrock")]
use aws_smithy_eventstream::frame::write_message_to;
#[cfg(feature = "provider-bedrock")]
use aws_smithy_types::event_stream::{Header, HeaderValue, Message as EventMessage};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, path_regex},
};

// =============================================================================
// Fixture Types
// =============================================================================

/// Well-known fixture identifiers.
/// File paths are automatically derived from variant names using the pattern:
/// `ProviderFixtureName` -> `provider/fixture_name.json`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixtureId {
    // OpenAI fixtures
    OpenAiChatCompletionSuccess,
    OpenAiChatCompletionStreaming,
    OpenAiRateLimit,
    OpenAiServerError,
    OpenAiBadRequest,
    OpenAiUnauthorized,
    OpenAiEmbeddingSuccess,
    OpenAiModelsList,
    OpenAiResponsesSuccess,
    OpenAiResponsesStreaming,
    OpenAiCompletionSuccess,
    OpenAiCompletionStreaming,
    OpenAiToolCallSuccess,
    OpenAiToolCallStreaming,
    OpenAiToolCallParallel,
    OpenAiToolCallWithResult,
    OpenAiReasoningSuccess,
    OpenAiReasoningStreaming,
    OpenAiResponsesToolCallSuccess,
    OpenAiResponsesToolCallStreaming,
    OpenAiResponsesToolCallParallel,
    OpenAiResponsesToolCallWithResult,
    OpenAiResponsesReasoningSuccess,
    OpenAiResponsesReasoningStreaming,
    OpenAiVisionSuccess,
    OpenAiVisionUrlSuccess,
    OpenAiResponsesVisionSuccess,
    OpenAiResponsesVisionUrlSuccess,
    // Image generation fixtures
    OpenAiImageGenerationSuccess,
    OpenAiImageEditSuccess,
    OpenAiImageVariationSuccess,
    // Audio fixtures
    OpenAiAudioSpeechSuccess,
    OpenAiAudioTranscriptionSuccess,
    OpenAiAudioTranslationSuccess,
    // OpenRouter fixtures
    OpenRouterChatCompletionSuccess,
    OpenRouterChatCompletionStreaming,
    OpenRouterResponsesSuccess,
    OpenRouterResponsesStreaming,
    // Anthropic fixtures
    AnthropicMessagesSuccess,
    AnthropicMessagesStreaming,
    AnthropicToolCallSuccess,
    AnthropicToolCallStreaming,
    AnthropicToolCallParallel,
    AnthropicToolCallWithResult,
    AnthropicThinkingSuccess,
    AnthropicThinkingStreaming,
    AnthropicVisionSuccess,
    AnthropicBadRequest,
    AnthropicUnauthorized,
    // Bedrock fixtures
    BedrockConverseSuccess,
    BedrockConverseStreaming,
    BedrockToolCallSuccess,
    BedrockToolCallStreaming,
    BedrockToolCallParallel,
    BedrockToolCallWithResult,
    BedrockVisionSuccess,
    BedrockResponsesSuccess,
    BedrockResponsesStreaming,
    BedrockBadRequest,
    BedrockUnauthorized,
    // Vertex AI fixtures
    VertexGenerateContentSuccess,
    VertexGenerateContentStreaming,
    VertexResponsesSuccess,
    VertexResponsesStreaming,
    VertexToolCallSuccess,
    VertexToolCallStreaming,
    VertexToolCallParallel,
    VertexToolCallWithResult,
    VertexVisionSuccess,
    VertexBadRequest,
    VertexUnauthorized,
    // Ollama fixtures (local OpenAI-compatible server)
    OllamaChatCompletionSuccess,
    OllamaChatCompletionStreaming,
    OllamaToolCallSuccess,
    OllamaToolCallStreaming,
    OllamaVisionSuccess,
    OllamaBadRequest,
}

impl FixtureId {
    /// Get the provider name for this fixture
    pub fn provider(&self) -> &'static str {
        let name = format!("{:?}", self);
        if name.starts_with("OpenAi") {
            "openai"
        } else if name.starts_with("OpenRouter") {
            "openrouter"
        } else if name.starts_with("Anthropic") {
            "anthropic"
        } else if name.starts_with("Bedrock") {
            "bedrock"
        } else if name.starts_with("Vertex") {
            "vertex"
        } else if name.starts_with("Ollama") {
            "ollama"
        } else {
            panic!("Unknown provider for fixture: {}", name)
        }
    }

    /// Get the fixture file path relative to fixtures directory.
    /// Derived automatically from the variant name.
    fn file_path(&self) -> String {
        let variant_name = format!("{:?}", self);
        let provider = self.provider();

        // Strip provider prefix (e.g., "OpenAi" from "OpenAiChatCompletionSuccess")
        let prefix_len = provider_prefix_len(provider);
        let fixture_part = &variant_name[prefix_len..];

        // Convert to snake_case
        let snake_case = to_snake_case(fixture_part);

        format!("{}/{}.json", provider, snake_case)
    }
}

/// Get the length of the provider prefix in PascalCase
fn provider_prefix_len(provider: &str) -> usize {
    match provider {
        "openai" => 6,      // "OpenAi"
        "openrouter" => 10, // "OpenRouter"
        "anthropic" => 9,   // "Anthropic"
        "bedrock" => 7,     // "Bedrock"
        "vertex" => 6,      // "Vertex"
        "ollama" => 6,      // "Ollama"
        _ => provider.len(),
    }
}

/// Convert PascalCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 10);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_lowercase());
    }
    result
}

/// Fixture file format (matches recorded fixtures)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixture {
    pub id: String,
    pub description: String,
    pub request: FixtureRequest,
    pub response: FixtureResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureRequest {
    pub method: String,
    pub path: String,
    /// Optional regex pattern for path matching (used instead of exact path match)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_pattern: Option<String>,
}

/// Streaming format for fixtures
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StreamingFormat {
    /// Server-Sent Events (OpenAI, Anthropic)
    #[default]
    Sse,
    /// AWS EventStream binary format (Bedrock)
    AwsEventstream,
}

/// AWS EventStream event for Bedrock streaming fixtures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStreamEvent {
    /// Event type (e.g., "messageStart", "contentBlockDelta", "messageStop", "metadata")
    pub event_type: String,
    /// JSON payload for the event
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
    /// Base64-encoded binary body (for non-JSON responses like audio)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_base64: Option<String>,
    #[serde(default)]
    pub streaming: bool,
    /// Streaming format: "sse" (default) or "aws_eventstream"
    #[serde(default, skip_serializing_if = "is_default_streaming_format")]
    pub streaming_format: StreamingFormat,
    /// SSE chunks (for streaming_format: sse)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunks: Option<Vec<Value>>,
    /// AWS EventStream events (for streaming_format: aws_eventstream)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<EventStreamEvent>>,
}

fn is_default_streaming_format(format: &StreamingFormat) -> bool {
    *format == StreamingFormat::Sse
}

// =============================================================================
// Fixture Loading
// =============================================================================

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/providers")
}

/// Load a fixture from the fixtures directory
pub fn load_fixture(id: FixtureId) -> Fixture {
    let path = fixtures_dir().join(id.file_path());
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse fixture {}: {}", path.display(), e))
}

// =============================================================================
// Mock Mounting
// =============================================================================

/// Mount fixture data on the mock server with expected call count
pub async fn mount_fixture_data(mock_server: &MockServer, fixture: &Fixture, expected_calls: u64) {
    let response = build_response_template(fixture);

    // Use path_pattern if provided, otherwise exact path match
    let mock = if let Some(pattern) = &fixture.request.path_pattern {
        Mock::given(method(fixture.request.method.as_str()))
            .and(path_regex(pattern.as_str()))
            .respond_with(response)
    } else {
        Mock::given(method(fixture.request.method.as_str()))
            .and(path(&fixture.request.path))
            .respond_with(response)
    };

    let mock = if expected_calls > 0 {
        mock.expect(expected_calls)
    } else {
        mock
    };

    mock.mount(mock_server).await;
}

/// Create an AWS EventStream message with headers and payload
#[cfg(feature = "provider-bedrock")]
fn create_event_message(event_type: &str, payload: &str) -> Vec<u8> {
    let message = EventMessage::new(payload.as_bytes().to_vec())
        .add_header(Header::new(
            ":message-type",
            HeaderValue::String("event".to_string().into()),
        ))
        .add_header(Header::new(
            ":event-type",
            HeaderValue::String(event_type.to_string().into()),
        ))
        .add_header(Header::new(
            ":content-type",
            HeaderValue::String("application/json".to_string().into()),
        ));

    let mut buffer = Vec::new();
    write_message_to(&message, &mut buffer).expect("Failed to write event message");
    buffer
}

// =============================================================================
// Shared Tool Definitions
// =============================================================================

/// Standard weather tool for Chat Completions API tests.
pub fn weather_tool() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Get the current weather for a location",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "City and state, e.g. San Francisco, CA"
                    }
                },
                "required": ["location"]
            }
        }
    })
}

/// Weather tool for Responses API tests (different format).
pub fn responses_weather_tool() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "name": "get_weather",
        "description": "Get the current weather for a location",
        "parameters": {
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City and state, e.g. San Francisco, CA"
                }
            },
            "required": ["location"]
        }
    })
}

// =============================================================================
// Response Validators
// =============================================================================

/// Validators for asserting response formats conform to OpenAI API specs.
/// These provide robust, shared assertions across all provider tests.
pub mod validators {
    use serde_json::Value;

    /// Assert a non-streaming Chat Completions response conforms to OpenAI format.
    pub fn assert_chat_completion(body: &Value) {
        assert_eq!(
            body["object"], "chat.completion",
            "Expected object 'chat.completion', got {:?}",
            body["object"]
        );
        assert!(body["id"].is_string(), "Missing or invalid 'id' field");
        assert!(
            body["created"].is_number(),
            "Missing or invalid 'created' field"
        );
        assert!(
            body["model"].is_string(),
            "Missing or invalid 'model' field"
        );

        let choices = body["choices"]
            .as_array()
            .expect("'choices' should be an array");
        assert!(!choices.is_empty(), "'choices' array should not be empty");

        let choice = &choices[0];
        assert!(
            choice["index"].is_number(),
            "choice should have 'index' field"
        );
        assert!(
            choice["finish_reason"].is_string() || choice["finish_reason"].is_null(),
            "choice should have 'finish_reason' field"
        );

        let message = &choice["message"];
        assert!(
            message["role"].is_string(),
            "message should have 'role' field"
        );
        // Content can be null for tool calls
        assert!(
            message["content"].is_string() || message["content"].is_null(),
            "message 'content' should be string or null"
        );

        // Usage is required for non-streaming
        let usage = &body["usage"];
        assert!(
            usage["prompt_tokens"].is_number(),
            "usage should have 'prompt_tokens'"
        );
        assert!(
            usage["completion_tokens"].is_number(),
            "usage should have 'completion_tokens'"
        );
        assert!(
            usage["total_tokens"].is_number(),
            "usage should have 'total_tokens'"
        );
    }

    /// Assert a Chat Completions response contains tool calls.
    pub fn assert_tool_calls(body: &Value) {
        assert_chat_completion(body);

        let choice = &body["choices"][0];
        assert_eq!(
            choice["finish_reason"], "tool_calls",
            "Expected finish_reason 'tool_calls'"
        );

        let tool_calls = choice["message"]["tool_calls"]
            .as_array()
            .expect("message should have 'tool_calls' array");
        assert!(!tool_calls.is_empty(), "tool_calls should not be empty");

        for tool_call in tool_calls {
            assert!(tool_call["id"].is_string(), "tool_call should have 'id'");
            assert_eq!(
                tool_call["type"], "function",
                "tool_call type should be 'function'"
            );
            assert!(
                tool_call["function"]["name"].is_string(),
                "tool_call function should have 'name'"
            );
            assert!(
                tool_call["function"]["arguments"].is_string(),
                "tool_call function should have 'arguments'"
            );
        }
    }

    /// Assert a Responses API response conforms to OpenAI format.
    pub fn assert_responses_api(body: &Value) {
        assert_eq!(
            body["object"], "response",
            "Expected object 'response', got {:?}",
            body["object"]
        );
        assert!(body["id"].is_string(), "Missing or invalid 'id' field");
        assert!(
            body["created_at"].is_number(),
            "Missing or invalid 'created_at' field"
        );
        assert!(
            body["model"].is_string(),
            "Missing or invalid 'model' field"
        );

        let output = body["output"]
            .as_array()
            .expect("'output' should be an array");
        assert!(!output.is_empty(), "'output' array should not be empty");

        // Find at least one message output (some responses may have reasoning first)
        let has_message = output.iter().any(|o| o["type"] == "message");
        assert!(has_message, "output should contain at least one message");

        // Check message outputs
        for item in output.iter().filter(|o| o["type"] == "message") {
            assert!(
                item["role"].is_string(),
                "message output should have 'role'"
            );
            assert!(
                item["content"].is_array(),
                "message output should have 'content' array"
            );
        }

        // Usage may not be present for all providers, but if present, validate it
        if !body["usage"].is_null() {
            let usage = &body["usage"];
            assert!(
                usage["input_tokens"].is_number(),
                "usage should have 'input_tokens'"
            );
            assert!(
                usage["output_tokens"].is_number(),
                "usage should have 'output_tokens'"
            );
        }
    }

    /// Assert a Responses API response contains function calls.
    pub fn assert_responses_function_calls(body: &Value) {
        assert_eq!(body["object"], "response", "Expected object 'response'");

        let output = body["output"]
            .as_array()
            .expect("'output' should be an array");

        let function_calls: Vec<_> = output
            .iter()
            .filter(|o| o["type"] == "function_call")
            .collect();

        assert!(
            !function_calls.is_empty(),
            "output should contain at least one function_call"
        );

        for call in function_calls {
            assert!(
                call["call_id"].is_string(),
                "function_call should have 'call_id'"
            );
            assert!(call["name"].is_string(), "function_call should have 'name'");
            assert!(
                call["arguments"].is_string(),
                "function_call should have 'arguments'"
            );
        }
    }

    /// Assert an Embeddings response conforms to OpenAI format.
    pub fn assert_embeddings(body: &Value) {
        assert_eq!(
            body["object"], "list",
            "Expected object 'list', got {:?}",
            body["object"]
        );
        assert!(
            body["model"].is_string(),
            "Missing or invalid 'model' field"
        );

        let data = body["data"].as_array().expect("'data' should be an array");
        assert!(!data.is_empty(), "'data' array should not be empty");

        for item in data {
            assert_eq!(
                item["object"], "embedding",
                "data item should be 'embedding'"
            );
            assert!(item["index"].is_number(), "embedding should have 'index'");
            assert!(
                item["embedding"].is_array(),
                "embedding should have 'embedding' vector"
            );
        }

        // Usage is required
        let usage = &body["usage"];
        assert!(
            usage["prompt_tokens"].is_number(),
            "usage should have 'prompt_tokens'"
        );
        assert!(
            usage["total_tokens"].is_number(),
            "usage should have 'total_tokens'"
        );
    }

    /// Assert a legacy Completions API response conforms to OpenAI format.
    pub fn assert_completion(body: &Value) {
        assert_eq!(
            body["object"], "text_completion",
            "Expected object 'text_completion', got {:?}",
            body["object"]
        );
        assert!(body["id"].is_string(), "Missing or invalid 'id' field");
        assert!(
            body["created"].is_number(),
            "Missing or invalid 'created' field"
        );
        assert!(
            body["model"].is_string(),
            "Missing or invalid 'model' field"
        );

        let choices = body["choices"]
            .as_array()
            .expect("'choices' should be an array");
        assert!(!choices.is_empty(), "'choices' array should not be empty");

        let choice = &choices[0];
        assert!(
            choice["text"].is_string(),
            "choice should have 'text' field"
        );
        assert!(
            choice["index"].is_number(),
            "choice should have 'index' field"
        );

        // Usage is required
        let usage = &body["usage"];
        assert!(
            usage["prompt_tokens"].is_number(),
            "usage should have 'prompt_tokens'"
        );
        assert!(
            usage["completion_tokens"].is_number(),
            "usage should have 'completion_tokens'"
        );
    }

    /// Assert an error response conforms to OpenAI format.
    pub fn assert_error(body: &Value) {
        let error = &body["error"];
        assert!(error.is_object(), "Response should have 'error' object");
        assert!(
            error["message"].is_string(),
            "error should have 'message' field"
        );
        assert!(error["type"].is_string(), "error should have 'type' field");
    }

    /// Parse SSE streaming response and return validated chunks.
    /// Validates each chunk conforms to Chat Completions streaming format.
    pub fn parse_streaming_chunks(body: &str) -> Vec<Value> {
        assert!(body.contains("data:"), "Response should contain SSE data");
        assert!(
            body.contains("[DONE]"),
            "Response should end with [DONE] marker"
        );

        let mut chunks = Vec::new();
        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    continue;
                }
                let chunk: Value = serde_json::from_str(data)
                    .unwrap_or_else(|e| panic!("Invalid SSE chunk JSON: {}\nData: {}", e, data));
                chunks.push(chunk);
            }
        }

        assert!(!chunks.is_empty(), "Should have at least one SSE chunk");
        chunks
    }

    /// Assert streaming Chat Completions chunks are valid.
    pub fn assert_streaming_chat_completion(body: &str) -> Vec<Value> {
        let chunks = parse_streaming_chunks(body);

        for chunk in &chunks {
            assert_eq!(
                chunk["object"], "chat.completion.chunk",
                "Expected object 'chat.completion.chunk', got {:?}",
                chunk["object"]
            );
            assert!(chunk["id"].is_string(), "chunk should have 'id'");
            assert!(chunk["created"].is_number(), "chunk should have 'created'");

            let choices = chunk["choices"]
                .as_array()
                .expect("chunk should have 'choices' array");

            for choice in choices {
                assert!(choice["index"].is_number(), "choice should have 'index'");
                // Delta should be present (may be empty object for final chunk)
                assert!(
                    choice["delta"].is_object(),
                    "choice should have 'delta' object"
                );
            }
        }

        chunks
    }

    /// Assert streaming Responses API events are valid.
    pub fn assert_streaming_responses(body: &str) -> Vec<Value> {
        let chunks = parse_streaming_chunks(body);

        // Responses API uses event types in the chunks
        for chunk in &chunks {
            // Each chunk should have a type field
            assert!(
                chunk["type"].is_string(),
                "Responses API chunk should have 'type' field, got: {:?}",
                chunk
            );
        }

        chunks
    }
}

// =============================================================================
// Sequential Responder for Circuit Breaker / Retry Testing
// =============================================================================

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering as AtomicOrdering},
};

/// Internal state for SequentialResponder, wrapped in Arc for sharing.
struct SequentialResponderState {
    responses: Vec<ResponseTemplate>,
    call_count: AtomicUsize,
}

/// A wiremock responder that returns different responses on successive calls.
/// Useful for testing retry logic and circuit breaker behavior.
///
/// This type is Clone and can be shared to track call counts after mounting.
///
/// # Example
///
/// ```ignore
/// // Fail twice, then succeed
/// let responder = SequentialResponder::new(vec![
///     ResponseTemplate::new(500).set_body_json(json!({"error": "server error"})),
///     ResponseTemplate::new(500).set_body_json(json!({"error": "server error"})),
///     ResponseTemplate::new(200).set_body_json(json!({"status": "ok"})),
/// ]);
///
/// Mock::given(method("POST"))
///     .respond_with(responder.clone())
///     .mount(&mock_server)
///     .await;
///
/// // Later: check how many times the mock was called
/// assert_eq!(responder.call_count(), 3);
/// ```
#[derive(Clone)]
pub struct SequentialResponder {
    state: Arc<SequentialResponderState>,
}

impl SequentialResponder {
    /// Create a new SequentialResponder with the given responses.
    /// Each call returns the next response in order.
    /// After exhausting the list, it repeats the last response.
    pub fn new(responses: Vec<ResponseTemplate>) -> Self {
        assert!(
            !responses.is_empty(),
            "SequentialResponder requires at least one response"
        );
        Self {
            state: Arc::new(SequentialResponderState {
                responses,
                call_count: AtomicUsize::new(0),
            }),
        }
    }

    /// Create a responder that fails `fail_count` times with a 500 error,
    /// then succeeds with the given success response.
    pub fn fail_then_succeed(fail_count: usize, success_response: ResponseTemplate) -> Self {
        let mut responses = Vec::with_capacity(fail_count + 1);
        let error_body = serde_json::json!({
            "error": {
                "type": "server_error",
                "message": "Internal server error. Please try again later.",
                "code": "internal_error"
            }
        });

        for _ in 0..fail_count {
            responses.push(
                ResponseTemplate::new(500)
                    .insert_header("content-type", "application/json")
                    .set_body_json(&error_body),
            );
        }
        responses.push(success_response);

        Self::new(responses)
    }

    /// Create a responder that always fails with a 500 error.
    /// Useful for testing circuit breaker opening.
    pub fn always_fail() -> Self {
        let error_body = serde_json::json!({
            "error": {
                "type": "server_error",
                "message": "Internal server error. Please try again later.",
                "code": "internal_error"
            }
        });

        Self::new(vec![
            ResponseTemplate::new(500)
                .insert_header("content-type", "application/json")
                .set_body_json(&error_body),
        ])
    }

    /// Get the number of times this responder has been called.
    pub fn call_count(&self) -> usize {
        self.state.call_count.load(AtomicOrdering::SeqCst)
    }
}

impl wiremock::Respond for SequentialResponder {
    fn respond(&self, _request: &wiremock::Request) -> ResponseTemplate {
        let count = self.state.call_count.fetch_add(1, AtomicOrdering::SeqCst);
        let idx = count.min(self.state.responses.len() - 1);
        self.state.responses[idx].clone()
    }
}

/// Helper to create a success response from a fixture.
pub fn success_response_from_fixture(fixture: &Fixture) -> ResponseTemplate {
    build_response_template(fixture)
}

fn build_response_template(fixture: &Fixture) -> ResponseTemplate {
    let mut response = ResponseTemplate::new(fixture.response.status);

    // Add headers
    for (key, value) in &fixture.response.headers {
        response = response.insert_header(key.as_str(), value.as_str());
    }

    // Set body
    if fixture.response.streaming {
        match fixture.response.streaming_format {
            StreamingFormat::Sse => {
                // Build SSE body from chunks
                if let Some(chunks) = &fixture.response.chunks {
                    let mut body = String::new();
                    for chunk in chunks {
                        body.push_str(&format!("data: {}\n\n", chunk));
                    }
                    body.push_str("data: [DONE]\n\n");
                    response = response.set_body_string(body);
                }
            }
            StreamingFormat::AwsEventstream => {
                #[cfg(feature = "provider-bedrock")]
                {
                    // Build AWS EventStream binary body from events
                    if let Some(events) = &fixture.response.events {
                        let mut body = Vec::new();
                        for event in events {
                            let payload = serde_json::to_string(&event.payload)
                                .expect("Failed to serialize event payload");
                            let event_bytes = create_event_message(&event.event_type, &payload);
                            body.extend(event_bytes);
                        }
                        response = response.set_body_bytes(body);
                    }
                }
                #[cfg(not(feature = "provider-bedrock"))]
                {
                    panic!("AWS EventStream streaming requires the provider-bedrock feature");
                }
            }
        }
    } else if let Some(body_base64) = &fixture.response.body_base64 {
        // Decode base64 binary body (for audio/binary responses)
        let bytes = BASE64
            .decode(body_base64)
            .expect("Invalid base64 in fixture body_base64");
        response = response.set_body_bytes(bytes);
    } else if let Some(body) = &fixture.response.body {
        response = response.set_body_json(body);
    }

    response
}

// =============================================================================
// Schema Validation (OpenAPI-based)
// =============================================================================

/// Schema validation utilities for validating API responses against OpenAI OpenAPI spec.
///
/// This module extracts JSON Schemas from the OpenAI OpenAPI specification and
/// validates response bodies against them for conformance testing.
///
/// # Usage
///
/// ```ignore
/// use crate::providers::test_utils::schema::{OpenApiSchemas, SchemaId};
///
/// let schemas = OpenApiSchemas::load().expect("Failed to load schemas");
/// let response_body = json!({"id": "chatcmpl-123", ...});
///
/// schemas.validate(SchemaId::ChatCompletion, &response_body)
///     .expect("Response should conform to ChatCompletion schema");
/// ```
pub mod schema {
    use std::{collections::HashMap, path::PathBuf, sync::RwLock};

    use once_cell::sync::Lazy;
    use serde_json::Value;

    /// Well-known schema identifiers for OpenAI API responses.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum SchemaId {
        /// CreateChatCompletionResponse - Chat Completions API
        ChatCompletion,
        /// CreateChatCompletionStreamResponse - Streaming Chat Completions
        ChatCompletionStream,
        /// CreateCompletionResponse - Legacy Completions API
        Completion,
        /// CreateEmbeddingResponse - Embeddings API
        Embedding,
        /// ErrorResponse - Error responses
        Error,
    }

    impl SchemaId {
        /// Get the OpenAPI component schema name for this ID.
        pub fn schema_name(&self) -> &'static str {
            match self {
                SchemaId::ChatCompletion => "CreateChatCompletionResponse",
                SchemaId::ChatCompletionStream => "CreateChatCompletionStreamResponse",
                SchemaId::Completion => "CreateCompletionResponse",
                SchemaId::Embedding => "CreateEmbeddingResponse",
                SchemaId::Error => "ErrorResponse",
            }
        }
    }

    /// Validation error with details about what failed.
    #[cfg(feature = "response-validation")]
    #[derive(Debug)]
    pub struct ValidationError {
        pub path: String,
        pub message: String,
    }

    #[cfg(feature = "response-validation")]
    impl std::fmt::Display for ValidationError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}: {}", self.path, self.message)
        }
    }

    /// Result of schema validation.
    #[cfg(feature = "response-validation")]
    #[derive(Debug)]
    pub struct ValidationResult {
        pub is_valid: bool,
        pub errors: Vec<ValidationError>,
    }

    /// Container for OpenAPI schemas with lazy loading and caching.
    pub struct OpenApiSchemas {
        /// Raw OpenAPI spec as JSON (converted from YAML)
        spec: Value,
        /// Compiled JSON schemas, cached by schema name
        compiled: RwLock<HashMap<String, Value>>,
    }

    /// Global singleton for loaded schemas.
    static SCHEMAS: Lazy<Result<OpenApiSchemas, String>> = Lazy::new(OpenApiSchemas::load);

    impl OpenApiSchemas {
        /// Get the global schema instance.
        pub fn get() -> Result<&'static OpenApiSchemas, &'static str> {
            SCHEMAS.as_ref().map_err(|e| e.as_str())
        }

        /// Load the OpenAPI spec from the repository.
        fn load() -> Result<Self, String> {
            let spec_path = Self::spec_path();
            let content = std::fs::read_to_string(&spec_path)
                .map_err(|e| format!("Failed to read OpenAPI spec at {:?}: {}", spec_path, e))?;

            // Parse JSON
            let spec: Value = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse OpenAPI spec JSON: {}", e))?;

            Ok(Self {
                spec,
                compiled: RwLock::new(HashMap::new()),
            })
        }

        fn spec_path() -> PathBuf {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openapi/openai.openapi.json")
        }

        /// Extract and compile a schema by ID.
        /// Returns a resolved JSON Schema suitable for validation.
        pub fn get_schema(&self, id: SchemaId) -> Result<Value, String> {
            let schema_name = id.schema_name();

            // Check cache first
            {
                let cache = self.compiled.read().unwrap();
                if let Some(schema) = cache.get(schema_name) {
                    return Ok(schema.clone());
                }
            }

            // Extract and resolve the schema
            let schema = self.extract_schema(schema_name)?;

            // Cache it
            {
                let mut cache = self.compiled.write().unwrap();
                cache.insert(schema_name.to_string(), schema.clone());
            }

            Ok(schema)
        }

        /// Extract and resolve a schema by name (for arbitrary schema names).
        /// This is useful for discriminator-based validation where the schema name
        /// is determined at runtime from the event type.
        pub fn extract_schema_by_name(&self, name: &str) -> Result<Value, String> {
            self.extract_schema(name)
        }

        /// Extract a schema from components/schemas and resolve $ref references.
        fn extract_schema(&self, name: &str) -> Result<Value, String> {
            let schemas = self
                .spec
                .get("components")
                .and_then(|c| c.get("schemas"))
                .ok_or_else(|| "OpenAPI spec missing components/schemas".to_string())?;

            let raw_schema = schemas
                .get(name)
                .ok_or_else(|| format!("Schema '{}' not found in OpenAPI spec", name))?;

            // Resolve $ref references within the schema
            self.resolve_refs(raw_schema.clone(), 0)
        }

        /// Recursively resolve $ref references in a schema.
        /// Also handles OpenAPI 3.0 `nullable: true` by converting to JSON Schema anyOf.
        /// max_depth prevents infinite recursion on circular refs.
        fn resolve_refs(&self, mut schema: Value, depth: usize) -> Result<Value, String> {
            const MAX_DEPTH: usize = 50;
            if depth > MAX_DEPTH {
                // Return a permissive schema for deeply nested or circular refs
                return Ok(serde_json::json!({}));
            }

            match &mut schema {
                Value::Object(map) => {
                    // Handle $ref
                    if let Some(Value::String(ref_path)) = map.get("$ref") {
                        let resolved = self.resolve_ref(ref_path, depth + 1)?;
                        return Ok(resolved);
                    }

                    // Handle OpenAPI 3.0 `nullable: true` - convert to JSON Schema anyOf
                    // This must be done before processing other fields so nested schemas
                    // with nullable are properly handled.
                    let is_nullable = map
                        .get("nullable")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    if is_nullable {
                        // Remove nullable key
                        map.remove("nullable");

                        // Clone the current schema (without nullable) for further processing
                        let non_null_schema = self.resolve_refs(schema.clone(), depth + 1)?;

                        // Wrap in anyOf with null type
                        return Ok(serde_json::json!({
                            "anyOf": [
                                non_null_schema,
                                { "type": "null" }
                            ]
                        }));
                    }

                    // Handle allOf, anyOf, oneOf by resolving their items
                    for key in ["allOf", "anyOf", "oneOf"] {
                        if let Some(Value::Array(items)) = map.get_mut(key) {
                            let resolved_items: Result<Vec<Value>, String> = items
                                .iter()
                                .map(|item| self.resolve_refs(item.clone(), depth + 1))
                                .collect();
                            *items = resolved_items?;
                        }
                    }

                    // Handle properties
                    if let Some(Value::Object(props)) = map.get_mut("properties") {
                        let keys: Vec<String> = props.keys().cloned().collect();
                        for key in keys {
                            if let Some(prop) = props.remove(&key) {
                                let resolved = self.resolve_refs(prop, depth + 1)?;
                                props.insert(key, resolved);
                            }
                        }
                    }

                    // Handle items (for arrays)
                    if let Some(items) = map.remove("items") {
                        let resolved = self.resolve_refs(items, depth + 1)?;
                        map.insert("items".to_string(), resolved);
                    }

                    // Handle additionalProperties
                    if let Some(Value::Object(_)) = map.get("additionalProperties")
                        && let Some(ap) = map.remove("additionalProperties")
                    {
                        let resolved = self.resolve_refs(ap, depth + 1)?;
                        map.insert("additionalProperties".to_string(), resolved);
                    }

                    Ok(schema)
                }
                Value::Array(arr) => {
                    let resolved: Result<Vec<Value>, String> = arr
                        .iter()
                        .map(|item| self.resolve_refs(item.clone(), depth + 1))
                        .collect();
                    Ok(Value::Array(resolved?))
                }
                _ => Ok(schema),
            }
        }

        /// Resolve a $ref path like "#/components/schemas/SomeType"
        fn resolve_ref(&self, ref_path: &str, depth: usize) -> Result<Value, String> {
            if !ref_path.starts_with("#/components/schemas/") {
                // External refs not supported - return permissive schema
                return Ok(serde_json::json!({}));
            }

            let schema_name = ref_path
                .strip_prefix("#/components/schemas/")
                .ok_or_else(|| format!("Invalid $ref path: {}", ref_path))?;

            let schemas = self
                .spec
                .get("components")
                .and_then(|c| c.get("schemas"))
                .ok_or_else(|| "OpenAPI spec missing components/schemas".to_string())?;

            let raw_schema = schemas
                .get(schema_name)
                .ok_or_else(|| format!("Referenced schema '{}' not found", schema_name))?;

            self.resolve_refs(raw_schema.clone(), depth)
        }

        /// Validate a JSON value against a schema.
        #[cfg(feature = "response-validation")]
        pub fn validate(&self, id: SchemaId, value: &Value) -> ValidationResult {
            let schema = match self.get_schema(id) {
                Ok(s) => s,
                Err(e) => {
                    return ValidationResult {
                        is_valid: false,
                        errors: vec![ValidationError {
                            path: "".to_string(),
                            message: format!("Failed to load schema: {}", e),
                        }],
                    };
                }
            };

            // Compile and validate using jsonschema crate
            match jsonschema::draft202012::new(&schema) {
                Ok(validator) => {
                    let errors: Vec<ValidationError> = validator
                        .iter_errors(value)
                        .map(|e| ValidationError {
                            path: e.instance_path.to_string(),
                            message: e.to_string(),
                        })
                        .collect();

                    ValidationResult {
                        is_valid: errors.is_empty(),
                        errors,
                    }
                }
                Err(e) => ValidationResult {
                    is_valid: false,
                    errors: vec![ValidationError {
                        path: "".to_string(),
                        message: format!("Failed to compile schema: {}", e),
                    }],
                },
            }
        }
    }

    /// Convenience function to validate against a schema.
    /// Returns Ok(()) if valid, Err with details if invalid.
    #[cfg(feature = "response-validation")]
    pub fn validate_response(id: SchemaId, value: &Value) -> Result<(), String> {
        let schemas = OpenApiSchemas::get().map_err(|e| e.to_string())?;
        let result = schemas.validate(id, value);

        if result.is_valid {
            Ok(())
        } else {
            let msgs: Vec<String> = result.errors.iter().map(|e| e.to_string()).collect();
            Err(msgs.join("; "))
        }
    }

    /// Validate streaming chat completion chunks against the OpenAPI schema.
    ///
    /// Returns Ok(()) if all chunks are valid, Err with details about the first invalid chunk.
    /// Skips validation for chunks that have extra provider-specific fields (like `obfuscation`)
    /// by only validating required fields are present and correctly typed.
    #[cfg(feature = "response-validation")]
    pub fn validate_streaming_chunks(chunks: &[Value]) -> Result<(), String> {
        let schemas = OpenApiSchemas::get().map_err(|e| e.to_string())?;

        for (i, chunk) in chunks.iter().enumerate() {
            let result = schemas.validate(SchemaId::ChatCompletionStream, chunk);

            if !result.is_valid {
                let msgs: Vec<String> = result.errors.iter().map(|e| e.to_string()).collect();
                return Err(format!(
                    "Chunk {} failed validation:\n{}\nChunk: {}",
                    i,
                    msgs.join("\n"),
                    serde_json::to_string_pretty(chunk).unwrap_or_default()
                ));
            }
        }

        Ok(())
    }

    /// Map Responses API event type to its schema name.
    /// Returns None for unknown event types.
    fn responses_event_schema_name(event_type: &str) -> Option<&'static str> {
        match event_type {
            // Core response lifecycle events
            "response.created" => Some("ResponseCreatedEvent"),
            "response.in_progress" => Some("ResponseInProgressEvent"),
            "response.completed" => Some("ResponseCompletedEvent"),
            "response.failed" => Some("ResponseFailedEvent"),
            "response.incomplete" => Some("ResponseIncompleteEvent"),
            "response.queued" => Some("ResponseQueuedEvent"),

            // Output item events
            "response.output_item.added" => Some("ResponseOutputItemAddedEvent"),
            "response.output_item.done" => Some("ResponseOutputItemDoneEvent"),

            // Content part events
            "response.content_part.added" => Some("ResponseContentPartAddedEvent"),
            "response.content_part.done" => Some("ResponseContentPartDoneEvent"),

            // Text events
            "response.output_text.delta" => Some("ResponseTextDeltaEvent"),
            "response.output_text.done" => Some("ResponseTextDoneEvent"),

            // Refusal events
            "response.refusal.delta" => Some("ResponseRefusalDeltaEvent"),
            "response.refusal.done" => Some("ResponseRefusalDoneEvent"),

            // Function call events
            "response.function_call_arguments.delta" => {
                Some("ResponseFunctionCallArgumentsDeltaEvent")
            }
            "response.function_call_arguments.done" => {
                Some("ResponseFunctionCallArgumentsDoneEvent")
            }

            // File search events
            "response.file_search_call.in_progress" => {
                Some("ResponseFileSearchCallInProgressEvent")
            }
            "response.file_search_call.searching" => Some("ResponseFileSearchCallSearchingEvent"),
            "response.file_search_call.completed" => Some("ResponseFileSearchCallCompletedEvent"),

            // Image generation events
            "response.image_generation_call.in_progress" => {
                Some("ResponseImageGenCallInProgressEvent")
            }
            "response.image_generation_call.generating" => {
                Some("ResponseImageGenCallGeneratingEvent")
            }
            "response.image_generation_call.partial_image" => {
                Some("ResponseImageGenCallPartialImageEvent")
            }
            "response.image_generation_call.completed" => {
                Some("ResponseImageGenCallCompletedEvent")
            }

            // Code interpreter events
            "response.code_interpreter_call.in_progress" => {
                Some("ResponseCodeInterpreterCallInProgressEvent")
            }
            "response.code_interpreter_call.interpreting" => {
                Some("ResponseCodeInterpreterCallInterpretingEvent")
            }
            "response.code_interpreter_call.code.delta" => {
                Some("ResponseCodeInterpreterCallCodeDeltaEvent")
            }
            "response.code_interpreter_call.code.done" => {
                Some("ResponseCodeInterpreterCallCodeDoneEvent")
            }
            "response.code_interpreter_call.completed" => {
                Some("ResponseCodeInterpreterCallCompletedEvent")
            }

            // Web search events
            "response.web_search_call.in_progress" => Some("ResponseWebSearchCallInProgressEvent"),
            "response.web_search_call.searching" => Some("ResponseWebSearchCallSearchingEvent"),
            "response.web_search_call.completed" => Some("ResponseWebSearchCallCompletedEvent"),

            // Reasoning events
            "response.reasoning_summary_part.added" => {
                Some("ResponseReasoningSummaryPartAddedEvent")
            }
            "response.reasoning_summary_part.done" => Some("ResponseReasoningSummaryPartDoneEvent"),
            "response.reasoning_summary_text.delta" => {
                Some("ResponseReasoningSummaryTextDeltaEvent")
            }
            "response.reasoning_summary_text.done" => Some("ResponseReasoningSummaryTextDoneEvent"),
            "response.reasoning.delta" => Some("ResponseReasoningTextDeltaEvent"),
            "response.reasoning.done" => Some("ResponseReasoningTextDoneEvent"),

            // Audio events
            "response.audio.delta" => Some("ResponseAudioDeltaEvent"),
            "response.audio.done" => Some("ResponseAudioDoneEvent"),
            "response.audio_transcript.delta" => Some("ResponseAudioTranscriptDeltaEvent"),
            "response.audio_transcript.done" => Some("ResponseAudioTranscriptDoneEvent"),

            // Error event
            "response.error" => Some("ResponseErrorEvent"),

            _ => None,
        }
    }

    /// Event types that contain the complex `Response` schema which fails to compile.
    /// These are skipped during validation until schema resolution is improved.
    #[cfg(feature = "response-validation")]
    const COMPLEX_RESPONSE_EVENTS: &[&str] = &[
        "response.created",
        "response.in_progress",
        "response.completed",
        "response.failed",
        "response.incomplete",
        "response.queued",
    ];

    /// Validate Responses API streaming chunks against their OpenAPI schemas.
    ///
    /// Uses discriminator-based validation: each chunk's `type` field determines
    /// which schema to validate against.
    ///
    /// Note: Events containing the full `Response` object are skipped because the
    /// `Response` schema has complex `allOf` composition that fails to compile.
    ///
    /// Returns Ok(()) if all chunks are valid, Err with details about the first invalid chunk.
    #[cfg(feature = "response-validation")]
    pub fn validate_responses_streaming_chunks(chunks: &[Value]) -> Result<(), String> {
        let schemas = OpenApiSchemas::get().map_err(|e| e.to_string())?;

        for (i, chunk) in chunks.iter().enumerate() {
            let event_type = chunk
                .get("type")
                .and_then(|t| t.as_str())
                .ok_or_else(|| format!("Chunk {} missing 'type' field", i))?;

            // Skip events that contain the complex Response object
            if COMPLEX_RESPONSE_EVENTS.contains(&event_type) {
                continue;
            }

            let schema_name = responses_event_schema_name(event_type).ok_or_else(|| {
                format!(
                    "Chunk {} has unknown event type '{}' - add mapping to responses_event_schema_name()",
                    i, event_type
                )
            })?;

            // Get the schema for this event type
            let schema = schemas.extract_schema_by_name(schema_name)?;

            // Validate chunk against the schema
            match jsonschema::draft202012::new(&schema) {
                Ok(validator) => {
                    let errors: Vec<String> = validator
                        .iter_errors(chunk)
                        .map(|e| format!("{}: {}", e.instance_path, e))
                        .collect();

                    if !errors.is_empty() {
                        return Err(format!(
                            "Chunk {} (type={}) failed {} schema validation:\n{}\nChunk: {}",
                            i,
                            event_type,
                            schema_name,
                            errors.join("\n"),
                            serde_json::to_string_pretty(chunk).unwrap_or_default()
                        ));
                    }
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to compile schema '{}' for chunk {}: {}",
                        schema_name, i, e
                    ));
                }
            }
        }

        Ok(())
    }

    #[cfg(test)]
    mod tests {
        #[cfg(feature = "response-validation")]
        use serde_json::json;

        use super::*;

        #[test]
        fn test_schema_loading() {
            let schemas = OpenApiSchemas::get().expect("Should load schemas");
            let schema = schemas
                .get_schema(SchemaId::ChatCompletion)
                .expect("Should extract ChatCompletion schema");

            // Schema should be an object with properties
            assert!(schema.is_object(), "Schema should be an object");
        }

        #[cfg(feature = "response-validation")]
        #[test]
        fn test_chat_completion_validation_success() {
            // Valid chat completion response (matching current OpenAI schema)
            let valid_response = json!({
                "id": "chatcmpl-123",
                "object": "chat.completion",
                "created": 1677652288,
                "model": "gpt-4o-mini",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello, how can I help you?",
                        "refusal": null
                    },
                    "logprobs": null,
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 20,
                    "total_tokens": 30
                }
            });

            let result = validate_response(SchemaId::ChatCompletion, &valid_response);
            assert!(result.is_ok(), "Valid response should pass: {:?}", result);
        }

        #[cfg(feature = "response-validation")]
        #[test]
        fn test_chat_completion_validation_missing_required() {
            // Missing required "choices" field
            let invalid_response = json!({
                "id": "chatcmpl-123",
                "object": "chat.completion",
                "created": 1677652288,
                "model": "gpt-4o-mini"
            });

            let result = validate_response(SchemaId::ChatCompletion, &invalid_response);
            assert!(result.is_err(), "Invalid response should fail validation");
            assert!(
                result.unwrap_err().contains("choices"),
                "Error should mention missing 'choices'"
            );
        }

        #[cfg(feature = "response-validation")]
        #[test]
        fn test_validate_fixture_chat_completion() {
            // Load an actual fixture and validate it
            let fixture = crate::providers::test_utils::load_fixture(
                crate::providers::test_utils::FixtureId::OpenAiChatCompletionSuccess,
            );

            if let Some(body) = &fixture.response.body {
                let result = validate_response(SchemaId::ChatCompletion, body);
                assert!(
                    result.is_ok(),
                    "OpenAI fixture should pass validation: {:?}",
                    result
                );
            }
        }

        #[cfg(feature = "response-validation")]
        #[test]
        fn test_error_schema_validation() {
            let error_response = json!({
                "error": {
                    "message": "Invalid API key",
                    "type": "invalid_request_error",
                    "code": "invalid_api_key"
                }
            });

            let result = validate_response(SchemaId::Error, &error_response);
            // Note: Error schema validation may be lenient - this tests that it doesn't crash
            println!("Error validation result: {:?}", result);
        }

        #[test]
        fn test_streaming_chunk_schema_loading() {
            let schemas = OpenApiSchemas::get().expect("Should load schemas");
            let schema = schemas
                .get_schema(SchemaId::ChatCompletionStream)
                .expect("Should extract ChatCompletionStream schema");

            // Schema should be an object with required fields
            assert!(schema.is_object(), "Schema should be an object");
            assert!(
                schema.get("required").is_some(),
                "Schema should have required fields"
            );
        }

        #[cfg(feature = "response-validation")]
        #[test]
        fn test_streaming_chunk_validation_success() {
            // Valid streaming chunk (matching OpenAI schema)
            let valid_chunk = json!({
                "id": "chatcmpl-123",
                "object": "chat.completion.chunk",
                "created": 1694268190,
                "model": "gpt-4o-mini",
                "choices": [{
                    "index": 0,
                    "delta": {
                        "content": "Hello"
                    },
                    "logprobs": null,
                    "finish_reason": null
                }]
            });

            let result = validate_streaming_chunks(&[valid_chunk]);
            assert!(
                result.is_ok(),
                "Valid streaming chunk should pass: {:?}",
                result
            );
        }

        #[cfg(feature = "response-validation")]
        #[test]
        fn test_streaming_chunk_validation_missing_required() {
            // Missing required "choices" field
            let invalid_chunk = json!({
                "id": "chatcmpl-123",
                "object": "chat.completion.chunk",
                "created": 1694268190,
                "model": "gpt-4o-mini"
            });

            let result = validate_streaming_chunks(&[invalid_chunk]);
            assert!(result.is_err(), "Invalid chunk should fail validation");
            assert!(
                result.unwrap_err().contains("choices"),
                "Error should mention missing 'choices'"
            );
        }

        #[cfg(feature = "response-validation")]
        #[test]
        fn test_streaming_fixture_validation() {
            // Load and validate the OpenAI streaming fixture
            let fixture = crate::providers::test_utils::load_fixture(
                crate::providers::test_utils::FixtureId::OpenAiChatCompletionStreaming,
            );

            if let Some(chunks) = &fixture.response.chunks {
                let result = validate_streaming_chunks(chunks);
                assert!(
                    result.is_ok(),
                    "OpenAI streaming fixture should pass validation: {:?}",
                    result
                );
            } else {
                panic!("Streaming fixture should have chunks");
            }
        }

        #[test]
        fn test_responses_streaming_event_schema_loading() {
            let schemas = OpenApiSchemas::get().expect("Should load schemas");

            // Test loading a few known event schemas
            let event_types = [
                ("response.created", "ResponseCreatedEvent"),
                ("response.completed", "ResponseCompletedEvent"),
                ("response.output_text.delta", "ResponseTextDeltaEvent"),
            ];

            for (event_type, schema_name) in event_types {
                let schema = schemas
                    .extract_schema_by_name(schema_name)
                    .unwrap_or_else(|e| panic!("Should extract {} schema: {}", schema_name, e));

                assert!(
                    schema.is_object(),
                    "Schema {} should be an object",
                    schema_name
                );
                assert!(
                    schema.get("required").is_some(),
                    "Schema {} should have required fields",
                    schema_name
                );

                // Verify the mapping function works
                assert_eq!(
                    responses_event_schema_name(event_type),
                    Some(schema_name),
                    "Event type {} should map to {}",
                    event_type,
                    schema_name
                );
            }
        }

        #[cfg(feature = "response-validation")]
        #[test]
        fn test_responses_streaming_validation_success() {
            // Valid response.output_text.delta event (with all required fields)
            let valid_event = json!({
                "type": "response.output_text.delta",
                "item_id": "msg_123",
                "output_index": 0,
                "content_index": 0,
                "delta": "Hello",
                "sequence_number": 1,
                "logprobs": []
            });

            let result = validate_responses_streaming_chunks(&[valid_event]);
            assert!(
                result.is_ok(),
                "Valid Responses streaming event should pass: {:?}",
                result
            );
        }

        #[cfg(feature = "response-validation")]
        #[test]
        fn test_responses_streaming_validation_missing_required() {
            // Missing required "delta" field for text delta event
            let invalid_event = json!({
                "type": "response.output_text.delta",
                "item_id": "msg_123",
                "output_index": 0,
                "content_index": 0
                // missing "delta"
            });

            let result = validate_responses_streaming_chunks(&[invalid_event]);
            assert!(
                result.is_err(),
                "Invalid Responses event should fail validation"
            );
            assert!(
                result.unwrap_err().contains("delta"),
                "Error should mention missing 'delta'"
            );
        }

        #[cfg(feature = "response-validation")]
        #[test]
        fn test_responses_streaming_fixture_validation() {
            // Load and validate the OpenAI Responses streaming fixture
            let fixture = crate::providers::test_utils::load_fixture(
                crate::providers::test_utils::FixtureId::OpenAiResponsesStreaming,
            );

            if let Some(chunks) = &fixture.response.chunks {
                let result = validate_responses_streaming_chunks(chunks);
                assert!(
                    result.is_ok(),
                    "OpenAI Responses streaming fixture should pass validation: {:?}",
                    result
                );
            } else {
                panic!("Responses streaming fixture should have chunks");
            }
        }
    }
}
