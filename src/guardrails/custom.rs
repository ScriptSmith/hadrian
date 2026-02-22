//! Custom HTTP guardrails provider for bring-your-own guardrails implementations.
//!
//! This provider allows integration with any HTTP-based guardrails service that
//! follows a compatible request/response format.
//!
//! # Request Format (OpenAI-compatible by default)
//!
//! ```json
//! {
//!     "input": "text to evaluate",
//!     "source": "user_input",
//!     "request_id": "optional-request-id",
//!     "user_id": "optional-user-id",
//!     "context": {}
//! }
//! ```
//!
//! # Response Format (OpenAI-compatible by default)
//!
//! ```json
//! {
//!     "passed": false,
//!     "violations": [
//!         {
//!             "category": "hate",
//!             "severity": "high",
//!             "confidence": 0.95,
//!             "message": "Hate speech detected"
//!         }
//!     ]
//! }
//! ```
//!
//! # Example Configuration
//!
//! ```toml
//! [features.guardrails.custom]
//! enabled = true
//!
//! [features.guardrails.custom.provider]
//! url = "https://my-guardrails.example.com/evaluate"
//! api_key = "my-api-key"
//! timeout_ms = 3000
//! retry_enabled = true
//! max_retries = 2
//!
//! [features.guardrails.custom.provider.headers]
//! X-Custom-Header = "value"
//! ```

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{
    Category, GuardrailsError, GuardrailsProvider, GuardrailsRequest, GuardrailsResponse,
    GuardrailsResult, Severity, Violation, inject_trace_context,
};
use crate::config::CustomGuardrailsProvider as CustomGuardrailsConfig;

/// Custom HTTP guardrails provider.
///
/// Implements the `GuardrailsProvider` trait for any HTTP-based guardrails service
/// that follows a compatible request/response format.
pub struct CustomHttpProvider {
    client: Client,
    url: String,
    api_key: Option<String>,
    headers: HashMap<String, String>,
    timeout: Duration,
}

impl CustomHttpProvider {
    /// Creates a new custom HTTP provider.
    ///
    /// # Arguments
    /// * `client` - HTTP client to use for requests
    /// * `url` - URL of the guardrails service
    pub fn new(client: Client, url: impl Into<String>) -> Self {
        Self {
            client,
            url: url.into(),
            api_key: None,
            headers: HashMap::new(),
            timeout: Duration::from_millis(5000),
        }
    }

    /// Sets the API key for authentication.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Sets custom headers to include in requests.
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = headers;
        self
    }

    /// Sets the request timeout.
    #[allow(dead_code)] // Guardrail infrastructure
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the request timeout in milliseconds.
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout = Duration::from_millis(timeout_ms);
        self
    }

    /// Creates provider from configuration.
    pub fn from_config(client: Client, config: &CustomGuardrailsConfig) -> GuardrailsResult<Self> {
        if config.url.is_empty() {
            return Err(GuardrailsError::config_error(
                "Custom guardrails provider requires a URL",
            ));
        }

        let mut provider = Self::new(client, &config.url)
            .with_headers(config.headers.clone())
            .with_timeout_ms(config.timeout_ms);

        if let Some(ref api_key) = config.api_key {
            provider = provider.with_api_key(api_key);
        }

        Ok(provider)
    }
}

#[async_trait]
impl GuardrailsProvider for CustomHttpProvider {
    fn name(&self) -> &str {
        "custom"
    }

