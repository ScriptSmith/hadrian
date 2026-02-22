//! Guardrails module for content filtering, PII detection, and safety enforcement.
//!
//! This module provides a unified interface for evaluating content against
//! various guardrails providers (OpenAI Moderation, AWS Bedrock, Azure Content Safety,
//! and custom HTTP providers).
//!
//! # Architecture
//!
//! ```text
//! Request
//!    │
//!    ▼
//! ┌──────────────────┐
//! │ Input Guardrails │──► Block/Warn/Redact/Modify
//! └──────────────────┘
//!    │
//!    ▼
//! ┌──────────────────┐
//! │   LLM Provider   │
//! └──────────────────┘
//!    │
//!    ▼
//! ┌───────────────────┐
//! │ Output Guardrails │──► Block/Warn/Redact/Modify
//! └───────────────────┘
//!    │
//!    ▼
//! Response
//! ```
//!
//! # Execution Modes
//!
//! - **Blocking**: Evaluate guardrails before/after LLM call (safest, adds latency)
//! - **Concurrent**: Evaluate input guardrails while LLM processes (reduces latency)
//!
//! # Example
//!
//! ```rust,ignore
//! use hadrian::guardrails::{GuardrailsEvaluator, GuardrailsRequest, ContentSource};
//!
//! let request = GuardrailsRequest::new(ContentSource::UserInput)
//!     .with_text("Hello, how are you?");
//!
//! let result = evaluator.evaluate(&request).await?;
//!
//! match result.action {
//!     ResolvedAction::Allow => { /* proceed */ }
//!     ResolvedAction::Block { reason } => { /* return error */ }
//!     ResolvedAction::Warn { violations } => { /* add headers, proceed */ }
//!     ResolvedAction::Redact { modified_content } => { /* use modified content */ }
//! }
//! ```

pub mod audit;
mod azure;
#[cfg(feature = "provider-bedrock")]
mod bedrock;
mod blocklist;
mod content_limits;
mod custom;
mod error;
mod evaluator;
mod openai;
mod pii_regex;
pub mod retry;
pub mod streaming;
mod types;

use std::collections::HashMap;

use async_trait::async_trait;
pub use azure::AzureContentSafetyProvider;
#[cfg(feature = "provider-bedrock")]
pub use bedrock::BedrockGuardrailsProvider;
pub use blocklist::BlocklistProvider;
pub use custom::CustomHttpProvider;
pub use error::{GuardrailsError, GuardrailsResult};
pub use evaluator::{
    InputGuardrails, InputGuardrailsResult, OutputGuardrails, OutputGuardrailsResult,
    extract_assistant_content_from_response, extract_text_from_completion_response,
    extract_text_from_responses_response, run_concurrent_evaluation,
};
pub use openai::OpenAIModerationProvider;
pub use retry::GuardrailsRetryConfig;
pub use streaming::{GuardrailsFilterStream, StreamingGuardrailsConfig};
pub use types::{
    Category, ContentSource, GuardrailsRequest, GuardrailsResponse, ResolvedAction, Severity,
    Violation,
};

/// Injects OpenTelemetry trace context headers into a HashMap for HTTP propagation.
///
/// This allows external guardrails providers to correlate their traces with
/// the gateway's distributed traces. Headers are injected in W3C Trace Context format
/// (traceparent, tracestate) if OpenTelemetry tracing is enabled.
///
/// # Example
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use hadrian::guardrails::inject_trace_context;
///
/// let mut headers = HashMap::new();
/// inject_trace_context(&mut headers);
///
/// // Headers now contains traceparent and possibly tracestate if tracing is enabled
/// for (key, value) in headers {
///     request_builder = request_builder.header(key, value);
/// }
/// ```
pub fn inject_trace_context(headers: &mut HashMap<String, String>) {
    #[cfg(feature = "otlp")]
    {
        use tracing_opentelemetry::OpenTelemetrySpanExt;

        struct HeaderInjector<'a>(&'a mut HashMap<String, String>);

        impl opentelemetry::propagation::Injector for HeaderInjector<'_> {
            fn set(&mut self, key: &str, value: String) {
                self.0.insert(key.to_string(), value);
            }
        }

        let context = tracing::Span::current().context();
        let propagator = opentelemetry::global::get_text_map_propagator(|p| {
            // Clone the propagator since we can't hold the reference
            p.fields()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join(",")
        });

        // Use the global propagator to inject context
        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&context, &mut HeaderInjector(headers));
        });

        // Only log if we actually injected something
        if !propagator.is_empty() {
            tracing::trace!(
                propagator_fields = %propagator,
                "Injected trace context into outgoing request"
            );
        }
    }
    #[cfg(not(feature = "otlp"))]
    let _ = headers;
}

