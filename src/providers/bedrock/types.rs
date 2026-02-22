//! Type definitions for AWS Bedrock API.
//!
//! This module contains all request/response types for:
//! - Bedrock Converse API (chat completions and responses API)
//! - Titan Embeddings API
//! - OpenAI-compatible response formats
//! - Streaming event types

use serde::{Deserialize, Serialize};

// ============================================================================
// Bedrock Converse API Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockConverseRequest {
    pub messages: Vec<BedrockMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<BedrockSystemContent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<BedrockInferenceConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<BedrockToolConfig>,
    /// Model-specific request fields (e.g., reasoning_config for Claude, reasoningConfig for Nova)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_model_request_fields: Option<serde_json::Value>,
}

/// System content block for Bedrock messages
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockSystemContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Cache point for prompt caching (Anthropic Claude models on Bedrock)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_point: Option<BedrockCachePoint>,
}

impl BedrockSystemContent {
    pub fn text(text: String) -> Self {
        Self {
            text: Some(text),
            cache_point: None,
        }
    }

    pub fn cache_point() -> Self {
        Self {
            text: None,
            cache_point: Some(BedrockCachePoint::default()),
        }
    }
}

// ============================================================================
// Cache Point Types (for Anthropic Claude models on Bedrock)
// ============================================================================

/// Cache point type for Bedrock prompt caching.
///
/// Currently only "default" is supported by AWS Bedrock.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BedrockCachePointType {
    #[default]
    Default,
}

/// Cache point block for Bedrock prompt caching.
///
/// Unlike Anthropic's inline `cache_control` property, Bedrock uses separate
/// `cachePoint` blocks that are inserted AFTER the content to be cached.
/// This is only supported by Anthropic Claude 3/4 models on Bedrock.
///
/// Example: To cache a system message, insert a cachePoint block after it:
/// ```json
/// "system": [
///   { "text": "You are a helpful assistant..." },
///   { "cachePoint": { "type": "default" } }
/// ]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BedrockCachePoint {
    #[serde(rename = "type")]
    pub type_: BedrockCachePointType,
}

#[derive(Debug, Serialize)]
pub(super) struct BedrockMessage {
    pub role: String,
    pub content: Vec<BedrockContent>,
}

/// Content block for Bedrock messages - can be text, image, tool use, tool result, or cache point
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<BedrockImage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use: Option<BedrockToolUse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<BedrockToolResult>,
    /// Cache point for prompt caching (Anthropic Claude models on Bedrock).
    /// Must be in a separate content block, inserted after the content to cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_point: Option<BedrockCachePoint>,
}

impl BedrockContent {
    pub fn text(text: String) -> Self {
        Self {
            text: Some(text),
            image: None,
            tool_use: None,
            tool_result: None,
            cache_point: None,
        }
    }

    pub fn image(format: String, bytes: String) -> Self {
        Self {
            text: None,
            image: Some(BedrockImage {
                format,
                source: BedrockImageSource { bytes },
            }),
            tool_use: None,
            tool_result: None,
            cache_point: None,
        }
    }

    pub fn tool_use(tool_use_id: String, name: String, input: serde_json::Value) -> Self {
        Self {
            text: None,
            image: None,
            tool_use: Some(BedrockToolUse {
                tool_use_id,
                name,
                input,
            }),
            tool_result: None,
            cache_point: None,
        }
    }

    pub fn tool_result(
        tool_use_id: String,
        content: String,
        status: Option<BedrockToolResultStatus>,
    ) -> Self {
        Self {
            text: None,
            image: None,
            tool_use: None,
            tool_result: Some(BedrockToolResult {
                tool_use_id,
                content: vec![BedrockToolResultContent { text: content }],
                status,
            }),
            cache_point: None,
        }
    }

    /// Creates a cache point content block.
    ///
    /// This should be inserted as a separate content block AFTER the content
    /// you want to cache. Only supported by Anthropic Claude 3/4 models on Bedrock.
    pub fn cache_point() -> Self {
        Self {
            text: None,
            image: None,
            tool_use: None,
            tool_result: None,
            cache_point: Some(BedrockCachePoint::default()),
        }
    }
}

