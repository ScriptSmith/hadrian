//! Built-in content limits provider for guardrails.
//!
//! This provider enforces size constraints on content without requiring
//! external API calls.
//!
//! # Example Configuration
//!
//! ```toml
//! [features.guardrails.input.provider]
//! type = "content_limits"
//! max_characters = 10000
//! max_words = 2000
//! max_lines = 100
//! ```

use std::time::Instant;

use async_trait::async_trait;
use tracing::instrument;

use super::{
    Category, GuardrailsProvider, GuardrailsRequest, GuardrailsResponse, GuardrailsResult,
    Severity, Violation,
};

/// Configuration for content limits.
#[derive(Debug, Clone, Default)]
pub struct ContentLimitsConfig {
    /// Maximum number of characters allowed.
    pub max_characters: Option<usize>,
    /// Maximum number of words allowed.
    pub max_words: Option<usize>,
    /// Maximum number of lines allowed.
    pub max_lines: Option<usize>,
}

/// Built-in content limits guardrails provider.
///
/// Enforces size constraints on content locally, without making any external
/// API calls.
pub struct ContentLimitsProvider {
    config: ContentLimitsConfig,
}

impl ContentLimitsProvider {
    /// Creates a new content limits provider with the specified configuration.
    pub fn new(config: ContentLimitsConfig) -> Self {
        Self { config }
    }

    /// Counts the number of words in the text.
    fn count_words(text: &str) -> usize {
        text.split_whitespace().count()
    }

    /// Counts the number of lines in the text.
    fn count_lines(text: &str) -> usize {
        if text.is_empty() {
            0
        } else {
            text.lines().count()
        }
    }
}

#[async_trait]
impl GuardrailsProvider for ContentLimitsProvider {
    fn name(&self) -> &str {
        "content_limits"
    }

