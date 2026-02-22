//! Configuration module for the AI Gateway.
//!
//! The gateway is configured via a TOML file, with support for environment
//! variable interpolation using `${VAR_NAME}` syntax.
//!
//! # Example
//!
//! ```toml
//! [server]
//! host = "0.0.0.0"
//! port = 8080
//!
//! [database]
//! type = "postgres"
//! url = "postgres://user:${DB_PASSWORD}@localhost/gateway"
//! ```

mod auth;
mod cache;
mod database;
mod docs;
mod features;
mod limits;
mod observability;
mod providers;
mod retention;
mod secrets;
mod server;
mod storage;
mod ui;

use std::path::Path;

pub use auth::*;
pub use cache::*;
pub use database::*;
pub use docs::*;
pub use features::*;
pub use limits::*;
pub use observability::*;
pub use providers::*;
pub use retention::*;
pub use secrets::*;
use serde::{Deserialize, Serialize};
pub use server::*;
pub use storage::*;
pub use ui::*;

/// Root configuration for the AI Gateway.
///
/// This struct represents the complete configuration file. All sections
/// are optional with sensible defaults, allowing minimal configuration
/// for simple deployments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GatewayConfig {
    /// HTTP server configuration.
    #[serde(default)]
    pub server: ServerConfig,

    /// Database configuration for persistent storage.
    /// If omitted, the gateway runs in stateless mode (local dev only).
    #[serde(default)]
    pub database: DatabaseConfig,

    /// Cache configuration for rate limiting and session data.
    #[serde(default)]
    pub cache: CacheConfig,

    /// Authentication and authorization configuration.
    #[serde(default)]
    pub auth: AuthConfig,

    /// Static provider configurations.
    /// Additional providers can be added dynamically via the database
    /// at the org/project level.
    #[serde(default)]
    pub providers: ProvidersConfig,

    /// Default rate limits and budgets.
    /// These can be overridden at the org, project, and user levels.
    #[serde(default)]
    pub limits: LimitsConfig,

    /// Feature flags for optional capabilities.
    #[serde(default)]
    pub features: FeaturesConfig,

    /// Observability configuration (logging, tracing, metrics).
    #[serde(default)]
    pub observability: ObservabilityConfig,

    /// UI configuration.
    #[serde(default)]
    pub ui: UiConfig,

    /// Documentation site configuration.
    #[serde(default)]
    pub docs: DocsConfig,

    /// Pricing configuration for cost calculation.
    #[serde(default)]
    pub pricing: crate::pricing::PricingConfig,

    /// Secrets manager configuration for provider API keys.
    #[serde(default)]
    pub secrets: SecretsConfig,

    /// Data retention configuration for automatic purging of old data.
    #[serde(default)]
    pub retention: RetentionConfig,

    /// Storage configuration for files and binary data.
    #[serde(default)]
    pub storage: StorageConfig,
}

impl GatewayConfig {
    /// Load configuration from a TOML file.
    ///
    /// Environment variables in the format `${VAR_NAME}` are expanded.
    /// Missing required variables will cause an error.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ConfigError::Io(e, path.as_ref().to_path_buf()))?;

