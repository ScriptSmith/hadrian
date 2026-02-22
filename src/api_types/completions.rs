#![allow(dead_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CompletionPrompt {
    Text(String),
    TextArray(Vec<String>),
    Tokens(Vec<f64>),
    TokenArrays(Vec<Vec<f64>>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CompletionStop {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionStreamOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_usage: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompletionResponseFormat {
    Text,
    JsonObject,
    JsonSchema {
        json_schema: CompletionJsonSchemaConfig,
    },
    Grammar {
        grammar: String,
    },
    Python,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionJsonSchemaConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// Create text completion request (OpenAI-compatible)
#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateCompletionPayload {
    /// The prompt to generate completions for
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub prompt: CompletionPrompt,

    /// Model to use for completion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// **Hadrian Extension:** List of models for multi-model routing (alternative to single model)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,

    /// Number of completions to generate and return the best
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_of: Option<i64>,

    /// Echo the prompt in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echo: Option<bool>,

    /// Penalize repeated tokens (-2.0 to 2.0)
    #[validate(range(min = -2.0, max = 2.0))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,

    /// Token bias map
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub logit_bias: Option<HashMap<String, f64>>,

    /// Number of log probabilities to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<i64>,

    /// Maximum tokens to generate
    #[validate(range(min = 1))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i64>,

    /// Number of completions to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<i64>,

    /// Penalize new topics (-2.0 to 2.0)
    #[validate(range(min = -2.0, max = 2.0))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,

    /// Random seed for reproducibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    /// Stop sequence(s)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub stop: Option<CompletionStop>,

    /// Enable streaming
    #[serde(default)]
    pub stream: bool,

    /// Stream options
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub stream_options: Option<CompletionStreamOptions>,

    /// Text to append after completion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,

    /// Sampling temperature (0.0 to 2.0)
    #[validate(range(min = 0.0, max = 2.0))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Nucleus sampling probability (0.0 to 1.0)
    #[validate(range(min = 0.0, max = 1.0))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    /// User identifier for abuse detection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// **Hadrian Extension:** Request metadata for tracking and filtering
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub metadata: Option<HashMap<String, String>>,

    /// **Hadrian Extension:** Response format (OpenAI only supports this on chat/completions)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub response_format: Option<CompletionResponseFormat>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompletionFinishReason {
    Stop,
    Length,
    ContentFilter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionLogprobs {
    pub tokens: Vec<String>,
    pub token_logprobs: Vec<f64>,
    pub top_logprobs: Option<Vec<HashMap<String, f64>>>,
    pub text_offset: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChoice {
    pub text: String,
    pub index: f64,
    pub logprobs: Option<CompletionLogprobs>,
    pub finish_reason: CompletionFinishReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionUsage {
    pub prompt_tokens: f64,
    pub completion_tokens: f64,
    pub total_tokens: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompletionObjectType {
    #[serde(rename = "text_completion")]
    TextCompletion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompletionResponse {
    pub id: String,
    pub object: CompletionObjectType,
    pub created: f64,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    pub choices: Vec<CompletionChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<CompletionUsage>,
}
