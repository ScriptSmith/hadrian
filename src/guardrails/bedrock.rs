//! AWS Bedrock Guardrails provider.
//!
//! This provider uses AWS Bedrock's ApplyGuardrail API to evaluate content against
//! configurable guardrail policies, including content filters, PII detection,
//! word filters, and topic filters.
//!
//! # Example Configuration
//!
//! ```toml
//! [features.guardrails.input.provider]
//! type = "bedrock"
//! guardrail_id = "abc123"
//! guardrail_version = "1"
//! region = "us-east-1"
//! trace_enabled = true
//! ```

use std::{collections::HashMap, time::Instant};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::{
    Category, ContentSource, GuardrailsError, GuardrailsProvider, GuardrailsRequest,
    GuardrailsResponse, GuardrailsResult, Severity, Violation, inject_trace_context,
};
use crate::{config::AwsCredentials, providers::aws::AwsRequestSigner};

/// Service name for AWS SigV4 signing.
const SERVICE_NAME: &str = "bedrock";

/// AWS Bedrock Guardrails provider.
///
/// Uses AWS Bedrock's ApplyGuardrail API to evaluate content against configured
/// guardrail policies.
pub struct BedrockGuardrailsProvider {
    client: Client,
    guardrail_id: String,
    guardrail_version: String,
    signer: AwsRequestSigner,
    trace_enabled: bool,
}

impl BedrockGuardrailsProvider {
    /// Creates a new Bedrock Guardrails provider.
    ///
    /// # Arguments
    /// * `client` - HTTP client to use for requests
    /// * `guardrail_id` - The Bedrock guardrail identifier
    /// * `guardrail_version` - The guardrail version
    /// * `region` - AWS region
    /// * `credential_source` - AWS credential source
    /// * `trace_enabled` - Whether to enable trace for debugging
    pub fn new(
        client: Client,
        guardrail_id: impl Into<String>,
        guardrail_version: impl Into<String>,
        region: impl Into<String>,
        credential_source: AwsCredentials,
        trace_enabled: bool,
    ) -> Self {
        let region = region.into();
        Self {
            client,
            guardrail_id: guardrail_id.into(),
            guardrail_version: guardrail_version.into(),
            signer: AwsRequestSigner::new(credential_source, region, SERVICE_NAME),
            trace_enabled,
        }
    }

    /// Creates provider from configuration.
    ///
    /// # Arguments
    /// * `client` - HTTP client to use for requests
    /// * `guardrail_id` - The Bedrock guardrail identifier
    /// * `guardrail_version` - The guardrail version
    /// * `region` - AWS region (uses default if None)
    /// * `access_key_id` - AWS access key ID (optional, uses default credentials if None)
    /// * `secret_access_key` - AWS secret access key (optional)
    /// * `trace_enabled` - Whether to enable trace for debugging
    /// * `default_region` - Default region to use if not specified
    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn from_config(
        client: Client,
        guardrail_id: String,
        guardrail_version: String,
        region: Option<String>,
        access_key_id: Option<String>,
        secret_access_key: Option<String>,
        trace_enabled: bool,
        default_region: Option<&str>,
    ) -> GuardrailsResult<Self> {
        let region = region
            .or_else(|| default_region.map(String::from))
            .ok_or_else(|| {
                GuardrailsError::config_error(
                    "Bedrock Guardrails requires a region. Set region in guardrails config or AWS_REGION environment variable."
                )
            })?;

        let credential_source = match (access_key_id, secret_access_key) {
            (Some(key_id), Some(secret)) => AwsCredentials::Static {
                access_key_id: key_id,
                secret_access_key: secret,
                session_token: None,
            },
            _ => AwsCredentials::Default,
        };

        Ok(Self::new(
            client,
            guardrail_id,
            guardrail_version,
            region,
            credential_source,
            trace_enabled,
        ))
    }