        Self::from_str(&contents)
    }

    /// Parse configuration from a TOML string.
    pub fn from_str(contents: &str) -> Result<Self, ConfigError> {
        // Expand environment variables
        let expanded = expand_env_vars(contents)?;

        // Pre-check: detect feature-gated config values before typed deserialization
        // to provide helpful error messages instead of cryptic serde "unknown variant" errors
        let raw: toml::Value = toml::from_str(&expanded).map_err(ConfigError::Parse)?;
        check_disabled_features(&raw)?;

        // Parse TOML
        let mut config: GatewayConfig = toml::from_str(&expanded).map_err(ConfigError::Parse)?;

        // Validate
        config.validate()?;

        Ok(config)
    }

    /// Validate the configuration for consistency and completeness.
    fn validate(&mut self) -> Result<(), ConfigError> {
        // If auth is enabled, we need a database
        if self.auth.gateway.is_enabled() && self.database.is_none() {
            return Err(ConfigError::Validation(
                "API authentication requires a database configuration".into(),
            ));
        }

        // Proxy auth without trusted_proxies is dangerous — anyone can spoof identity headers.
        if matches!(self.auth.admin, Some(AdminAuthConfig::ProxyAuth(_)))
            && !self.server.trusted_proxies.is_configured()
        {
            if !self.server.host.is_loopback() {
                return Err(ConfigError::Validation(
                    "Proxy auth (auth.admin.type = \"proxy_auth\") is enabled and the server \
                     binds to a non-localhost address, but server.trusted_proxies is not \
                     configured. This allows any client to spoof identity headers. Either \
                     configure server.trusted_proxies.cidrs with your proxy's IP ranges, \
                     or bind to localhost (server.host = \"127.0.0.1\")."
                        .into(),
                ));
            }
            tracing::warn!(
                "Proxy auth is enabled without server.trusted_proxies configured. \
                 Identity headers will be accepted from ANY source. This is safe only if \
                 the gateway is exclusively accessible through a trusted reverse proxy. \
                 Configure server.trusted_proxies.cidrs for production deployments."
            );
        }

        // Validate individual sections
        self.database.validate()?;
        self.cache.validate()?;
        self.auth.validate()?;
        self.providers.validate()?;
        self.storage
            .files
            .validate()
            .map_err(ConfigError::Validation)?;
        self.features.validate().map_err(ConfigError::Validation)?;

        Ok(())
    }

    /// Check if this is a minimal/local configuration (no auth, no database).
    pub fn is_local_mode(&self) -> bool {
        self.database.is_none() && !self.auth.gateway.is_enabled()
    }

    /// Generate the JSON schema for the gateway configuration.
    #[cfg(feature = "json-schema")]
    pub fn json_schema() -> schemars::schema::RootSchema {
        schemars::schema_for!(GatewayConfig)
    }

    /// Generate the JSON schema as a pretty-printed JSON string.
    #[cfg(feature = "json-schema")]
    pub fn json_schema_string() -> String {
        serde_json::to_string_pretty(&Self::json_schema())
            .expect("schema serialization should not fail")
    }
}

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file {1}: {0}")]
    Io(std::io::Error, std::path::PathBuf),

    #[error("Failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("Environment variable not found: {0}")]
    EnvVarNotFound(String),

    #[error("Configuration validation error: {0}")]
    Validation(String),
}

/// Check for feature-gated configuration values before typed deserialization.
///
/// When a user configures a provider or secrets backend that requires a cargo feature
/// not compiled into this binary, serde produces cryptic "unknown variant" errors.
/// This function inspects the raw TOML to detect such cases and produce actionable
/// error messages telling the user exactly which features to enable.
fn check_disabled_features(raw: &toml::Value) -> Result<(), ConfigError> {
    let mut issues: Vec<(String, &str)> = Vec::new();

    // Check provider types
    if let Some(providers) = raw.get("providers").and_then(|v| v.as_table()) {
        for (name, provider) in providers {
            if name == "default_provider" {
                continue;
            }
            if let Some(type_val) = provider.get("type").and_then(|v| v.as_str()) {
                check_provider_feature(name, type_val, &mut issues);
            }
        }
    }

    // Check database type
    if let Some(type_val) = raw
        .get("database")
        .and_then(|v| v.get("type"))
        .and_then(|v| v.as_str())
    {
        check_database_feature(type_val, &mut issues);
    }

    // Check secrets type
    if let Some(type_val) = raw
        .get("secrets")
        .and_then(|v| v.get("type"))
        .and_then(|v| v.as_str())
    {
        check_secrets_feature(type_val, &mut issues);
    }

    // Check cache type
    if let Some(type_val) = raw
        .get("cache")
        .and_then(|v| v.get("type"))
        .and_then(|v| v.as_str())
    {
        check_cache_feature(type_val, &mut issues);
    }

    // Check RBAC (requires CEL)
    if raw
        .get("auth")
        .and_then(|v| v.get("rbac"))
        .and_then(|v| v.get("enabled"))
        .and_then(|v| v.as_bool())
        == Some(true)
    {
        check_rbac_feature(&mut issues);
    }

    // Check metrics (requires Prometheus)
    if raw
        .get("observability")
        .and_then(|v| v.get("metrics"))
        .and_then(|v| v.get("enabled"))
        .and_then(|v| v.as_bool())
        == Some(true)
    {
        check_metrics_feature(&mut issues);
    }

    // Check OTLP tracing
    if raw
        .get("observability")
        .and_then(|v| v.get("tracing"))
        .and_then(|v| v.get("otlp"))
        .is_some()
    {
        check_otlp_feature(&mut issues);
    }

    if issues.is_empty() {
        return Ok(());
    }

    let details = issues
        .iter()
        .map(|(msg, _)| msg.as_str())
        .collect::<Vec<_>>()
        .join("\n  - ");
    let features = issues
        .iter()
        .map(|(_, feat)| *feat)
        .collect::<Vec<_>>()
        .join(",");

    Err(ConfigError::Validation(format!(
        "Configuration requires features not compiled in this build:\n  \
         - {details}\n\n\
         Rebuild with: cargo build --features {features}\n\
         Or use the 'full' profile: cargo build --features full\n\
         Run 'gateway features' to see all available features."
    )))
}