    #[instrument(
        skip(self, request),
        fields(
            provider = "custom",
            url = %self.url,
            text_length = request.text.len()
        )
    )]
    async fn evaluate(&self, request: &GuardrailsRequest) -> GuardrailsResult<GuardrailsResponse> {
        let start = Instant::now();

        let api_request = CustomRequest {
            input: &request.text,
            source: &request.source.to_string(),
            request_id: request.request_id.as_deref(),
            user_id: request.user_id.as_deref(),
            context: request.context.as_ref(),
        };

        let mut req_builder = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .timeout(self.timeout)
            .json(&api_request);

        // Add API key if configured
        if let Some(ref api_key) = self.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        // Add custom headers
        for (key, value) in &self.headers {
            req_builder = req_builder.header(key, value);
        }

        // Inject trace context for distributed tracing
        let mut trace_headers = HashMap::new();
        inject_trace_context(&mut trace_headers);
        for (key, value) in trace_headers {
            req_builder = req_builder.header(key, value);
        }

        let response = req_builder.send().await.map_err(|e| {
            if e.is_timeout() {
                GuardrailsError::timeout("custom", self.timeout.as_millis() as u64)
            } else {
                GuardrailsError::from_reqwest("custom", e)
            }
        })?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(GuardrailsError::auth_error(
                "custom",
                format!("Authentication failed: {}", status),
            ));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            return Err(GuardrailsError::rate_limited("custom", retry_after));
        }

        if status.is_server_error() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(GuardrailsError::retryable_error(
                "custom",
                format!("Server error {}: {}", status, error_text),
            ));
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(GuardrailsError::provider_error(
                "custom",
                format!("API returned {}: {}", status, error_text),
            ));
        }

        let api_response: CustomResponse = response.json().await.map_err(|e| {
            GuardrailsError::provider_error("custom", format!("Failed to parse response: {}", e))
        })?;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Convert API response to our standard format
        let violations: Vec<Violation> = api_response
            .violations
            .into_iter()
            .map(|v| {
                let category = Category::from(v.category.as_str());
                let severity = v
                    .severity
                    .as_ref()
                    .map(|s| parse_severity(s))
                    .unwrap_or(Severity::Medium);

                let mut violation = Violation::new(category, severity, v.confidence.unwrap_or(1.0));

                if let Some(message) = v.message {
                    violation = violation.with_message(message);
                }

                if let Some(span) = v.span
                    && let (Some(start), Some(end)) = (span.start, span.end)
                {
                    violation = violation.with_span(start, end);
                }

                if let Some(details) = v.details {
                    violation = violation.with_details(details);
                }

                violation
            })
            .collect();

        let mut response = GuardrailsResponse::with_violations(violations).with_latency(latency_ms);

        if let Some(metadata) = api_response.metadata {
            response = response.with_metadata(metadata);
        }

        Ok(response)
    }

    fn supported_categories(&self) -> &[Category] {
        // Custom providers may support any category
        Category::all_standard()
    }
}

/// Parses a severity string into a Severity enum.
fn parse_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "info" | "informational" => Severity::Info,
        "low" => Severity::Low,
        "medium" | "moderate" => Severity::Medium,
        "high" => Severity::High,
        "critical" | "severe" => Severity::Critical,
        _ => Severity::Medium, // Default to medium for unknown severities
    }
}

/// Request body for custom guardrails API.
#[derive(Debug, Serialize)]
struct CustomRequest<'a> {
    /// Text content to evaluate.
    input: &'a str,
    /// Source of the content (user_input, llm_output, system).
    source: &'a str,
    /// Optional request ID for correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<&'a str>,
    /// Optional user ID for audit logging.
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<&'a str>,
    /// Optional additional context.
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<&'a serde_json::Value>,
}