/// Trait for guardrails providers.
///
/// Implementations include:
/// - OpenAI Moderation API
/// - AWS Bedrock Guardrails
/// - Azure Content Safety
/// - Custom HTTP providers
///
/// # Example Implementation
///
/// ```rust,ignore
/// struct MyGuardrailsProvider {
///     client: reqwest::Client,
///     api_key: String,
/// }
///
/// #[async_trait]
/// impl GuardrailsProvider for MyGuardrailsProvider {
///     fn name(&self) -> &str {
///         "my-provider"
///     }
///
///     async fn evaluate(&self, request: &GuardrailsRequest) -> GuardrailsResult<GuardrailsResponse> {
///         // Call external API and parse response
///         // Map violations to common Category/Severity types
///         Ok(GuardrailsResponse::passed())
///     }
/// }
/// ```
#[async_trait]
pub trait GuardrailsProvider: Send + Sync {
    /// Returns the name of this provider (e.g., "openai", "bedrock", "azure").
    fn name(&self) -> &str;

    /// Evaluates content against guardrails policies.
    ///
    /// # Arguments
    /// * `request` - The content to evaluate with metadata
    ///
    /// # Returns
    /// * `Ok(GuardrailsResponse)` - Evaluation result with any violations found
    /// * `Err(GuardrailsError)` - If evaluation failed (network error, auth error, etc.)
    async fn evaluate(&self, request: &GuardrailsRequest) -> GuardrailsResult<GuardrailsResponse>;

    /// Returns supported categories for this provider.
    ///
    /// This is used for validation and documentation.
    fn supported_categories(&self) -> &[Category] {
        Category::all_standard()
    }
}

/// Action executor that applies configured actions based on guardrails evaluation.
///
/// Takes a `GuardrailsResponse` and the configured action mappings, and determines
/// the final action to take (block, warn, redact, etc.).
#[derive(Clone)]
pub struct ActionExecutor {
    /// Per-category action overrides.
    actions: std::collections::HashMap<String, crate::config::GuardrailsAction>,
    /// Default action for categories not in the map.
    default_action: crate::config::GuardrailsAction,
}

impl ActionExecutor {
    /// Creates a new action executor with the given configuration.
    pub fn new(
        actions: std::collections::HashMap<String, crate::config::GuardrailsAction>,
        default_action: crate::config::GuardrailsAction,
    ) -> Self {
        Self {
            actions,
            default_action,
        }
    }

    /// Creates an action executor from input guardrails config.
    pub fn from_input_config(config: &crate::config::InputGuardrailsConfig) -> Self {
        Self::new(config.actions.clone(), config.default_action.clone())
    }

    /// Creates an action executor from output guardrails config.
    pub fn from_output_config(config: &crate::config::OutputGuardrailsConfig) -> Self {
        Self::new(config.actions.clone(), config.default_action.clone())
    }

