//! Middleware types extracted by middleware layers and consumed by route handlers.
//!
//! These types are always available on all targets (including WASM) so that
//! route handlers can compile without the server-only middleware *functions*.

use std::sync::Arc;

#[cfg(feature = "server")]
use serde_json::json;
#[cfg(feature = "server")]
use tokio_util::task::TaskTracker;
use uuid::Uuid;

#[cfg(feature = "server")]
use crate::models::{AuditActorType, CreateAuditLog};
use crate::{
    auth::Identity,
    authz::{
        AuthzEngine, AuthzError, AuthzResult, PolicyContext, PolicyRegistry, RequestContext,
        Subject,
    },
    config::{AuthzAuditConfig, PolicyEffect},
    services::AuditLogService,
};

/// Client connection metadata extracted by middleware for audit logging.
#[derive(Debug, Clone, Default)]
pub struct ClientInfo {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Admin authentication result.
#[derive(Debug, Clone)]
pub struct AdminAuth {
    /// The authenticated identity
    pub identity: Identity,
}

/// Extension containing the request ID for the current request.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

impl RequestId {
    /// Generate a new request ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing ID.
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Authorization context extracted from request.
#[derive(Clone)]
pub struct AuthzContext {
    pub subject: Subject,
    pub engine: Arc<AuthzEngine>,
    /// Per-organization policy registry for org-scoped authorization.
    /// When available, allows evaluation of org-specific RBAC policies.
    pub registry: Option<Arc<PolicyRegistry>>,
    /// Audit log service for logging authorization decisions (optional)
    audit_service: Option<AuditLogService>,
    /// Task tracker for async logging
    #[cfg(feature = "server")]
    task_tracker: Option<TaskTracker>,
    /// Request metadata for audit logs
    request_ip: Option<String>,
    request_user_agent: Option<String>,
    /// Audit logging configuration
    audit_config: AuthzAuditConfig,
    /// Default effect for API authorization when no policy matches.
    /// This allows API endpoints to have a different default (e.g., "allow")
    /// than admin endpoints (e.g., "deny").
    api_default_effect: PolicyEffect,
}

impl AuthzContext {
    /// Create a permissive authorization context (RBAC disabled, all checks pass).
    ///
    /// Always available on all targets. Used by WASM and development routes.
    pub fn permissive(engine: Arc<AuthzEngine>) -> Self {
        Self {
            subject: Subject::new(),
            engine,
            registry: None,
            audit_service: None,
            #[cfg(feature = "server")]
            task_tracker: None,
            request_ip: None,
            request_user_agent: None,
            audit_config: AuthzAuditConfig::default(),
            api_default_effect: PolicyEffect::Allow,
        }
    }

    /// Create a full authorization context (server middleware only).
    #[cfg(feature = "server")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        subject: Subject,
        engine: Arc<AuthzEngine>,
        registry: Option<Arc<PolicyRegistry>>,
        audit_service: Option<AuditLogService>,
        task_tracker: Option<TaskTracker>,
        request_ip: Option<String>,
        request_user_agent: Option<String>,
        audit_config: AuthzAuditConfig,
        api_default_effect: PolicyEffect,
    ) -> Self {
        Self {
            subject,
            engine,
            registry,
            audit_service,
            task_tracker,
            request_ip,
            request_user_agent,
            audit_config,
            api_default_effect,
        }
    }

    /// Check if the subject is authorized for an action on a resource.
    ///
    /// Parameters:
    /// - `resource`: The type of resource being accessed (e.g., "team", "project")
    /// - `action`: The action being performed (e.g., "read", "create", "delete")
    /// - `resource_id`: The specific resource ID being accessed
    /// - `org_id`: The organization scope
    /// - `team_id`: The team scope (if applicable)
    /// - `project_id`: The project scope (if applicable)
    pub fn authorize(
        &self,
        resource: &str,
        action: &str,
        resource_id: Option<&str>,
        org_id: Option<&str>,
        team_id: Option<&str>,
        project_id: Option<&str>,
    ) -> AuthzResult {
        let mut context = PolicyContext::new(resource, action);
        if let Some(id) = resource_id {
            context = context.with_resource_id(id);
        }
        if let Some(id) = org_id {
            context = context.with_org_id(id);
        }
        if let Some(id) = team_id {
            context = context.with_team_id(id);
        }
        if let Some(id) = project_id {
            context = context.with_project_id(id);
        }
        self.engine.authorize(&self.subject, &context)
    }