/// Response from custom guardrails API.
#[derive(Debug, Deserialize)]
struct CustomResponse {
    /// Whether the content passed evaluation (no violations).
    /// If not present, inferred from violations being empty.
    #[serde(default)]
    #[allow(dead_code)] // Guardrail infrastructure
    passed: Option<bool>,
    /// List of violations found.
    #[serde(default)]
    violations: Vec<CustomViolation>,
    /// Optional metadata from the provider.
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

/// Violation from custom guardrails API.
#[derive(Debug, Deserialize)]
struct CustomViolation {
    /// Category of the violation.
    category: String,
    /// Severity level (info, low, medium, high, critical).
    #[serde(default)]
    severity: Option<String>,
    /// Confidence score (0.0 to 1.0).
    #[serde(default)]
    confidence: Option<f64>,
    /// Human-readable message.
    #[serde(default)]
    message: Option<String>,
    /// Character span of the violation.
    #[serde(default)]
    span: Option<CustomSpan>,
    /// Provider-specific details.
    #[serde(default)]
    details: Option<serde_json::Value>,
}

/// Span from custom guardrails API.
#[derive(Debug, Deserialize)]
struct CustomSpan {
    /// Start character index.
    start: Option<usize>,
    /// End character index.
    end: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_severity() {
        assert_eq!(parse_severity("info"), Severity::Info);
        assert_eq!(parse_severity("INFO"), Severity::Info);
        assert_eq!(parse_severity("informational"), Severity::Info);
        assert_eq!(parse_severity("low"), Severity::Low);
        assert_eq!(parse_severity("medium"), Severity::Medium);
        assert_eq!(parse_severity("moderate"), Severity::Medium);
        assert_eq!(parse_severity("high"), Severity::High);
        assert_eq!(parse_severity("critical"), Severity::Critical);
        assert_eq!(parse_severity("severe"), Severity::Critical);
        assert_eq!(parse_severity("unknown"), Severity::Medium);
    }

    #[test]
    fn test_custom_provider_builder() {
        let client = Client::new();
        let mut headers = HashMap::new();
        headers.insert("X-Custom".to_string(), "value".to_string());

        let provider = CustomHttpProvider::new(client, "https://example.com/evaluate")
            .with_api_key("test-key")
            .with_headers(headers)
            .with_timeout_ms(3000);

        assert_eq!(provider.url, "https://example.com/evaluate");
        assert_eq!(provider.api_key, Some("test-key".to_string()));
        assert_eq!(provider.headers.get("X-Custom"), Some(&"value".to_string()));
        assert_eq!(provider.timeout, Duration::from_millis(3000));
    }

    #[test]
    fn test_custom_provider_name() {
        let client = Client::new();
        let provider = CustomHttpProvider::new(client, "https://example.com");
        assert_eq!(provider.name(), "custom");
    }

    #[test]
    fn test_from_config() {
        let client = Client::new();
        let mut headers = HashMap::new();
        headers.insert("X-Test".to_string(), "header-value".to_string());

        let config = CustomGuardrailsConfig {
            url: "https://guardrails.example.com/check".to_string(),
            api_key: Some("secret-key".to_string()),
            headers,
            timeout_ms: 2500,
            retry_enabled: true,
            max_retries: 3,
        };

        let provider = CustomHttpProvider::from_config(client, &config).unwrap();
        assert_eq!(provider.url, "https://guardrails.example.com/check");
        assert_eq!(provider.api_key, Some("secret-key".to_string()));
        assert_eq!(
            provider.headers.get("X-Test"),
            Some(&"header-value".to_string())
        );
        assert_eq!(provider.timeout, Duration::from_millis(2500));
    }

