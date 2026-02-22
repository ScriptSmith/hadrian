//! Azure AI Content Safety provider for guardrails.
//!
//! This provider uses Azure AI Content Safety's Text Analysis API to detect harmful content
//! across categories including Hate, Violence, SelfHarm, and Sexual content.
//!
//! # Example Configuration
//!
//! ```toml
//! [features.guardrails.input.provider]
//! type = "azure_content_safety"
//! endpoint = "https://myservice.cognitiveservices.azure.com"
//! api_key = "${AZURE_CONTENT_SAFETY_KEY}"
//! api_version = "2024-09-01"  # Optional, defaults to 2024-09-01
//!
//! [features.guardrails.input.provider.thresholds]
//! Hate = 2      # Block severity 2 and above (0-6 scale)
//! Violence = 4  # Block severity 4 and above
//! ```
//!
//! # Azure Content Safety Categories
//!
//! Azure returns severity scores (0-6) for each category:
//! - **Hate**: Content expressing hatred or discrimination
//! - **Violence**: Content describing physical violence
//! - **SelfHarm**: Content related to self-harm
//! - **Sexual**: Sexually explicit content
//!
//! Default thresholds block severity 2+ for all categories.

use std::{collections::HashMap, time::Instant};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{
    Category, GuardrailsError, GuardrailsProvider, GuardrailsRequest, GuardrailsResponse,
    GuardrailsResult, Severity, Violation, inject_trace_context,
};

/// Default Azure Content Safety API version.
#[allow(dead_code)] // Guardrail infrastructure
const DEFAULT_API_VERSION: &str = "2024-09-01";

/// Default severity threshold for flagging content (0-6 scale).
const DEFAULT_SEVERITY_THRESHOLD: u8 = 2;

/// Azure AI Content Safety provider.
///
/// Uses Azure's Text Analysis API to evaluate content for policy violations.
/// Supports configurable severity thresholds per category and blocklist checking.
pub struct AzureContentSafetyProvider {
    client: Client,
    endpoint: String,
    api_key: String,
    api_version: String,
    /// Per-category severity thresholds (0-6). Content at or above threshold is flagged.
    thresholds: HashMap<String, u8>,
    /// Blocklist names to check against.
    blocklist_names: Vec<String>,
}

impl AzureContentSafetyProvider {
    /// Creates a new Azure Content Safety provider.
    ///
    /// # Arguments
    /// * `client` - HTTP client to use for requests
    /// * `endpoint` - Azure Content Safety endpoint URL
    /// * `api_key` - Azure API key
    /// * `api_version` - API version (e.g., "2024-09-01")
    pub fn new(
        client: Client,
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        api_version: impl Into<String>,
    ) -> Self {
        Self {
            client,
            endpoint: endpoint.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            api_version: api_version.into(),
            thresholds: HashMap::new(),
            blocklist_names: Vec::new(),
        }
    }

    /// Sets custom severity thresholds for categories.
    ///
    /// Content with severity at or above the threshold will be flagged as a violation.
    /// Default threshold is 2 for all categories.
    pub fn with_thresholds(mut self, thresholds: HashMap<String, u8>) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Sets blocklist names to check against.
    pub fn with_blocklists(mut self, blocklist_names: Vec<String>) -> Self {
        self.blocklist_names = blocklist_names;
        self
    }

    /// Creates provider from configuration.
    #[allow(dead_code)] // Guardrail infrastructure
    pub fn from_config(
        client: Client,
        endpoint: String,
        api_key: String,
        api_version: Option<String>,
        thresholds: HashMap<String, u8>,
        blocklist_names: Vec<String>,
    ) -> GuardrailsResult<Self> {
        if endpoint.is_empty() {
            return Err(GuardrailsError::config_error(
                "Azure Content Safety requires an endpoint URL",
            ));
        }

        if api_key.is_empty() {
            return Err(GuardrailsError::config_error(
                "Azure Content Safety requires an API key",
            ));
        }

        let api_version = api_version.unwrap_or_else(|| DEFAULT_API_VERSION.to_string());

        Ok(Self::new(client, endpoint, api_key, api_version)
            .with_thresholds(thresholds)
            .with_blocklists(blocklist_names))
    }

    /// Gets the threshold for a category, returning the default if not configured.
    fn get_threshold(&self, category: &str) -> u8 {
        self.thresholds
            .get(category)
            .copied()
            .unwrap_or(DEFAULT_SEVERITY_THRESHOLD)
    }

