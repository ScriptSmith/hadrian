//! Built-in blocklist provider for guardrails.
//!
//! This provider uses local pattern matching (exact keywords or regex) to detect
//! content violations without requiring external API calls.
//!
//! # Example Configuration
//!
//! ```toml
//! [features.guardrails.input.provider]
//! type = "blocklist"
//! case_insensitive = true
//!
//! [[features.guardrails.input.provider.patterns]]
//! pattern = "(?i)\\b(password|secret|api.?key)\\b"
//! is_regex = true
//! category = "confidential"
//! severity = "high"
//! message = "Potential secret or credential detected"
//!
//! [[features.guardrails.input.provider.patterns]]
//! pattern = "competitor_name"
//! category = "competitor_mention"
//! severity = "medium"
//! ```

use std::time::Instant;

use async_trait::async_trait;
use regex::Regex;
use tracing::instrument;
use unicode_normalization::UnicodeNormalization;

use super::{
    Category, GuardrailsError, GuardrailsProvider, GuardrailsRequest, GuardrailsResponse,
    GuardrailsResult, Severity, Violation,
};
use crate::config::BlocklistPattern;

/// A compiled pattern for matching content.
#[derive(Debug)]
struct CompiledPattern {
    /// The compiled regex (works for both literal strings and regex patterns).
    regex: Regex,
    /// Category to assign on match.
    category: Category,
    /// Severity level.
    severity: Severity,
    /// Human-readable message.
    message: Option<String>,
    /// Original pattern string for error messages.
    original_pattern: String,
}

/// Built-in blocklist guardrails provider.
///
/// Evaluates content against a list of patterns (keywords or regex) locally,
/// without making any external API calls. Patterns are pre-compiled at startup
/// for optimal performance.
pub struct BlocklistProvider {
    /// Pre-compiled patterns for matching.
    patterns: Vec<CompiledPattern>,
}

impl BlocklistProvider {
    /// Creates a new blocklist provider from configuration.
    ///
    /// # Arguments
    /// * `patterns` - List of pattern configurations
    /// * `case_insensitive` - Whether to match case-insensitively
    ///
    /// # Errors
    /// Returns an error if any regex pattern fails to compile.
    pub fn new(patterns: Vec<BlocklistPattern>, case_insensitive: bool) -> GuardrailsResult<Self> {
        let compiled = patterns
            .into_iter()
            .map(|p| Self::compile_pattern(p, case_insensitive))
            .collect::<GuardrailsResult<Vec<_>>>()?;

        Ok(Self { patterns: compiled })
    }

    /// Compiles a single pattern configuration into a CompiledPattern.
    fn compile_pattern(
        config: BlocklistPattern,
        case_insensitive: bool,
    ) -> GuardrailsResult<CompiledPattern> {
        let pattern_str = if config.is_regex {
            // Use the pattern as-is if it's already a regex
            if case_insensitive && !config.pattern.starts_with("(?i)") {
                format!("(?i){}", config.pattern)
            } else {
                config.pattern.clone()
            }
        } else {
            // Escape the pattern for literal matching
            let escaped = regex::escape(&config.pattern);
            if case_insensitive {
                format!("(?i){}", escaped)
            } else {
                escaped
            }
        };

        let regex = Regex::new(&pattern_str).map_err(|e| {
            GuardrailsError::config_error(format!(
                "Invalid blocklist pattern '{}': {}",
                config.pattern, e
            ))
        })?;

        let category = Category::from(config.category.as_str());
        let severity = parse_severity(&config.severity);

        Ok(CompiledPattern {
            regex,
            category,
            severity,
            message: config.message,
            original_pattern: config.pattern,
        })
    }

    /// Finds all matches for a pattern in the content.
    fn find_matches<'a>(
        &'a self,
        content: &'a str,
    ) -> impl Iterator<Item = (&'a CompiledPattern, regex::Match<'a>)> {
        self.patterns
            .iter()
            .flat_map(move |pattern| pattern.regex.find_iter(content).map(move |m| (pattern, m)))
    }
}