/// Image content for Bedrock
#[derive(Debug, Serialize)]
pub(super) struct BedrockImage {
    pub format: String,
    pub source: BedrockImageSource,
}

/// Image source - base64 encoded bytes
#[derive(Debug, Serialize)]
pub(super) struct BedrockImageSource {
    pub bytes: String,
}

/// Tool use block (model calling a tool)
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockToolUse {
    pub tool_use_id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// Tool result block (returning results to the model)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockToolResult {
    pub tool_use_id: String,
    pub content: Vec<BedrockToolResultContent>,
    /// Status of the tool execution: "success" or "error".
    /// Only supported by Amazon Nova and Anthropic Claude 3/4 models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<BedrockToolResultStatus>,
}

/// Status of a tool result execution
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum BedrockToolResultStatus {
    Success,
    Error,
}

/// Content within a tool result
#[derive(Debug, Serialize)]
pub(super) struct BedrockToolResultContent {
    pub text: String,
}

/// Tool configuration for Bedrock
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockToolConfig {
    pub tools: Vec<BedrockTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<BedrockToolChoice>,
}

/// Tool definition for Bedrock.
///
/// Can contain either a tool specification or a cache point (but not both).
/// Cache points in the tools array signal that all tools up to that point
/// should be cached for future requests.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_spec: Option<BedrockToolSpec>,
    /// Cache point for prompt caching (Anthropic Claude models on Bedrock).
    /// When present, this should be a standalone entry in the tools array.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_point: Option<BedrockCachePoint>,
}

impl BedrockTool {
    /// Creates a tool with a specification.
    pub fn with_spec(spec: BedrockToolSpec) -> Self {
        Self {
            tool_spec: Some(spec),
            cache_point: None,
        }
    }

    /// Creates a cache point entry for the tools array.
    ///
    /// This should be inserted after tools that should be cached.
    pub fn cache_point() -> Self {
        Self {
            tool_spec: None,
            cache_point: Some(BedrockCachePoint::default()),
        }
    }
}

/// Tool specification for Bedrock
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockToolSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: BedrockInputSchema,
}

/// Input schema wrapper for Bedrock tools
#[derive(Debug, Serialize)]
pub(super) struct BedrockInputSchema {
    pub json: serde_json::Value,
}

/// Tool choice for Bedrock
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum BedrockToolChoice {
    Auto {},
    Any {},
    Tool { name: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockInferenceConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockConverseResponse {
    pub output: BedrockOutput,
    pub usage: BedrockUsage,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct BedrockOutput {
    pub message: BedrockOutputMessage,
}

#[derive(Debug, Deserialize)]
pub(super) struct BedrockOutputMessage {
    #[allow(dead_code)] // Deserialization field
    pub role: String,
    pub content: Vec<BedrockOutputContent>,
}

/// Output content from Bedrock - can be text, tool use, or reasoning content
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockOutputContent {
    pub text: Option<String>,
    pub tool_use: Option<BedrockToolUse>,
    /// Reasoning/thinking content from extended thinking (Claude 4+ models on Bedrock)
    pub reasoning_content: Option<BedrockReasoningContent>,
}

/// Reasoning content block from extended thinking (Claude 4+ / Nova models on Bedrock)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockReasoningContent {
    /// The reasoning text content
    pub reasoning_text: Option<BedrockReasoningText>,
}

/// Reasoning text with optional signature
#[derive(Debug, Deserialize)]
pub(super) struct BedrockReasoningText {
    /// The thinking/reasoning text
    pub text: String,
    /// Cryptographic signature for thinking content verification (Claude models)
    #[serde(default)]
    pub signature: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    /// Tokens read from the prompt cache (cache hit)
    #[serde(default)]
    pub cache_read_input_tokens: i64,
    /// Tokens written to the prompt cache (cache miss, will be cached).
    /// Captured for observability; OpenAI format doesn't have a field for this.
    #[serde(default)]
    #[allow(dead_code)] // Deserialization field
    pub cache_write_input_tokens: i64,
}

// ============================================================================
// OpenAI Response Types (for format conversion)
// ============================================================================

#[derive(Debug, Serialize)]
pub(super) struct OpenAIResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    pub usage: Option<OpenAIUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct OpenAIChoice {
    pub index: i32,
    pub message: OpenAIMessage,
    pub finish_reason: Option<String>,
    pub logprobs: Option<()>,
}

#[derive(Debug, Serialize)]
pub(super) struct OpenAIMessage {
    pub role: String,
    pub content: Option<String>,
    /// The refusal message generated by the model (required per OpenAI schema, null if not a refusal)
    pub refusal: Option<String>,
    /// Reasoning/thinking content from extended thinking (Hadrian extension)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Serialize)]
pub(super) struct OpenAIToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: OpenAIToolCallFunction,
}

#[derive(Debug, Serialize)]
pub(super) struct OpenAIToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// Breakdown of prompt tokens (OpenAI-compatible)
#[derive(Debug, Serialize)]
pub(super) struct PromptTokensDetails {
    /// Cached tokens read from prompt cache
    pub cached_tokens: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct OpenAIUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    /// Breakdown of prompt tokens including cache information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
}

// ============================================================================
// Titan Embeddings API Types
// ============================================================================

/// Titan Text Embeddings V2 request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TitanEmbeddingsRequest {
    pub input_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalize: Option<bool>,
}

/// Titan Text Embeddings response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TitanEmbeddingsResponse {
    pub embedding: Vec<f64>,
    pub input_text_token_count: i64,
}

