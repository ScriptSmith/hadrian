//! Type definitions for Vertex AI API.

use serde::{Deserialize, Serialize};

// ============================================================================
// Vertex AI Gemini API Request Types
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexGenerateContentRequest {
    pub contents: Vec<VertexContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<VertexContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<VertexGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<VertexTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<VertexToolConfig>,
}

#[derive(Debug, Serialize)]
pub(super) struct VertexContent {
    pub role: String,
    pub parts: Vec<VertexPart>,
}

/// Vertex AI part - can be text, inline data (image), function call, or function response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<VertexInlineData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<VertexFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<VertexFunctionResponse>,
}

/// Inline data for images in Vertex AI
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexInlineData {
    pub mime_type: String,
    pub data: String,
}

impl VertexPart {
    pub fn text(text: String) -> Self {
        Self {
            text: Some(text),
            inline_data: None,
            function_call: None,
            function_response: None,
        }
    }

    pub fn inline_data(mime_type: String, data: String) -> Self {
        Self {
            text: None,
            inline_data: Some(VertexInlineData { mime_type, data }),
            function_call: None,
            function_response: None,
        }
    }

    pub fn function_call(name: String, args: serde_json::Value) -> Self {
        Self {
            text: None,
            inline_data: None,
            function_call: Some(VertexFunctionCall { name, args }),
            function_response: None,
        }
    }

    pub fn function_response(name: String, response: serde_json::Value) -> Self {
        Self {
            text: None,
            inline_data: None,
            function_call: None,
            function_response: Some(VertexFunctionResponse { name, response }),
        }
    }
}

/// Function call in Vertex AI format
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct VertexFunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

/// Function response in Vertex AI format
#[derive(Debug, Serialize)]
pub(super) struct VertexFunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

/// Tool definition for Vertex AI
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexTool {
    pub function_declarations: Vec<VertexFunctionDeclaration>,
}

/// Function declaration for Vertex AI
#[derive(Debug, Serialize)]
pub(super) struct VertexFunctionDeclaration {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Tool config for Vertex AI (controls how tools are used)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexToolConfig {
    pub function_calling_config: VertexFunctionCallingConfig,
}

/// Function calling config
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexFunctionCallingConfig {
    pub mode: VertexFunctionCallingMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_function_names: Option<Vec<String>>,
}

/// Function calling mode
#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum VertexFunctionCallingMode {
    Auto,
    Any,
    None,
}

/// Thinking configuration for Gemini 2.5+ and 3+ models.
///
/// Gemini 3 models use `thinking_level` (LOW, MEDIUM, HIGH).
/// Gemini 2.5 models use `thinking_budget` (0-24576, or -1 for dynamic).
/// Setting both will cause an error on Gemini 3 models.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexThinkingConfig {
    /// Thinking level for Gemini 3+ models (MINIMAL, LOW, MEDIUM, HIGH).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<VertexThinkingLevel>,
    /// Token budget for Gemini 2.5 models (0-24576, or -1 for dynamic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,
    /// Whether to include thought summaries in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
}

/// Thinking level for Gemini 3+ models.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum VertexThinkingLevel {
    /// Minimal reasoning (Gemini 3 Flash only)
    Minimal,
    /// Low reasoning depth
    Low,
    /// Medium reasoning (Gemini 3 Flash only)
    Medium,
    /// High reasoning (default)
    High,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Thinking configuration for Gemini 2.5+ and 3+ models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<VertexThinkingConfig>,
}

// ============================================================================
// Vertex AI Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexGenerateContentResponse {
    pub candidates: Vec<VertexCandidate>,
    pub usage_metadata: Option<VertexUsageMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexCandidate {
    pub content: VertexResponseContent,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct VertexResponseContent {
    pub parts: Vec<VertexResponsePart>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexResponsePart {
    pub text: Option<String>,
    pub function_call: Option<VertexFunctionCall>,
    /// Whether this part contains thinking/reasoning content (thought summary).
    #[serde(default)]
    pub thought: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct VertexUsageMetadata {
    /// Prompt token count (only present in final chunk with finishReason)
    #[serde(default)]
    pub prompt_token_count: i64,
    /// Candidates/completion token count (only present in final chunk)
    #[serde(default)]
    pub candidates_token_count: i64,
    /// Total token count (only present in final chunk)
    #[serde(default)]
    pub total_token_count: i64,
    /// Thinking/reasoning token count (for models with thinking enabled)
    #[serde(default)]
    pub thoughts_token_count: i64,
}

// ============================================================================
// OpenAI Response Types (for Chat Completion conversion)
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
    /// Reasoning/thinking content from the model (when thinking is enabled)
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

#[derive(Debug, Serialize)]
pub(super) struct OpenAIUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

// ============================================================================
// Embeddings API Types
// ============================================================================

/// Vertex AI embeddings request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexEmbeddingsRequest {
    pub instances: Vec<VertexEmbeddingInstance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<VertexEmbeddingParameters>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexEmbeddingInstance {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexEmbeddingParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_truncate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dimensionality: Option<i64>,
}

/// Vertex AI embeddings response
#[derive(Debug, Deserialize)]
pub(super) struct VertexEmbeddingsResponse {
    pub predictions: Vec<VertexEmbeddingPrediction>,
}

#[derive(Debug, Deserialize)]
pub(super) struct VertexEmbeddingPrediction {
    pub embeddings: VertexEmbedding,
}

#[derive(Debug, Deserialize)]
pub(super) struct VertexEmbedding {
    pub values: Vec<f64>,
    pub statistics: Option<VertexEmbeddingStatistics>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VertexEmbeddingStatistics {
    pub token_count: i64,
    #[allow(dead_code)] // Deserialization field
    pub truncated: bool,
}