#[async_trait]
impl GuardrailsProvider for BlocklistProvider {
    fn name(&self) -> &str {
        "blocklist"
    }

    #[instrument(
        skip(self, request),
        fields(
            provider = "blocklist",
            pattern_count = self.patterns.len(),
            text_length = request.text.len()
        )
    )]
    async fn evaluate(&self, request: &GuardrailsRequest) -> GuardrailsResult<GuardrailsResponse> {
        let start = Instant::now();

        // Apply NFKC normalization to defeat Unicode confusable bypasses.
        // NFKC maps visually similar characters (e.g., fullwidth, accented) to their
        // canonical forms, so "ｐａｓｓｗｏｒｄ" becomes "password".
        let normalized: String = request.text.nfkc().collect();

        let violations: Vec<Violation> = self
            .find_matches(&normalized)
            .map(|(pattern, m)| {
                let message = pattern.message.clone().unwrap_or_else(|| {
                    format!(
                        "Content matched blocked pattern '{}'",
                        pattern.original_pattern
                    )
                });

                Violation::new(pattern.category.clone(), pattern.severity, 1.0)
                    .with_message(message)
                    .with_span(m.start(), m.end())
            })
            .collect();

        let latency_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            violation_count = violations.len(),
            latency_ms = latency_ms,
            "Blocklist evaluation complete"
        );

        Ok(GuardrailsResponse::with_violations(violations).with_latency(latency_ms))
    }

    fn supported_categories(&self) -> &[Category] {
        // Blocklist can support any category configured by the user
        Category::all_standard()
    }
}