    /// Sign a request using AWS SigV4.
    async fn sign_request(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> GuardrailsResult<Vec<(String, String)>> {
        self.signer
            .sign_request(method, url, headers, body)
            .await
            .map_err(|e| GuardrailsError::auth_error("bedrock", e.to_string()))
    }

    /// Returns the AWS region.
    #[allow(dead_code)] // Guardrail infrastructure
    pub fn region(&self) -> &str {
        self.signer.region()
    }

    fn base_url(&self) -> String {
        format!(
            "https://bedrock-runtime.{}.amazonaws.com",
            self.signer.region()
        )
    }

    /// Extracts violations from Bedrock ApplyGuardrail response.
    fn extract_violations(&self, response: &ApplyGuardrailResponse) -> Vec<Violation> {
        let mut violations = Vec::new();

        for assessment in &response.assessments {
            // Content policy filters (HATE, INSULTS, SEXUAL, VIOLENCE, MISCONDUCT, PROMPT_ATTACK)
            if let Some(content_policy) = &assessment.content_policy {
                for filter in &content_policy.filters {
                    if filter.detected.unwrap_or(false)
                        && filter.action == Some("BLOCKED".to_string())
                    {
                        let category = map_bedrock_content_filter(&filter.filter_type);
                        let severity = map_bedrock_confidence(&filter.confidence);

                        violations.push(
                            Violation::new(
                                category.clone(),
                                severity,
                                confidence_to_score(&filter.confidence),
                            )
                            .with_message(format!(
                                "Content flagged for {} (confidence: {})",
                                filter.filter_type,
                                filter.confidence.as_deref().unwrap_or("unknown")
                            ))
                            .with_details(serde_json::json!({
                                "filter_type": filter.filter_type,
                                "confidence": filter.confidence,
                                "filter_strength": filter.filter_strength,
                                "action": filter.action,
                            })),
                        );
                    }
                }
            }

            // Word policy (custom words and managed word lists)
            if let Some(word_policy) = &assessment.word_policy {
                for word in &word_policy.custom_words {
                    if word.detected.unwrap_or(false) && word.action == Some("BLOCKED".to_string())
                    {
                        violations.push(
                            Violation::new(
                                Category::Custom("word_filter".to_string()),
                                Severity::High,
                                1.0,
                            )
                            .with_message(format!("Custom word detected: {}", word.match_text))
                            .with_details(serde_json::json!({
                                "match": word.match_text,
                                "action": word.action,
                            })),
                        );
                    }
                }

                for word in &word_policy.managed_word_lists {
                    if word.detected.unwrap_or(false) && word.action == Some("BLOCKED".to_string())
                    {
                        violations.push(
                            Violation::new(
                                Category::Custom("managed_word_filter".to_string()),
                                Severity::High,
                                1.0,
                            )
                            .with_message(format!("Managed word list match: {}", word.match_text))
                            .with_details(serde_json::json!({
                                "match": word.match_text,
                                "type": word.word_type,
                                "action": word.action,
                            })),
                        );
                    }
                }
            }

            // Topic policy
            if let Some(topic_policy) = &assessment.topic_policy {
                for topic in &topic_policy.topics {
                    if topic.detected.unwrap_or(false)
                        && topic.action == Some("BLOCKED".to_string())
                    {
                        violations.push(
                            Violation::new(Category::OffTopic, Severity::High, 1.0)
                                .with_message(format!("Topic violation: {}", topic.name))
                                .with_details(serde_json::json!({
                                    "name": topic.name,
                                    "type": topic.topic_type,
                                    "action": topic.action,
                                })),
                        );
                    }
                }
            }

            // Sensitive information policy (PII)
            if let Some(sensitive_info) = &assessment.sensitive_information_policy {
                for pii in &sensitive_info.pii_entities {
                    if pii.detected.unwrap_or(false) && pii.action == Some("BLOCKED".to_string()) {
                        let category = map_bedrock_pii_type(&pii.pii_type);

                        violations.push(
                            Violation::new(category, Severity::High, 1.0)
                                .with_message(format!(
                                    "PII detected: {} ({})",
                                    pii.pii_type, pii.match_text
                                ))
                                .with_details(serde_json::json!({
                                    "type": pii.pii_type,
                                    "match": pii.match_text,
                                    "action": pii.action,
                                })),
                        );
                    }
                }

                for regex in &sensitive_info.regexes {
                    if regex.detected.unwrap_or(false)
                        && regex.action == Some("BLOCKED".to_string())
                    {
                        violations.push(
                            Violation::new(Category::PiiOther, Severity::High, 1.0)
                                .with_message(format!("Regex pattern match: {}", regex.name))
                                .with_details(serde_json::json!({
                                    "name": regex.name,
                                    "regex": regex.regex,
                                    "match": regex.match_text,
                                    "action": regex.action,
                                })),
                        );
                    }
                }
            }
        }

        violations
    }
}

#[async_trait]
impl GuardrailsProvider for BedrockGuardrailsProvider {
    fn name(&self) -> &str {
        "bedrock"
    }

