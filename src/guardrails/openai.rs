//! OpenAI Moderation API provider for guardrails.
//!
//! This provider uses OpenAI's free moderation endpoint to detect harmful content
//! across multiple categories including hate, harassment, violence, and sexual content.
//!
//! # Example Configuration
//!
//! ```toml
//! [features.guardrails.input.provider]
//! type = "openai_moderation"
//! api_key = "sk-..."  # Optional, uses default OpenAI key if not set
//! base_url = "https://api.openai.com/v1"  # Optional, for proxies or compatible endpoints
//! model = "omni-moderation-latest"  # Optional, defaults to text-moderation-latest
//! ```

use std::{collections::HashMap, time::Instant};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{
    Category, GuardrailsError, GuardrailsProvider, GuardrailsRequest, GuardrailsResponse,
    GuardrailsResult, Severity, Violation, inject_trace_context,
};

/// Default OpenAI Moderation API base URL.
#[allow(dead_code)] // Guardrail infrastructure
const DEFAULT_OPENAI_MODERATION_BASE_URL: &str = "https://api.openai.com/v1";

/// OpenAI Moderation provider.
///
/// Uses OpenAI's moderation endpoint to evaluate content for policy violations.
/// The endpoint is free to use and supports multiple categories of harmful content.
pub struct OpenAIModerationProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    /// Per-category score thresholds (0.0 to 1.0). Content above threshold is flagged.
    /// If not set for a category, uses OpenAI's default flagging.
    thresholds: HashMap<String, f64>,
}

impl OpenAIModerationProvider {
    /// Creates a new OpenAI Moderation provider with default base URL.
    ///
    /// # Arguments
    /// * `client` - HTTP client to use for requests
    /// * `api_key` - OpenAI API key
    /// * `model` - Model to use (e.g., "text-moderation-latest", "omni-moderation-latest")
    #[allow(dead_code)] // Guardrail infrastructure
    pub fn new(client: Client, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::with_base_url(client, api_key, DEFAULT_OPENAI_MODERATION_BASE_URL, model)
    }

    /// Creates a new OpenAI Moderation provider with custom base URL.
    ///
    /// # Arguments
    /// * `client` - HTTP client to use for requests
    /// * `api_key` - OpenAI API key
    /// * `base_url` - Base URL for the moderation API (e.g., "https://api.openai.com/v1")
    /// * `model` - Model to use (e.g., "text-moderation-latest", "omni-moderation-latest")
    pub fn with_base_url(
        client: Client,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            client,
            api_key: api_key.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            thresholds: HashMap::new(),
        }
    }

    /// Sets custom score thresholds for categories.
    ///
    /// Content with scores above the threshold will be flagged as a violation,
    /// even if OpenAI's default flagging didn't trigger.
    #[allow(dead_code)] // Guardrail infrastructure
    pub fn with_thresholds(mut self, thresholds: HashMap<String, f64>) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Creates provider from configuration.
    #[allow(dead_code)] // Guardrail infrastructure
    pub fn from_config(
        client: Client,
        api_key: Option<String>,
        base_url: String,
        model: String,
        default_api_key: Option<&str>,
    ) -> GuardrailsResult<Self> {
        let api_key = api_key
            .or_else(|| default_api_key.map(|s| s.to_string()))
            .ok_or_else(|| {
                GuardrailsError::config_error(
                    "OpenAI Moderation requires an API key. Set api_key in guardrails config or configure a default OpenAI provider."
                )
            })?;

        Ok(Self::with_base_url(client, api_key, base_url, model))
    }
}

#[async_trait]
impl GuardrailsProvider for OpenAIModerationProvider {
    fn name(&self) -> &str {
        "openai_moderation"
    }