fn check_provider_feature(_name: &str, type_val: &str, _issues: &mut Vec<(String, &str)>) {
    match type_val {
        #[cfg(not(feature = "provider-bedrock"))]
        "bedrock" => _issues.push((
            format!(
                "provider '{_name}' uses type 'bedrock' which requires the 'provider-bedrock' feature"
            ),
            "provider-bedrock",
        )),
        #[cfg(not(feature = "provider-vertex"))]
        "vertex" => _issues.push((
            format!(
                "provider '{_name}' uses type 'vertex' which requires the 'provider-vertex' feature"
            ),
            "provider-vertex",
        )),
        #[cfg(not(feature = "provider-azure"))]
        "azure_openai" => _issues.push((
            format!(
                "provider '{_name}' uses type 'azure_openai' which requires the 'provider-azure' feature"
            ),
            "provider-azure",
        )),
        _ => {}
    }
}

fn check_database_feature(type_val: &str, _issues: &mut Vec<(String, &str)>) {
    match type_val {
        #[cfg(not(feature = "database-sqlite"))]
        "sqlite" => _issues.push((
            "database type 'sqlite' requires the 'database-sqlite' feature".into(),
            "database-sqlite",
        )),
        #[cfg(not(feature = "database-postgres"))]
        "postgres" => _issues.push((
            "database type 'postgres' requires the 'database-postgres' feature".into(),
            "database-postgres",
        )),
        _ => {}
    }
}

fn check_secrets_feature(type_val: &str, _issues: &mut Vec<(String, &str)>) {
    match type_val {
        #[cfg(not(feature = "vault"))]
        "vault" => _issues.push((
            "secrets type 'vault' requires the 'vault' feature".into(),
            "vault",
        )),
        #[cfg(not(feature = "secrets-aws"))]
        "aws" => _issues.push((
            "secrets type 'aws' requires the 'secrets-aws' feature".into(),
            "secrets-aws",
        )),
        #[cfg(not(feature = "secrets-azure"))]
        "azure" => _issues.push((
            "secrets type 'azure' requires the 'secrets-azure' feature".into(),
            "secrets-azure",
        )),
        #[cfg(not(feature = "secrets-gcp"))]
        "gcp" => _issues.push((
            "secrets type 'gcp' requires the 'secrets-gcp' feature".into(),
            "secrets-gcp",
        )),
        _ => {}
    }
}

fn check_cache_feature(type_val: &str, _issues: &mut Vec<(String, &str)>) {
    match type_val {
        #[cfg(not(feature = "redis"))]
        "redis" => _issues.push((
            "cache type 'redis' requires the 'redis' feature".into(),
            "redis",
        )),
        _ => {}
    }
}

fn check_rbac_feature(_issues: &mut Vec<(String, &str)>) {
    #[cfg(not(feature = "cel"))]
    _issues.push((
        "auth.rbac.enabled requires the 'cel' feature for CEL policy evaluation".into(),
        "cel",
    ));
}

fn check_metrics_feature(_issues: &mut Vec<(String, &str)>) {
    #[cfg(not(feature = "prometheus"))]
    _issues.push((
        "observability.metrics.enabled requires the 'prometheus' feature".into(),
        "prometheus",
    ));
}

fn check_otlp_feature(_issues: &mut Vec<(String, &str)>) {
    #[cfg(not(feature = "otlp"))]
    _issues.push((
        "observability.tracing.otlp requires the 'otlp' feature".into(),
        "otlp",
    ));
}