    /// Check authorization and return an error if denied.
    /// Logs authorization decisions based on audit configuration.
    ///
    /// This method evaluates **system policies only** (from config file). It does NOT
    /// evaluate per-organization policies from the database. This is by design:
    ///
    /// - **Admin endpoints** use `require()` - controlled by platform operators via system policies
    /// - **API endpoints** use `require_api()` - also evaluates org policies for customer-specific rules
    ///
    /// This separation ensures that:
    /// 1. Admin operations are governed by platform-wide rules (simpler, more predictable)
    /// 2. Org admins can customize API access (model usage, rate limits) without affecting admin operations
    /// 3. Synchronous evaluation avoids async complexity in admin handlers
    ///
    /// Parameters:
    /// - `resource`: The type of resource being accessed (e.g., "team", "project")
    /// - `action`: The action being performed (e.g., "read", "create", "delete")
    /// - `resource_id`: The specific resource ID being accessed
    /// - `org_id`: The organization scope
    /// - `team_id`: The team scope (if applicable)
    /// - `project_id`: The project scope (if applicable)
    pub fn require(
        &self,
        resource: &str,
        action: &str,
        resource_id: Option<&str>,
        org_id: Option<&str>,
        team_id: Option<&str>,
        project_id: Option<&str>,
    ) -> Result<(), AuthzError> {
        let result = self.authorize(resource, action, resource_id, org_id, team_id, project_id);
        if result.allowed {
            // Log allowed decisions if configured
            if self.audit_config.log_allowed {
                self.log_authorization_decision(
                    resource,
                    action,
                    resource_id,
                    org_id,
                    team_id,
                    project_id,
                    &result,
                );
            }
            Ok(())
        } else {
            // Log denied decisions if configured
            if self.audit_config.log_denied {
                self.log_authorization_decision(
                    resource,
                    action,
                    resource_id,
                    org_id,
                    team_id,
                    project_id,
                    &result,
                );
            }
            Err(AuthzError::AccessDenied(
                result.reason.unwrap_or_else(|| "Access denied".to_string()),
            ))
        }
    }

    /// Log an authorization decision asynchronously.
    /// Logs to the audit log with full context for security monitoring.
    #[allow(clippy::too_many_arguments)]
    fn log_authorization_decision(
        &self,
        resource: &str,
        action: &str,
        resource_id: Option<&str>,
        org_id: Option<&str>,
        team_id: Option<&str>,
        project_id: Option<&str>,
        result: &AuthzResult,
    ) {
        // Only log if audit service and task tracker are available
        #[cfg(not(feature = "server"))]
        {
            let _ = (
                resource,
                action,
                resource_id,
                org_id,
                team_id,
                project_id,
                result,
            );
            return;
        }
        #[cfg(feature = "server")]
        {
            let (Some(audit_service), Some(task_tracker)) =
                (self.audit_service.clone(), self.task_tracker.clone())
            else {
                return;
            };

            // Build audit log entry
            let actor_type = if self.subject.user_id.is_some() {
                AuditActorType::User
            } else {
                AuditActorType::System
            };

            let actor_id = self
                .subject
                .user_id
                .as_ref()
                .and_then(|id| Uuid::parse_str(id).ok());

            // Use provided resource_id or generate a nil UUID for the audit log
            let audit_resource_id = resource_id
                .and_then(|id| Uuid::parse_str(id).ok())
                .unwrap_or_else(Uuid::nil);

            let parsed_org_id = org_id.and_then(|id| Uuid::parse_str(id).ok());
            let parsed_project_id = project_id.and_then(|id| Uuid::parse_str(id).ok());

            // Build details JSON with authorization context
            let details = json!({
                "decision": if result.allowed { "allow" } else { "deny" },
                "policy_name": result.policy_name,
                "reason": result.reason,
                "resource": resource,
                "action": action,
                "org_id": org_id,
                "team_id": team_id,
                "project_id": project_id,
                "resource_id": resource_id,
                "subject": {
                    "user_id": self.subject.user_id,
                    "external_id": self.subject.external_id,
                    "email": self.subject.email,
                    "roles": self.subject.roles,
                    "team_ids": self.subject.team_ids,
                }
            });

            let audit_action = format!("authz.{}", if result.allowed { "allow" } else { "deny" });
            let ip_address = self.request_ip.clone();
            let user_agent = self.request_user_agent.clone();
            let resource_type = resource.to_string();

            // Spawn async task to write audit log (non-blocking)
            task_tracker.spawn(async move {
                let entry = CreateAuditLog {
                    actor_type,
                    actor_id,
                    action: audit_action,
                    resource_type,
                    resource_id: audit_resource_id,
                    org_id: parsed_org_id,
                    project_id: parsed_project_id,
                    details,
                    ip_address,
                    user_agent,
                };

                if let Err(e) = audit_service.create(entry).await {
                    tracing::warn!(
                        error = %e,
                        "Failed to log authorization decision to audit log"
                    );
                }
            });
        }
    }

