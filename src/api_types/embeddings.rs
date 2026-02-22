#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use validator::Validate;

use super::responses::{
    DataVectorStore, ProviderMaxPrice, ProviderNameOrString, ProviderSort, Quantization,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingInputTextContent {
    #[serde(rename = "type")]
    pub type_: EmbeddingInputTextType,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingInputTextType {
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingInputImageUrl {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingInputImageContent {
    #[serde(rename = "type")]
    pub type_: EmbeddingInputImageType,
    pub image_url: EmbeddingInputImageUrl,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingInputImageType {
    ImageUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInputContentItem {
    Text(EmbeddingInputTextContent),
    Image(EmbeddingInputImageContent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingInputMultimodalItem {
    pub content: Vec<EmbeddingInputContentItem>,
}

/// Embedding input (text, array of texts, tokens, or multimodal)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(untagged)]
pub enum EmbeddingInput {
    Text(String),
    TextArray(Vec<String>),
    Tokens(Vec<f64>),
    TokenArrays(Vec<Vec<f64>>),
    #[cfg_attr(feature = "utoipa", schema(value_type = Vec<Object>))]
    Multimodal(Vec<EmbeddingInputMultimodalItem>),
}

/// Encoding format for embeddings
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum EncodingFormat {
    Float,
    Base64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_vector_store: Option<DataVectorStore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<ProviderNameOrString>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<ProviderNameOrString>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<ProviderNameOrString>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<Quantization>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<ProviderSort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_price: Option<ProviderMaxPrice>,
}

/// Create embedding request (OpenAI-compatible)
#[derive(Debug, Clone, Validate, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateEmbeddingPayload {
    /// Input to embed
    pub input: EmbeddingInput,

    /// Model to use for embedding
    pub model: String,

    /// Output encoding format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<EncodingFormat>,

    /// Number of dimensions for output embeddings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<i64>,

    /// User identifier for abuse detection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// **Hadrian Extension:** Provider routing configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "utoipa", schema(value_type = Object))]
    pub provider: Option<EmbeddingProviderConfig>,

    /// **Hadrian Extension:** Input type hint for embedding providers that support it (e.g., Cohere)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingObjectType {
    Embedding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingVector {
    Float(Vec<f64>),
    Base64(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub object: EmbeddingObjectType,
    pub embedding: EmbeddingVector,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: f64,
    pub total_tokens: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingResponseObjectType {
    List,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEmbeddingResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub object: EmbeddingResponseObjectType,
    pub data: Vec<EmbeddingData>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<EmbeddingUsage>,
}
