//! Core types for guardrails evaluation.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Request for guardrails evaluation.
#[derive(Debug, Clone)]
pub struct GuardrailsRequest {
    /// The source of the content being evaluated.
    pub source: ContentSource,

    /// Text content to evaluate.
    pub text: String,

    /// Optional request ID for correlation.
    pub request_id: Option<String>,

    /// Optional user/API key identifier for audit logging.
    pub user_id: Option<String>,

    /// Additional context for the evaluation (provider-specific).
    pub context: Option<serde_json::Value>,
}

impl GuardrailsRequest {
    /// Creates a new guardrails request.
    pub fn new(source: ContentSource, text: impl Into<String>) -> Self {
        Self {
            source,
            text: text.into(),
            request_id: None,
            user_id: None,
            context: None,
        }
    }

    /// Creates a request for evaluating user input.
    pub fn user_input(text: impl Into<String>) -> Self {
        Self::new(ContentSource::UserInput, text)
    }

    /// Creates a request for evaluating LLM output.
    pub fn llm_output(text: impl Into<String>) -> Self {
        Self::new(ContentSource::LlmOutput, text)
    }

    /// Sets the request ID for correlation.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Sets the user ID for audit logging.
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Sets additional context for the evaluation.
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = Some(context);
        self
    }
}

/// Source of the content being evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentSource {
    /// User input (pre-request evaluation).
    UserInput,
    /// LLM output (post-response evaluation).
    LlmOutput,
    /// System prompt or other internal content.
    System,
}

impl fmt::Display for ContentSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentSource::UserInput => write!(f, "user_input"),
            ContentSource::LlmOutput => write!(f, "llm_output"),
            ContentSource::System => write!(f, "system"),
        }
    }
}

/// Response from guardrails evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailsResponse {
    /// Whether the content passed evaluation (no violations).
    pub passed: bool,

    /// List of violations found (empty if passed).
    pub violations: Vec<Violation>,

    /// Provider-specific metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_metadata: Option<serde_json::Value>,

    /// Evaluation latency in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

impl GuardrailsResponse {
    /// Creates a response indicating content passed evaluation.
    pub fn passed() -> Self {
        Self {
            passed: true,
            violations: Vec::new(),
            provider_metadata: None,
            latency_ms: None,
        }
    }

    /// Creates a response with violations.
    pub fn with_violations(violations: Vec<Violation>) -> Self {
        Self {
            passed: violations.is_empty(),
            violations,
            provider_metadata: None,
            latency_ms: None,
        }
    }

    /// Sets provider-specific metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.provider_metadata = Some(metadata);
        self
    }

    /// Sets evaluation latency.
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    /// Returns the highest severity violation, if any.
    pub fn highest_severity(&self) -> Option<Severity> {
        self.violations
            .iter()
            .map(|v| v.severity)
            .max_by_key(|s| s.level())
    }
}

/// A policy violation detected by guardrails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    /// Category of the violation.
    pub category: Category,

    /// Severity level.
    pub severity: Severity,

    /// Confidence score (0.0 to 1.0).
    pub confidence: f64,

    /// Human-readable message describing the violation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Character span of the violation in the original text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,

    /// Provider-specific details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_details: Option<serde_json::Value>,
}

impl Violation {
    /// Creates a new violation.
    pub fn new(category: Category, severity: Severity, confidence: f64) -> Self {
        Self {
            category,
            severity,
            confidence,
            message: None,
            span: None,
            provider_details: None,
        }
    }

    /// Sets the violation message.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Sets the character span.
    pub fn with_span(mut self, start: usize, end: usize) -> Self {
        self.span = Some(Span { start, end });
        self
    }

    /// Sets provider-specific details.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.provider_details = Some(details);
        self
    }
}

/// Character span in the original text.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Span {
    /// Start character index (inclusive).
    pub start: usize,
    /// End character index (exclusive).
    pub end: usize,
}

