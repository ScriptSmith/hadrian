//! Authorization middleware for enforcing policy-based access control.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tokio_util::task::TaskTracker;
use uuid::Uuid;

use crate::{
    AppState,
    auth::AuthenticatedRequest,
    authz::{
        AuthzEngine, AuthzError, AuthzResult, PolicyContext, PolicyRegistry, RequestContext,
        Subject,
    },
    config::{AuthzAuditConfig, PolicyEffect},
    middleware::AdminAuth,
    models::{AuditActorType, CreateAuditLog},
    services::AuditLogService,
};

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
        // Only log if audit service is available
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

/// Middleware that builds authorization context from the authenticated request.
/// This must run after admin_auth_middleware.
///
/// IMPORTANT: This middleware always inserts an AuthzContext, even when RBAC is disabled.
/// This ensures handlers can require AuthzContext (fail-closed) rather than using Option
/// (fail-open). When RBAC is disabled, the AuthzContext will allow all operations.
pub async fn authz_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AuthzResponse> {
    // Build the authorization engine from config
    let engine = match AuthzEngine::new(state.config.auth.rbac.clone()) {
        Ok(e) => Arc::new(e),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create authorization engine");
            return Err(AuthzResponse::InternalError(format!(
                "Authorization configuration error: {}",
                e
            )));
        }
    };

    // Extract request metadata for audit logging
    let request_ip = extract_client_ip(&req, &state.config.server.trusted_proxies);
    let request_user_agent = req
        .headers()
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Get audit service if available
    let audit_service = state.services.as_ref().map(|s| s.audit_logs.clone());

    // Build subject from AdminAuth if available, otherwise use empty subject
    // When RBAC is disabled, the engine will allow all operations regardless of subject
    let subject = if let Some(admin_auth) = req.extensions().get::<AdminAuth>() {
        let identity = &admin_auth.identity;
        let mapped_roles = engine.map_roles(&identity.roles);

        let subject = Subject::new()
            .with_external_id(&identity.external_id)
            .with_roles(mapped_roles)
            .with_org_ids(identity.org_ids.clone())
            .with_team_ids(identity.team_ids.clone())
            .with_project_ids(identity.project_ids.clone());

        // Add optional fields
        let subject = if let Some(user_id) = identity.user_id {
            subject.with_user_id(user_id.to_string())
        } else {
            subject
        };
        if let Some(email) = &identity.email {
            subject.with_email(email)
        } else {
            subject
        }
    } else if engine.is_enabled() {
        // RBAC is enabled but no AdminAuth - this is an error
        return Err(AuthzResponse::Unauthorized("Not authenticated".to_string()));
    } else {
        // RBAC is disabled, use empty subject (engine will allow all)
        Subject::new()
    };

    // Always add authz context to request (fail-closed pattern)
    // Admin endpoints use the main RBAC default effect (typically "deny")
    req.extensions_mut().insert(AuthzContext {
        subject,
        engine,
        registry: state.policy_registry.clone(),
        audit_service,
        task_tracker: Some(state.task_tracker.clone()),
        request_ip,
        request_user_agent,
        audit_config: state.config.auth.rbac.audit.clone(),
        api_default_effect: state.config.auth.rbac.default_effect,
    });

    Ok(next.run(req).await)
}

/// Extract client IP address from request, respecting trusted proxy configuration.
fn extract_client_ip(
    req: &Request,
    trusted_proxies: &crate::config::TrustedProxiesConfig,
) -> Option<String> {
    super::rate_limit::extract_client_ip(req, trusted_proxies).map(|ip| ip.to_string())
}

