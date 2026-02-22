//! Authorization engine using CEL for policy evaluation.

#[cfg(feature = "cel")]
use std::{panic, sync::Arc};

#[cfg(feature = "cel")]
use cel_interpreter::{Context, Program, Value, to_value};
use serde::Serialize;

use super::AuthzError;
#[cfg(feature = "cel")]
use crate::config::PolicyConfig;
use crate::config::{PolicyEffect, RbacConfig};

/// Result of simulating a single system policy.
#[derive(Debug, Clone)]
pub struct SystemPolicySimulationResult {
    /// Policy name
    pub name: String,
    /// Policy description
    pub description: Option<String>,
    /// Whether the policy's resource/action pattern matched
    pub pattern_matched: bool,
    /// Whether the policy's CEL condition evaluated to true (None if not evaluated)
    pub condition_matched: Option<bool>,
    /// Policy effect (allow/deny)
    pub effect: PolicyEffect,
    /// Policy priority
    pub priority: i32,
    /// Error message if condition evaluation failed
    pub error: Option<String>,
}

/// Result of simulating all system policies.
#[derive(Debug, Clone)]
pub struct SystemSimulationResult {
    /// Whether RBAC is enabled
    pub rbac_enabled: bool,
    /// Default effect when no policy matches
    pub default_effect: PolicyEffect,
    /// Results from evaluating each system policy
    pub policies_evaluated: Vec<SystemPolicySimulationResult>,
    /// The policy that matched and its decision (policy_name, allowed)
    pub matched: Option<(String, bool)>,
}

/// Subject (actor) making the request.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Subject {
    /// User ID (internal)
    pub user_id: Option<String>,
    /// External ID from IdP (e.g., "sub" claim)
    pub external_id: Option<String>,
    /// Email
    pub email: Option<String>,
    /// User's roles from IdP (after mapping)
    pub roles: Vec<String>,
    /// Organization IDs the user belongs to (from claims or database)
    pub org_ids: Vec<String>,
    /// Team IDs the user belongs to (from claims or database)
    pub team_ids: Vec<String>,
    /// Project IDs the user belongs to (from claims or database)
    pub project_ids: Vec<String>,
    /// Service account ID (if authenticated via service account-owned API key)
    pub service_account_id: Option<String>,
}

impl Subject {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    pub fn with_external_id(mut self, external_id: impl Into<String>) -> Self {
        self.external_id = Some(external_id.into());
        self
    }

    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    pub fn with_org_ids(mut self, org_ids: Vec<String>) -> Self {
        self.org_ids = org_ids;
        self
    }

    pub fn with_team_ids(mut self, team_ids: Vec<String>) -> Self {
        self.team_ids = team_ids;
        self
    }

    pub fn with_project_ids(mut self, project_ids: Vec<String>) -> Self {
        self.project_ids = project_ids;
        self
    }

    pub fn with_service_account_id(mut self, service_account_id: impl Into<String>) -> Self {
        self.service_account_id = Some(service_account_id.into());
        self
    }

    /// Check if the subject has a specific role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    /// Check if the subject is a member of an organization.
    pub fn is_org_member(&self, org_id: &str) -> bool {
        self.org_ids.iter().any(|id| id == org_id)
    }

    /// Check if the subject is a member of a team.
    pub fn is_team_member(&self, team_id: &str) -> bool {
        self.team_ids.iter().any(|id| id == team_id)
    }

    /// Check if the subject is a member of a project.
    pub fn is_project_member(&self, project_id: &str) -> bool {
        self.project_ids.iter().any(|id| id == project_id)
    }
}

/// Request context for API endpoint authorization.
///
/// Contains information extracted from API requests (e.g., chat completions,
/// images, audio) for use in CEL policy evaluation.
///
/// # CEL Context Variables
///
/// All fields are available under `context.request.*` in CEL expressions:
///
/// ## Chat Completion Fields
/// - `context.request.max_tokens` - Maximum tokens requested (optional u64)
/// - `context.request.messages_count` - Number of messages in conversation (u64)
/// - `context.request.has_tools` - Whether request includes tools/functions (bool)
/// - `context.request.has_file_search` - Whether request includes file_search tool (bool)
/// - `context.request.stream` - Whether streaming is requested (bool)
/// - `context.request.reasoning_effort` - Reasoning/thinking effort level: "none", "minimal", "low", "medium", "high" (optional string)
/// - `context.request.response_format` - Output format: "text", "json_object", "json_schema", "grammar", "python" (optional string)
/// - `context.request.temperature` - Sampling temperature 0.0-2.0 (optional f64)
/// - `context.request.has_images` - Whether request contains image content (bool)
///
/// ## Image Generation Fields
/// - `context.request.image_count` - Number of images to generate (optional u32)
/// - `context.request.image_size` - Image size: "256x256", "512x512", "1024x1024", etc. (optional string)
/// - `context.request.image_quality` - Quality level: "standard", "hd", "low", "medium", "high", "auto" (optional string)
///
/// ## Audio TTS Fields
/// - `context.request.character_count` - Length of text input in characters (optional u64)
/// - `context.request.voice` - Voice name: "alloy", "echo", "fable", etc. (optional string)
///
/// ## Audio Transcription Fields
/// - `context.request.language` - ISO-639-1 language code (optional string)
#[derive(Debug, Clone, Default, Serialize)]
pub struct RequestContext {
    // ========== Chat Completion Fields ==========
    /// Maximum tokens requested (max_tokens or max_completion_tokens)
    pub max_tokens: Option<u64>,
    /// Number of messages in the conversation
    pub messages_count: u64,
    /// Whether the request includes tools/functions
    pub has_tools: bool,
    /// Whether the request includes file_search tool
    pub has_file_search: bool,
    /// Whether streaming is requested
    pub stream: bool,
    /// Reasoning/thinking effort level (for extended thinking models)
    /// Values: "none", "minimal", "low", "medium", "high"
    pub reasoning_effort: Option<String>,
    /// Response format type
    /// Values: "text", "json_object", "json_schema", "grammar", "python"
    pub response_format: Option<String>,
    /// Sampling temperature (0.0 to 2.0)
    pub temperature: Option<f64>,
    /// Whether the request contains image content (multimodal)
    pub has_images: bool,

    // ========== Image Generation Fields ==========
    /// Number of images to generate (n parameter)
    pub image_count: Option<u32>,
    /// Image size (e.g., "256x256", "1024x1024", "1536x1024")
    pub image_size: Option<String>,
    /// Image quality level (e.g., "standard", "hd", "high")
    pub image_quality: Option<String>,