    /// Check if the subject has a specific role.
    #[allow(dead_code)] // Public API for CEL policy evaluation
    pub fn has_role(&self, role: &str) -> bool {
        self.subject.has_role(role)
    }

    /// Check if the subject is a member of an organization.
    #[allow(dead_code)] // Public API for CEL policy evaluation
    pub fn is_org_member(&self, org_id: &str) -> bool {
        self.subject.is_org_member(org_id)
    }

    /// Check if the subject is a member of a team.
    #[allow(dead_code)] // Public API for CEL policy evaluation
    pub fn is_team_member(&self, team_id: &str) -> bool {
        self.subject.is_team_member(team_id)
    }

    /// Check if the subject is a member of a project.
    #[allow(dead_code)] // Public API for CEL policy evaluation
    pub fn is_project_member(&self, project_id: &str) -> bool {
        self.subject.is_project_member(project_id)
    }

    /// Authorize an API request with model and request-specific context.
    ///
    /// This is used for `/v1/*` API endpoints where authorization depends on
    /// the specific model, request parameters, and time of day.
    ///
    /// Parameters:
    /// - `resource`: The resource type (e.g., "model", "chat", "embeddings")
    /// - `action`: The action being performed (e.g., "use", "complete")
    /// - `model`: The model being requested (e.g., "gpt-4o", "claude-3-opus")
    /// - `request`: Request-specific context (tokens, tools, etc.)
    /// - `org_id`: Organization scope (from API key or identity)
    /// - `project_id`: Project scope (from API key)
    #[allow(dead_code)] // Public API for CEL policy evaluation on API endpoints
    pub fn authorize_api(
        &self,
        resource: &str,
        action: &str,
        model: Option<&str>,
        request: Option<RequestContext>,
        org_id: Option<&str>,
        project_id: Option<&str>,
    ) -> AuthzResult {
        let mut context = PolicyContext::new(resource, action).with_current_time();

        if let Some(m) = model {
            context = context.with_model(m);
        }
        if let Some(req) = request {
            context = context.with_request(req);
        }
        if let Some(id) = org_id {
            context = context.with_org_id(id);
        }
        if let Some(id) = project_id {
            context = context.with_project_id(id);
        }

        self.engine.authorize(&self.subject, &context)
    }