    #[instrument(
        skip(self, request),
        fields(
            provider = "bedrock",
            guardrail_id = %self.guardrail_id,
            guardrail_version = %self.guardrail_version,
            text_length = request.text.len()
        )
    )]
    async fn evaluate(&self, request: &GuardrailsRequest) -> GuardrailsResult<GuardrailsResponse> {
        let start = Instant::now();

        // Build the ApplyGuardrail request
        let source = match request.source {
            ContentSource::UserInput | ContentSource::System => "INPUT",
            ContentSource::LlmOutput => "OUTPUT",
        };

        let api_request = ApplyGuardrailRequest {
            source: source.to_string(),
            content: vec![GuardrailContentBlock {
                text: Some(GuardrailTextBlock {
                    text: request.text.clone(),
                    qualifiers: None,
                }),
                image: None,
            }],
            output_scope: if self.trace_enabled {
                Some("FULL".to_string())
            } else {
                Some("INTERVENTIONS".to_string())
            },
        };

        let body = serde_json::to_vec(&api_request).map_err(|e| {
            GuardrailsError::internal(format!("Failed to serialize request: {}", e))
        })?;

        let url = format!(
            "{}/guardrail/{}/version/{}/apply",
            self.base_url(),
            self.guardrail_id,
            self.guardrail_version
        );

        // Sign the request
        let headers = [("content-type", "application/json")];
        let signed_headers = self.sign_request("POST", &url, &headers, &body).await?;

        let mut req = self
            .client
            .post(&url)
            .header("content-type", "application/json");

        for (name, value) in signed_headers {
            req = req.header(name, value);
        }

        // Inject trace context for distributed tracing (after signing since these don't need to be signed)
        let mut trace_headers = HashMap::new();
        inject_trace_context(&mut trace_headers);
        for (key, value) in trace_headers {
            req = req.header(key, value);
        }

        let response = req
            .body(body)
            .send()
            .await
            .map_err(|e| GuardrailsError::from_reqwest("bedrock", e))?;

        let status = response.status();

        // Handle error status codes
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(GuardrailsError::auth_error(
                "bedrock",
                "Invalid AWS credentials or insufficient permissions",
            ));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok());
            return Err(GuardrailsError::rate_limited("bedrock", retry_after));
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(GuardrailsError::config_error(format!(
                "Guardrail not found: {} version {}",
                self.guardrail_id, self.guardrail_version
            )));
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(GuardrailsError::provider_error(
                "bedrock",
                format!("API returned {}: {}", status, error_text),
            ));
        }

        let api_response: ApplyGuardrailResponse = response.json().await.map_err(|e| {
            GuardrailsError::provider_error("bedrock", format!("Failed to parse response: {}", e))
        })?;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Check if the guardrail intervened
        let violated = api_response.action == "GUARDRAIL_INTERVENED";

        // Extract violations from assessments
        let violations = if violated {
            self.extract_violations(&api_response)
        } else {
            Vec::new()
        };

        Ok(GuardrailsResponse::with_violations(violations)
            .with_latency(latency_ms)
            .with_metadata(serde_json::json!({
                "action": api_response.action,
                "action_reason": api_response.action_reason,
                "usage": api_response.usage,
                "guardrail_id": self.guardrail_id,
                "guardrail_version": self.guardrail_version,
            })))
    }

    fn supported_categories(&self) -> &[Category] {
        &[
            Category::Hate,
            Category::Harassment, // INSULTS maps to this
            Category::Sexual,
            Category::Violence,
            Category::Dangerous, // MISCONDUCT maps to this
            Category::PromptAttack,
            // PII categories
            Category::PiiEmail,
            Category::PiiPhone,
            Category::PiiSsn,
            Category::PiiCreditCard,
            Category::PiiAddress,
            Category::PiiName,
            Category::PiiOther,
            // Topic filtering
            Category::OffTopic,
        ]
    }
}