    // ========== Audio TTS Fields ==========
    /// Character count of text input for TTS
    pub character_count: Option<u64>,
    /// Voice name for TTS (e.g., "alloy", "echo")
    pub voice: Option<String>,

    // ========== Audio Transcription Fields ==========
    /// Language code (ISO-639-1) for transcription
    pub language: Option<String>,
}

impl RequestContext {
    pub fn new() -> Self {
        Self::default()
    }

    // ========== Chat Completion Methods ==========

    pub fn with_max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_messages_count(mut self, count: u64) -> Self {
        self.messages_count = count;
        self
    }

    pub fn with_tools(mut self, has_tools: bool) -> Self {
        self.has_tools = has_tools;
        self
    }

    pub fn with_file_search(mut self, has_file_search: bool) -> Self {
        self.has_file_search = has_file_search;
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    pub fn with_reasoning_effort(mut self, effort: impl Into<String>) -> Self {
        self.reasoning_effort = Some(effort.into());
        self
    }

    pub fn with_response_format(mut self, format: impl Into<String>) -> Self {
        self.response_format = Some(format.into());
        self
    }

    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn with_images(mut self, has_images: bool) -> Self {
        self.has_images = has_images;
        self
    }

    // ========== Image Generation Methods ==========

    pub fn with_image_count(mut self, count: u32) -> Self {
        self.image_count = Some(count);
        self
    }

    pub fn with_image_size(mut self, size: impl Into<String>) -> Self {
        self.image_size = Some(size.into());
        self
    }

    pub fn with_image_quality(mut self, quality: impl Into<String>) -> Self {
        self.image_quality = Some(quality.into());
        self
    }

    // ========== Audio TTS Methods ==========

    pub fn with_character_count(mut self, count: u64) -> Self {
        self.character_count = Some(count);
        self
    }

    pub fn with_voice(mut self, voice: impl Into<String>) -> Self {
        self.voice = Some(voice.into());
        self
    }

    // ========== Audio Transcription Methods ==========

    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }
}

/// Time context for time-based policy evaluation.
///
/// Contains current time information for policies that restrict access
/// based on time of day, day of week, etc.
#[derive(Debug, Clone, Serialize)]
pub struct TimeContext {
    /// Current hour (0-23)
    pub hour: u8,
    /// Day of week (1=Monday, 7=Sunday, following ISO 8601)
    pub day_of_week: u8,
    /// Unix timestamp (seconds since epoch)
    pub timestamp: i64,
}

impl TimeContext {
    /// Create a new TimeContext with the current time.
    pub fn now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let timestamp = now.as_secs() as i64;

        // Calculate hour and day_of_week from timestamp
        // This is a simplified calculation - in production you might want chrono
        let secs_per_day = 86400i64;
        let secs_per_hour = 3600i64;

        // Days since Unix epoch (Thursday, Jan 1, 1970)
        let days_since_epoch = timestamp / secs_per_day;
        // Thursday = 4 in ISO 8601, so we offset
        let day_of_week = ((days_since_epoch + 3) % 7 + 1) as u8; // 1=Monday, 7=Sunday

        let hour = ((timestamp % secs_per_day) / secs_per_hour) as u8;

        Self {
            hour,
            day_of_week,
            timestamp,
        }
    }

    /// Create a TimeContext with specific values (for testing).
    pub fn with_values(hour: u8, day_of_week: u8, timestamp: i64) -> Self {
        Self {
            hour,
            day_of_week,
            timestamp,
        }
    }
}

impl Default for TimeContext {
    fn default() -> Self {
        Self::now()
    }
}

/// Context for policy evaluation.
#[derive(Debug, Clone, Serialize)]
pub struct PolicyContext {
    /// Resource type being accessed
    pub resource_type: String,
    /// Action being performed
    pub action: String,
    /// Specific resource ID being accessed (if applicable)
    pub resource_id: Option<String>,
    /// Organization ID scope
    pub org_id: Option<String>,
    /// Team ID scope
    pub team_id: Option<String>,
    /// Project ID scope
    pub project_id: Option<String>,
    /// Model being requested (for API endpoints)
    pub model: Option<String>,
    /// Request-specific context (for API endpoints)
    pub request: Option<RequestContext>,
    /// Current time context (for time-based policies)
    pub now: Option<TimeContext>,
}

impl PolicyContext {
    pub fn new(resource_type: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            resource_type: resource_type.into(),
            action: action.into(),
            resource_id: None,
            org_id: None,
            team_id: None,
            project_id: None,
            model: None,
            request: None,
            now: None,
        }
    }

    pub fn with_resource_id(mut self, id: impl Into<String>) -> Self {
        self.resource_id = Some(id.into());
        self
    }

    pub fn with_org_id(mut self, org_id: impl Into<String>) -> Self {
        self.org_id = Some(org_id.into());
        self
    }

    pub fn with_team_id(mut self, team_id: impl Into<String>) -> Self {
        self.team_id = Some(team_id.into());
        self
    }

    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    /// Set the model being requested (for API endpoint authorization).
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the request context (for API endpoint authorization).
    pub fn with_request(mut self, request: RequestContext) -> Self {
        self.request = Some(request);
        self
    }

    /// Set the time context (for time-based policies).
    /// If not set, policies using `now` will fail to match.
    pub fn with_time(mut self, time: TimeContext) -> Self {
        self.now = Some(time);
        self
    }

    /// Set the time context to the current time.
    pub fn with_current_time(mut self) -> Self {
        self.now = Some(TimeContext::now());
        self
    }
}

/// Result of an authorization check.
#[derive(Debug, Clone)]
pub struct AuthzResult {
    /// Whether access is allowed
    pub allowed: bool,
    /// The policy that made this decision (if any)
    pub policy_name: Option<String>,
    /// Human-readable reason
    pub reason: Option<String>,
}

impl AuthzResult {
    pub fn allow() -> Self {
        Self {
            allowed: true,
            policy_name: None,
            reason: None,
        }
    }

    pub fn allow_by_policy(name: impl Into<String>) -> Self {
        Self {
            allowed: true,
            policy_name: Some(name.into()),
            reason: None,
        }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            policy_name: None,
            reason: Some(reason.into()),
        }
    }

    pub fn deny_by_policy(name: impl Into<String>, reason: Option<String>) -> Self {
        Self {
            allowed: false,
            policy_name: Some(name.into()),
            reason,
        }
    }

    pub fn deny_default() -> Self {
        Self {
            allowed: false,
            policy_name: None,
            reason: Some("No matching policy (default deny)".to_string()),
        }
    }

    pub fn allow_default() -> Self {
        Self {
            allowed: true,
            policy_name: None,
            reason: Some("No matching policy (default allow)".to_string()),
        }
    }
}