// ============================================================================
// Streaming Response Types
// ============================================================================

/// OpenAI-compatible streaming chunk
#[derive(Debug, Serialize)]
pub(super) struct OpenAIStreamChunk {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIStreamChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIStreamUsage>,
}

#[derive(Debug, Serialize)]
pub(super) struct OpenAIStreamChoice {
    pub index: i32,
    pub delta: OpenAIDelta,
    pub finish_reason: Option<String>,
    pub logprobs: Option<()>,
}

#[derive(Debug, Default, Serialize)]
pub(super) struct OpenAIDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Reasoning/thinking content from extended thinking (Hadrian extension)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIStreamToolCall>>,
}

#[derive(Debug, Serialize)]
pub(super) struct OpenAIStreamToolCall {
    pub index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<&'static str>,
    pub function: OpenAIStreamFunction,
}

#[derive(Debug, Serialize)]
pub(super) struct OpenAIStreamFunction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Breakdown of prompt tokens for streaming (OpenAI-compatible)
#[derive(Debug, Serialize)]
pub(super) struct StreamPromptTokensDetails {
    /// Cached tokens read from prompt cache
    pub cached_tokens: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct OpenAIStreamUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    /// Breakdown of prompt tokens including cache information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<StreamPromptTokensDetails>,
}

// ============================================================================
// Bedrock ConverseStream Event Types (JSON payloads within event stream messages)
// ============================================================================

/// Role info from messageStart event
#[derive(Debug, Deserialize)]
pub(super) struct BedrockMessageStart {
    #[allow(dead_code)] // Deserialization field
    pub role: String,
}

/// Content block start event - indicates beginning of a content block
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockContentBlockStart {
    pub content_block_index: i32,
    #[serde(default)]
    pub start: Option<BedrockContentBlockStartData>,
}

/// Data within content block start (for tool use or reasoning)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockContentBlockStartData {
    #[serde(default)]
    pub tool_use: Option<BedrockToolUseStart>,
    /// Reasoning content block start (Claude 4+ / Nova models with extended thinking)
    #[serde(default)]
    pub reasoning_content: Option<BedrockReasoningContentStart>,
}

/// Reasoning content block start marker
#[derive(Debug, Deserialize)]
pub(super) struct BedrockReasoningContentStart {
    // The start block is empty but signals that reasoning content is coming
}

/// Tool use start info
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockToolUseStart {
    pub tool_use_id: String,
    pub name: String,
}

/// Content block delta event - contains incremental content
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockContentBlockDelta {
    pub content_block_index: i32,
    pub delta: BedrockDelta,
}

/// Delta content - can be text, tool use input, or reasoning content
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockDelta {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub tool_use: Option<BedrockToolUseDelta>,
    /// Reasoning/thinking content delta from extended thinking
    #[serde(default)]
    pub reasoning_content: Option<BedrockReasoningText>,
}