/// Maps Bedrock content filter types to standard categories.
fn map_bedrock_content_filter(filter_type: &str) -> Category {
    match filter_type.to_uppercase().as_str() {
        "HATE" => Category::Hate,
        "INSULTS" => Category::Harassment,
        "SEXUAL" => Category::Sexual,
        "VIOLENCE" => Category::Violence,
        "MISCONDUCT" => Category::Dangerous,
        "PROMPT_ATTACK" => Category::PromptAttack,
        other => Category::Custom(other.to_string()),
    }
}

/// Maps Bedrock PII types to standard categories.
fn map_bedrock_pii_type(pii_type: &str) -> Category {
    match pii_type.to_uppercase().as_str() {
        "EMAIL" => Category::PiiEmail,
        "PHONE" => Category::PiiPhone,
        "SSN" | "US_SOCIAL_SECURITY_NUMBER" => Category::PiiSsn,
        "CREDIT_DEBIT_CARD_NUMBER" | "CREDIT_CARD" => Category::PiiCreditCard,
        "ADDRESS" => Category::PiiAddress,
        "NAME" => Category::PiiName,
        _ => Category::PiiOther,
    }
}

/// Maps Bedrock confidence levels to severity.
fn map_bedrock_confidence(confidence: &Option<String>) -> Severity {
    match confidence.as_deref() {
        Some("NONE") => Severity::Info,
        Some("LOW") => Severity::Low,
        Some("MEDIUM") => Severity::Medium,
        Some("HIGH") => Severity::High,
        _ => Severity::Medium, // Default to medium if unknown
    }
}

/// Converts Bedrock confidence level to a numeric score.
fn confidence_to_score(confidence: &Option<String>) -> f64 {
    match confidence.as_deref() {
        Some("NONE") => 0.0,
        Some("LOW") => 0.33,
        Some("MEDIUM") => 0.66,
        Some("HIGH") => 1.0,
        _ => 0.5,
    }
}

// ============================================================================
// Bedrock ApplyGuardrail API Types
// ============================================================================

/// Request body for ApplyGuardrail API.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyGuardrailRequest {
    /// Source of the content: INPUT or OUTPUT.
    source: String,

    /// Content blocks to evaluate.
    content: Vec<GuardrailContentBlock>,

    /// Output scope: INTERVENTIONS or FULL.
    #[serde(skip_serializing_if = "Option::is_none")]
    output_scope: Option<String>,
}

/// Content block for guardrail evaluation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuardrailContentBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<GuardrailTextBlock>,

    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<serde_json::Value>, // Image support for future
}

/// Text block for guardrail evaluation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuardrailTextBlock {
    text: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    qualifiers: Option<Vec<String>>,
}

/// Response from ApplyGuardrail API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyGuardrailResponse {
    /// Action taken: NONE or GUARDRAIL_INTERVENED.
    action: String,

    /// Reason for the action.
    #[serde(default)]
    action_reason: Option<String>,

    /// Assessment results.
    #[serde(default)]
    assessments: Vec<GuardrailAssessment>,

    /// Output text (if modified).
    #[serde(default)]
    #[allow(dead_code)] // Guardrail infrastructure
    outputs: Vec<GuardrailOutput>,

    /// Usage information.
    #[serde(default)]
    usage: Option<GuardrailUsage>,
}

/// Output block from guardrail (for modified content).
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Guardrail infrastructure
struct GuardrailOutput {
    text: Option<String>,
}