/// Parses a severity string into a Severity enum.
fn parse_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "info" => Severity::Info,
        "low" => Severity::Low,
        "medium" => Severity::Medium,
        "high" => Severity::High,
        "critical" => Severity::Critical,
        _ => Severity::High, // Default to high for unknown severities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pattern(
        pattern: &str,
        is_regex: bool,
        category: &str,
        severity: &str,
    ) -> BlocklistPattern {
        BlocklistPattern {
            pattern: pattern.to_string(),
            is_regex,
            category: category.to_string(),
            severity: severity.to_string(),
            message: None,
        }
    }

    #[test]
    fn test_literal_pattern_matching() {
        let patterns = vec![make_pattern("password", false, "confidential", "high")];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        // Test matching
        let matches: Vec<_> = provider.find_matches("my password is secret").collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1.as_str(), "password");
    }

    #[test]
    fn test_case_insensitive_matching() {
        let patterns = vec![make_pattern("SECRET", false, "confidential", "high")];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        let matches: Vec<_> = provider.find_matches("this is a secret message").collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1.as_str(), "secret");
    }

    #[test]
    fn test_case_sensitive_matching() {
        let patterns = vec![make_pattern("SECRET", false, "confidential", "high")];
        let provider = BlocklistProvider::new(patterns, false).unwrap();

        // Should not match lowercase
        let matches: Vec<_> = provider.find_matches("this is a secret message").collect();
        assert_eq!(matches.len(), 0);

        // Should match exact case
        let matches: Vec<_> = provider.find_matches("this is a SECRET message").collect();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_regex_pattern_matching() {
        let patterns = vec![make_pattern(
            r"\b(password|secret|api.?key)\b",
            true,
            "confidential",
            "high",
        )];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        let matches: Vec<_> = provider.find_matches("my api_key is here").collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1.as_str(), "api_key");

        let matches: Vec<_> = provider.find_matches("password and secret").collect();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_multiple_patterns() {
        let patterns = vec![
            make_pattern("password", false, "confidential", "high"),
            make_pattern("secret", false, "confidential", "medium"),
        ];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        let matches: Vec<_> = provider.find_matches("my password is secret").collect();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_no_matches() {
        let patterns = vec![make_pattern("password", false, "confidential", "high")];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        let matches: Vec<_> = provider.find_matches("nothing to see here").collect();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_special_regex_chars_escaped_for_literal() {
        // Characters like . * + should be escaped when is_regex=false
        let patterns = vec![make_pattern("a.b", false, "test", "high")];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        // Should match literal "a.b" only
        let matches: Vec<_> = provider.find_matches("a.b").collect();
        assert_eq!(matches.len(), 1);

        // Should NOT match "aXb" (the dot is escaped)
        let matches: Vec<_> = provider.find_matches("aXb").collect();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_invalid_regex_returns_error() {
        let patterns = vec![make_pattern("[invalid", true, "test", "high")];
        let result = BlocklistProvider::new(patterns, true);

        assert!(result.is_err());
        match result {
            Err(GuardrailsError::ConfigError { message }) => {
                assert!(message.contains("Invalid blocklist pattern"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[tokio::test]
    async fn test_evaluate_returns_violations() {
        let patterns = vec![BlocklistPattern {
            pattern: "password".to_string(),
            is_regex: false,
            category: "confidential".to_string(),
            severity: "high".to_string(),
            message: Some("Password detected".to_string()),
        }];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        let request = GuardrailsRequest::user_input("my password is secret123");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(!response.passed);
        assert_eq!(response.violations.len(), 1);
        assert_eq!(response.violations[0].category, Category::Confidential);
        assert_eq!(response.violations[0].severity, Severity::High);
        assert_eq!(
            response.violations[0].message,
            Some("Password detected".to_string())
        );
        assert!(response.violations[0].span.is_some());
        let span = response.violations[0].span.unwrap();
        assert_eq!(span.start, 3); // "my " is 3 chars
        assert_eq!(span.end, 11); // "password" is 8 chars, so 3+8=11
    }

    #[tokio::test]
    async fn test_evaluate_no_violations() {
        let patterns = vec![make_pattern("password", false, "confidential", "high")];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        let request = GuardrailsRequest::user_input("nothing sensitive here");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(response.passed);
        assert!(response.violations.is_empty());
    }

    #[test]
    fn test_parse_severity() {
        assert_eq!(parse_severity("info"), Severity::Info);
        assert_eq!(parse_severity("low"), Severity::Low);
        assert_eq!(parse_severity("medium"), Severity::Medium);
        assert_eq!(parse_severity("high"), Severity::High);
        assert_eq!(parse_severity("critical"), Severity::Critical);
        assert_eq!(parse_severity("INFO"), Severity::Info); // case insensitive
        assert_eq!(parse_severity("unknown"), Severity::High); // default
    }

    #[test]
    fn test_provider_name() {
        let provider = BlocklistProvider::new(vec![], true).unwrap();
        assert_eq!(provider.name(), "blocklist");
    }

    #[test]
    fn test_category_mapping() {
        let patterns = vec![
            make_pattern("hate", false, "hate", "high"),
            make_pattern("pii", false, "pii_email", "medium"),
            make_pattern("custom", false, "my_custom_category", "low"),
        ];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        let matches: Vec<_> = provider.find_matches("hate pii custom").collect();
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].0.category, Category::Hate);
        assert_eq!(matches[1].0.category, Category::PiiEmail);
        assert_eq!(
            matches[2].0.category,
            Category::Custom("my_custom_category".to_string())
        );
    }

    #[tokio::test]
    async fn test_multiple_matches_same_pattern() {
        let patterns = vec![make_pattern("test", false, "test_category", "medium")];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        let request = GuardrailsRequest::user_input("test one test two test three");
        let response = provider.evaluate(&request).await.unwrap();

        assert_eq!(response.violations.len(), 3);
    }

    #[test]
    fn test_empty_patterns() {
        let provider = BlocklistProvider::new(vec![], true).unwrap();
        assert_eq!(provider.patterns.len(), 0);
    }

    #[test]
    fn test_regex_with_existing_case_insensitive_flag() {
        // If pattern already has (?i), don't add another
        let patterns = vec![make_pattern("(?i)already", true, "test", "high")];
        let provider = BlocklistProvider::new(patterns, true).unwrap();

        let matches: Vec<_> = provider.find_matches("ALREADY here").collect();
        assert_eq!(matches.len(), 1);
    }
}