/// Violation category.
///
/// These categories are normalized across providers. Provider-specific categories
/// are mapped to these common categories during evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    // Content safety categories
    /// Hate speech, discrimination, slurs.
    Hate,
    /// Harassment, bullying, threats against individuals.
    Harassment,
    /// Self-harm instructions or glorification.
    SelfHarm,
    /// Sexual content.
    Sexual,
    /// Violence, gore, graphic content.
    Violence,
    /// Dangerous or illegal activities.
    Dangerous,

    // Security categories
    /// Jailbreak attempts, prompt injection.
    PromptAttack,
    /// Attempts to extract system prompts.
    PromptLeakage,
    /// Malicious code or malware.
    MaliciousCode,

    // PII categories
    /// Email addresses.
    PiiEmail,
    /// Phone numbers.
    PiiPhone,
    /// Social security numbers.
    PiiSsn,
    /// Credit card numbers.
    PiiCreditCard,
    /// Physical addresses.
    PiiAddress,
    /// Personal names.
    PiiName,
    /// Other PII types.
    PiiOther,

    // Business policy categories
    /// Off-topic content (topic filter violation).
    OffTopic,
    /// Competitor mentions.
    CompetitorMention,
    /// Confidential information.
    Confidential,

    /// Unknown or unmapped category.
    Unknown,

    // Catch-all for provider-specific categories - must be last for serde(untagged)
    /// Custom category with provider-specific name.
    #[serde(untagged)]
    Custom(String),
}

impl Category {
    /// Returns all standard (non-custom) categories.
    pub fn all_standard() -> &'static [Category] {
        &[
            Category::Hate,
            Category::Harassment,
            Category::SelfHarm,
            Category::Sexual,
            Category::Violence,
            Category::Dangerous,
            Category::PromptAttack,
            Category::PromptLeakage,
            Category::MaliciousCode,
            Category::PiiEmail,
            Category::PiiPhone,
            Category::PiiSsn,
            Category::PiiCreditCard,
            Category::PiiAddress,
            Category::PiiName,
            Category::PiiOther,
            Category::OffTopic,
            Category::CompetitorMention,
            Category::Confidential,
        ]
    }

    /// Returns true if this is a PII category.
    pub fn is_pii(&self) -> bool {
        matches!(
            self,
            Category::PiiEmail
                | Category::PiiPhone
                | Category::PiiSsn
                | Category::PiiCreditCard
                | Category::PiiAddress
                | Category::PiiName
                | Category::PiiOther
        )
    }

    /// Returns true if this is a security-related category.
    pub fn is_security(&self) -> bool {
        matches!(
            self,
            Category::PromptAttack | Category::PromptLeakage | Category::MaliciousCode
        )
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Category::Hate => write!(f, "hate"),
            Category::Harassment => write!(f, "harassment"),
            Category::SelfHarm => write!(f, "self_harm"),
            Category::Sexual => write!(f, "sexual"),
            Category::Violence => write!(f, "violence"),
            Category::Dangerous => write!(f, "dangerous"),
            Category::PromptAttack => write!(f, "prompt_attack"),
            Category::PromptLeakage => write!(f, "prompt_leakage"),
            Category::MaliciousCode => write!(f, "malicious_code"),
            Category::PiiEmail => write!(f, "pii_email"),
            Category::PiiPhone => write!(f, "pii_phone"),
            Category::PiiSsn => write!(f, "pii_ssn"),
            Category::PiiCreditCard => write!(f, "pii_credit_card"),
            Category::PiiAddress => write!(f, "pii_address"),
            Category::PiiName => write!(f, "pii_name"),
            Category::PiiOther => write!(f, "pii_other"),
            Category::OffTopic => write!(f, "off_topic"),
            Category::CompetitorMention => write!(f, "competitor_mention"),
            Category::Confidential => write!(f, "confidential"),
            Category::Custom(s) => write!(f, "{}", s),
            Category::Unknown => write!(f, "unknown"),
        }
    }
}

impl From<&str> for Category {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "hate" | "hate_speech" | "hate/threatening" => Category::Hate,
            "harassment" | "harassment/threatening" => Category::Harassment,
            "self_harm" | "self-harm" | "self_harm/intent" | "self_harm/instructions" => {
                Category::SelfHarm
            }
            "sexual" | "sexual/minors" => Category::Sexual,
            "violence" | "violence/graphic" => Category::Violence,
            "dangerous" | "dangerous_content" | "misconduct" => Category::Dangerous,
            "prompt_attack" | "jailbreak" | "prompt_injection" => Category::PromptAttack,
            "prompt_leakage" | "prompt_extraction" => Category::PromptLeakage,
            "malicious_code" | "malware" => Category::MaliciousCode,
            "pii_email" | "email" => Category::PiiEmail,
            "pii_phone" | "phone" => Category::PiiPhone,
            "pii_ssn" | "ssn" => Category::PiiSsn,
            "pii_credit_card" | "credit_card" => Category::PiiCreditCard,
            "pii_address" | "address" => Category::PiiAddress,
            "pii_name" | "name" => Category::PiiName,
            "pii" | "pii_other" => Category::PiiOther,
            "off_topic" | "topic" => Category::OffTopic,
            "competitor" | "competitor_mention" => Category::CompetitorMention,
            "confidential" | "confidential_info" => Category::Confidential,
            other => Category::Custom(other.to_string()),
        }
    }
}