// ============================================================================
// CEL-enabled implementation
// ============================================================================

/// Authorization engine using config-based policies.
#[cfg(feature = "cel")]
pub struct AuthzEngine {
    config: RbacConfig,
    /// Compiled CEL programs for each policy
    compiled_policies: Vec<(PolicyConfig, Arc<Program>)>,
}

#[cfg(feature = "cel")]
impl AuthzEngine {
    /// Create a new authorization engine from config.
    pub fn new(config: RbacConfig) -> Result<Self, AuthzError> {
        let mut compiled_policies = Vec::new();

        // Sort policies by priority (descending), then by effect (deny before allow)
        let mut policies = config.policies.clone();
        policies.sort_by(|a, b| {
            match b.priority.cmp(&a.priority) {
                std::cmp::Ordering::Equal => {
                    // Deny before allow at same priority
                    match (&a.effect, &b.effect) {
                        (PolicyEffect::Deny, PolicyEffect::Allow) => std::cmp::Ordering::Less,
                        (PolicyEffect::Allow, PolicyEffect::Deny) => std::cmp::Ordering::Greater,
                        _ => std::cmp::Ordering::Equal,
                    }
                }
                other => other,
            }
        });

        // Compile all policies upfront
        for policy in policies {
            // Validate expression length
            if config.max_expression_length > 0
                && policy.condition.len() > config.max_expression_length
            {
                return Err(AuthzError::InvalidExpression(format!(
                    "Policy '{}': CEL expression length ({} bytes) exceeds maximum ({} bytes)",
                    policy.name,
                    policy.condition.len(),
                    config.max_expression_length
                )));
            }

            let program = Program::compile(&policy.condition).map_err(|e| {
                AuthzError::InvalidExpression(format!("Policy '{}': {}", policy.name, e))
            })?;
            compiled_policies.push((policy, Arc::new(program)));
        }

        Ok(Self {
            config,
            compiled_policies,
        })
    }

    /// Check if RBAC is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the role claim name from config.
    pub fn role_claim(&self) -> &str {
        &self.config.role_claim
    }

    /// Get the org claim name from config (if set).
    pub fn org_claim(&self) -> Option<&str> {
        self.config.org_claim.as_deref()
    }

    /// Get the team claim name from config (if set).
    pub fn team_claim(&self) -> Option<&str> {
        self.config.team_claim.as_deref()
    }

    /// Get the project claim name from config (if set).
    pub fn project_claim(&self) -> Option<&str> {
        self.config.project_claim.as_deref()
    }

    /// Whether to fail-closed on policy evaluation errors.
    pub fn fail_on_evaluation_error(&self) -> bool {
        self.config.fail_on_evaluation_error
    }

    /// Get the maximum allowed expression length (0 = unlimited).
    pub fn max_expression_length(&self) -> usize {
        self.config.max_expression_length
    }

    /// Map roles from IdP naming to internal naming.
    pub fn map_roles(&self, roles: &[String]) -> Vec<String> {
        self.config.map_roles(roles)
    }

    /// Authorize an action.
    pub fn authorize(&self, subject: &Subject, context: &PolicyContext) -> AuthzResult {
        // If RBAC is disabled, allow everything
        if !self.config.enabled {
            return AuthzResult::allow();
        }

        // Evaluate policies in order (already sorted by priority)
        for (policy, program) in &self.compiled_policies {
            // Check if policy applies to this resource/action
            if !self.policy_matches(policy, context) {
                continue;
            }

            tracing::debug!(
                policy = %policy.name,
                priority = policy.priority,
                effect = ?policy.effect,
                resource = %context.resource_type,
                action = %context.action,
                subject_user_id = ?subject.user_id,
                context_resource_id = ?context.resource_id,
                "Evaluating policy"
            );

            // Evaluate the CEL condition
            match self.evaluate_condition(program, subject, context) {
                Ok(true) => {
                    tracing::debug!(
                        policy = %policy.name,
                        effect = ?policy.effect,
                        "Policy condition matched"
                    );
                    // Policy condition matched
                    return match policy.effect {
                        PolicyEffect::Allow => AuthzResult::allow_by_policy(&policy.name),
                        PolicyEffect::Deny => {
                            AuthzResult::deny_by_policy(&policy.name, policy.description.clone())
                        }
                    };
                }
                Ok(false) => {
                    tracing::debug!(
                        policy = %policy.name,
                        "Policy condition did not match, trying next"
                    );
                    // Condition didn't match, try next policy
                    continue;
                }
                Err(e) => {
                    // Log the error
                    tracing::warn!(
                        policy = %policy.name,
                        error = %e,
                        fail_on_error = self.config.fail_on_evaluation_error,
                        "Policy evaluation error"
                    );

                    // If fail_on_evaluation_error is true (default), deny the request
                    // to avoid security holes from silently skipping policies
                    if self.config.fail_on_evaluation_error {
                        return AuthzResult {
                            allowed: false,
                            policy_name: Some(policy.name.clone()),
                            reason: Some(format!(
                                "Policy '{}' failed to evaluate: {}",
                                policy.name, e
                            )),
                        };
                    }

                    // Otherwise, skip this policy and continue to the next one
                    continue;
                }
            }
        }

        // No policy matched, use default effect
        match self.config.default_effect {
            PolicyEffect::Allow => AuthzResult::allow_default(),
            PolicyEffect::Deny => AuthzResult::deny_default(),
        }
    }

    /// Check if a policy applies to the given resource/action.
    ///
    /// Uses pattern matching that supports:
    /// - `*` to match any value
    /// - `foo*` to match any value starting with `foo`
    /// - `foo` for exact match only
    fn policy_matches(&self, policy: &PolicyConfig, context: &PolicyContext) -> bool {
        let resource_matches = super::pattern_matches(&policy.resource, &context.resource_type);
        let action_matches = super::pattern_matches(&policy.action, &context.action);
        resource_matches && action_matches
    }

