//! Model catalog module for enriching API responses with model metadata.
//!
//! This module provides access to the models.dev model catalog, which contains
//! per-model metadata including capabilities, pricing, context limits, and modalities.
//!
//! The catalog is optionally embedded at build time as a fallback (via the `embed-catalog`
//! feature) and can be synced at runtime via a background job.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::catalog::ModelCatalogRegistry;
//!
//! let registry = ModelCatalogRegistry::new();
//! if let Some(json) = embedded_catalog() {
//!     registry.load_from_json(&json)?;
//! }
//!
//! // Look up model metadata
//! if let Some(enrichment) = registry.lookup("anthropic", "claude-opus-4-5") {
//!     println!("Vision support: {}", enrichment.capabilities.vision);
//!     println!("Context length: {:?}", enrichment.limits.context_length);
//! }
//! ```

mod registry;
mod types;

pub use registry::{
    ModelCapabilities, ModelCatalogRegistry, ModelModalities, resolve_catalog_provider_id,
};

/// Embedded catalog assets from the data/ directory.
/// Gated behind the `embed-catalog` feature and uses `allow_missing` so the
/// build succeeds even when `data/models-dev-catalog.json` is absent.
#[cfg(feature = "embed-catalog")]
#[derive(rust_embed::Embed)]
#[folder = "data"]
#[include = "models-dev-catalog.json"]
#[allow_missing = true]
struct CatalogAssets;

/// Returns the embedded model catalog JSON, if the `embed-catalog` feature is
/// enabled and the catalog file was present at build time.
pub fn embedded_catalog() -> Option<String> {
    #[cfg(feature = "embed-catalog")]
    {
        CatalogAssets::get("models-dev-catalog.json")
            .map(|file| String::from_utf8_lossy(&file.data).into_owned())
    }
    #[cfg(not(feature = "embed-catalog"))]
    {
        None
    }
}

#[cfg(all(test, feature = "embed-catalog"))]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_catalog_parses() {
        let json = embedded_catalog().expect("Embedded catalog should be present");
        let catalog: types::ModelCatalog =
            serde_json::from_str(&json).expect("Embedded catalog should be valid JSON");

        // Should have multiple providers
        assert!(
            catalog.len() > 10,
            "Expected many providers, got {}",
            catalog.len()
        );

        // Verify some known providers exist
        assert!(
            catalog.contains_key("anthropic"),
            "Should have anthropic provider"
        );
        assert!(
            catalog.contains_key("openai"),
            "Should have openai provider"
        );

        // Verify anthropic has some models
        let anthropic = catalog.get("anthropic").unwrap();
        assert!(!anthropic.models.is_empty(), "Anthropic should have models");
    }

    #[test]
    fn test_load_embedded_catalog_into_registry() {
        let json = embedded_catalog().expect("Embedded catalog should be present");
        let registry = ModelCatalogRegistry::new();
        registry
            .load_from_json(&json)
            .expect("Should load embedded catalog");

        // Should have many models
        assert!(
            registry.model_count() > 100,
            "Expected many models, got {}",
            registry.model_count()
        );

        // Verify we can look up a known model
        let enrichment = registry
            .lookup("anthropic", "claude-opus-4-5")
            .expect("Should find claude-opus-4-5");
        assert!(
            enrichment.capabilities.vision,
            "Claude Opus 4.5 should support vision"
        );
        assert!(
            enrichment.capabilities.reasoning,
            "Claude Opus 4.5 should support reasoning"
        );
        assert!(
            enrichment.capabilities.tool_call,
            "Claude Opus 4.5 should support tool calls"
        );
    }
}