    #[instrument(
        skip(self, request),
        fields(
            provider = "content_limits",
            text_length = request.text.len()
        )
    )]
    async fn evaluate(&self, request: &GuardrailsRequest) -> GuardrailsResult<GuardrailsResponse> {
        let start = Instant::now();
        let mut violations = Vec::new();

        let char_count = request.text.chars().count();
        if let Some(max_chars) = self.config.max_characters
            && char_count > max_chars
        {
            violations.push(
                Violation::new(
                    Category::Custom("content_limit_characters".to_string()),
                    Severity::High,
                    1.0,
                )
                .with_message(format!(
                    "Content exceeds character limit: {} characters (max: {})",
                    char_count, max_chars
                )),
            );
        }

        if let Some(max_words) = self.config.max_words {
            let word_count = Self::count_words(&request.text);
            if word_count > max_words {
                violations.push(
                    Violation::new(
                        Category::Custom("content_limit_words".to_string()),
                        Severity::High,
                        1.0,
                    )
                    .with_message(format!(
                        "Content exceeds word limit: {} words (max: {})",
                        word_count, max_words
                    )),
                );
            }
        }

        if let Some(max_lines) = self.config.max_lines {
            let line_count = Self::count_lines(&request.text);
            if line_count > max_lines {
                violations.push(
                    Violation::new(
                        Category::Custom("content_limit_lines".to_string()),
                        Severity::High,
                        1.0,
                    )
                    .with_message(format!(
                        "Content exceeds line limit: {} lines (max: {})",
                        line_count, max_lines
                    )),
                );
            }
        }

        let latency_ms = start.elapsed().as_millis() as u64;

        tracing::debug!(
            violation_count = violations.len(),
            latency_ms = latency_ms,
            "Content limits evaluation complete"
        );

        Ok(GuardrailsResponse::with_violations(violations).with_latency(latency_ms))
    }

    fn supported_categories(&self) -> &[Category] {
        // Content limits uses custom categories
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_words() {
        assert_eq!(ContentLimitsProvider::count_words(""), 0);
        assert_eq!(ContentLimitsProvider::count_words("hello"), 1);
        assert_eq!(ContentLimitsProvider::count_words("hello world"), 2);
        assert_eq!(
            ContentLimitsProvider::count_words("  multiple   spaces   here  "),
            3
        );
        assert_eq!(ContentLimitsProvider::count_words("one\ntwo\nthree"), 3);
    }

    #[test]
    fn test_count_lines() {
        assert_eq!(ContentLimitsProvider::count_lines(""), 0);
        assert_eq!(ContentLimitsProvider::count_lines("single line"), 1);
        assert_eq!(ContentLimitsProvider::count_lines("line1\nline2"), 2);
        assert_eq!(ContentLimitsProvider::count_lines("line1\nline2\nline3"), 3);
        assert_eq!(ContentLimitsProvider::count_lines("line1\nline2\n"), 2); // trailing newline
    }

    #[tokio::test]
    async fn test_no_limits_configured() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig::default());
        let request = GuardrailsRequest::user_input("any content here");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(response.passed);
        assert!(response.violations.is_empty());
    }

    #[tokio::test]
    async fn test_character_limit_not_exceeded() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig {
            max_characters: Some(100),
            ..Default::default()
        });
        let request = GuardrailsRequest::user_input("short text");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(response.passed);
        assert!(response.violations.is_empty());
    }

    #[tokio::test]
    async fn test_character_limit_exceeded() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig {
            max_characters: Some(10),
            ..Default::default()
        });
        let request = GuardrailsRequest::user_input("this text is longer than ten characters");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(!response.passed);
        assert_eq!(response.violations.len(), 1);
        assert_eq!(
            response.violations[0].category,
            Category::Custom("content_limit_characters".to_string())
        );
        assert!(
            response.violations[0]
                .message
                .as_ref()
                .unwrap()
                .contains("character limit")
        );
    }

    #[tokio::test]
    async fn test_word_limit_not_exceeded() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig {
            max_words: Some(10),
            ..Default::default()
        });
        let request = GuardrailsRequest::user_input("one two three");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(response.passed);
        assert!(response.violations.is_empty());
    }

    #[tokio::test]
    async fn test_word_limit_exceeded() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig {
            max_words: Some(3),
            ..Default::default()
        });
        let request = GuardrailsRequest::user_input("one two three four five");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(!response.passed);
        assert_eq!(response.violations.len(), 1);
        assert_eq!(
            response.violations[0].category,
            Category::Custom("content_limit_words".to_string())
        );
        assert!(
            response.violations[0]
                .message
                .as_ref()
                .unwrap()
                .contains("word limit")
        );
    }

    #[tokio::test]
    async fn test_line_limit_not_exceeded() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig {
            max_lines: Some(5),
            ..Default::default()
        });
        let request = GuardrailsRequest::user_input("line1\nline2\nline3");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(response.passed);
        assert!(response.violations.is_empty());
    }

    #[tokio::test]
    async fn test_line_limit_exceeded() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig {
            max_lines: Some(2),
            ..Default::default()
        });
        let request = GuardrailsRequest::user_input("line1\nline2\nline3\nline4");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(!response.passed);
        assert_eq!(response.violations.len(), 1);
        assert_eq!(
            response.violations[0].category,
            Category::Custom("content_limit_lines".to_string())
        );
        assert!(
            response.violations[0]
                .message
                .as_ref()
                .unwrap()
                .contains("line limit")
        );
    }

    #[tokio::test]
    async fn test_multiple_limits_exceeded() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig {
            max_characters: Some(10),
            max_words: Some(2),
            max_lines: Some(1),
        });
        let request = GuardrailsRequest::user_input("this is a long text\nwith multiple lines");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(!response.passed);
        assert_eq!(response.violations.len(), 3);
    }

    #[tokio::test]
    async fn test_exact_limit_not_exceeded() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig {
            max_characters: Some(5),
            max_words: Some(1),
            max_lines: Some(1),
        });
        let request = GuardrailsRequest::user_input("hello");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(response.passed);
        assert!(response.violations.is_empty());
    }

    #[tokio::test]
    async fn test_unicode_characters() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig {
            max_characters: Some(5),
            ..Default::default()
        });
        // "héllo" is 5 characters
        let request = GuardrailsRequest::user_input("héllo");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(response.passed);

        // "héllo!" is 6 characters
        let request = GuardrailsRequest::user_input("héllo!");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(!response.passed);
    }

    #[tokio::test]
    async fn test_empty_content() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig {
            max_characters: Some(10),
            max_words: Some(5),
            max_lines: Some(3),
        });
        let request = GuardrailsRequest::user_input("");
        let response = provider.evaluate(&request).await.unwrap();

        assert!(response.passed);
        assert!(response.violations.is_empty());
    }

    #[test]
    fn test_provider_name() {
        let provider = ContentLimitsProvider::new(ContentLimitsConfig::default());
        assert_eq!(provider.name(), "content_limits");
    }
}