    /// Determines the final action to take based on the guardrails response.
    ///
    /// Applies the highest-priority action across all violations:
    /// 1. Block (highest priority)
    /// 2. Redact
    /// 3. Modify
    /// 4. Warn
    /// 5. Log (lowest priority)
    ///
    /// If no violations, returns `ResolvedAction::Allow`.
    #[tracing::instrument(
        name = "guardrails.action",
        skip(self, response, original_content),
        fields(
            violation_count = response.violations.len(),
        )
    )]
    pub fn resolve_action(
        &self,
        response: &GuardrailsResponse,
        original_content: &str,
    ) -> ResolvedAction {
        use crate::config::GuardrailsAction;

        if response.violations.is_empty() {
            tracing::Span::current().record("action", "allow");
            return ResolvedAction::Allow;
        }

        // Track the highest priority action and relevant violations
        let mut should_block = false;
        let mut should_redact = false;
        let mut redact_replacement = String::from("[REDACTED]");
        let mut should_warn = false;
        let mut should_log = false;
        let mut block_reason = String::new();
        let mut warn_violations = Vec::new();
        let mut log_violations = Vec::new();

        for violation in &response.violations {
            let category_key = violation.category.to_string();
            let action = self
                .actions
                .get(&category_key)
                .unwrap_or(&self.default_action);

            // Record span event for each violation with its configured action
            let action_name = match action {
                GuardrailsAction::Block => "block",
                GuardrailsAction::Redact { .. } => "redact",
                GuardrailsAction::Modify => "modify",
                GuardrailsAction::Warn => "warn",
                GuardrailsAction::Log => "log",
            };
            tracing::event!(
                tracing::Level::DEBUG,
                category = %violation.category,
                severity = %violation.severity,
                confidence = violation.confidence,
                action = action_name,
                "Processing violation"
            );

            match action {
                GuardrailsAction::Block => {
                    should_block = true;
                    if block_reason.is_empty() {
                        block_reason =
                            format!("Content blocked due to {} violation", violation.category);
                    }
                }
                GuardrailsAction::Redact { replacement } => {
                    should_redact = true;
                    redact_replacement.clone_from(replacement);
                }
                GuardrailsAction::Modify => {
                    // Modify is treated similarly to redact but provider-specific
                    should_redact = true;
                }
                GuardrailsAction::Warn => {
                    should_warn = true;
                    warn_violations.push(violation.clone());
                }
                GuardrailsAction::Log => {
                    should_log = true;
                    log_violations.push(violation.clone());
                }
            }
        }

        // Return highest priority action
        if should_block {
            tracing::Span::current().record("action", "block");
            ResolvedAction::Block {
                reason: block_reason,
                violations: response.violations.clone(),
            }
        } else if should_redact {
            tracing::Span::current().record("action", "redact");
            // For simplicity, replace entire content. In practice, this would
            // use violation spans to selectively redact.
            ResolvedAction::Redact {
                original_content: original_content.to_string(),
                modified_content: redact_replacement,
                violations: response.violations.clone(),
            }
        } else if should_warn {
            tracing::Span::current().record("action", "warn");
            ResolvedAction::Warn {
                violations: warn_violations,
            }
        } else if should_log {
            tracing::Span::current().record("action", "log");
            ResolvedAction::Log {
                violations: log_violations,
            }
        } else {
            tracing::Span::current().record("action", "allow");
            ResolvedAction::Allow
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::config::GuardrailsAction;

    fn make_violation(category: Category, severity: Severity) -> Violation {
        Violation {
            category,
            severity,
            confidence: 0.95,
            message: Some("Test violation".to_string()),
            span: None,
            provider_details: None,
        }
    }

    #[test]
    fn test_action_executor_no_violations() {
        let executor = ActionExecutor::new(HashMap::new(), GuardrailsAction::Block);
        let response = GuardrailsResponse::passed();
        let action = executor.resolve_action(&response, "test content");
        assert!(matches!(action, ResolvedAction::Allow));
    }

    #[test]
    fn test_action_executor_block() {
        let mut actions = HashMap::new();
        actions.insert("hate".to_string(), GuardrailsAction::Block);

        let executor = ActionExecutor::new(actions, GuardrailsAction::Warn);

        let response = GuardrailsResponse::with_violations(vec![make_violation(
            Category::Hate,
            Severity::High,
        )]);

        let action = executor.resolve_action(&response, "test content");
        assert!(matches!(action, ResolvedAction::Block { .. }));
    }

    #[test]
    fn test_action_executor_warn() {
        let mut actions = HashMap::new();
        actions.insert("violence".to_string(), GuardrailsAction::Warn);

        let executor = ActionExecutor::new(actions, GuardrailsAction::Block);

        let response = GuardrailsResponse::with_violations(vec![make_violation(
            Category::Violence,
            Severity::Medium,
        )]);

        let action = executor.resolve_action(&response, "test content");
        assert!(matches!(action, ResolvedAction::Warn { .. }));
    }

    #[test]
    fn test_action_executor_redact() {
        let mut actions = HashMap::new();
        actions.insert(
            "sexual".to_string(),
            GuardrailsAction::Redact {
                replacement: "[CONTENT REMOVED]".to_string(),
            },
        );

        let executor = ActionExecutor::new(actions, GuardrailsAction::Block);

        let response = GuardrailsResponse::with_violations(vec![make_violation(
            Category::Sexual,
            Severity::Medium,
        )]);

        let action = executor.resolve_action(&response, "original content");
        match action {
            ResolvedAction::Redact {
                modified_content, ..
            } => {
                assert_eq!(modified_content, "[CONTENT REMOVED]");
            }
            _ => panic!("Expected Redact action"),
        }
    }

    #[test]
    fn test_action_executor_default_action() {
        let executor = ActionExecutor::new(HashMap::new(), GuardrailsAction::Log);

        // Use a category not in the map - should use default
        let response = GuardrailsResponse::with_violations(vec![make_violation(
            Category::SelfHarm,
            Severity::Low,
        )]);

        let action = executor.resolve_action(&response, "test content");
        assert!(matches!(action, ResolvedAction::Log { .. }));
    }

    #[test]
    fn test_action_executor_priority_block_over_warn() {
        let mut actions = HashMap::new();
        actions.insert("hate".to_string(), GuardrailsAction::Block);
        actions.insert("violence".to_string(), GuardrailsAction::Warn);

        let executor = ActionExecutor::new(actions, GuardrailsAction::Log);

        // Multiple violations - block should take priority
        let response = GuardrailsResponse::with_violations(vec![
            make_violation(Category::Violence, Severity::Medium),
            make_violation(Category::Hate, Severity::High),
        ]);

        let action = executor.resolve_action(&response, "test content");
        assert!(matches!(action, ResolvedAction::Block { .. }));
    }

    #[test]
    fn test_action_executor_priority_redact_over_warn() {
        let mut actions = HashMap::new();
        actions.insert(
            "hate".to_string(),
            GuardrailsAction::Redact {
                replacement: "[REMOVED]".to_string(),
            },
        );
        actions.insert("violence".to_string(), GuardrailsAction::Warn);

        let executor = ActionExecutor::new(actions, GuardrailsAction::Log);

        let response = GuardrailsResponse::with_violations(vec![
            make_violation(Category::Violence, Severity::Medium),
            make_violation(Category::Hate, Severity::High),
        ]);

        let action = executor.resolve_action(&response, "test content");
        assert!(matches!(action, ResolvedAction::Redact { .. }));
    }
}