    /// Evaluate a CEL condition.
    fn evaluate_condition(
        &self,
        program: &Program,
        subject: &Subject,
        context: &PolicyContext,
    ) -> Result<bool, AuthzError> {
        let mut ctx = Context::default();

        // Add subject to context
        let subject_value = to_value(subject).map_err(|e| {
            AuthzError::PolicyEvaluation(format!("Failed to serialize subject: {}", e))
        })?;
        ctx.add_variable("subject", subject_value);

        // Add context to context
        let context_value = to_value(context).map_err(|e| {
            AuthzError::PolicyEvaluation(format!("Failed to serialize context: {}", e))
        })?;
        ctx.add_variable("context", context_value);

        // Execute with panic protection - the CEL interpreter uses ANTLR-generated code
        // which can potentially panic on edge cases during execution
        let exec_result = panic::catch_unwind(panic::AssertUnwindSafe(|| program.execute(&ctx)));

        let result = match exec_result {
            Ok(Ok(value)) => value,
            Ok(Err(e)) => {
                return Err(AuthzError::PolicyEvaluation(format!(
                    "Execution error: {}",
                    e
                )));
            }
            Err(_) => {
                return Err(AuthzError::PolicyEvaluation(
                    "CEL expression execution failed (internal error)".to_string(),
                ));
            }
        };

        match result {
            Value::Bool(b) => Ok(b),
            _ => Err(AuthzError::PolicyEvaluation(
                "Policy condition must evaluate to boolean".to_string(),
            )),
        }
    }

    /// Validate a CEL expression without a full authorization check.
    ///
    /// This wraps the CEL compilation in `catch_unwind` because the underlying
    /// ANTLR parser can panic on certain malformed expressions instead of
    /// returning an error.
    pub fn validate_expression(expression: &str) -> Result<(), AuthzError> {
        Self::validate_expression_with_max_length(expression, 0)
    }

    /// Validate a CEL expression with an optional maximum length.
    ///
    /// If `max_length` is 0, no length check is performed.
    pub fn validate_expression_with_max_length(
        expression: &str,
        max_length: usize,
    ) -> Result<(), AuthzError> {
        if max_length > 0 && expression.len() > max_length {
            return Err(AuthzError::InvalidExpression(format!(
                "CEL expression length ({} bytes) exceeds maximum ({} bytes)",
                expression.len(),
                max_length
            )));
        }

        let expr = expression.to_string();
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| Program::compile(&expr)));

        match result {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(AuthzError::InvalidExpression(format!("{}", e))),
            Err(_) => Err(AuthzError::InvalidExpression(
                "CEL expression parsing failed (malformed syntax)".to_string(),
            )),
        }
    }

    /// Simulate policy evaluation, returning detailed results for each policy.
    ///
    /// Unlike `authorize()`, this method evaluates all policies and returns
    /// information about each one, useful for debugging and testing policy
    /// configurations in the UI.
    pub fn simulate(&self, subject: &Subject, context: &PolicyContext) -> SystemSimulationResult {
        let mut policies_evaluated = Vec::new();
        let mut matched: Option<(String, bool)> = None;

        // Evaluate all policies regardless of RBAC being enabled
        // (we want to show what would happen)
        for (policy, program) in &self.compiled_policies {
            // Check if policy applies to this resource/action
            let pattern_matched = self.policy_matches(policy, context);

            let mut result = SystemPolicySimulationResult {
                name: policy.name.clone(),
                description: policy.description.clone(),
                pattern_matched,
                condition_matched: None,
                effect: policy.effect,
                priority: policy.priority,
                error: None,
            };

            // Only evaluate condition if pattern matched
            if pattern_matched {
                match self.evaluate_condition(program, subject, context) {
                    Ok(condition_result) => {
                        result.condition_matched = Some(condition_result);

                        // If condition matched and we haven't found a match yet
                        if condition_result && matched.is_none() {
                            let allowed = matches!(policy.effect, PolicyEffect::Allow);
                            matched = Some((policy.name.clone(), allowed));
                        }
                    }
                    Err(e) => {
                        result.error = Some(e.to_string());
                    }
                }
            }

            policies_evaluated.push(result);
        }

        SystemSimulationResult {
            rbac_enabled: self.config.enabled,
            default_effect: self.config.default_effect,
            policies_evaluated,
            matched,
        }
    }

    /// Get the default effect from config.
    pub fn default_effect(&self) -> PolicyEffect {
        self.config.default_effect
    }
}

// ============================================================================
// Stub implementation when CEL feature is disabled
// ============================================================================

/// Authorization engine stub (CEL feature disabled).
///
/// When the `cel` feature is not enabled, the engine stores config but cannot
/// evaluate CEL expressions. `authorize()` always returns the configured
/// `default_effect`, and `validate_expression()` returns an error.
#[cfg(not(feature = "cel"))]
pub struct AuthzEngine {
    config: RbacConfig,
}

#[cfg(not(feature = "cel"))]
impl AuthzEngine {
    /// Create a new authorization engine from config (no CEL compilation).
    pub fn new(config: RbacConfig) -> Result<Self, AuthzError> {
        if !config.policies.is_empty() {
            tracing::warn!(
                policy_count = config.policies.len(),
                "RBAC policies configured but the 'cel' feature is not enabled; policies will be ignored"
            );
        }
        Ok(Self { config })
    }

    /// Check if RBAC is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the role claim name from config.
    pub fn role_claim(&self) -> &str {
        &self.config.role_claim
    }

    /// Get the org claim name from config (if set).
    pub fn org_claim(&self) -> Option<&str> {
        self.config.org_claim.as_deref()
    }

    /// Get the team claim name from config (if set).
    pub fn team_claim(&self) -> Option<&str> {
        self.config.team_claim.as_deref()
    }

    /// Get the project claim name from config (if set).
    pub fn project_claim(&self) -> Option<&str> {
        self.config.project_claim.as_deref()
    }

    /// Whether to fail-closed on policy evaluation errors.
    pub fn fail_on_evaluation_error(&self) -> bool {
        self.config.fail_on_evaluation_error
    }

    /// Get the maximum allowed expression length (0 = unlimited).
    pub fn max_expression_length(&self) -> usize {
        self.config.max_expression_length
    }

    /// Map roles from IdP naming to internal naming.
    pub fn map_roles(&self, roles: &[String]) -> Vec<String> {
        self.config.map_roles(roles)
    }

    /// Authorize an action (stub: returns default_effect, no CEL evaluation).
    pub fn authorize(&self, _subject: &Subject, _context: &PolicyContext) -> AuthzResult {
        if !self.config.enabled {
            return AuthzResult::allow();
        }

        match self.config.default_effect {
            PolicyEffect::Allow => AuthzResult::allow_default(),
            PolicyEffect::Deny => AuthzResult::deny_default(),
        }
    }

