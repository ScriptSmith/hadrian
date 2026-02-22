//! Type definitions for the models.dev model catalog.
//!
//! The catalog provides per-model metadata including capabilities, pricing,
//! context limits, and modalities from <https://models.dev>.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The complete model catalog, mapping provider IDs to provider definitions.
pub type ModelCatalog = HashMap<String, CatalogProvider>;

/// A provider in the catalog with its models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogProvider {
    /// Provider identifier (e.g., "anthropic", "openai")
    pub id: String,

    /// Human-readable provider name
    pub name: String,

    /// API base URL
    #[serde(default)]
    pub api: Option<String>,

    /// Documentation URL
    #[serde(default)]
    pub doc: Option<String>,

    /// Environment variables for authentication
    #[serde(default)]
    pub env: Vec<String>,

    /// Models provided by this provider
    #[serde(default)]
    pub models: HashMap<String, CatalogModel>,
}

/// A model in the catalog with its capabilities and pricing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogModel {
    /// Model identifier (e.g., "claude-opus-4-5")
    pub id: String,

    /// Human-readable model name
    pub name: String,

    /// Model family (e.g., "claude-opus", "gpt-4")
    #[serde(default)]
    pub family: Option<String>,

    /// Whether the model supports image/file attachments (vision)
    #[serde(default)]
    pub attachment: bool,

    /// Whether the model supports reasoning/thinking mode
    #[serde(default)]
    pub reasoning: bool,

    /// Whether the model supports tool/function calling
    #[serde(default)]
    pub tool_call: bool,

    /// Whether the model supports structured output (JSON mode)
    #[serde(default)]
    pub structured_output: bool,

    /// Whether the model supports temperature control
    #[serde(default)]
    pub temperature: bool,

    /// Whether the model has open weights
    #[serde(default)]
    pub open_weights: bool,

    /// Knowledge cutoff date (YYYY-MM format)
    #[serde(default)]
    pub knowledge: Option<String>,

    /// Model release date (YYYY-MM-DD format)
    #[serde(default)]
    pub release_date: Option<String>,

    /// Last updated date (YYYY-MM-DD format)
    #[serde(default)]
    pub last_updated: Option<String>,

    /// Input/output modalities
    #[serde(default)]
    pub modalities: CatalogModalities,

    /// Pricing information (dollars per 1M tokens)
    #[serde(default)]
    pub cost: CatalogCost,

    /// Context and output limits
    #[serde(default)]
    pub limit: CatalogLimit,

    /// Interleaved field configuration (for thinking models)
    /// Can be a boolean or an object with a `field` property.
    #[serde(default, deserialize_with = "deserialize_interleaved")]
    pub interleaved: Option<CatalogInterleaved>,
}

/// Interleaved reasoning configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogInterleaved {
    /// Whether interleaved mode is enabled
    #[serde(default)]
    pub enabled: bool,

    /// The field name containing reasoning content
    #[serde(default)]
    pub field: Option<String>,
}

/// Custom deserializer that accepts both boolean and object forms of interleaved.
fn deserialize_interleaved<'de, D>(deserializer: D) -> Result<Option<CatalogInterleaved>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct InterleavedVisitor;

    impl<'de> Visitor<'de> for InterleavedVisitor {
        type Value = Option<CatalogInterleaved>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a boolean or an object with optional field property")
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(CatalogInterleaved {
                enabled: v,
                field: None,
            }))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
        where
            A: de::MapAccess<'de>,
        {
            #[derive(Deserialize)]
            struct InterleavedObj {
                #[serde(default)]
                field: Option<String>,
            }

            let obj: InterleavedObj =
                de::Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))?;
            Ok(Some(CatalogInterleaved {
                enabled: true,
                field: obj.field,
            }))
        }
    }

    deserializer.deserialize_any(InterleavedVisitor)
}

/// Input/output modalities for a model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogModalities {
    /// Supported input modalities (e.g., "text", "image", "audio", "video", "pdf")
    #[serde(default)]
    pub input: Vec<String>,

    /// Supported output modalities (e.g., "text", "audio")
    #[serde(default)]
    pub output: Vec<String>,
}

/// Pricing information in dollars per 1M tokens.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogCost {
    /// Input token cost ($/1M tokens)
    #[serde(default)]
    pub input: f64,

    /// Output token cost ($/1M tokens)
    #[serde(default)]
    pub output: f64,

    /// Reasoning token cost ($/1M tokens)
    #[serde(default)]
    pub reasoning: Option<f64>,

    /// Cache read cost ($/1M tokens)
    #[serde(default)]
    pub cache_read: Option<f64>,

    /// Cache write cost ($/1M tokens)
    #[serde(default)]
    pub cache_write: Option<f64>,

    /// Audio input cost ($/1M tokens)
    #[serde(default)]
    pub input_audio: Option<f64>,

    /// Audio output cost ($/1M tokens)
    #[serde(default)]
    pub output_audio: Option<f64>,
}

/// Context and output limits for a model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogLimit {
    /// Maximum context window size (tokens)
    #[serde(default)]
    pub context: i64,

    /// Maximum output tokens
    #[serde(default)]
    pub output: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_catalog_provider() {
        let json = r#"{
            "id": "anthropic",
            "name": "Anthropic",
            "api": "https://api.anthropic.com/v1",
            "doc": "https://docs.anthropic.com",
            "env": ["ANTHROPIC_API_KEY"],
            "models": {}
        }"#;

        let provider: CatalogProvider = serde_json::from_str(json).unwrap();
        assert_eq!(provider.id, "anthropic");
        assert_eq!(provider.name, "Anthropic");
        assert_eq!(provider.env, vec!["ANTHROPIC_API_KEY"]);
    }

    #[test]
    fn test_parse_catalog_model() {
        let json = r#"{
            "id": "claude-opus-4-5",
            "name": "Claude Opus 4.5",
            "family": "claude-opus",
            "attachment": true,
            "reasoning": true,
            "tool_call": true,
            "structured_output": false,
            "temperature": true,
            "open_weights": false,
            "knowledge": "2025-03-31",
            "release_date": "2025-11-24",
            "modalities": {
                "input": ["text", "image", "pdf"],
                "output": ["text"]
            },
            "cost": {
                "input": 5.0,
                "output": 25.0,
                "cache_read": 0.5,
                "cache_write": 6.25
            },
            "limit": {
                "context": 200000,
                "output": 64000
            }
        }"#;

        let model: CatalogModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "claude-opus-4-5");
        assert_eq!(model.name, "Claude Opus 4.5");
        assert_eq!(model.family, Some("claude-opus".to_string()));
        assert!(model.attachment);
        assert!(model.reasoning);
        assert!(model.tool_call);
        assert!(!model.structured_output);
        assert!(!model.open_weights);
        assert_eq!(model.cost.input, 5.0);
        assert_eq!(model.cost.output, 25.0);
        assert_eq!(model.cost.cache_read, Some(0.5));
        assert_eq!(model.limit.context, 200000);
        assert_eq!(model.limit.output, 64000);
        assert_eq!(model.modalities.input, vec!["text", "image", "pdf"]);
        assert_eq!(model.modalities.output, vec!["text"]);
    }

    #[test]
    fn test_parse_model_with_missing_fields() {
        // Models may have missing optional fields
        let json = r#"{
            "id": "test-model",
            "name": "Test Model"
        }"#;

        let model: CatalogModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "test-model");
        assert!(!model.attachment);
        assert!(!model.reasoning);
        assert!(model.family.is_none());
        assert_eq!(model.cost.input, 0.0);
        assert_eq!(model.limit.context, 0);
    }
}