    #[test]
    fn test_from_config_empty_url() {
        let client = Client::new();
        let config = CustomGuardrailsConfig {
            url: String::new(),
            api_key: None,
            headers: HashMap::new(),
            timeout_ms: 5000,
            retry_enabled: false,
            max_retries: 0,
        };

        let result = CustomHttpProvider::from_config(client, &config);
        assert!(result.is_err());
        match result {
            Err(GuardrailsError::ConfigError { message }) => {
                assert!(message.contains("URL"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_custom_request_serialization() {
        let request = CustomRequest {
            input: "test text",
            source: "user_input",
            request_id: Some("req-123"),
            user_id: None,
            context: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"input\":\"test text\""));
        assert!(json.contains("\"source\":\"user_input\""));
        assert!(json.contains("\"request_id\":\"req-123\""));
        // user_id and context should be omitted
        assert!(!json.contains("user_id"));
        assert!(!json.contains("context"));
    }

    #[test]
    fn test_custom_response_parsing_full() {
        let json = r#"{
            "passed": false,
            "violations": [
                {
                    "category": "hate",
                    "severity": "high",
                    "confidence": 0.95,
                    "message": "Hate speech detected",
                    "span": {"start": 10, "end": 25}
                }
            ],
            "metadata": {"provider_version": "1.0"}
        }"#;

        let response: CustomResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.passed, Some(false));
        assert_eq!(response.violations.len(), 1);
        assert_eq!(response.violations[0].category, "hate");
        assert_eq!(response.violations[0].severity, Some("high".to_string()));
        assert!((response.violations[0].confidence.unwrap() - 0.95).abs() < f64::EPSILON);
        assert_eq!(
            response.violations[0].message,
            Some("Hate speech detected".to_string())
        );
        assert_eq!(
            response.violations[0].span.as_ref().unwrap().start,
            Some(10)
        );
        assert_eq!(response.violations[0].span.as_ref().unwrap().end, Some(25));
        assert!(response.metadata.is_some());
    }

    #[test]
    fn test_custom_response_parsing_minimal() {
        let json = r#"{
            "violations": [
                {"category": "violence"}
            ]
        }"#;

        let response: CustomResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.passed, None);
        assert_eq!(response.violations.len(), 1);
        assert_eq!(response.violations[0].category, "violence");
        assert!(response.violations[0].severity.is_none());
        assert!(response.violations[0].confidence.is_none());
        assert!(response.violations[0].message.is_none());
    }

    #[test]
    fn test_custom_response_parsing_passed() {
        let json = r#"{
            "passed": true,
            "violations": []
        }"#;

        let response: CustomResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.passed, Some(true));
        assert!(response.violations.is_empty());
    }

    #[test]
    fn test_supported_categories() {
        let client = Client::new();
        let provider = CustomHttpProvider::new(client, "https://example.com");
        let categories = provider.supported_categories();

        // Should return all standard categories
        assert!(categories.contains(&Category::Hate));
        assert!(categories.contains(&Category::Violence));
        assert!(categories.contains(&Category::PromptAttack));
    }

    #[test]
    fn test_with_timeout() {
        let client = Client::new();
        let provider = CustomHttpProvider::new(client, "https://example.com")
            .with_timeout(Duration::from_secs(10));

        assert_eq!(provider.timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_violation_conversion() {
        let json = r#"{
            "violations": [
                {
                    "category": "prompt_injection",
                    "severity": "critical",
                    "confidence": 0.99,
                    "message": "Prompt injection attempt detected",
                    "span": {"start": 0, "end": 50},
                    "details": {"pattern": "ignore all previous"}
                },
                {
                    "category": "pii_email",
                    "severity": "medium",
                    "confidence": 0.85,
                    "message": "Email address found"
                }
            ]
        }"#;

        let response: CustomResponse = serde_json::from_str(json).unwrap();

        let violations: Vec<Violation> = response
            .violations
            .into_iter()
            .map(|v| {
                let category = Category::from(v.category.as_str());
                let severity = v
                    .severity
                    .as_ref()
                    .map(|s| parse_severity(s))
                    .unwrap_or(Severity::Medium);

                let mut violation = Violation::new(category, severity, v.confidence.unwrap_or(1.0));

                if let Some(message) = v.message {
                    violation = violation.with_message(message);
                }

                violation
            })
            .collect();

        assert_eq!(violations.len(), 2);

        assert_eq!(violations[0].category, Category::PromptAttack);
        assert_eq!(violations[0].severity, Severity::Critical);
        assert!((violations[0].confidence - 0.99).abs() < f64::EPSILON);

        assert_eq!(violations[1].category, Category::PiiEmail);
        assert_eq!(violations[1].severity, Severity::Medium);
    }
}
