use serde::{Deserialize, Serialize};

use super::AssetSource;

/// Documentation site configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DocsConfig {
    /// Enable the documentation site.
    #[serde(default)]
    pub enabled: bool,

    /// Path to serve the documentation from (default: /docs).
    #[serde(default = "default_docs_path")]
    pub path: String,

    /// Static assets configuration.
    #[serde(default)]
    pub assets: DocsAssetsConfig,
}

impl Default for DocsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: default_docs_path(),
            assets: DocsAssetsConfig::default(),
        }
    }
}

fn default_docs_path() -> String {
    "/docs".to_string()
}

/// Documentation static assets configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DocsAssetsConfig {
    /// Source of static assets.
    #[serde(default)]
    pub source: AssetSource,

    /// Cache control header for static assets.
    #[serde(default = "default_cache_control")]
    pub cache_control: String,
}

impl Default for DocsAssetsConfig {
    fn default() -> Self {
        Self {
            source: AssetSource::default(),
            cache_control: default_cache_control(),
        }
    }
}

fn default_cache_control() -> String {
    "public, max-age=3600".to_string()
}