    #[instrument(
        skip(self, request),
        fields(
            provider = "openai_moderation",
            model = %self.model,
            text_length = request.text.len()
        )
    )]
    async fn evaluate(&self, request: &GuardrailsRequest) -> GuardrailsResult<GuardrailsResponse> {
        let start = Instant::now();

        let api_request = ModerationRequest {
            input: &request.text,
            model: &self.model,
        };

        let url = format!("{}/moderations", self.base_url);

        // Inject trace context for distributed tracing
        let mut trace_headers = HashMap::new();
        inject_trace_context(&mut trace_headers);

        let mut req_builder = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        for (key, value) in trace_headers {
            req_builder = req_builder.header(key, value);
        }

        let response = req_builder
            .json(&api_request)
            .send()
            .await
            .map_err(|e| GuardrailsError::from_reqwest("openai_moderation", e))?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(GuardrailsError::auth_error(
                "openai_moderation",
                "Invalid API key",
            ));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            return Err(GuardrailsError::rate_limited(
                "openai_moderation",
                retry_after,
            ));
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(GuardrailsError::provider_error(
                "openai_moderation",
                format!("API returned {}: {}", status, error_text),
            ));
        }

        let api_response: ModerationResponse = response.json().await.map_err(|e| {
            GuardrailsError::provider_error(
                "openai_moderation",
                format!("Failed to parse response: {}", e),
            )
        })?;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Convert API response to our standard format
        let violations = self.extract_violations(&api_response);

        Ok(GuardrailsResponse::with_violations(violations)
            .with_latency(latency_ms)
            .with_metadata(serde_json::json!({
                "model": api_response.model,
                "id": api_response.id,
            })))
    }

    fn supported_categories(&self) -> &[Category] {
        &[
            Category::Hate,
            Category::Harassment,
            Category::SelfHarm,
            Category::Sexual,
            Category::Violence,
            Category::Dangerous, // Maps to illicit
        ]
    }
}

impl OpenAIModerationProvider {
    /// Extracts violations from OpenAI moderation response.
    fn extract_violations(&self, response: &ModerationResponse) -> Vec<Violation> {
        let mut violations = Vec::new();

        for result in &response.results {
            // Check each category
            for (category_name, &flagged) in &result.categories {
                let score = result
                    .category_scores
                    .get(category_name)
                    .copied()
                    .unwrap_or(0.0);

                // Check if flagged by OpenAI or exceeds our custom threshold
                let exceeds_threshold = self
                    .thresholds
                    .get(category_name)
                    .is_some_and(|&threshold| score > threshold);

                if flagged || exceeds_threshold {
                    let category = map_openai_category(category_name);
                    let severity = Severity::from_score(score);

                    violations.push(Violation::new(category, severity, score).with_message(
                        format!(
                            "Content flagged for {} (score: {:.3})",
                            category_name, score
                        ),
                    ));
                }
            }
        }

        violations
    }
}

/// Maps OpenAI category names to our standard Category enum.
fn map_openai_category(openai_category: &str) -> Category {
    match openai_category {
        "hate" | "hate/threatening" => Category::Hate,
        "harassment" | "harassment/threatening" => Category::Harassment,
        "self-harm" | "self-harm/intent" | "self-harm/instructions" => Category::SelfHarm,
        "sexual" | "sexual/minors" => Category::Sexual,
        "violence" | "violence/graphic" => Category::Violence,
        "illicit" | "illicit/violent" => Category::Dangerous,
        other => Category::Custom(other.to_string()),
    }
}

/// Request body for OpenAI Moderation API.
#[derive(Debug, Serialize)]
struct ModerationRequest<'a> {
    input: &'a str,
    model: &'a str,
}

/// Response from OpenAI Moderation API.
#[derive(Debug, Deserialize)]
struct ModerationResponse {
    id: String,
    model: String,
    results: Vec<ModerationResult>,
}