/// Tool use delta (partial JSON input)
#[derive(Debug, Deserialize)]
pub(super) struct BedrockToolUseDelta {
    pub input: String,
}

/// Content block stop event
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockContentBlockStop {
    #[allow(dead_code)] // Deserialization field
    pub content_block_index: i32,
}

/// Metadata event with usage and stop reason
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockMetadata {
    pub usage: BedrockStreamUsage,
    #[allow(dead_code)] // Deserialization field
    #[serde(default)]
    pub metrics: Option<BedrockMetrics>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockStreamUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    /// Tokens read from the prompt cache (cache hit)
    #[serde(default)]
    pub cache_read_input_tokens: i64,
    /// Tokens written to the prompt cache (cache miss, will be cached).
    /// Captured for observability; OpenAI format doesn't have a field for this.
    #[serde(default)]
    #[allow(dead_code)] // Deserialization field
    pub cache_write_input_tokens: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockMetrics {
    #[allow(dead_code)] // Deserialization field
    pub latency_ms: Option<i64>,
}

/// Message stop event (contains stop reason)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BedrockMessageStop {
    pub stop_reason: String,
}

// ============================================================================
// Bedrock Control Plane API Types (ListInferenceProfiles)
// ============================================================================

/// Response from ListInferenceProfiles API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListInferenceProfilesResponse {
    pub inference_profile_summaries: Vec<InferenceProfileSummary>,
    #[serde(default)]
    #[allow(dead_code)] // Deserialization field
    pub next_token: Option<String>,
}

/// Summary of an inference profile
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceProfileSummary {
    pub inference_profile_id: String,
    #[allow(dead_code)] // Deserialization field
    pub inference_profile_name: String,
    #[serde(default)]
    #[allow(dead_code)] // Deserialization field
    pub inference_profile_arn: Option<String>,
    #[serde(default)]
    #[allow(dead_code)] // Deserialization field
    pub description: Option<String>,
    /// Type: SYSTEM_DEFINED or APPLICATION
    #[serde(rename = "type")]
    #[allow(dead_code)] // Deserialization field
    pub profile_type: String,
    #[serde(default)]
    #[allow(dead_code)] // Deserialization field
    pub status: Option<String>,
    #[serde(default)]
    pub models: Vec<InferenceProfileModel>,
}

/// Model reference within an inference profile
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceProfileModel {
    pub model_arn: String,
}

// ============================================================================
// Bedrock Control Plane API Types (ListFoundationModels)
// ============================================================================

/// Response from ListFoundationModels API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFoundationModelsResponse {
    #[serde(default)]
    pub model_summaries: Vec<FoundationModelSummary>,
}

/// Summary of a foundation model
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundationModelSummary {
    /// The model identifier (e.g., "anthropic.claude-3-sonnet-20240229-v1:0")
    pub model_id: String,
    /// The Amazon Resource Name (ARN) of the model
    #[serde(default)]
    #[allow(dead_code)] // Deserialization field
    pub model_arn: Option<String>,
    /// The model display name
    #[serde(default)]
    #[allow(dead_code)] // Deserialization field
    pub model_name: Option<String>,
    /// The provider name (e.g., "Anthropic", "Amazon", "Meta")
    #[serde(default)]
    pub provider_name: Option<String>,
    /// Supported input modalities (TEXT, IMAGE, EMBEDDING)
    #[serde(default)]
    pub input_modalities: Vec<String>,
    /// Supported output modalities (TEXT, IMAGE, EMBEDDING)
    #[serde(default)]
    pub output_modalities: Vec<String>,
    /// Whether the model supports response streaming
    #[serde(default)]
    pub response_streaming_supported: Option<bool>,
    /// Supported inference types (ON_DEMAND, PROVISIONED)
    #[serde(default)]
    #[allow(dead_code)] // Deserialization field
    pub inference_types_supported: Vec<String>,
    /// Supported customization types
    #[serde(default)]
    #[allow(dead_code)] // Deserialization field
    pub customizations_supported: Vec<String>,
    /// Model lifecycle status
    #[serde(default)]
    pub model_lifecycle: Option<ModelLifecycle>,
}

/// Model lifecycle information
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelLifecycle {
    /// Current status (e.g., "ACTIVE", "LEGACY")
    pub status: String,
}