    /// Extracts violations from Azure Content Safety response.
    fn extract_violations(&self, response: &TextAnalysisResponse) -> Vec<Violation> {
        let mut violations = Vec::new();

        // Check standard categories
        if let Some(ref result) = response.categories_analysis {
            for analysis in result {
                let threshold = self.get_threshold(&analysis.category);
                let severity = analysis.severity.unwrap_or(0);

                if severity >= threshold {
                    let category = map_azure_category(&analysis.category);
                    let normalized_severity = Severity::from_azure_threshold(severity);

                    violations.push(
                        Violation::new(category, normalized_severity, severity_to_score(severity))
                            .with_message(format!(
                                "Content flagged for {} (severity: {})",
                                analysis.category, severity
                            ))
                            .with_details(serde_json::json!({
                                "category": analysis.category,
                                "severity": severity,
                                "threshold": threshold,
                            })),
                    );
                }
            }
        }

        // Check blocklist matches
        if let Some(ref blocklist_match) = response.blocklists_match {
            for item in blocklist_match {
                violations.push(
                    Violation::new(
                        Category::Custom(format!("blocklist:{}", item.blocklist_name)),
                        Severity::High,
                        1.0,
                    )
                    .with_message(format!(
                        "Blocklist match: {} in {}",
                        item.blocklist_item_text, item.blocklist_name
                    ))
                    .with_details(serde_json::json!({
                        "blocklist_name": item.blocklist_name,
                        "blocklist_item_id": item.blocklist_item_id,
                        "blocklist_item_text": item.blocklist_item_text,
                    })),
                );
            }
        }

        violations
    }
}

#[async_trait]
impl GuardrailsProvider for AzureContentSafetyProvider {
    fn name(&self) -> &str {
        "azure_content_safety"
    }

    #[instrument(
        skip(self, request),
        fields(
            provider = "azure_content_safety",
            endpoint = %self.endpoint,
            text_length = request.text.len()
        )
    )]
    async fn evaluate(&self, request: &GuardrailsRequest) -> GuardrailsResult<GuardrailsResponse> {
        let start = Instant::now();

        // Build the request body
        let mut api_request = TextAnalysisRequest {
            text: request.text.clone(),
            categories: None,      // Use default categories
            blocklist_names: None, // Set below if configured
            halt_on_blocklist_hit: None,
            output_type: Some("FourSeverityLevels".to_string()),
        };

        // Add blocklist names if configured
        if !self.blocklist_names.is_empty() {
            api_request.blocklist_names = Some(self.blocklist_names.clone());
            api_request.halt_on_blocklist_hit = Some(false); // Continue analysis after blocklist hit
        }

        let url = format!(
            "{}/contentsafety/text:analyze?api-version={}",
            self.endpoint, self.api_version
        );

        // Inject trace context for distributed tracing
        let mut trace_headers = HashMap::new();
        inject_trace_context(&mut trace_headers);

        let mut req_builder = self
            .client
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .header("Content-Type", "application/json");

        for (key, value) in trace_headers {
            req_builder = req_builder.header(key, value);
        }

        let response = req_builder
            .json(&api_request)
            .send()
            .await
            .map_err(|e| GuardrailsError::from_reqwest("azure_content_safety", e))?;

        let status = response.status();

        // Handle error status codes
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(GuardrailsError::auth_error(
                "azure_content_safety",
                "Invalid API key or insufficient permissions",
            ));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            return Err(GuardrailsError::rate_limited(
                "azure_content_safety",
                retry_after,
            ));
        }

        if status == reqwest::StatusCode::BAD_REQUEST {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(GuardrailsError::provider_error(
                "azure_content_safety",
                format!("Invalid request: {}", error_text),
            ));
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(GuardrailsError::provider_error(
                "azure_content_safety",
                format!("API returned {}: {}", status, error_text),
            ));
        }

        let api_response: TextAnalysisResponse = response.json().await.map_err(|e| {
            GuardrailsError::provider_error(
                "azure_content_safety",
                format!("Failed to parse response: {}", e),
            )
        })?;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Extract violations based on thresholds
        let violations = self.extract_violations(&api_response);

        Ok(GuardrailsResponse::with_violations(violations)
            .with_latency(latency_ms)
            .with_metadata(serde_json::json!({
                "categories_analysis": api_response.categories_analysis,
                "blocklists_match": api_response.blocklists_match,
            })))
    }

    fn supported_categories(&self) -> &[Category] {
        &[
            Category::Hate,
            Category::Violence,
            Category::SelfHarm,
            Category::Sexual,
        ]
    }
}