/// Single moderation result.
#[derive(Debug, Deserialize)]
struct ModerationResult {
    /// Whether the content was flagged as violating any policy.
    #[allow(dead_code)] // Guardrail infrastructure
    flagged: bool,
    /// Boolean flags for each category.
    categories: HashMap<String, bool>,
    /// Confidence scores for each category (0.0 to 1.0).
    category_scores: HashMap<String, f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_response(
        flagged: bool,
        hate_score: f64,
        violence_score: f64,
    ) -> ModerationResponse {
        let mut categories = HashMap::new();
        categories.insert("hate".to_string(), hate_score > 0.5);
        categories.insert("hate/threatening".to_string(), false);
        categories.insert("harassment".to_string(), false);
        categories.insert("harassment/threatening".to_string(), false);
        categories.insert("self-harm".to_string(), false);
        categories.insert("self-harm/intent".to_string(), false);
        categories.insert("self-harm/instructions".to_string(), false);
        categories.insert("sexual".to_string(), false);
        categories.insert("sexual/minors".to_string(), false);
        categories.insert("violence".to_string(), violence_score > 0.5);
        categories.insert("violence/graphic".to_string(), false);
        categories.insert("illicit".to_string(), false);
        categories.insert("illicit/violent".to_string(), false);

        let mut category_scores = HashMap::new();
        category_scores.insert("hate".to_string(), hate_score);
        category_scores.insert("hate/threatening".to_string(), 0.0001);
        category_scores.insert("harassment".to_string(), 0.001);
        category_scores.insert("harassment/threatening".to_string(), 0.0001);
        category_scores.insert("self-harm".to_string(), 0.0001);
        category_scores.insert("self-harm/intent".to_string(), 0.0001);
        category_scores.insert("self-harm/instructions".to_string(), 0.0001);
        category_scores.insert("sexual".to_string(), 0.001);
        category_scores.insert("sexual/minors".to_string(), 0.0001);
        category_scores.insert("violence".to_string(), violence_score);
        category_scores.insert("violence/graphic".to_string(), 0.001);
        category_scores.insert("illicit".to_string(), 0.001);
        category_scores.insert("illicit/violent".to_string(), 0.0001);

        ModerationResponse {
            id: "modr-123".to_string(),
            model: "text-moderation-007".to_string(),
            results: vec![ModerationResult {
                flagged,
                categories,
                category_scores,
            }],
        }
    }

    #[test]
    fn test_extract_violations_safe_content() {
        let client = Client::new();
        let provider = OpenAIModerationProvider::new(client, "test-key", "text-moderation-latest");

        let response = create_mock_response(false, 0.001, 0.002);
        let violations = provider.extract_violations(&response);

        assert!(violations.is_empty());
    }

    #[test]
    fn test_extract_violations_flagged_hate() {
        let client = Client::new();
        let provider = OpenAIModerationProvider::new(client, "test-key", "text-moderation-latest");

        let response = create_mock_response(true, 0.95, 0.1);
        let violations = provider.extract_violations(&response);

        assert_eq!(violations.len(), 1);
        let violation = &violations[0];
        assert_eq!(violation.category, Category::Hate);
        assert!(violation.confidence > 0.9);
        assert_eq!(violation.severity, Severity::Critical);
    }

    #[test]
    fn test_extract_violations_multiple() {
        let client = Client::new();
        let provider = OpenAIModerationProvider::new(client, "test-key", "text-moderation-latest");

        // Both hate and violence flagged
        let response = create_mock_response(true, 0.85, 0.75);
        let violations = provider.extract_violations(&response);

        assert_eq!(violations.len(), 2);
        assert!(violations.iter().any(|v| v.category == Category::Hate));
        assert!(violations.iter().any(|v| v.category == Category::Violence));
    }