/// Permissive authorization middleware for unprotected (development) routes.
///
/// This middleware always inserts an AuthzContext with RBAC disabled,
/// which means all authorization checks will pass. This is used when
/// auth is not configured (development mode or external auth proxy).
///
/// SECURITY NOTE: Only use this for development or when an external auth
/// proxy handles authentication/authorization.
pub async fn permissive_authz_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AuthzResponse> {
    // Create a disabled RBAC config
    let mut disabled_config = state.config.auth.rbac.clone();
    disabled_config.enabled = false;

    let engine = match AuthzEngine::new(disabled_config) {
        Ok(e) => Arc::new(e),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create authorization engine");
            return Err(AuthzResponse::InternalError(format!(
                "Authorization configuration error: {}",
                e
            )));
        }
    };

    // Insert ClientInfo for unprotected routes (no admin middleware to extract it).
    req.extensions_mut().insert(super::ClientInfo::default());

    // Insert a default AdminAuth with system identity for unprotected routes.
    // This allows handlers to extract AdminAuth for audit logging purposes.
    // Use the default_user_id and default_org_id if available (for anonymous access).
    use crate::auth::Identity;
    req.extensions_mut().insert(AdminAuth {
        identity: Identity {
            external_id: "anonymous".to_string(),
            email: Some("anonymous@localhost".to_string()),
            name: Some("Anonymous User".to_string()),
            user_id: state.default_user_id,
            roles: vec!["admin".to_string()],
            idp_groups: Vec::new(),
            org_ids: state
                .default_org_id
                .map(|id| vec![id.to_string()])
                .unwrap_or_default(),
            team_ids: Vec::new(),
            project_ids: Vec::new(),
        },
    });

    // Insert permissive AuthzContext with empty subject
    // Since RBAC is disabled, all authorization checks will pass (no denials to log)
    req.extensions_mut().insert(AuthzContext {
        subject: Subject::new(),
        engine,
        registry: None,      // No registry in permissive mode
        audit_service: None, // No audit logging in permissive mode
        task_tracker: None,
        request_ip: None,
        request_user_agent: None,
        audit_config: AuthzAuditConfig::default(), // Use defaults (no logging in permissive mode anyway)
        api_default_effect: PolicyEffect::Allow,   // Permissive mode always allows
    });

    Ok(next.run(req).await)
}

/// Authorization middleware for API endpoints (`/v1/*`).
///
/// This middleware builds authorization context from `AuthenticatedRequest`
/// (set by `api_middleware`). Unlike admin authz, this supports:
/// - API key authentication (from `ApiKeyAuth`)
/// - Identity authentication (from proxy headers or OIDC)
/// - Combined authentication (both API key and identity)
///
/// Configuration (`auth.rbac.gateway`):
/// - `enabled`: When false (default), all API requests are allowed (backwards compatible)
/// - `default_effect`: Default is "allow" (fail-open) for backwards compatibility
///
/// When API RBAC is disabled, an AuthzContext is still created with a permissive engine
/// so handlers can optionally call authorization methods without errors.
///
/// IMPORTANT: This must run AFTER `api_middleware` which sets up `AuthenticatedRequest`.
pub async fn api_authz_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AuthzResponse> {
    // Check if API RBAC is enabled
    let api_rbac_config = &state.config.auth.rbac.gateway;

    // Build the authorization engine
    // If API RBAC is disabled, create an engine with enabled=false so all checks pass
    let rbac_config = if api_rbac_config.enabled {
        // Use main RBAC config with API-specific default effect
        let mut config = state.config.auth.rbac.clone();
        config.enabled = true;
        config.default_effect = api_rbac_config.default_effect;
        config
    } else {
        // Create a disabled config - all authorization checks will pass
        let mut config = state.config.auth.rbac.clone();
        config.enabled = false;
        config
    };

    let engine = match AuthzEngine::new(rbac_config) {
        Ok(e) => Arc::new(e),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create authorization engine");
            return Err(AuthzResponse::InternalError(format!(
                "Authorization configuration error: {}",
                e
            )));
        }
    };

    // Extract request metadata for audit logging
    let request_ip = extract_client_ip(&req, &state.config.server.trusted_proxies);
    let request_user_agent = req
        .headers()
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Get audit service if available
    let audit_service = state.services.as_ref().map(|s| s.audit_logs.clone());

    // Build subject from AuthenticatedRequest if available
    let subject = if let Some(auth) = req.extensions().get::<AuthenticatedRequest>() {
        build_subject_from_auth(auth, &engine)
    } else if engine.is_enabled() {
        // RBAC is enabled but no auth - use empty subject
        // The policy's default_effect will determine if this is allowed
        Subject::new()
    } else {
        // RBAC is disabled, use empty subject (engine will allow all)
        Subject::new()
    };

    // Always add authz context to request
    // API endpoints use the API-specific default effect (typically "allow" for backwards compatibility)
    req.extensions_mut().insert(AuthzContext {
        subject,
        engine,
        registry: state.policy_registry.clone(),
        audit_service,
        task_tracker: Some(state.task_tracker.clone()),
        request_ip,
        request_user_agent,
        audit_config: state.config.auth.rbac.audit.clone(),
        api_default_effect: api_rbac_config.default_effect,
    });

    Ok(next.run(req).await)
}