/// Maps Azure category names to standard Category enum.
fn map_azure_category(azure_category: &str) -> Category {
    match azure_category {
        "Hate" => Category::Hate,
        "Violence" => Category::Violence,
        "SelfHarm" => Category::SelfHarm,
        "Sexual" => Category::Sexual,
        other => Category::Custom(other.to_string()),
    }
}

/// Converts Azure severity (0-6) to a normalized score (0.0-1.0).
fn severity_to_score(severity: u8) -> f64 {
    (severity as f64 / 6.0).min(1.0)
}

// ============================================================================
// Azure Content Safety API Types
// ============================================================================

/// Request body for Azure Text Analysis API.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TextAnalysisRequest {
    /// The text to analyze.
    text: String,

    /// Categories to analyze (default: all).
    #[serde(skip_serializing_if = "Option::is_none")]
    categories: Option<Vec<String>>,

    /// Blocklist names to check.
    #[serde(skip_serializing_if = "Option::is_none")]
    blocklist_names: Option<Vec<String>>,

    /// Whether to halt on blocklist hit.
    #[serde(skip_serializing_if = "Option::is_none")]
    halt_on_blocklist_hit: Option<bool>,

    /// Output type for severity levels.
    /// "FourSeverityLevels" for 0, 2, 4, 6
    /// "EightSeverityLevels" for 0-7
    #[serde(skip_serializing_if = "Option::is_none")]
    output_type: Option<String>,
}

/// Response from Azure Text Analysis API.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TextAnalysisResponse {
    /// Analysis results per category.
    #[serde(default)]
    categories_analysis: Option<Vec<CategoryAnalysis>>,

    /// Blocklist match results.
    #[serde(default)]
    blocklists_match: Option<Vec<BlocklistMatch>>,
}

/// Analysis result for a single category.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CategoryAnalysis {
    /// Category name (Hate, Violence, SelfHarm, Sexual).
    category: String,

    /// Severity level (0-6 or 0-7 depending on output_type).
    #[serde(default)]
    severity: Option<u8>,
}

