use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Owner scope for model pricing configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PricingOwner {
    /// Global pricing (no specific owner)
    Global,
    /// Organization-scoped pricing
    Organization { org_id: Uuid },
    /// Team-scoped pricing
    Team { team_id: Uuid },
    /// Project-scoped pricing
    Project { project_id: Uuid },
    /// User-scoped pricing
    User { user_id: Uuid },
}

/// Source of pricing data
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum PricingSource {
    /// Manually configured by user
    #[default]
    Manual,
    /// Fetched from provider API (e.g., OpenRouter)
    ProviderApi,
    /// Default pricing from static configuration
    Default,
}

impl PricingSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::ProviderApi => "provider_api",
            Self::Default => "default",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "provider_api" => Self::ProviderApi,
            "default" => Self::Default,
            _ => Self::Manual,
        }
    }
}

/// Database model for model pricing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DbModelPricing {
    pub id: Uuid,
    pub owner: PricingOwner,
    pub provider: String,
    pub model: String,
    /// Cost per 1M input tokens in microcents
    pub input_per_1m_tokens: i64,
    /// Cost per 1M output tokens in microcents
    pub output_per_1m_tokens: i64,
    /// Cost per image in microcents
    pub per_image: Option<i64>,
    /// Cost per request in microcents
    pub per_request: Option<i64>,
    /// Cost per 1M cached input tokens in microcents
    pub cached_input_per_1m_tokens: Option<i64>,
    /// Cost per 1M cache write tokens in microcents
    pub cache_write_per_1m_tokens: Option<i64>,
    /// Cost per 1M reasoning tokens in microcents
    pub reasoning_per_1m_tokens: Option<i64>,
    /// Cost per second of audio in microcents (for transcription/translation)
    pub per_second: Option<i64>,
    /// Cost per 1M characters in microcents (for TTS)
    pub per_1m_characters: Option<i64>,
    /// Source of this pricing
    pub source: PricingSource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DbModelPricing {
    /// Convert to the pricing module's ModelPricing struct
    pub fn to_model_pricing(&self) -> crate::pricing::ModelPricing {
        crate::pricing::ModelPricing {
            input_per_1m_tokens: self.input_per_1m_tokens,
            output_per_1m_tokens: self.output_per_1m_tokens,
            per_image: self.per_image,
            image_pricing: None,
            per_request: self.per_request,
            cached_input_per_1m_tokens: self.cached_input_per_1m_tokens,
            cache_write_per_1m_tokens: self.cache_write_per_1m_tokens,
            reasoning_per_1m_tokens: self.reasoning_per_1m_tokens,
            per_second: self.per_second,
            per_1m_characters: self.per_1m_characters,
        }
    }
}

/// Request to create model pricing
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateModelPricing {
    pub owner: PricingOwner,
    #[validate(length(min = 1, max = 64))]
    pub provider: String,
    #[validate(length(min = 1, max = 128))]
    pub model: String,
    /// Cost per 1M input tokens in microcents
    #[serde(default)]
    pub input_per_1m_tokens: i64,
    /// Cost per 1M output tokens in microcents
    #[serde(default)]
    pub output_per_1m_tokens: i64,
    /// Cost per image in microcents
    #[serde(default)]
    pub per_image: Option<i64>,
    /// Cost per request in microcents
    #[serde(default)]
    pub per_request: Option<i64>,
    /// Cost per 1M cached input tokens in microcents
    #[serde(default)]
    pub cached_input_per_1m_tokens: Option<i64>,
    /// Cost per 1M cache write tokens in microcents
    #[serde(default)]
    pub cache_write_per_1m_tokens: Option<i64>,
    /// Cost per 1M reasoning tokens in microcents
    #[serde(default)]
    pub reasoning_per_1m_tokens: Option<i64>,
    /// Cost per second of audio in microcents (for transcription/translation)
    #[serde(default)]
    pub per_second: Option<i64>,
    /// Cost per 1M characters in microcents (for TTS)
    #[serde(default)]
    pub per_1m_characters: Option<i64>,
    #[serde(default)]
    pub source: PricingSource,
}

/// Request to update model pricing
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateModelPricing {
    /// Cost per 1M input tokens in microcents
    pub input_per_1m_tokens: Option<i64>,
    /// Cost per 1M output tokens in microcents
    pub output_per_1m_tokens: Option<i64>,
    /// Cost per image in microcents
    pub per_image: Option<i64>,
    /// Cost per request in microcents
    pub per_request: Option<i64>,
    /// Cost per 1M cached input tokens in microcents
    pub cached_input_per_1m_tokens: Option<i64>,
    /// Cost per 1M cache write tokens in microcents
    pub cache_write_per_1m_tokens: Option<i64>,
    /// Cost per 1M reasoning tokens in microcents
    pub reasoning_per_1m_tokens: Option<i64>,
    /// Cost per second of audio in microcents (for transcription/translation)
    pub per_second: Option<i64>,
    /// Cost per 1M characters in microcents (for TTS)
    pub per_1m_characters: Option<i64>,
    pub source: Option<PricingSource>,
}