    /// Validate a CEL expression (stub: always returns error).
    pub fn validate_expression(_expression: &str) -> Result<(), AuthzError> {
        Err(AuthzError::InvalidExpression(
            "CEL policy evaluation requires the 'cel' feature to be enabled".to_string(),
        ))
    }

    /// Validate a CEL expression with max length (stub: always returns error).
    pub fn validate_expression_with_max_length(
        _expression: &str,
        _max_length: usize,
    ) -> Result<(), AuthzError> {
        Err(AuthzError::InvalidExpression(
            "CEL policy evaluation requires the 'cel' feature to be enabled".to_string(),
        ))
    }

    /// Simulate policy evaluation (stub: returns empty results with default effect).
    pub fn simulate(&self, _subject: &Subject, _context: &PolicyContext) -> SystemSimulationResult {
        SystemSimulationResult {
            rbac_enabled: self.config.enabled,
            default_effect: self.config.default_effect,
            policies_evaluated: vec![],
            matched: None,
        }
    }

    /// Get the default effect from config.
    pub fn default_effect(&self) -> PolicyEffect {
        self.config.default_effect
    }
}

#[cfg(all(test, feature = "cel"))]
mod tests {
    use super::*;
    use crate::config::PolicyConfig;

    fn test_config() -> RbacConfig {
        RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Deny,
            role_claim: "roles".to_string(),
            org_claim: Some("org_ids".to_string()),
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![
                PolicyConfig {
                    name: "admin-full-access".to_string(),
                    description: Some("Admins can do anything".to_string()),
                    resource: "*".to_string(),
                    action: "*".to_string(),
                    condition: "'admin' in subject.roles".to_string(),
                    effect: PolicyEffect::Allow,
                    priority: 100,
                },
                PolicyConfig {
                    name: "org-member-read".to_string(),
                    description: None,
                    resource: "organization".to_string(),
                    action: "read".to_string(),
                    condition: "context.org_id != null && context.org_id in subject.org_ids"
                        .to_string(),
                    effect: PolicyEffect::Allow,
                    priority: 50,
                },
                PolicyConfig {
                    name: "deny-self-delete".to_string(),
                    description: Some("Users cannot delete themselves".to_string()),
                    resource: "user".to_string(),
                    action: "delete".to_string(),
                    condition: "subject.user_id == context.resource_id".to_string(),
                    effect: PolicyEffect::Deny,
                    priority: 200, // High priority deny
                },
            ],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        }
    }

    #[test]
    fn test_admin_access() {
        let engine = AuthzEngine::new(test_config()).unwrap();
        let subject = Subject::new().with_roles(vec!["admin".to_string()]);
        let context = PolicyContext::new("organization", "delete");

        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
        assert_eq!(result.policy_name, Some("admin-full-access".to_string()));
    }

    #[test]
    fn test_org_member_read() {
        let engine = AuthzEngine::new(test_config()).unwrap();
        let subject = Subject::new()
            .with_roles(vec!["member".to_string()])
            .with_org_ids(vec!["org-123".to_string()]);
        let context = PolicyContext::new("organization", "read").with_org_id("org-123");

        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
        assert_eq!(result.policy_name, Some("org-member-read".to_string()));
    }

    #[test]
    fn test_org_member_wrong_org() {
        let engine = AuthzEngine::new(test_config()).unwrap();
        let subject = Subject::new()
            .with_roles(vec!["member".to_string()])
            .with_org_ids(vec!["org-123".to_string()]);
        let context = PolicyContext::new("organization", "read").with_org_id("org-456");

        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed); // Default deny
    }

    #[test]
    fn test_deny_self_delete() {
        let engine = AuthzEngine::new(test_config()).unwrap();
        let subject = Subject::new()
            .with_user_id("user-123")
            .with_roles(vec!["admin".to_string()]); // Even admin can't self-delete
        let context = PolicyContext::new("user", "delete").with_resource_id("user-123");

        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);
        assert_eq!(result.policy_name, Some("deny-self-delete".to_string()));
    }

    #[test]
    fn test_admin_can_delete_others() {
        let engine = AuthzEngine::new(test_config()).unwrap();
        let subject = Subject::new()
            .with_user_id("user-123")
            .with_roles(vec!["admin".to_string()]);
        let context = PolicyContext::new("user", "delete").with_resource_id("user-456");

        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
    }

    #[test]
    fn test_default_deny() {
        let engine = AuthzEngine::new(test_config()).unwrap();
        let subject = Subject::new().with_roles(vec!["viewer".to_string()]);
        let context = PolicyContext::new("organization", "delete");

        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("default deny"));
    }

    #[test]
    fn test_disabled_allows_all() {
        let mut config = test_config();
        config.enabled = false;
        let engine = AuthzEngine::new(config).unwrap();
        let subject = Subject::new(); // No roles at all
        let context = PolicyContext::new("anything", "anything");

        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
    }

    #[test]
    fn test_role_mapping() {
        let mut config = test_config();
        config
            .role_mapping
            .insert("Administrator".to_string(), "admin".to_string());
        let engine = AuthzEngine::new(config).unwrap();

        let mapped = engine.map_roles(&["Administrator".to_string(), "viewer".to_string()]);
        assert_eq!(mapped, vec!["admin".to_string(), "viewer".to_string()]);
    }

    #[test]
    fn test_validate_expression() {
        assert!(AuthzEngine::validate_expression("true").is_ok());
        assert!(AuthzEngine::validate_expression("'admin' in subject.roles").is_ok());
        assert!(AuthzEngine::validate_expression("invalid!!!").is_err());
    }

    #[test]
    fn test_validate_expression_handles_parser_panic() {
        // This malformed expression causes the ANTLR parser to panic
        // instead of returning an error. Our catch_unwind should handle it.
        let result = AuthzEngine::validate_expression("subject.roles.exists(r, r ==");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("malformed syntax") || err.to_string().contains("Invalid"),
            "Expected error about malformed syntax, got: {}",
            err
        );
    }

    #[test]
    fn test_team_membership() {
        let subject =
            Subject::new().with_team_ids(vec!["team-123".to_string(), "team-456".to_string()]);

        assert!(subject.is_team_member("team-123"));
        assert!(subject.is_team_member("team-456"));
        assert!(!subject.is_team_member("team-789"));
    }

    #[test]
    fn test_team_scope_in_policy() {
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Deny,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![PolicyConfig {
                name: "team-member-read".to_string(),
                description: Some("Team members can read team resources".to_string()),
                resource: "team".to_string(),
                action: "read".to_string(),
                condition: "context.team_id != null && context.team_id in subject.team_ids"
                    .to_string(),
                effect: PolicyEffect::Allow,
                priority: 50,
            }],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();

        // Team member can read their team
        let subject = Subject::new().with_team_ids(vec!["team-123".to_string()]);
        let context = PolicyContext::new("team", "read").with_team_id("team-123");
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
        assert_eq!(result.policy_name, Some("team-member-read".to_string()));

        // Non-member cannot read team
        let subject = Subject::new().with_team_ids(vec!["team-456".to_string()]);
        let context = PolicyContext::new("team", "read").with_team_id("team-123");
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);
    }

    #[test]
    fn test_model_access_policy() {
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Deny,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![
                PolicyConfig {
                    name: "premium-models-require-premium".to_string(),
                    description: Some("Premium models require premium role".to_string()),
                    resource: "model".to_string(),
                    action: "use".to_string(),
                    condition: "context.model == 'gpt-4o' && !('premium' in subject.roles)"
                        .to_string(),
                    effect: PolicyEffect::Deny,
                    priority: 100,
                },
                PolicyConfig {
                    name: "allow-all-models-for-users".to_string(),
                    description: None,
                    resource: "model".to_string(),
                    action: "use".to_string(),
                    condition: "'user' in subject.roles".to_string(),
                    effect: PolicyEffect::Allow,
                    priority: 50,
                },
            ],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();

        // Regular user can use basic models
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let context = PolicyContext::new("model", "use").with_model("gpt-3.5-turbo");
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);

        // Regular user cannot use premium model
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let context = PolicyContext::new("model", "use").with_model("gpt-4o");
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);
        assert_eq!(
            result.policy_name,
            Some("premium-models-require-premium".to_string())
        );

        // Premium user can use premium model
        let subject = Subject::new().with_roles(vec!["user".to_string(), "premium".to_string()]);
        let context = PolicyContext::new("model", "use").with_model("gpt-4o");
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
    }

    #[test]
    fn test_request_context_token_limit() {
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Allow,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![PolicyConfig {
                name: "limit-tokens-for-basic".to_string(),
                description: Some("Basic users limited to 4096 tokens".to_string()),
                resource: "chat".to_string(),
                action: "complete".to_string(),
                condition: "'basic' in subject.roles && context.request.max_tokens > 4096"
                    .to_string(),
                effect: PolicyEffect::Deny,
                priority: 100,
            }],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();

        // Basic user with small token request - allowed
        let subject = Subject::new().with_roles(vec!["basic".to_string()]);
        let request = RequestContext::new().with_max_tokens(2000);
        let context = PolicyContext::new("chat", "complete").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);

        // Basic user with large token request - denied
        let subject = Subject::new().with_roles(vec!["basic".to_string()]);
        let request = RequestContext::new().with_max_tokens(8000);
        let context = PolicyContext::new("chat", "complete").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);
        assert_eq!(
            result.policy_name,
            Some("limit-tokens-for-basic".to_string())
        );

        // Premium user with large token request - allowed
        let subject = Subject::new().with_roles(vec!["premium".to_string()]);
        let request = RequestContext::new().with_max_tokens(8000);
        let context = PolicyContext::new("chat", "complete").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
    }

    #[test]
    fn test_request_context_tools_gating() {
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Allow,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![PolicyConfig {
                name: "require-tools-feature".to_string(),
                description: Some("Tools require tools feature".to_string()),
                resource: "chat".to_string(),
                action: "complete".to_string(),
                condition: "context.request.has_tools && !('tools_enabled' in subject.roles)"
                    .to_string(),
                effect: PolicyEffect::Deny,
                priority: 100,
            }],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();

        // User without tools_enabled can make request without tools
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new().with_tools(false);
        let context = PolicyContext::new("chat", "complete").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);

        // User without tools_enabled cannot use tools
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new().with_tools(true);
        let context = PolicyContext::new("chat", "complete").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);

        // User with tools_enabled can use tools
        let subject =
            Subject::new().with_roles(vec!["user".to_string(), "tools_enabled".to_string()]);
        let request = RequestContext::new().with_tools(true);
        let context = PolicyContext::new("chat", "complete").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
    }

    #[test]
    fn test_time_based_policy() {
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Allow,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![PolicyConfig {
                name: "business-hours-only".to_string(),
                description: Some("API access restricted to business hours".to_string()),
                resource: "*".to_string(),
                action: "*".to_string(),
                // Deny if outside 9-17 on weekdays (day_of_week 1-5)
                condition:
                    "!('admin' in subject.roles) && (context.now.hour < 9 || context.now.hour >= 17 || context.now.day_of_week > 5)"
                        .to_string(),
                effect: PolicyEffect::Deny,
                priority: 100,
            }],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,

        };

        let engine = AuthzEngine::new(config).unwrap();

        // During business hours on Monday (day 1) at 10am - allowed
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let time = TimeContext::with_values(10, 1, 1000000);
        let context = PolicyContext::new("chat", "complete").with_time(time);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);

        // After hours on Monday at 6pm - denied
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let time = TimeContext::with_values(18, 1, 1000000);
        let context = PolicyContext::new("chat", "complete").with_time(time);
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);

        // Weekend (Saturday = 6) during business hours - denied
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let time = TimeContext::with_values(10, 6, 1000000);
        let context = PolicyContext::new("chat", "complete").with_time(time);
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);

        // Admin can access anytime
        let subject = Subject::new().with_roles(vec!["admin".to_string()]);
        let time = TimeContext::with_values(23, 7, 1000000); // Sunday 11pm
        let context = PolicyContext::new("chat", "complete").with_time(time);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
    }

    #[test]
    fn test_time_context_calculation() {
        // Test that TimeContext::now() produces valid values
        let time = TimeContext::now();
        assert!(time.hour < 24);
        assert!(time.day_of_week >= 1 && time.day_of_week <= 7);
        assert!(time.timestamp > 0);

        // Test specific timestamp: Jan 1, 2024 12:00:00 UTC (Monday)
        // Unix timestamp: 1704110400
        let time = TimeContext::with_values(12, 1, 1704110400);
        assert_eq!(time.hour, 12);
        assert_eq!(time.day_of_week, 1); // Monday
        assert_eq!(time.timestamp, 1704110400);
    }

    #[test]
    fn test_request_context_builder() {
        let request = RequestContext::new()
            .with_max_tokens(4096)
            .with_messages_count(10)
            .with_tools(true)
            .with_file_search(true)
            .with_stream(true);

        assert_eq!(request.max_tokens, Some(4096));
        assert_eq!(request.messages_count, 10);
        assert!(request.has_tools);
        assert!(request.has_file_search);
        assert!(request.stream);
    }

    #[test]
    fn test_policy_context_with_api_fields() {
        let request = RequestContext::new().with_max_tokens(2000).with_tools(true);
        let time = TimeContext::with_values(14, 3, 1704110400);

        let context = PolicyContext::new("model", "use")
            .with_model("gpt-4o")
            .with_request(request)
            .with_time(time)
            .with_org_id("org-123");

        assert_eq!(context.resource_type, "model");
        assert_eq!(context.action, "use");
        assert_eq!(context.model, Some("gpt-4o".to_string()));
        assert_eq!(context.org_id, Some("org-123".to_string()));
        assert!(context.request.is_some());
        assert!(context.now.is_some());

        let req = context.request.unwrap();
        assert_eq!(req.max_tokens, Some(2000));
        assert!(req.has_tools);

        let now = context.now.unwrap();
        assert_eq!(now.hour, 14);
        assert_eq!(now.day_of_week, 3); // Wednesday
    }

    #[test]
    fn test_request_context_extended_fields() {
        // Test all the new RequestContext fields
        let request = RequestContext::new()
            .with_max_tokens(4096)
            .with_messages_count(10)
            .with_tools(true)
            .with_file_search(true)
            .with_stream(true)
            .with_reasoning_effort("high")
            .with_response_format("json_schema")
            .with_temperature(0.7)
            .with_images(true)
            .with_image_count(4)
            .with_image_size("1024x1024")
            .with_image_quality("hd")
            .with_character_count(1500)
            .with_voice("alloy")
            .with_language("en");

        // Verify chat completion fields
        assert_eq!(request.max_tokens, Some(4096));
        assert_eq!(request.messages_count, 10);
        assert!(request.has_tools);
        assert!(request.has_file_search);
        assert!(request.stream);
        assert_eq!(request.reasoning_effort, Some("high".to_string()));
        assert_eq!(request.response_format, Some("json_schema".to_string()));
        assert_eq!(request.temperature, Some(0.7));
        assert!(request.has_images);

        // Verify image generation fields
        assert_eq!(request.image_count, Some(4));
        assert_eq!(request.image_size, Some("1024x1024".to_string()));
        assert_eq!(request.image_quality, Some("hd".to_string()));

        // Verify audio TTS fields
        assert_eq!(request.character_count, Some(1500));
        assert_eq!(request.voice, Some("alloy".to_string()));

        // Verify audio transcription fields
        assert_eq!(request.language, Some("en".to_string()));
    }

    #[test]
    fn test_reasoning_effort_policy() {
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Allow,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![PolicyConfig {
                name: "restrict-extended-thinking".to_string(),
                description: Some("Extended thinking requires premium".to_string()),
                resource: "model".to_string(),
                action: "use".to_string(),
                condition:
                    "context.request.reasoning_effort == 'high' && !('premium' in subject.roles)"
                        .to_string(),
                effect: PolicyEffect::Deny,
                priority: 100,
            }],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();

        // Basic user with no reasoning - allowed
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new();
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);

        // Basic user with high reasoning - denied
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new().with_reasoning_effort("high");
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);

        // Premium user with high reasoning - allowed
        let subject = Subject::new().with_roles(vec!["user".to_string(), "premium".to_string()]);
        let request = RequestContext::new().with_reasoning_effort("high");
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
    }

    #[test]
    fn test_image_generation_policy() {
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Allow,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![
                PolicyConfig {
                    name: "limit-image-count".to_string(),
                    description: Some("Free tier limited to 2 images".to_string()),
                    resource: "model".to_string(),
                    action: "use".to_string(),
                    condition: "context.request.image_count > 2 && !('premium' in subject.roles)"
                        .to_string(),
                    effect: PolicyEffect::Deny,
                    priority: 100,
                },
                PolicyConfig {
                    name: "restrict-hd-quality".to_string(),
                    description: Some("HD requires premium".to_string()),
                    resource: "model".to_string(),
                    action: "use".to_string(),
                    condition:
                        "context.request.image_quality == 'hd' && !('premium' in subject.roles)"
                            .to_string(),
                    effect: PolicyEffect::Deny,
                    priority: 100,
                },
            ],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();

        // Basic user with 2 images - allowed
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new().with_image_count(2);
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);

        // Basic user with 4 images - denied
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new().with_image_count(4);
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);

        // Basic user with HD quality - denied
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new()
            .with_image_count(1)
            .with_image_quality("hd");
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);

        // Premium user with HD quality - allowed
        let subject = Subject::new().with_roles(vec!["user".to_string(), "premium".to_string()]);
        let request = RequestContext::new()
            .with_image_count(4)
            .with_image_quality("hd");
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
    }

    #[test]
    fn test_audio_tts_policy() {
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Allow,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![PolicyConfig {
                name: "limit-tts-characters".to_string(),
                description: Some("Free tier limited to 1000 characters".to_string()),
                resource: "model".to_string(),
                action: "use".to_string(),
                condition:
                    "context.request.character_count > 1000 && !('tts_extended' in subject.roles)"
                        .to_string(),
                effect: PolicyEffect::Deny,
                priority: 100,
            }],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();

        // Basic user with 500 characters - allowed
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new().with_character_count(500);
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);

        // Basic user with 2000 characters - denied
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new().with_character_count(2000);
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);

        // TTS extended user with 2000 characters - allowed
        let subject =
            Subject::new().with_roles(vec!["user".to_string(), "tts_extended".to_string()]);
        let request = RequestContext::new().with_character_count(2000);
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
    }

    #[test]
    fn test_multimodal_vision_policy() {
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Allow,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![PolicyConfig {
                name: "restrict-vision".to_string(),
                description: Some("Vision requires vision role".to_string()),
                resource: "model".to_string(),
                action: "use".to_string(),
                condition: "context.request.has_images && !('vision' in subject.roles)".to_string(),
                effect: PolicyEffect::Deny,
                priority: 100,
            }],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();

        // User without vision role, text only - allowed
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new().with_images(false);
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);

        // User without vision role, with images - denied
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let request = RequestContext::new().with_images(true);
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(!result.allowed);

        // User with vision role, with images - allowed
        let subject = Subject::new().with_roles(vec!["user".to_string(), "vision".to_string()]);
        let request = RequestContext::new().with_images(true);
        let context = PolicyContext::new("model", "use").with_request(request);
        let result = engine.authorize(&subject, &context);
        assert!(result.allowed);
    }

    #[test]
    fn test_prefix_wildcard_patterns() {
        // Test that prefix wildcards (e.g., "team*") work correctly
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Deny,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![
                PolicyConfig {
                    name: "team-wildcard".to_string(),
                    description: Some("Allow all team-related resources".to_string()),
                    resource: "team*".to_string(),
                    action: "*".to_string(),
                    condition: "true".to_string(),
                    effect: PolicyEffect::Allow,
                    priority: 50,
                },
                PolicyConfig {
                    name: "read-wildcard".to_string(),
                    description: Some("Allow all read-like actions".to_string()),
                    resource: "*".to_string(),
                    action: "read*".to_string(),
                    condition: "true".to_string(),
                    effect: PolicyEffect::Allow,
                    priority: 40,
                },
            ],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();
        let subject = Subject::new();

        // team* should match: team, teams, team_admin, team_member
        let result = engine.authorize(&subject, &PolicyContext::new("team", "write"));
        assert!(result.allowed, "team should match team*");
        assert_eq!(result.policy_name, Some("team-wildcard".to_string()));

        let result = engine.authorize(&subject, &PolicyContext::new("teams", "write"));
        assert!(result.allowed, "teams should match team*");

        let result = engine.authorize(&subject, &PolicyContext::new("team_admin", "write"));
        assert!(result.allowed, "team_admin should match team*");

        let result = engine.authorize(&subject, &PolicyContext::new("team_member", "write"));
        assert!(result.allowed, "team_member should match team*");

        // team* should NOT match: project, organization
        let result = engine.authorize(&subject, &PolicyContext::new("project", "write"));
        assert!(!result.allowed, "project should NOT match team*");

        let result = engine.authorize(&subject, &PolicyContext::new("organization", "write"));
        assert!(!result.allowed, "organization should NOT match team*");

        // read* should match: read, read_all, readonly
        let result = engine.authorize(&subject, &PolicyContext::new("project", "read"));
        assert!(result.allowed, "read should match read*");
        assert_eq!(result.policy_name, Some("read-wildcard".to_string()));

        let result = engine.authorize(&subject, &PolicyContext::new("project", "read_all"));
        assert!(result.allowed, "read_all should match read*");

        let result = engine.authorize(&subject, &PolicyContext::new("project", "readonly"));
        assert!(result.allowed, "readonly should match read*");

        // read* should NOT match: write, delete
        let result = engine.authorize(&subject, &PolicyContext::new("project", "write"));
        assert!(!result.allowed, "write should NOT match read*");

        let result = engine.authorize(&subject, &PolicyContext::new("project", "delete"));
        assert!(!result.allowed, "delete should NOT match read*");
    }

    #[test]
    fn test_fail_on_evaluation_error_default_denies() {
        // Default behavior (fail_on_evaluation_error = true): deny on CEL evaluation error
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Allow, // Would allow if no policy matched
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            // This policy condition compiles but returns a non-boolean (integer),
            // which causes a runtime evaluation error
            policies: vec![PolicyConfig {
                name: "bad-policy".to_string(),
                description: Some("Policy with runtime error".to_string()),
                resource: "*".to_string(),
                action: "*".to_string(),
                condition: "1".to_string(), // Returns integer, not boolean
                effect: PolicyEffect::Allow,
                priority: 100,
            }],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            // Default is true (fail-closed)
            fail_on_evaluation_error: true,
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let context = PolicyContext::new("test", "read");

        let result = engine.authorize(&subject, &context);

        // Should be denied due to evaluation error (fail-closed)
        assert!(!result.allowed);
        assert_eq!(result.policy_name, Some("bad-policy".to_string()));
        assert!(
            result
                .reason
                .as_ref()
                .unwrap()
                .contains("failed to evaluate")
        );
    }

    #[test]
    fn test_fail_on_evaluation_error_false_skips_policy() {
        // With fail_on_evaluation_error = false: skip erroring policy and continue
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Allow, // Will apply since bad policy is skipped
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![PolicyConfig {
                name: "bad-policy".to_string(),
                description: Some("Policy with runtime error".to_string()),
                resource: "*".to_string(),
                action: "*".to_string(),
                condition: "1".to_string(), // Returns integer, not boolean
                effect: PolicyEffect::Deny,
                priority: 100,
            }],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: false, // Skip erroring policies
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let context = PolicyContext::new("test", "read");

        let result = engine.authorize(&subject, &context);

        // Should be allowed because:
        // 1. Bad policy is skipped (evaluation error)
        // 2. No other policy matches
        // 3. Default effect is Allow
        assert!(result.allowed);
        assert!(result.policy_name.is_none()); // Fell through to default
    }

    #[test]
    fn test_fail_on_evaluation_error_continues_to_next_policy() {
        // With fail_on_evaluation_error = false: should continue to next policy
        let config = RbacConfig {
            enabled: true,
            default_effect: PolicyEffect::Deny,
            role_claim: "roles".to_string(),
            org_claim: None,
            team_claim: None,
            project_claim: None,
            role_mapping: Default::default(),
            policies: vec![
                PolicyConfig {
                    name: "bad-policy".to_string(),
                    description: Some("Policy with runtime error".to_string()),
                    resource: "*".to_string(),
                    action: "*".to_string(),
                    condition: "1".to_string(), // Returns integer, not boolean
                    effect: PolicyEffect::Deny,
                    priority: 100,
                },
                PolicyConfig {
                    name: "good-policy".to_string(),
                    description: Some("Working policy".to_string()),
                    resource: "*".to_string(),
                    action: "*".to_string(),
                    condition: "true".to_string(), // Always matches
                    effect: PolicyEffect::Allow,
                    priority: 50,
                },
            ],
            audit: Default::default(),
            gateway: Default::default(),
            policy_cache_ttl_ms: 1000,
            fail_on_evaluation_error: false, // Skip erroring policies
            lazy_load_policies: false,
            max_cached_orgs: 0,
            policy_eviction_batch_size: 100,
            max_expression_length: 4096,
        };

        let engine = AuthzEngine::new(config).unwrap();
        let subject = Subject::new().with_roles(vec!["user".to_string()]);
        let context = PolicyContext::new("test", "read");

        let result = engine.authorize(&subject, &context);

        // Should be allowed by good-policy after skipping bad-policy
        assert!(result.allowed);
        assert_eq!(result.policy_name, Some("good-policy".to_string()));
    }
}