/// Blocklist match result.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BlocklistMatch {
    /// Name of the blocklist that matched.
    blocklist_name: String,

    /// ID of the blocklist item that matched.
    blocklist_item_id: String,

    /// Text of the blocklist item that matched.
    blocklist_item_text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_response(
        hate_severity: u8,
        violence_severity: u8,
        self_harm_severity: u8,
        sexual_severity: u8,
    ) -> TextAnalysisResponse {
        TextAnalysisResponse {
            categories_analysis: Some(vec![
                CategoryAnalysis {
                    category: "Hate".to_string(),
                    severity: Some(hate_severity),
                },
                CategoryAnalysis {
                    category: "Violence".to_string(),
                    severity: Some(violence_severity),
                },
                CategoryAnalysis {
                    category: "SelfHarm".to_string(),
                    severity: Some(self_harm_severity),
                },
                CategoryAnalysis {
                    category: "Sexual".to_string(),
                    severity: Some(sexual_severity),
                },
            ]),
            blocklists_match: None,
        }
    }

    #[test]
    fn test_extract_violations_safe_content() {
        let client = Client::new();
        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01");

        let response = create_mock_response(0, 0, 0, 0);
        let violations = provider.extract_violations(&response);

        assert!(violations.is_empty());
    }

    #[test]
    fn test_extract_violations_above_threshold() {
        let client = Client::new();
        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01");

        // Hate severity 4 should be flagged (default threshold is 2)
        let response = create_mock_response(4, 0, 0, 0);
        let violations = provider.extract_violations(&response);

        assert_eq!(violations.len(), 1);
        let violation = &violations[0];
        assert_eq!(violation.category, Category::Hate);
        assert_eq!(violation.severity, Severity::Medium); // 4 maps to Medium
    }

    #[test]
    fn test_extract_violations_at_threshold() {
        let client = Client::new();
        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01");

        // Severity exactly at threshold should be flagged
        let response = create_mock_response(2, 0, 0, 0);
        let violations = provider.extract_violations(&response);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].category, Category::Hate);
    }

    #[test]
    fn test_extract_violations_below_threshold() {
        let client = Client::new();
        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01");

        // Severity below threshold should not be flagged
        let response = create_mock_response(1, 1, 1, 1);
        let violations = provider.extract_violations(&response);

        assert!(violations.is_empty());
    }

    #[test]
    fn test_extract_violations_custom_thresholds() {
        let client = Client::new();
        let mut thresholds = HashMap::new();
        thresholds.insert("Hate".to_string(), 4); // Higher threshold for Hate
        thresholds.insert("Violence".to_string(), 0); // Lower threshold for Violence

        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01")
                .with_thresholds(thresholds);

        // Hate at 2 should not be flagged (threshold is 4)
        // Violence at 0 should be flagged (threshold is 0)
        let response = create_mock_response(2, 0, 0, 0);
        let violations = provider.extract_violations(&response);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].category, Category::Violence);
    }

    #[test]
    fn test_extract_violations_multiple() {
        let client = Client::new();
        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01");

        let response = create_mock_response(4, 6, 0, 2);
        let violations = provider.extract_violations(&response);

        assert_eq!(violations.len(), 3);
        assert!(violations.iter().any(|v| v.category == Category::Hate));
        assert!(violations.iter().any(|v| v.category == Category::Violence));
        assert!(violations.iter().any(|v| v.category == Category::Sexual));
    }

    #[test]
    fn test_extract_violations_blocklist_match() {
        let client = Client::new();
        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01");

        let response = TextAnalysisResponse {
            categories_analysis: Some(vec![]),
            blocklists_match: Some(vec![BlocklistMatch {
                blocklist_name: "bad_words".to_string(),
                blocklist_item_id: "item-123".to_string(),
                blocklist_item_text: "badword".to_string(),
            }]),
        };

        let violations = provider.extract_violations(&response);

        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations[0].category,
            Category::Custom("blocklist:bad_words".to_string())
        );
        assert_eq!(violations[0].severity, Severity::High);
    }

    #[test]
    fn test_map_azure_category() {
        assert_eq!(map_azure_category("Hate"), Category::Hate);
        assert_eq!(map_azure_category("Violence"), Category::Violence);
        assert_eq!(map_azure_category("SelfHarm"), Category::SelfHarm);
        assert_eq!(map_azure_category("Sexual"), Category::Sexual);
        assert_eq!(
            map_azure_category("UnknownCategory"),
            Category::Custom("UnknownCategory".to_string())
        );
    }

    #[test]
    fn test_severity_to_score() {
        assert!((severity_to_score(0) - 0.0).abs() < f64::EPSILON);
        assert!((severity_to_score(3) - 0.5).abs() < f64::EPSILON);
        assert!((severity_to_score(6) - 1.0).abs() < f64::EPSILON);
        // Test clamping for values above 6
        assert!((severity_to_score(7) - 1.0).abs() < 0.2);
    }

    #[test]
    fn test_provider_name() {
        let client = Client::new();
        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01");
        assert_eq!(provider.name(), "azure_content_safety");
    }

    #[test]
    fn test_supported_categories() {
        let client = Client::new();
        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01");
        let categories = provider.supported_categories();

        assert!(categories.contains(&Category::Hate));
        assert!(categories.contains(&Category::Violence));
        assert!(categories.contains(&Category::SelfHarm));
        assert!(categories.contains(&Category::Sexual));
        assert_eq!(categories.len(), 4);
    }

    #[test]
    fn test_from_config_valid() {
        let client = Client::new();
        let result = AzureContentSafetyProvider::from_config(
            client,
            "https://my-service.cognitiveservices.azure.com".to_string(),
            "my-api-key".to_string(),
            Some("2024-09-01".to_string()),
            HashMap::new(),
            vec!["blocklist1".to_string()],
        );

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(
            provider.endpoint,
            "https://my-service.cognitiveservices.azure.com"
        );
        assert_eq!(provider.api_key, "my-api-key");
        assert_eq!(provider.api_version, "2024-09-01");
        assert_eq!(provider.blocklist_names.len(), 1);
    }

    #[test]
    fn test_from_config_default_version() {
        let client = Client::new();
        let result = AzureContentSafetyProvider::from_config(
            client,
            "https://test.azure.com".to_string(),
            "key".to_string(),
            None, // No version specified
            HashMap::new(),
            vec![],
        );

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.api_version, "2024-09-01");
    }

    #[test]
    fn test_from_config_empty_endpoint() {
        let client = Client::new();
        let result = AzureContentSafetyProvider::from_config(
            client,
            "".to_string(), // Empty endpoint
            "key".to_string(),
            None,
            HashMap::new(),
            vec![],
        );

        assert!(result.is_err());
        match result {
            Err(GuardrailsError::ConfigError { message }) => {
                assert!(message.contains("endpoint"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_from_config_empty_api_key() {
        let client = Client::new();
        let result = AzureContentSafetyProvider::from_config(
            client,
            "https://test.azure.com".to_string(),
            "".to_string(), // Empty API key
            None,
            HashMap::new(),
            vec![],
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
    fn test_endpoint_trailing_slash_trimmed() {
        let client = Client::new();
        let provider = AzureContentSafetyProvider::new(
            client,
            "https://test.azure.com/", // Trailing slash
            "key",
            "2024-09-01",
        );

        assert_eq!(provider.endpoint, "https://test.azure.com");
    }

    #[test]
    fn test_with_thresholds() {
        let client = Client::new();
        let mut thresholds = HashMap::new();
        thresholds.insert("Hate".to_string(), 4);
        thresholds.insert("Violence".to_string(), 6);

        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01")
                .with_thresholds(thresholds);

        assert_eq!(provider.get_threshold("Hate"), 4);
        assert_eq!(provider.get_threshold("Violence"), 6);
        assert_eq!(provider.get_threshold("Sexual"), 2); // Default
    }

    #[test]
    fn test_with_blocklists() {
        let client = Client::new();
        let provider =
            AzureContentSafetyProvider::new(client, "https://test.azure.com", "key", "2024-09-01")
                .with_blocklists(vec!["list1".to_string(), "list2".to_string()]);

        assert_eq!(provider.blocklist_names.len(), 2);
        assert!(provider.blocklist_names.contains(&"list1".to_string()));
        assert!(provider.blocklist_names.contains(&"list2".to_string()));
    }

    #[test]
    fn test_text_analysis_request_serialization() {
        let request = TextAnalysisRequest {
            text: "Hello, world!".to_string(),
            categories: None,
            blocklist_names: Some(vec!["blocklist1".to_string()]),
            halt_on_blocklist_hit: Some(false),
            output_type: Some("FourSeverityLevels".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"text\":\"Hello, world!\""));
        assert!(json.contains("\"blocklistNames\":[\"blocklist1\"]"));
        assert!(json.contains("\"haltOnBlocklistHit\":false"));
        assert!(json.contains("\"outputType\":\"FourSeverityLevels\""));
        // categories should not be serialized when None
        assert!(!json.contains("categories"));
    }

    #[test]
    fn test_text_analysis_response_parsing() {
        let json = r#"{
            "categoriesAnalysis": [
                {"category": "Hate", "severity": 2},
                {"category": "Violence", "severity": 0},
                {"category": "SelfHarm", "severity": 0},
                {"category": "Sexual", "severity": 4}
            ]
        }"#;

        let response: TextAnalysisResponse = serde_json::from_str(json).unwrap();
        let categories = response.categories_analysis.unwrap();
        assert_eq!(categories.len(), 4);
        assert_eq!(categories[0].category, "Hate");
        assert_eq!(categories[0].severity, Some(2));
        assert_eq!(categories[3].category, "Sexual");
        assert_eq!(categories[3].severity, Some(4));
    }

    #[test]
    fn test_text_analysis_response_with_blocklist() {
        let json = r#"{
            "categoriesAnalysis": [],
            "blocklistsMatch": [
                {
                    "blocklistName": "profanity",
                    "blocklistItemId": "123",
                    "blocklistItemText": "badword"
                }
            ]
        }"#;

        let response: TextAnalysisResponse = serde_json::from_str(json).unwrap();
        let blocklist_matches = response.blocklists_match.unwrap();
        assert_eq!(blocklist_matches.len(), 1);
        assert_eq!(blocklist_matches[0].blocklist_name, "profanity");
        assert_eq!(blocklist_matches[0].blocklist_item_text, "badword");
    }

    #[test]
    fn test_severity_from_azure_threshold() {
        // Test the mapping from Azure severity (0-6) to our Severity enum
        assert_eq!(Severity::from_azure_threshold(0), Severity::Info);
        assert_eq!(Severity::from_azure_threshold(1), Severity::Low);
        assert_eq!(Severity::from_azure_threshold(2), Severity::Low);
        assert_eq!(Severity::from_azure_threshold(3), Severity::Medium);
        assert_eq!(Severity::from_azure_threshold(4), Severity::Medium);
        assert_eq!(Severity::from_azure_threshold(5), Severity::High);
        assert_eq!(Severity::from_azure_threshold(6), Severity::Critical);
    }
}