/// Build a Subject from AuthenticatedRequest.
///
/// Extracts identity information from the authenticated request,
/// prioritizing identity auth over API key auth when both are present.
///
/// For service account-owned API keys, the service account's roles are
/// mapped through the role_mappings configuration and injected into the subject,
/// enabling RBAC evaluation for machine identities.
fn build_subject_from_auth(auth: &AuthenticatedRequest, engine: &AuthzEngine) -> Subject {
    // Start with empty subject
    let mut subject = Subject::new();

    // Extract from API key if present
    if let Some(api_key) = auth.api_key() {
        // API key provides org_id, team_id, project_id, user_id
        if let Some(org_id) = api_key.org_id {
            subject = subject.with_org_ids(vec![org_id.to_string()]);
        }
        if let Some(team_id) = api_key.team_id {
            subject = subject.with_team_ids(vec![team_id.to_string()]);
        }
        if let Some(project_id) = api_key.project_id {
            subject = subject.with_project_ids(vec![project_id.to_string()]);
        }
        if let Some(user_id) = api_key.user_id {
            subject = subject.with_user_id(user_id.to_string());
        }

        // For service account-owned API keys, inject the service account ID and roles
        if let Some(service_account_id) = api_key.service_account_id {
            subject = subject.with_service_account_id(service_account_id.to_string());

            // Map service account roles through the role_mappings config
            if let Some(sa_roles) = &api_key.service_account_roles {
                let mapped_roles = engine.map_roles(sa_roles);
                subject = subject.with_roles(mapped_roles);
            }
        }
    }

    // Extract from identity if present (overrides API key values including SA roles)
    if let Some(identity) = auth.identity() {
        let mapped_roles = engine.map_roles(&identity.roles);
        subject = subject
            .with_external_id(&identity.external_id)
            .with_roles(mapped_roles);

        // Use identity's org/team/project IDs if available
        if !identity.org_ids.is_empty() {
            subject = subject.with_org_ids(identity.org_ids.clone());
        }
        if !identity.team_ids.is_empty() {
            subject = subject.with_team_ids(identity.team_ids.clone());
        }
        if !identity.project_ids.is_empty() {
            subject = subject.with_project_ids(identity.project_ids.clone());
        }

        // Use identity's user_id if available
        if let Some(user_id) = identity.user_id {
            subject = subject.with_user_id(user_id.to_string());
        }
        if let Some(email) = &identity.email {
            subject = subject.with_email(email);
        }
    }

    subject
}

/// Response type for authorization errors.
#[derive(Debug)]
pub enum AuthzResponse {
    Unauthorized(String),
    #[allow(dead_code)] // Error variant for completeness in authorization responses
    Forbidden(String),
    InternalError(String),
}