    /// Check API authorization and return an error if denied.
    /// Logs authorization decisions based on audit configuration.
    ///
    /// This method evaluates both system policies (from config) and org policies
    /// (from database) when a PolicyRegistry is available and org_id is provided.
    ///
    /// Parameters:
    /// - `resource`: The resource type (e.g., "model", "chat", "embeddings")
    /// - `action`: The action being performed (e.g., "use", "complete")
    /// - `model`: The model being requested
    /// - `request`: Request-specific context (tokens, tools, etc.)
    /// - `org_id`: Organization scope
    /// - `project_id`: Project scope
    pub async fn require_api(
        &self,
        resource: &str,
        action: &str,
        model: Option<&str>,
        request: Option<RequestContext>,
        org_id: Option<&str>,
        project_id: Option<&str>,
    ) -> Result<(), AuthzError> {
        // Build the policy context
        let mut context = PolicyContext::new(resource, action).with_current_time();

        if let Some(m) = model {
            context = context.with_model(m);
        }
        if let Some(req) = request.clone() {
            context = context.with_request(req);
        }
        if let Some(id) = org_id {
            context = context.with_org_id(id);
        }
        if let Some(id) = project_id {
            context = context.with_project_id(id);
        }

        // Evaluate using registry if available (includes org policies), otherwise engine only
        // Use the API-specific default effect when no policy matches
        let result = if let Some(ref registry) = self.registry {
            let parsed_org_id = org_id.and_then(|id| Uuid::parse_str(id).ok());
            registry
                .authorize_with_org_and_default(
                    parsed_org_id,
                    &self.subject,
                    &context,
                    self.api_default_effect,
                )
                .await
        } else {
            // No registry available, use system policies only
            self.engine.authorize(&self.subject, &context)
        };

        if result.allowed {
            if self.audit_config.log_allowed {
                self.log_api_authorization_decision(
                    resource,
                    action,
                    model,
                    request.as_ref(),
                    org_id,
                    project_id,
                    &result,
                );
            }
            Ok(())
        } else {
            if self.audit_config.log_denied {
                self.log_api_authorization_decision(
                    resource,
                    action,
                    model,
                    request.as_ref(),
                    org_id,
                    project_id,
                    &result,
                );
            }
            Err(AuthzError::AccessDenied(
                result.reason.unwrap_or_else(|| "Access denied".to_string()),
            ))
        }
    }

    /// Log an API authorization decision asynchronously.
    #[allow(clippy::too_many_arguments)]
    fn log_api_authorization_decision(
        &self,
        resource: &str,
        action: &str,
        model: Option<&str>,
        request: Option<&RequestContext>,
        org_id: Option<&str>,
        project_id: Option<&str>,
        result: &AuthzResult,
    ) {
        #[cfg(not(feature = "server"))]
        {
            let _ = (resource, action, model, request, org_id, project_id, result);
            return;
        }
        #[cfg(feature = "server")]
        {
            let (Some(audit_service), Some(task_tracker)) =
                (self.audit_service.clone(), self.task_tracker.clone())
            else {
                return;
            };

            let actor_type = if self.subject.user_id.is_some() {
                AuditActorType::User
            } else {
                AuditActorType::System
            };

            let actor_id = self
                .subject
                .user_id
                .as_ref()
                .and_then(|id| Uuid::parse_str(id).ok());

            let parsed_org_id = org_id.and_then(|id| Uuid::parse_str(id).ok());
            let parsed_project_id = project_id.and_then(|id| Uuid::parse_str(id).ok());

            let details = json!({
                "decision": if result.allowed { "allow" } else { "deny" },
                "policy_name": result.policy_name,
                "reason": result.reason,
                "resource": resource,
                "action": action,
                "model": model,
                "request": request.map(|r| json!({
                    "max_tokens": r.max_tokens,
                    "messages_count": r.messages_count,
                    "has_tools": r.has_tools,
                    "has_file_search": r.has_file_search,
                    "stream": r.stream,
                })),
                "org_id": org_id,
                "project_id": project_id,
                "subject": {
                    "user_id": self.subject.user_id,
                    "external_id": self.subject.external_id,
                    "email": self.subject.email,
                    "roles": self.subject.roles,
                }
            });

            let audit_action = format!(
                "api_authz.{}",
                if result.allowed { "allow" } else { "deny" }
            );
            let ip_address = self.request_ip.clone();
            let user_agent = self.request_user_agent.clone();
            let resource_type = resource.to_string();

            task_tracker.spawn(async move {
                let entry = CreateAuditLog {
                    actor_type,
                    actor_id,
                    action: audit_action,
                    resource_type,
                    resource_id: Uuid::nil(), // API requests don't have a single resource ID
                    org_id: parsed_org_id,
                    project_id: parsed_project_id,
                    details,
                    ip_address,
                    user_agent,
                };

                if let Err(e) = audit_service.create(entry).await {
                    tracing::warn!(
                        error = %e,
                        "Failed to log API authorization decision to audit log"
                    );
                }
            });
        }
    }
}