/// Usage information from guardrail evaluation.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuardrailUsage {
    #[serde(default)]
    topic_policy_units: Option<i64>,
    #[serde(default)]
    content_policy_units: Option<i64>,
    #[serde(default)]
    word_policy_units: Option<i64>,
    #[serde(default)]
    sensitive_information_policy_units: Option<i64>,
    #[serde(default)]
    contextual_grounding_policy_units: Option<i64>,
}

/// Assessment from guardrail evaluation.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuardrailAssessment {
    #[serde(default)]
    content_policy: Option<ContentPolicyAssessment>,

    #[serde(default)]
    word_policy: Option<WordPolicyAssessment>,

    #[serde(default)]
    topic_policy: Option<TopicPolicyAssessment>,

    #[serde(default)]
    sensitive_information_policy: Option<SensitiveInfoPolicyAssessment>,
}

/// Content policy assessment (content filters).
#[derive(Debug, Deserialize)]
struct ContentPolicyAssessment {
    #[serde(default)]
    filters: Vec<ContentFilter>,
}

/// Content filter result.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentFilter {
    /// Filter type: HATE, INSULTS, SEXUAL, VIOLENCE, MISCONDUCT, PROMPT_ATTACK.
    #[serde(rename = "type")]
    filter_type: String,

    /// Confidence level: NONE, LOW, MEDIUM, HIGH.
    #[serde(default)]
    confidence: Option<String>,

    /// Filter strength: NONE, LOW, MEDIUM, HIGH.
    #[serde(default)]
    filter_strength: Option<String>,

    /// Action taken: NONE, BLOCKED.
    #[serde(default)]
    action: Option<String>,

    /// Whether the filter detected a violation.
    #[serde(default)]
    detected: Option<bool>,
}

/// Word policy assessment.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WordPolicyAssessment {
    #[serde(default)]
    custom_words: Vec<WordMatch>,

    #[serde(default)]
    managed_word_lists: Vec<ManagedWordMatch>,
}

/// Custom word match result.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WordMatch {
    /// The matched text.
    #[serde(rename = "match")]
    match_text: String,

    /// Action taken: NONE, BLOCKED.
    #[serde(default)]
    action: Option<String>,

    /// Whether detected.
    #[serde(default)]
    detected: Option<bool>,
}

/// Managed word list match result.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedWordMatch {
    /// The matched text.
    #[serde(rename = "match")]
    match_text: String,

    /// Word list type.
    #[serde(rename = "type")]
    #[serde(default)]
    word_type: Option<String>,

    /// Action taken: NONE, BLOCKED.
    #[serde(default)]
    action: Option<String>,

    /// Whether detected.
    #[serde(default)]
    detected: Option<bool>,
}

/// Topic policy assessment.
#[derive(Debug, Deserialize)]
struct TopicPolicyAssessment {
    #[serde(default)]
    topics: Vec<TopicMatch>,
}

/// Topic match result.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TopicMatch {
    /// Topic name.
    name: String,

    /// Topic type: DENY.
    #[serde(rename = "type")]
    #[serde(default)]
    topic_type: Option<String>,

    /// Action taken: NONE, BLOCKED.
    #[serde(default)]
    action: Option<String>,

    /// Whether detected.
    #[serde(default)]
    detected: Option<bool>,
}

/// Sensitive information policy assessment (PII).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SensitiveInfoPolicyAssessment {
    #[serde(default)]
    pii_entities: Vec<PiiEntity>,

    #[serde(default)]
    regexes: Vec<RegexMatch>,
}

/// PII entity detection result.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiiEntity {
    /// PII type (EMAIL, PHONE, SSN, etc.).
    #[serde(rename = "type")]
    pii_type: String,

    /// The matched text.
    #[serde(rename = "match")]
    match_text: String,

    /// Action taken: NONE, BLOCKED, ANONYMIZED.
    #[serde(default)]
    action: Option<String>,

    /// Whether detected.
    #[serde(default)]
    detected: Option<bool>,
}