    #[test]
    fn test_custom_thresholds() {
        let client = Client::new();
        let mut thresholds = HashMap::new();
        thresholds.insert("violence".to_string(), 0.2); // Lower threshold

        let provider = OpenAIModerationProvider::new(client, "test-key", "text-moderation-latest")
            .with_thresholds(thresholds);

        // Violence score is 0.3, not flagged by OpenAI default but exceeds our threshold
        let response = create_mock_response(false, 0.01, 0.3);
        let violations = provider.extract_violations(&response);

        // Should flag violence because score (0.3) > threshold (0.2)
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].category, Category::Violence);
    }

    #[test]
    fn test_custom_thresholds_not_exceeded() {
        let client = Client::new();
        let mut thresholds = HashMap::new();
        thresholds.insert("violence".to_string(), 0.5); // Higher threshold

        let provider = OpenAIModerationProvider::new(client, "test-key", "text-moderation-latest")
            .with_thresholds(thresholds);

        // Violence score is 0.3, below our threshold
        let response = create_mock_response(false, 0.01, 0.3);
        let violations = provider.extract_violations(&response);

        // Should not flag violence because score (0.3) < threshold (0.5)
        assert!(violations.is_empty());
    }

    #[test]
    fn test_map_openai_category() {
        assert_eq!(map_openai_category("hate"), Category::Hate);
        assert_eq!(map_openai_category("hate/threatening"), Category::Hate);
        assert_eq!(map_openai_category("harassment"), Category::Harassment);
        assert_eq!(
            map_openai_category("harassment/threatening"),
            Category::Harassment
        );
        assert_eq!(map_openai_category("self-harm"), Category::SelfHarm);
        assert_eq!(map_openai_category("self-harm/intent"), Category::SelfHarm);
        assert_eq!(
            map_openai_category("self-harm/instructions"),
            Category::SelfHarm
        );
        assert_eq!(map_openai_category("sexual"), Category::Sexual);
        assert_eq!(map_openai_category("sexual/minors"), Category::Sexual);
        assert_eq!(map_openai_category("violence"), Category::Violence);
        assert_eq!(map_openai_category("violence/graphic"), Category::Violence);
        assert_eq!(map_openai_category("illicit"), Category::Dangerous);
        assert_eq!(map_openai_category("illicit/violent"), Category::Dangerous);
        assert_eq!(
            map_openai_category("unknown_category"),
            Category::Custom("unknown_category".to_string())
        );
    }

    #[test]
    fn test_severity_from_score() {
        assert_eq!(Severity::from_score(0.1), Severity::Low);
        assert_eq!(Severity::from_score(0.3), Severity::Medium);
        assert_eq!(Severity::from_score(0.6), Severity::High);
        assert_eq!(Severity::from_score(0.9), Severity::Critical);
    }

    #[test]
    fn test_provider_name() {
        let client = Client::new();
        let provider = OpenAIModerationProvider::new(client, "test-key", "text-moderation-latest");
        assert_eq!(provider.name(), "openai_moderation");
    }

    #[test]
    fn test_supported_categories() {
        let client = Client::new();
        let provider = OpenAIModerationProvider::new(client, "test-key", "text-moderation-latest");
        let categories = provider.supported_categories();

        assert!(categories.contains(&Category::Hate));
        assert!(categories.contains(&Category::Harassment));
        assert!(categories.contains(&Category::SelfHarm));
        assert!(categories.contains(&Category::Sexual));
        assert!(categories.contains(&Category::Violence));
        assert!(categories.contains(&Category::Dangerous));
    }

    #[test]
    fn test_from_config_with_api_key() {
        let client = Client::new();
        let result = OpenAIModerationProvider::from_config(
            client,
            Some("test-key".to_string()),
            "https://api.openai.com/v1".to_string(),
            "text-moderation-latest".to_string(),
            None,
        );

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.api_key, "test-key");
        assert_eq!(provider.base_url, "https://api.openai.com/v1");
        assert_eq!(provider.model, "text-moderation-latest");
    }

    #[test]
    fn test_from_config_with_default_key() {
        let client = Client::new();
        let result = OpenAIModerationProvider::from_config(
            client,
            None,
            "https://api.openai.com/v1".to_string(),
            "text-moderation-latest".to_string(),
            Some("default-key"),
        );

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.api_key, "default-key");
    }

    #[test]
    fn test_from_config_custom_base_url() {
        let client = Client::new();
        let result = OpenAIModerationProvider::from_config(
            client,
            Some("test-key".to_string()),
            "https://my-proxy.example.com/v1/".to_string(), // Trailing slash should be trimmed
            "text-moderation-latest".to_string(),
            None,
        );

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.base_url, "https://my-proxy.example.com/v1");
    }

    #[test]
    fn test_from_config_no_key() {
        let client = Client::new();
        let result = OpenAIModerationProvider::from_config(
            client,
            None,
            "https://api.openai.com/v1".to_string(),
            "text-moderation-latest".to_string(),
            None,
        );

        assert!(result.is_err());
        match result {
            Err(GuardrailsError::ConfigError { message }) => {
                assert!(message.contains("API key"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_with_base_url() {
        let client = Client::new();
        let provider = OpenAIModerationProvider::with_base_url(
            client,
            "test-key",
            "https://custom.example.com/v1",
            "text-moderation-latest",
        );

        assert_eq!(provider.base_url, "https://custom.example.com/v1");
    }

    #[test]
    fn test_moderation_response_parsing() {
        let json = r#"{
            "id": "modr-123",
            "model": "text-moderation-007",
            "results": [{
                "flagged": true,
                "categories": {
                    "hate": true,
                    "violence": false
                },
                "category_scores": {
                    "hate": 0.95,
                    "violence": 0.01
                }
            }]
        }"#;

        let response: ModerationResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "modr-123");
        assert_eq!(response.model, "text-moderation-007");
        assert_eq!(response.results.len(), 1);
        assert!(response.results[0].flagged);
        assert_eq!(response.results[0].categories.get("hate"), Some(&true));
        assert!(
            (response.results[0].category_scores.get("hate").unwrap() - 0.95).abs() < f64::EPSILON
        );
    }
}