/// Expand environment variables in the format `${VAR_NAME}`.
/// Skips commented lines (lines where content before the variable is a comment).
fn expand_env_vars(input: &str) -> Result<String, ConfigError> {
    let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
    let mut result = String::with_capacity(input.len());

    for line in input.lines() {
        // Find if there's a comment on this line
        let comment_pos = line.find('#');

        // Process the line, only expanding variables that appear before any comment
        let mut line_result = String::with_capacity(line.len());
        let mut last_end = 0;

        for cap in re.captures_iter(line) {
            let match_start = cap.get(0).unwrap().start();

            // Skip if this variable is inside a comment
            if let Some(pos) = comment_pos
                && match_start >= pos
            {
                continue;
            }

            // Add text before this match
            line_result.push_str(&line[last_end..match_start]);

            // Expand the variable
            let var_name = &cap[1];
            let value = std::env::var(var_name)
                .map_err(|_| ConfigError::EnvVarNotFound(var_name.to_string()))?;
            line_result.push_str(&value);

            last_end = cap.get(0).unwrap().end();
        }

        // Add remaining text after last match
        line_result.push_str(&line[last_end..]);
        result.push_str(&line_result);
        result.push('\n');
    }

    // Remove trailing newline if input didn't have one
    if !input.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_config() {
        let config = GatewayConfig::from_str(
            r#"
            [providers.my-openai]
            type = "open_ai"
            api_key = "sk-test"
        "#,
        )
        .unwrap();

        assert!(config.is_local_mode());
        assert!(config.providers.get("my-openai").is_some());
    }

    #[test]
    fn test_multiple_providers_config() {
        let config = GatewayConfig::from_str(
            r#"
            [providers]
            default_provider = "openrouter"

            [providers.openrouter]
            type = "open_ai"
            api_key = "sk-or-xxx"
            base_url = "https://openrouter.ai/api/v1/"

            [providers.claude]
            type = "anthropic"
            api_key = "sk-ant-xxx"

            [providers.local]
            type = "open_ai"
            base_url = "http://localhost:11434/v1"
        "#,
        )
        .unwrap();

        assert_eq!(config.providers.providers.len(), 3);
    }

    #[test]
    fn test_env_var_expansion() {
        temp_env::with_var("TEST_API_KEY", Some("sk-secret"), || {
            let result = expand_env_vars("key = \"${TEST_API_KEY}\"").unwrap();
            assert_eq!(result, "key = \"sk-secret\"");
        });
    }

    #[test]
    fn test_env_var_in_comment_ignored() {
        // Variables in comments should not be expanded
        let result = expand_env_vars("# api_key = \"${NONEXISTENT_VAR}\"").unwrap();
        assert_eq!(result, "# api_key = \"${NONEXISTENT_VAR}\"");
    }

    #[test]
    fn test_env_var_after_comment_ignored() {
        // Variables after # on the same line should not be expanded
        let result = expand_env_vars("key = \"value\" # ${NONEXISTENT_VAR}").unwrap();
        assert_eq!(result, "key = \"value\" # ${NONEXISTENT_VAR}");
    }

    #[test]
    fn test_env_var_before_comment_expanded() {
        temp_env::with_var("TEST_BEFORE_COMMENT", Some("expanded"), || {
            let result =
                expand_env_vars("key = \"${TEST_BEFORE_COMMENT}\" # comment here").unwrap();
            assert_eq!(result, "key = \"expanded\" # comment here");
        });
    }

    #[test]
    fn test_multiline_with_comments() {
        temp_env::with_var("TEST_MULTI", Some("value1"), || {
            let input = r#"key1 = "${TEST_MULTI}"
# key2 = "${NONEXISTENT}"
key3 = "literal""#;
            let result = expand_env_vars(input).unwrap();
            assert_eq!(
                result,
                r#"key1 = "value1"
# key2 = "${NONEXISTENT}"
key3 = "literal""#
            );
        });
    }

    #[test]
    #[cfg(not(feature = "provider-bedrock"))]
    fn test_disabled_provider_bedrock_error() {
        let err = GatewayConfig::from_str(
            r#"
            [providers.my-bedrock]
            type = "bedrock"
            region = "us-east-1"
        "#,
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("provider-bedrock"),
            "should mention the required feature: {msg}"
        );
        assert!(
            msg.contains("my-bedrock"),
            "should mention the provider name: {msg}"
        );
        assert!(
            msg.contains("cargo build --features"),
            "should include rebuild instructions: {msg}"
        );
    }

    #[test]
    #[cfg(not(feature = "vault"))]
    fn test_disabled_secrets_vault_error() {
        let err = GatewayConfig::from_str(
            r#"
            [secrets]
            type = "vault"
            address = "https://vault.example.com:8200"
            auth = "token"
            token = "hvs.test"
        "#,
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("vault"),
            "should mention the required feature: {msg}"
        );
        assert!(
            msg.contains("cargo build --features"),
            "should include rebuild instructions: {msg}"
        );
    }

    #[test]
    #[cfg(not(feature = "provider-bedrock"))]
    fn test_disabled_multiple_features_error() {
        let err = GatewayConfig::from_str(
            r#"
            [providers.my-bedrock]
            type = "bedrock"
            region = "us-east-1"

            [providers.my-openai]
            type = "open_ai"
            api_key = "sk-test"
        "#,
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("provider-bedrock"),
            "should mention bedrock feature: {msg}"
        );
        // Verify enabled providers don't cause issues
        assert!(
            !msg.contains("open_ai"),
            "should not flag enabled providers: {msg}"
        );
    }

    #[test]
    #[cfg(not(feature = "database-sqlite"))]
    fn test_disabled_database_sqlite_error() {
        let err = GatewayConfig::from_str(
            r#"
            [database]
            type = "sqlite"
            url = "sqlite://hadrian.db"

            [providers.my-openai]
            type = "open_ai"
            api_key = "sk-test"
        "#,
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("database-sqlite"),
            "should mention the required feature: {msg}"
        );
        assert!(
            msg.contains("cargo build --features"),
            "should include rebuild instructions: {msg}"
        );
    }

    #[test]
    #[cfg(not(feature = "database-postgres"))]
    fn test_disabled_database_postgres_error() {
        let err = GatewayConfig::from_str(
            r#"
            [database]
            type = "postgres"
            url = "postgres://user:pass@localhost/gateway"

            [providers.my-openai]
            type = "open_ai"
            api_key = "sk-test"
        "#,
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("database-postgres"),
            "should mention the required feature: {msg}"
        );
        assert!(
            msg.contains("cargo build --features"),
            "should include rebuild instructions: {msg}"
        );
    }

    #[test]
    fn test_enabled_features_pass_check() {
        // Configs using only always-available types should pass the pre-check
        let raw: toml::Value = toml::from_str(
            r#"
            [providers.my-openai]
            type = "open_ai"
            api_key = "sk-test"

            [secrets]
            type = "env"
        "#,
        )
        .unwrap();

        assert!(
            check_disabled_features(&raw).is_ok(),
            "should pass for enabled features"
        );
    }

    #[test]
    fn test_proxy_auth_without_trusted_proxies_non_localhost_errors() {
        // Proxy auth on 0.0.0.0 without trusted_proxies should fail
        let err = GatewayConfig::from_str(
            r#"
            [server]
            host = "0.0.0.0"

            [auth.admin]
            type = "proxy_auth"
            identity_header = "X-Forwarded-User"

            [providers.my-openai]
            type = "open_ai"
            api_key = "sk-test"
        "#,
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("trusted_proxies"),
            "should mention trusted_proxies: {msg}"
        );
        assert!(
            msg.contains("proxy_auth") || msg.contains("Proxy auth"),
            "should mention proxy auth: {msg}"
        );
    }

    #[test]
    fn test_proxy_auth_without_trusted_proxies_localhost_warns_but_ok() {
        // Proxy auth on localhost without trusted_proxies should succeed (just warn)
        let result = GatewayConfig::from_str(
            r#"
            [server]
            host = "127.0.0.1"

            [auth.admin]
            type = "proxy_auth"
            identity_header = "X-Forwarded-User"

            [providers.my-openai]
            type = "open_ai"
            api_key = "sk-test"
        "#,
        );

        assert!(
            result.is_ok(),
            "proxy auth on localhost without trusted_proxies should be allowed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_proxy_auth_with_trusted_proxies_non_localhost_ok() {
        // Proxy auth on 0.0.0.0 with trusted_proxies configured should succeed
        let result = GatewayConfig::from_str(
            r#"
            [server]
            host = "0.0.0.0"

            [server.trusted_proxies]
            cidrs = ["10.0.0.0/8"]

            [auth.admin]
            type = "proxy_auth"
            identity_header = "X-Forwarded-User"

            [providers.my-openai]
            type = "open_ai"
            api_key = "sk-test"
        "#,
        );

        assert!(
            result.is_ok(),
            "proxy auth with trusted_proxies should be allowed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_proxy_auth_with_dangerously_trust_all_non_localhost_ok() {
        // Proxy auth with dangerously_trust_all should also pass validation
        let result = GatewayConfig::from_str(
            r#"
            [server]
            host = "0.0.0.0"

            [server.trusted_proxies]
            dangerously_trust_all = true

            [auth.admin]
            type = "proxy_auth"
            identity_header = "X-Forwarded-User"

            [providers.my-openai]
            type = "open_ai"
            api_key = "sk-test"
        "#,
        );

        assert!(
            result.is_ok(),
            "proxy auth with dangerously_trust_all should be allowed: {:?}",
            result.err()
        );
    }
}