/// Custom regex match result.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegexMatch {
    /// Regex name.
    name: String,

    /// The regex pattern.
    #[serde(default)]
    regex: Option<String>,

    /// The matched text.
    #[serde(rename = "match")]
    match_text: String,

    /// Action taken: NONE, BLOCKED, ANONYMIZED.
    #[serde(default)]
    action: Option<String>,

    /// Whether detected.
    #[serde(default)]
    detected: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_bedrock_content_filter() {
        assert_eq!(map_bedrock_content_filter("HATE"), Category::Hate);
        assert_eq!(map_bedrock_content_filter("hate"), Category::Hate);
        assert_eq!(map_bedrock_content_filter("INSULTS"), Category::Harassment);
        assert_eq!(map_bedrock_content_filter("SEXUAL"), Category::Sexual);
        assert_eq!(map_bedrock_content_filter("VIOLENCE"), Category::Violence);
        assert_eq!(
            map_bedrock_content_filter("MISCONDUCT"),
            Category::Dangerous
        );
        assert_eq!(
            map_bedrock_content_filter("PROMPT_ATTACK"),
            Category::PromptAttack
        );
        assert_eq!(
            map_bedrock_content_filter("UNKNOWN"),
            Category::Custom("UNKNOWN".to_string())
        );
    }

    #[test]
    fn test_map_bedrock_pii_type() {
        assert_eq!(map_bedrock_pii_type("EMAIL"), Category::PiiEmail);
        assert_eq!(map_bedrock_pii_type("PHONE"), Category::PiiPhone);
        assert_eq!(map_bedrock_pii_type("SSN"), Category::PiiSsn);
        assert_eq!(
            map_bedrock_pii_type("US_SOCIAL_SECURITY_NUMBER"),
            Category::PiiSsn
        );
        assert_eq!(
            map_bedrock_pii_type("CREDIT_DEBIT_CARD_NUMBER"),
            Category::PiiCreditCard
        );
        assert_eq!(map_bedrock_pii_type("ADDRESS"), Category::PiiAddress);
        assert_eq!(map_bedrock_pii_type("NAME"), Category::PiiName);
        assert_eq!(map_bedrock_pii_type("OTHER"), Category::PiiOther);
    }

    #[test]
    fn test_map_bedrock_confidence() {
        assert_eq!(
            map_bedrock_confidence(&Some("NONE".to_string())),
            Severity::Info
        );
        assert_eq!(
            map_bedrock_confidence(&Some("LOW".to_string())),
            Severity::Low
        );
        assert_eq!(
            map_bedrock_confidence(&Some("MEDIUM".to_string())),
            Severity::Medium
        );
        assert_eq!(
            map_bedrock_confidence(&Some("HIGH".to_string())),
            Severity::High
        );
        assert_eq!(map_bedrock_confidence(&None), Severity::Medium);
    }

    #[test]
    fn test_confidence_to_score() {
        assert!((confidence_to_score(&Some("NONE".to_string())) - 0.0).abs() < f64::EPSILON);
        assert!((confidence_to_score(&Some("LOW".to_string())) - 0.33).abs() < f64::EPSILON);
        assert!((confidence_to_score(&Some("MEDIUM".to_string())) - 0.66).abs() < f64::EPSILON);
        assert!((confidence_to_score(&Some("HIGH".to_string())) - 1.0).abs() < f64::EPSILON);
        assert!((confidence_to_score(&None) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_provider_name() {
        let client = Client::new();
        let provider = BedrockGuardrailsProvider::new(
            client,
            "test-guardrail",
            "1",
            "us-east-1",
            AwsCredentials::Default,
            false,
        );
        assert_eq!(provider.name(), "bedrock");
    }

    #[test]
    fn test_supported_categories() {
        let client = Client::new();
        let provider = BedrockGuardrailsProvider::new(
            client,
            "test-guardrail",
            "1",
            "us-east-1",
            AwsCredentials::Default,
            false,
        );
        let categories = provider.supported_categories();

        assert!(categories.contains(&Category::Hate));
        assert!(categories.contains(&Category::Harassment));
        assert!(categories.contains(&Category::Sexual));
        assert!(categories.contains(&Category::Violence));
        assert!(categories.contains(&Category::Dangerous));
        assert!(categories.contains(&Category::PromptAttack));
        assert!(categories.contains(&Category::PiiEmail));
        assert!(categories.contains(&Category::PiiPhone));
        assert!(categories.contains(&Category::OffTopic));
    }

    #[test]
    fn test_from_config_with_credentials() {
        let client = Client::new();
        let result = BedrockGuardrailsProvider::from_config(
            client,
            "test-guardrail".to_string(),
            "1".to_string(),
            Some("us-west-2".to_string()),
            Some("AKIAIOSFODNN7EXAMPLE".to_string()),
            Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string()),
            true,
            None,
        );

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.guardrail_id, "test-guardrail");
        assert_eq!(provider.guardrail_version, "1");
        assert_eq!(provider.region(), "us-west-2");
        assert!(provider.trace_enabled);
    }

    #[test]
    fn test_from_config_with_default_region() {
        let client = Client::new();
        let result = BedrockGuardrailsProvider::from_config(
            client,
            "test-guardrail".to_string(),
            "1".to_string(),
            None,
            None,
            None,
            false,
            Some("eu-west-1"),
        );

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.region(), "eu-west-1");
    }

    #[test]
    fn test_from_config_no_region() {
        let client = Client::new();
        let result = BedrockGuardrailsProvider::from_config(
            client,
            "test-guardrail".to_string(),
            "1".to_string(),
            None,
            None,
            None,
            false,
            None,
        );

        assert!(result.is_err());
        match result {
            Err(GuardrailsError::ConfigError { message }) => {
                assert!(message.contains("region"));
            }
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_base_url() {
        let client = Client::new();
        let provider = BedrockGuardrailsProvider::new(
            client,
            "test-guardrail",
            "1",
            "us-east-1",
            AwsCredentials::Default,
            false,
        );
        assert_eq!(
            provider.base_url(),
            "https://bedrock-runtime.us-east-1.amazonaws.com"
        );
    }

    #[test]
    fn test_extract_violations_no_intervention() {
        let client = Client::new();
        let provider = BedrockGuardrailsProvider::new(
            client,
            "test-guardrail",
            "1",
            "us-east-1",
            AwsCredentials::Default,
            false,
        );

        let response = ApplyGuardrailResponse {
            action: "NONE".to_string(),
            action_reason: None,
            assessments: vec![],
            outputs: vec![],
            usage: None,
        };

        let violations = provider.extract_violations(&response);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_extract_violations_content_filter() {
        let client = Client::new();
        let provider = BedrockGuardrailsProvider::new(
            client,
            "test-guardrail",
            "1",
            "us-east-1",
            AwsCredentials::Default,
            false,
        );

        let response = ApplyGuardrailResponse {
            action: "GUARDRAIL_INTERVENED".to_string(),
            action_reason: Some("Content filtered".to_string()),
            assessments: vec![GuardrailAssessment {
                content_policy: Some(ContentPolicyAssessment {
                    filters: vec![ContentFilter {
                        filter_type: "HATE".to_string(),
                        confidence: Some("HIGH".to_string()),
                        filter_strength: Some("HIGH".to_string()),
                        action: Some("BLOCKED".to_string()),
                        detected: Some(true),
                    }],
                }),
                word_policy: None,
                topic_policy: None,
                sensitive_information_policy: None,
            }],
            outputs: vec![],
            usage: None,
        };

        let violations = provider.extract_violations(&response);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].category, Category::Hate);
        assert_eq!(violations[0].severity, Severity::High);
    }

    #[test]
    fn test_extract_violations_pii() {
        let client = Client::new();
        let provider = BedrockGuardrailsProvider::new(
            client,
            "test-guardrail",
            "1",
            "us-east-1",
            AwsCredentials::Default,
            false,
        );

        let response = ApplyGuardrailResponse {
            action: "GUARDRAIL_INTERVENED".to_string(),
            action_reason: Some("PII detected".to_string()),
            assessments: vec![GuardrailAssessment {
                content_policy: None,
                word_policy: None,
                topic_policy: None,
                sensitive_information_policy: Some(SensitiveInfoPolicyAssessment {
                    pii_entities: vec![PiiEntity {
                        pii_type: "EMAIL".to_string(),
                        match_text: "test@example.com".to_string(),
                        action: Some("BLOCKED".to_string()),
                        detected: Some(true),
                    }],
                    regexes: vec![],
                }),
            }],
            outputs: vec![],
            usage: None,
        };

        let violations = provider.extract_violations(&response);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].category, Category::PiiEmail);
    }

    #[test]
    fn test_extract_violations_multiple() {
        let client = Client::new();
        let provider = BedrockGuardrailsProvider::new(
            client,
            "test-guardrail",
            "1",
            "us-east-1",
            AwsCredentials::Default,
            false,
        );

        let response = ApplyGuardrailResponse {
            action: "GUARDRAIL_INTERVENED".to_string(),
            action_reason: Some("Multiple violations".to_string()),
            assessments: vec![GuardrailAssessment {
                content_policy: Some(ContentPolicyAssessment {
                    filters: vec![
                        ContentFilter {
                            filter_type: "HATE".to_string(),
                            confidence: Some("HIGH".to_string()),
                            filter_strength: Some("HIGH".to_string()),
                            action: Some("BLOCKED".to_string()),
                            detected: Some(true),
                        },
                        ContentFilter {
                            filter_type: "VIOLENCE".to_string(),
                            confidence: Some("MEDIUM".to_string()),
                            filter_strength: Some("MEDIUM".to_string()),
                            action: Some("BLOCKED".to_string()),
                            detected: Some(true),
                        },
                    ],
                }),
                word_policy: None,
                topic_policy: None,
                sensitive_information_policy: None,
            }],
            outputs: vec![],
            usage: None,
        };

        let violations = provider.extract_violations(&response);
        assert_eq!(violations.len(), 2);
        assert!(violations.iter().any(|v| v.category == Category::Hate));
        assert!(violations.iter().any(|v| v.category == Category::Violence));
    }

    #[test]
    fn test_apply_guardrail_request_serialization() {
        let request = ApplyGuardrailRequest {
            source: "INPUT".to_string(),
            content: vec![GuardrailContentBlock {
                text: Some(GuardrailTextBlock {
                    text: "Hello, world!".to_string(),
                    qualifiers: None,
                }),
                image: None,
            }],
            output_scope: Some("INTERVENTIONS".to_string()),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"source\":\"INPUT\""));
        assert!(json.contains("\"text\":\"Hello, world!\""));
        assert!(json.contains("\"outputScope\":\"INTERVENTIONS\""));
    }

    #[test]
    fn test_apply_guardrail_response_parsing() {
        let json = r#"{
            "action": "GUARDRAIL_INTERVENED",
            "actionReason": "Content blocked",
            "assessments": [{
                "contentPolicy": {
                    "filters": [{
                        "type": "HATE",
                        "confidence": "HIGH",
                        "filterStrength": "HIGH",
                        "action": "BLOCKED",
                        "detected": true
                    }]
                }
            }],
            "outputs": [],
            "usage": {
                "contentPolicyUnits": 1
            }
        }"#;

        let response: ApplyGuardrailResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.action, "GUARDRAIL_INTERVENED");
        assert_eq!(response.action_reason, Some("Content blocked".to_string()));
        assert_eq!(response.assessments.len(), 1);

        let content_policy = response.assessments[0].content_policy.as_ref().unwrap();
        assert_eq!(content_policy.filters.len(), 1);
        assert_eq!(content_policy.filters[0].filter_type, "HATE");
        assert_eq!(
            content_policy.filters[0].confidence,
            Some("HIGH".to_string())
        );
    }

    #[test]
    fn test_apply_guardrail_response_parsing_none() {
        let json = r#"{
            "action": "NONE",
            "assessments": [],
            "outputs": []
        }"#;

        let response: ApplyGuardrailResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.action, "NONE");
        assert!(response.action_reason.is_none());
        assert!(response.assessments.is_empty());
    }
}