impl IntoResponse for AuthzResponse {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg),
            Self::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg),
            Self::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", msg),
        };

        let body = crate::openapi::ErrorResponse::new(code, message);

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;
    use crate::{
        auth::{ApiKeyAuth, AuthenticatedRequest, Identity, IdentityKind},
        config::RbacConfig,
        models::{ApiKey, ApiKeyOwner},
    };

    fn test_engine() -> AuthzEngine {
        AuthzEngine::new(RbacConfig::default()).expect("Failed to create test engine")
    }

    fn make_test_api_key() -> ApiKey {
        ApiKey {
            id: Uuid::new_v4(),
            key_prefix: "test_".to_string(),
            name: "Test Key".to_string(),
            owner: ApiKeyOwner::Organization {
                org_id: Uuid::new_v4(),
            },
            budget_limit_cents: None,
            budget_period: None,
            created_at: Utc::now(),
            expires_at: None,
            revoked_at: None,
            last_used_at: None,
            scopes: None,
            allowed_models: None,
            ip_allowlist: None,
            rate_limit_rpm: None,
            rate_limit_tpm: None,
            rotated_from_key_id: None,
            rotation_grace_until: None,
        }
    }

    #[test]
    fn test_build_subject_service_account_roles() {
        let engine = test_engine();

        // Create an API key owned by a service account with roles
        let sa_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(),
            org_id: Some(org_id),
            team_id: None,
            project_id: None,
            user_id: None,
            service_account_id: Some(sa_id),
            service_account_roles: Some(vec!["deployer".to_string(), "viewer".to_string()]),
        };

        let auth = AuthenticatedRequest::new(IdentityKind::ApiKey(api_key_auth));
        let subject = build_subject_from_auth(&auth, &engine);

        // Verify service account ID is set
        assert_eq!(subject.service_account_id, Some(sa_id.to_string()));

        // Verify roles are set from service account
        assert_eq!(subject.roles, vec!["deployer", "viewer"]);

        // Verify org_id is set
        assert_eq!(subject.org_ids, vec![org_id.to_string()]);
    }

    #[test]
    fn test_build_subject_service_account_role_mapping() {
        // Create engine with role mappings
        let config = RbacConfig {
            role_mapping: [("deployer".to_string(), "deploy_admin".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let engine = AuthzEngine::new(config).expect("Failed to create test engine");

        let sa_id = Uuid::new_v4();
        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(),
            org_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            service_account_id: Some(sa_id),
            service_account_roles: Some(vec!["deployer".to_string()]),
        };

        let auth = AuthenticatedRequest::new(IdentityKind::ApiKey(api_key_auth));
        let subject = build_subject_from_auth(&auth, &engine);

        // Verify role is mapped through role_mapping
        assert_eq!(subject.roles, vec!["deploy_admin"]);
    }

    #[test]
    fn test_build_subject_identity_overrides_service_account_roles() {
        let engine = test_engine();

        let sa_id = Uuid::new_v4();
        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(),
            org_id: None,
            team_id: None,
            project_id: None,
            user_id: None,
            service_account_id: Some(sa_id),
            service_account_roles: Some(vec!["sa_role".to_string()]),
        };

        let identity = Identity {
            external_id: "user@example.com".to_string(),
            email: Some("user@example.com".to_string()),
            name: Some("Test User".to_string()),
            user_id: None,
            roles: vec!["identity_role".to_string()],
            idp_groups: vec![],
            org_ids: vec![],
            team_ids: vec![],
            project_ids: vec![],
        };

        let auth = AuthenticatedRequest::new(IdentityKind::Both {
            api_key: Box::new(api_key_auth),
            identity,
        });
        let subject = build_subject_from_auth(&auth, &engine);

        // Identity roles should override service account roles
        assert_eq!(subject.roles, vec!["identity_role"]);

        // Service account ID should still be set
        assert_eq!(subject.service_account_id, Some(sa_id.to_string()));
    }

    #[test]
    fn test_build_subject_api_key_without_service_account() {
        let engine = test_engine();

        // API key owned by an organization (not a service account)
        let org_id = Uuid::new_v4();
        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(),
            org_id: Some(org_id),
            team_id: None,
            project_id: None,
            user_id: None,
            service_account_id: None,
            service_account_roles: None,
        };

        let auth = AuthenticatedRequest::new(IdentityKind::ApiKey(api_key_auth));
        let subject = build_subject_from_auth(&auth, &engine);

        // No service account ID
        assert!(subject.service_account_id.is_none());

        // No roles (no identity auth, no service account)
        assert!(subject.roles.is_empty());

        // Org ID should still be set
        assert_eq!(subject.org_ids, vec![org_id.to_string()]);
    }
}
