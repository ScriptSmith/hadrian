//! Model catalog module for enriching API responses with model metadata.
//!
//! This module provides access to the models.dev model catalog, which contains
//! per-model metadata including capabilities, pricing, context limits, and modalities.
//!
//! The catalog is embedded at build time as a fallback and optionally synced at
//! runtime via a background job.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::catalog::ModelCatalogRegistry;
//!
//! let registry = ModelCatalogRegistry::new();
//! registry.load_from_json(EMBEDDED_CATALOG)?;
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

/// The embedded model catalog from models.dev.
///
/// This is loaded at compile time and serves as a fallback when runtime sync is
/// disabled or unavailable. The catalog is fetched via `scripts/fetch-model-catalog.sh`.
pub const EMBEDDED_CATALOG: &str = include_str!("../../data/models-dev-catalog.json");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_catalog_parses() {
        let catalog: types::ModelCatalog =
            serde_json::from_str(EMBEDDED_CATALOG).expect("Embedded catalog should be valid JSON");

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
        let registry = ModelCatalogRegistry::new();
        registry
            .load_from_json(EMBEDDED_CATALOG)
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