/// Severity level of a violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational - no action typically required.
    Info,
    /// Low severity - may warrant logging.
    Low,
    /// Medium severity - may warrant warning.
    Medium,
    /// High severity - typically requires action.
    High,
    /// Critical severity - immediate action required.
    Critical,
}

impl Severity {
    /// Returns a numeric level for comparison (higher = more severe).
    pub fn level(&self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Low => 1,
            Severity::Medium => 2,
            Severity::High => 3,
            Severity::Critical => 4,
        }
    }

    /// Converts from a numeric threshold (0-6 scale used by Azure).
    pub fn from_azure_threshold(threshold: u8) -> Self {
        match threshold {
            0 => Severity::Info,
            1..=2 => Severity::Low,
            3..=4 => Severity::Medium,
            5 => Severity::High,
            _ => Severity::Critical,
        }
    }

    /// Converts from a score (0.0-1.0 scale used by OpenAI).
    pub fn from_score(score: f64) -> Self {
        if score < 0.2 {
            Severity::Low
        } else if score < 0.5 {
            Severity::Medium
        } else if score < 0.8 {
            Severity::High
        } else {
            Severity::Critical
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Low => write!(f, "low"),
            Severity::Medium => write!(f, "medium"),
            Severity::High => write!(f, "high"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

/// Resolved action to take based on guardrails evaluation.
///
/// This is the final action determined by the `ActionExecutor` after
/// applying configured action mappings to the violations found.
#[derive(Debug, Clone)]
pub enum ResolvedAction {
    /// Content passed - allow through.
    Allow,

    /// Content should be blocked.
    Block {
        /// Reason for blocking.
        reason: String,
        /// Violations that triggered the block.
        violations: Vec<Violation>,
    },

    /// Content allowed but warnings should be added.
    Warn {
        /// Violations to include in warning headers.
        violations: Vec<Violation>,
    },

    /// Content should be logged but allowed.
    Log {
        /// Violations to log.
        violations: Vec<Violation>,
    },

    /// Content should be redacted/modified.
    Redact {
        /// Original content before redaction.
        original_content: String,
        /// Content after redaction.
        modified_content: String,
        /// Violations that triggered redaction.
        violations: Vec<Violation>,
    },
}

impl ResolvedAction {
    /// Returns true if content should be blocked.
    pub fn is_blocked(&self) -> bool {
        matches!(self, ResolvedAction::Block { .. })
    }

    /// Returns true if content was modified.
    pub fn is_modified(&self) -> bool {
        matches!(self, ResolvedAction::Redact { .. })
    }

    /// Returns the violations associated with this action.
    pub fn violations(&self) -> &[Violation] {
        match self {
            ResolvedAction::Allow => &[],
            ResolvedAction::Block { violations, .. } => violations,
            ResolvedAction::Warn { violations } => violations,
            ResolvedAction::Log { violations } => violations,
            ResolvedAction::Redact { violations, .. } => violations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guardrails_request_builder() {
        let request = GuardrailsRequest::user_input("Hello world")
            .with_request_id("req-123")
            .with_user_id("user-456");

        assert_eq!(request.source, ContentSource::UserInput);
        assert_eq!(request.text, "Hello world");
        assert_eq!(request.request_id, Some("req-123".to_string()));
        assert_eq!(request.user_id, Some("user-456".to_string()));
    }

    #[test]
    fn test_guardrails_response_passed() {
        let response = GuardrailsResponse::passed();
        assert!(response.passed);
        assert!(response.violations.is_empty());
        assert!(response.highest_severity().is_none());
    }

    #[test]
    fn test_guardrails_response_with_violations() {
        let violations = vec![
            Violation::new(Category::Hate, Severity::High, 0.9),
            Violation::new(Category::Violence, Severity::Medium, 0.7),
        ];

        let response = GuardrailsResponse::with_violations(violations);
        assert!(!response.passed);
        assert_eq!(response.violations.len(), 2);
        assert_eq!(response.highest_severity(), Some(Severity::High));
    }

    #[test]
    fn test_category_from_str() {
        assert_eq!(Category::from("hate"), Category::Hate);
        assert_eq!(Category::from("HATE"), Category::Hate);
        assert_eq!(Category::from("hate/threatening"), Category::Hate);
        assert_eq!(Category::from("self-harm"), Category::SelfHarm);
        assert_eq!(Category::from("prompt_injection"), Category::PromptAttack);
        assert_eq!(
            Category::from("custom_category"),
            Category::Custom("custom_category".to_string())
        );
    }

    #[test]
    fn test_category_display() {
        assert_eq!(Category::Hate.to_string(), "hate");
        assert_eq!(Category::SelfHarm.to_string(), "self_harm");
        assert_eq!(Category::PromptAttack.to_string(), "prompt_attack");
        assert_eq!(
            Category::Custom("my_custom".to_string()).to_string(),
            "my_custom"
        );
    }

    #[test]
    fn test_category_is_pii() {
        assert!(Category::PiiEmail.is_pii());
        assert!(Category::PiiPhone.is_pii());
        assert!(Category::PiiSsn.is_pii());
        assert!(!Category::Hate.is_pii());
        assert!(!Category::PromptAttack.is_pii());
    }

    #[test]
    fn test_category_is_security() {
        assert!(Category::PromptAttack.is_security());
        assert!(Category::PromptLeakage.is_security());
        assert!(Category::MaliciousCode.is_security());
        assert!(!Category::Hate.is_security());
        assert!(!Category::PiiEmail.is_security());
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
        assert!(Severity::Low > Severity::Info);
    }

    #[test]
    fn test_severity_from_score() {
        assert_eq!(Severity::from_score(0.1), Severity::Low);
        assert_eq!(Severity::from_score(0.3), Severity::Medium);
        assert_eq!(Severity::from_score(0.6), Severity::High);
        assert_eq!(Severity::from_score(0.9), Severity::Critical);
    }

    #[test]
    fn test_severity_from_azure_threshold() {
        assert_eq!(Severity::from_azure_threshold(0), Severity::Info);
        assert_eq!(Severity::from_azure_threshold(2), Severity::Low);
        assert_eq!(Severity::from_azure_threshold(4), Severity::Medium);
        assert_eq!(Severity::from_azure_threshold(5), Severity::High);
        assert_eq!(Severity::from_azure_threshold(6), Severity::Critical);
    }

    #[test]
    fn test_violation_builder() {
        let violation = Violation::new(Category::Hate, Severity::High, 0.95)
            .with_message("Hate speech detected")
            .with_span(10, 25);

        assert_eq!(violation.category, Category::Hate);
        assert_eq!(violation.severity, Severity::High);
        assert!((violation.confidence - 0.95).abs() < f64::EPSILON);
        assert_eq!(violation.message, Some("Hate speech detected".to_string()));
        assert!(violation.span.is_some());
        let span = violation.span.unwrap();
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 25);
    }

    #[test]
    fn test_resolved_action_is_blocked() {
        assert!(!ResolvedAction::Allow.is_blocked());
        assert!(
            ResolvedAction::Block {
                reason: "test".to_string(),
                violations: vec![]
            }
            .is_blocked()
        );
        assert!(!ResolvedAction::Warn { violations: vec![] }.is_blocked());
    }

    #[test]
    fn test_resolved_action_is_modified() {
        assert!(!ResolvedAction::Allow.is_modified());
        assert!(
            !ResolvedAction::Block {
                reason: "test".to_string(),
                violations: vec![]
            }
            .is_modified()
        );
        assert!(
            ResolvedAction::Redact {
                original_content: "original".to_string(),
                modified_content: "[REDACTED]".to_string(),
                violations: vec![]
            }
            .is_modified()
        );
    }

    #[test]
    fn test_content_source_display() {
        assert_eq!(ContentSource::UserInput.to_string(), "user_input");
        assert_eq!(ContentSource::LlmOutput.to_string(), "llm_output");
        assert_eq!(ContentSource::System.to_string(), "system");
    }
}
