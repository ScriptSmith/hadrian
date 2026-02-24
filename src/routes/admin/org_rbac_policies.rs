//! Admin API endpoints for per-organization RBAC policy management.
//!
//! Each organization can have multiple RBAC policies that extend the system-level
//! authorization rules. Policies use CEL expressions for flexible condition evaluation.

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_valid::Valid;
#[cfg(feature = "cel")]
use cel_interpreter::{Context, Program, Value, to_value};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use super::{AuditActor, error::AdminError, organizations::ListQuery};
#[cfg(feature = "cel")]
use crate::authz::AuthzEngine;
use crate::{
    AppState,
    authz::{PolicyContext, RequestContext, Subject, pattern_matches},
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    models::{
        CreateAuditLog, CreateOrgRbacPolicy, OrgRbacPolicy, OrgRbacPolicyVersion, RbacPolicyEffect,
        RollbackOrgRbacPolicy, UpdateOrgRbacPolicy,
    },
    openapi::PaginationMeta,
    services::Services,
};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Evaluate a CEL condition for policy simulation.
#[cfg(feature = "cel")]
fn evaluate_cel_condition(
    expression: &str,
    subject: &Subject,
    context: &PolicyContext,
) -> Result<bool, String> {
    // Compile the CEL expression
    let program = Program::compile(expression).map_err(|e| format!("Compile error: {}", e))?;

    // Build CEL context with subject and context variables
    let mut ctx = Context::default();

    let subject_value =
        to_value(subject).map_err(|e| format!("Failed to serialize subject: {}", e))?;
    ctx.add_variable("subject", subject_value);

    let context_value =
        to_value(context).map_err(|e| format!("Failed to serialize context: {}", e))?;
    ctx.add_variable("context", context_value);

    // Execute the CEL program
    let result = program
        .execute(&ctx)
        .map_err(|e| format!("Execution error: {}", e))?;

    match result {
        Value::Bool(b) => Ok(b),
        _ => Err("Policy condition must evaluate to boolean".to_string()),
    }
}

// ============================================================================
// Response Types
// ============================================================================

/// Paginated list of RBAC policies
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgRbacPolicyListResponse {
    /// List of RBAC policies
    pub data: Vec<OrgRbacPolicy>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

/// Paginated list of RBAC policy versions
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgRbacPolicyVersionListResponse {
    /// List of policy versions
    pub data: Vec<OrgRbacPolicyVersion>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

// ============================================================================
// Simulate Types
// ============================================================================

/// Subject information for policy simulation
#[derive(Debug, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SimulateSubject {
    /// User ID (internal)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// External ID from IdP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    /// Email address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// User's roles
    #[serde(default)]
    pub roles: Vec<String>,
    /// Organization IDs the user belongs to
    #[serde(default)]
    pub org_ids: Vec<String>,
    /// Team IDs the user belongs to
    #[serde(default)]
    pub team_ids: Vec<String>,
    /// Project IDs the user belongs to
    #[serde(default)]
    pub project_ids: Vec<String>,
    /// Service account ID (if simulating service account auth)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_id: Option<String>,
}

impl From<SimulateSubject> for Subject {
    fn from(s: SimulateSubject) -> Self {
        Subject {
            user_id: s.user_id,
            external_id: s.external_id,
            email: s.email,
            roles: s.roles,
            org_ids: s.org_ids,
            team_ids: s.team_ids,
            project_ids: s.project_ids,
            service_account_id: s.service_account_id,
        }
    }
}

/// Request context for policy simulation (matches RequestContext but all fields optional)
#[derive(Debug, Deserialize, Validate, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SimulateRequestContext {
    /// Maximum tokens requested
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    /// Number of messages in the conversation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages_count: Option<u64>,
    /// Whether the request includes tools/functions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_tools: Option<bool>,
    /// Whether the request includes file_search tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_file_search: Option<bool>,
    /// Whether streaming is requested
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Reasoning/thinking effort level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    /// Response format type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,
    /// Sampling temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Whether the request contains image content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_images: Option<bool>,
    /// Number of images to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_count: Option<u32>,
    /// Image size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_size: Option<String>,
    /// Image quality level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_quality: Option<String>,
    /// Character count for TTS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_count: Option<u64>,
    /// Voice name for TTS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    /// Language code for transcription
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Context information for policy simulation
#[derive(Debug, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SimulateContext {
    /// Resource type being accessed (e.g., "projects", "teams", "*")
    #[validate(length(min = 1, max = 128))]
    pub resource_type: String,
    /// Action being performed (e.g., "read", "write", "delete", "*")
    #[validate(length(min = 1, max = 64))]
    pub action: String,
    /// Optional resource ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    /// Optional organization ID context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
    /// Optional team ID context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    /// Optional project ID context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Model being requested (for API endpoints, e.g., "gpt-4o", "claude-3-opus")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Request-specific context (for API endpoints)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(nested)]
    pub request: Option<SimulateRequestContext>,
}

/// Request to simulate policy evaluation
#[derive(Debug, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SimulatePolicyRequest {
    /// Subject info to test (roles, user_id, etc.)
    #[validate(nested)]
    pub subject: SimulateSubject,
    /// Context to test (resource_type, action, etc.)
    #[validate(nested)]
    pub context: SimulateContext,
    /// Optional: Only test a specific policy by ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
}

/// Source of an RBAC policy (system config vs organization database)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum PolicySource {
    /// Policy from system config file (hadrian.toml)
    System,
    /// Policy from organization database
    Organization,
}

/// Result of evaluating a single policy
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PolicyEvaluationResult {
    /// Policy name
    pub name: String,
    /// Policy ID (None for system policies)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    /// Source of this policy (system or organization)
    pub source: PolicySource,
    /// Policy description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the policy's resource/action pattern matched
    pub pattern_matched: bool,
    /// Whether the policy's CEL condition evaluated to true
    pub condition_matched: Option<bool>,
    /// Reason why condition was not evaluated (e.g., "Policy is disabled")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_reason: Option<String>,
    /// Policy effect (allow/deny)
    pub effect: RbacPolicyEffect,
    /// Policy priority
    pub priority: i32,
    /// Error message if condition evaluation failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response from policy simulation
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SimulatePolicyResponse {
    /// Whether RBAC is enabled (if false, all requests are allowed)
    pub rbac_enabled: bool,
    /// Whether the request would be allowed
    pub allowed: bool,
    /// Which policy determined the decision (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_policy: Option<String>,
    /// Source of the matched policy (system or organization)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_policy_source: Option<PolicySource>,
    /// Reason for the decision
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// System policies evaluated (from config file, in priority order)
    pub system_policies_evaluated: Vec<PolicyEvaluationResult>,
    /// Organization policies evaluated (from database, in priority order)
    pub org_policies_evaluated: Vec<PolicyEvaluationResult>,
}

// ============================================================================
// Validate Types
// ============================================================================

/// Request to validate a CEL expression
#[derive(Debug, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ValidateCelRequest {
    /// CEL expression to validate
    #[validate(length(min = 1, max = 4096))]
    pub condition: String,
}

/// Response from CEL validation
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ValidateCelResponse {
    /// Whether the expression is valid
    pub valid: bool,
    /// Error message if invalid
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// CRUD Endpoints
// ============================================================================

/// List RBAC policies for an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/rbac-policies",
    tag = "rbac-policies",
    operation_id = "org_rbac_policy_list",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of RBAC policies", body = OrgRbacPolicyListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.rbac_policies.list", skip(state, authz, query), fields(%org_slug))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<OrgRbacPolicyListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission
    authz.require(
        "rbac_policy",
        "list",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;
    let result = services
        .org_rbac_policies
        .list_by_org_paginated(org.id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(OrgRbacPolicyListResponse {
        data: result.items,
        pagination,
    }))
}

/// Create a new RBAC policy for an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/rbac-policies",
    tag = "rbac-policies",
    operation_id = "org_rbac_policy_create",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = CreateOrgRbacPolicy,
    responses(
        (status = 201, description = "RBAC policy created", body = OrgRbacPolicy),
        (status = 400, description = "Invalid CEL expression", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Policy with same name already exists", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.rbac_policies.create", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<CreateOrgRbacPolicy>>,
) -> Result<(StatusCode, Json<OrgRbacPolicy>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require create permission
    authz.require(
        "rbac_policy",
        "create",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Check policy limit
    let max_policies = state.config.limits.resource_limits.max_policies_per_org;
    if max_policies > 0 {
        let policy_count = services.org_rbac_policies.count_by_org(org.id).await?;
        if policy_count >= max_policies as i64 {
            return Err(AdminError::Conflict(format!(
                "Organization has reached the maximum number of RBAC policies ({})",
                max_policies
            )));
        }
    }

    // Create the policy
    let policy = services
        .org_rbac_policies
        .create(org.id, input, actor.actor_id)
        .await?;

    // Refresh the registry cache
    services
        .org_rbac_policies
        .refresh_registry(org.id, state.policy_registry.as_ref().map(|v| v.as_ref()))
        .await?;

    // Log audit event (fire-and-forget). We intentionally discard the Result because:
    // 1. The policy was already created successfully - audit logging failure shouldn't undo that
    // 2. Audit logs are observability/compliance concerns, not critical path
    // 3. Blocking on audit failures would degrade user experience for a secondary concern
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "rbac_policy.create".to_string(),
            resource_type: "rbac_policy".to_string(),
            resource_id: policy.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "name": policy.name,
                "effect": policy.effect.to_string(),
                "priority": policy.priority,
                "enabled": policy.enabled,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok((StatusCode::CREATED, Json(policy)))
}

/// Get an RBAC policy by ID
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/rbac-policies/{policy_id}",
    tag = "rbac-policies",
    operation_id = "org_rbac_policy_get",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("policy_id" = Uuid, Path, description = "Policy ID"),
    ),
    responses(
        (status = 200, description = "RBAC policy found", body = OrgRbacPolicy),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or policy not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.rbac_policies.get", skip(state, authz), fields(%org_slug, %policy_id))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, policy_id)): Path<(String, Uuid)>,
) -> Result<Json<OrgRbacPolicy>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get the policy
    let policy = services
        .org_rbac_policies
        .get_by_id(policy_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("RBAC policy '{}' not found", policy_id)))?;

    // Verify policy belongs to this org
    if policy.org_id != org.id {
        return Err(AdminError::NotFound(format!(
            "RBAC policy '{}' not found in organization '{}'",
            policy_id, org_slug
        )));
    }

    // Require read permission
    authz.require(
        "rbac_policy",
        "read",
        Some(&policy_id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    Ok(Json(policy))
}

/// Update an RBAC policy
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{org_slug}/rbac-policies/{policy_id}",
    tag = "rbac-policies",
    operation_id = "org_rbac_policy_update",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("policy_id" = Uuid, Path, description = "Policy ID"),
    ),
    request_body = UpdateOrgRbacPolicy,
    responses(
        (status = 200, description = "RBAC policy updated", body = OrgRbacPolicy),
        (status = 400, description = "Invalid CEL expression", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or policy not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.rbac_policies.update", skip(state, admin_auth, authz, input), fields(%org_slug, %policy_id))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, policy_id)): Path<(String, Uuid)>,
    Valid(Json(input)): Valid<Json<UpdateOrgRbacPolicy>>,
) -> Result<Json<OrgRbacPolicy>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get the existing policy
    let existing = services
        .org_rbac_policies
        .get_by_id(policy_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("RBAC policy '{}' not found", policy_id)))?;

    // Verify policy belongs to this org
    if existing.org_id != org.id {
        return Err(AdminError::NotFound(format!(
            "RBAC policy '{}' not found in organization '{}'",
            policy_id, org_slug
        )));
    }

    // Require update permission
    authz.require(
        "rbac_policy",
        "update",
        Some(&policy_id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Update the policy
    let updated = services
        .org_rbac_policies
        .update(policy_id, input.clone(), actor.actor_id)
        .await?;

    // Refresh the registry cache
    services
        .org_rbac_policies
        .refresh_registry(org.id, state.policy_registry.as_ref().map(|v| v.as_ref()))
        .await?;

    // Log audit event (fire-and-forget). We intentionally discard the Result because:
    // 1. The policy was already updated successfully - audit logging failure shouldn't undo that
    // 2. Audit logs are observability/compliance concerns, not critical path
    // 3. Blocking on audit failures would degrade user experience for a secondary concern
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "rbac_policy.update".to_string(),
            resource_type: "rbac_policy".to_string(),
            resource_id: policy_id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "name": input.name,
                "effect": input.effect.map(|e| e.to_string()),
                "priority": input.priority,
                "enabled": input.enabled,
                "condition_changed": input.condition.is_some(),
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(updated))
}

/// Delete an RBAC policy
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/rbac-policies/{policy_id}",
    tag = "rbac-policies",
    operation_id = "org_rbac_policy_delete",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("policy_id" = Uuid, Path, description = "Policy ID"),
    ),
    responses(
        (status = 200, description = "RBAC policy deleted"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or policy not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.rbac_policies.delete", skip(state, admin_auth, authz), fields(%org_slug, %policy_id))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, policy_id)): Path<(String, Uuid)>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get the existing policy for audit log
    let existing = services
        .org_rbac_policies
        .get_by_id(policy_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("RBAC policy '{}' not found", policy_id)))?;

    // Verify policy belongs to this org
    if existing.org_id != org.id {
        return Err(AdminError::NotFound(format!(
            "RBAC policy '{}' not found in organization '{}'",
            policy_id, org_slug
        )));
    }

    // Require delete permission
    authz.require(
        "rbac_policy",
        "delete",
        Some(&policy_id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Capture details for audit log before deletion
    let policy_name = existing.name.clone();

    // Delete the policy
    services.org_rbac_policies.delete(policy_id).await?;

    // Refresh the registry cache
    services
        .org_rbac_policies
        .refresh_registry(org.id, state.policy_registry.as_ref().map(|v| v.as_ref()))
        .await?;

    // Log audit event (fire-and-forget). We intentionally discard the Result because:
    // 1. The policy was already deleted successfully - audit logging failure shouldn't undo that
    // 2. Audit logs are observability/compliance concerns, not critical path
    // 3. Blocking on audit failures would degrade user experience for a secondary concern
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "rbac_policy.delete".to_string(),
            resource_type: "rbac_policy".to_string(),
            resource_id: policy_id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "name": policy_name,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}

// ============================================================================
// Version History Endpoints
// ============================================================================

/// List version history for an RBAC policy
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/rbac-policies/{policy_id}/versions",
    tag = "rbac-policies",
    operation_id = "org_rbac_policy_list_versions",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("policy_id" = Uuid, Path, description = "Policy ID"),
        ListQuery,
    ),
    responses(
        (status = 200, description = "List of policy versions", body = OrgRbacPolicyVersionListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or policy not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.rbac_policies.list_versions", skip(state, authz, query), fields(%org_slug, %policy_id))]
pub async fn list_versions(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path((org_slug, policy_id)): Path<(String, Uuid)>,
    Query(query): Query<ListQuery>,
) -> Result<Json<OrgRbacPolicyVersionListResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get the policy to verify it exists and belongs to this org
    let policy = services
        .org_rbac_policies
        .get_by_id(policy_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("RBAC policy '{}' not found", policy_id)))?;

    if policy.org_id != org.id {
        return Err(AdminError::NotFound(format!(
            "RBAC policy '{}' not found in organization '{}'",
            policy_id, org_slug
        )));
    }

    // Require read permission
    authz.require(
        "rbac_policy",
        "read",
        Some(&policy_id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let limit = query.limit.unwrap_or(100);
    let params = query.try_into_with_cursor()?;
    let result = services
        .org_rbac_policies
        .list_versions_cursor(policy_id, params)
        .await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(OrgRbacPolicyVersionListResponse {
        data: result.items,
        pagination,
    }))
}

/// Rollback an RBAC policy to a previous version
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/rbac-policies/{policy_id}/rollback",
    tag = "rbac-policies",
    operation_id = "org_rbac_policy_rollback",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("policy_id" = Uuid, Path, description = "Policy ID"),
    ),
    request_body = RollbackOrgRbacPolicy,
    responses(
        (status = 200, description = "Policy rolled back", body = OrgRbacPolicy),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization, policy, or version not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.rbac_policies.rollback", skip(state, admin_auth, authz, input), fields(%org_slug, %policy_id))]
pub async fn rollback(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((org_slug, policy_id)): Path<(String, Uuid)>,
    Valid(Json(input)): Valid<Json<RollbackOrgRbacPolicy>>,
) -> Result<Json<OrgRbacPolicy>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get the policy to verify it exists and belongs to this org
    let policy = services
        .org_rbac_policies
        .get_by_id(policy_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("RBAC policy '{}' not found", policy_id)))?;

    if policy.org_id != org.id {
        return Err(AdminError::NotFound(format!(
            "RBAC policy '{}' not found in organization '{}'",
            policy_id, org_slug
        )));
    }

    // Require update permission (rollback is a form of update)
    authz.require(
        "rbac_policy",
        "update",
        Some(&policy_id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let target_version = input.target_version;

    // Rollback the policy
    let rolled_back = services
        .org_rbac_policies
        .rollback(policy_id, input, actor.actor_id)
        .await?;

    // Refresh the registry cache
    services
        .org_rbac_policies
        .refresh_registry(org.id, state.policy_registry.as_ref().map(|v| v.as_ref()))
        .await?;

    // Log audit event (fire-and-forget). We intentionally discard the Result because:
    // 1. The policy was already rolled back successfully - audit logging failure shouldn't undo that
    // 2. Audit logs are observability/compliance concerns, not critical path
    // 3. Blocking on audit failures would degrade user experience for a secondary concern
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "rbac_policy.rollback".to_string(),
            resource_type: "rbac_policy".to_string(),
            resource_id: policy_id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "name": rolled_back.name,
                "target_version": target_version,
                "new_version": rolled_back.version,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(rolled_back))
}

// ============================================================================
// Simulate & Validate Endpoints
// ============================================================================

/// Simulate policy evaluation for an organization
///
/// Tests what decision would be made for a given subject and context.
/// Evaluates both system policies (from config) and organization policies (from database),
/// matching the runtime authorization flow.
///
/// ## Evaluation Order
///
/// 1. Check if RBAC is disabled → return allow with `rbac_enabled: false`
/// 2. Evaluate system policies (from config file) first
/// 3. If a system policy matches → return that decision
/// 4. Evaluate organization policies (from database)
/// 5. If an org policy matches → return that decision
/// 6. No policy matched → apply configured `default_effect`
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/rbac-policies/simulate",
    tag = "rbac-policies",
    operation_id = "org_rbac_policy_simulate",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = SimulatePolicyRequest,
    responses(
        (status = 200, description = "Simulation result", body = SimulatePolicyResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.rbac_policies.simulate", skip(state, authz, input), fields(%org_slug))]
pub async fn simulate(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<SimulatePolicyRequest>>,
) -> Result<Json<SimulatePolicyResponse>, AdminError> {
    use crate::config::PolicyEffect;

    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission (simulation is read-only)
    authz.require(
        "rbac_policy",
        "read",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Build the subject and context
    let subject: Subject = input.subject.into();
    let mut policy_context =
        PolicyContext::new(&input.context.resource_type, &input.context.action);

    if let Some(ref resource_id) = input.context.resource_id {
        policy_context = policy_context.with_resource_id(resource_id);
    }
    if let Some(ref org_id) = input.context.org_id {
        policy_context = policy_context.with_org_id(org_id);
    }
    if let Some(ref team_id) = input.context.team_id {
        policy_context = policy_context.with_team_id(team_id);
    }
    if let Some(ref project_id) = input.context.project_id {
        policy_context = policy_context.with_project_id(project_id);
    }
    if let Some(ref model) = input.context.model {
        policy_context = policy_context.with_model(model);
    }
    if let Some(ref sim_request) = input.context.request {
        let mut req_ctx = RequestContext::new();
        if let Some(max_tokens) = sim_request.max_tokens {
            req_ctx = req_ctx.with_max_tokens(max_tokens);
        }
        if let Some(messages_count) = sim_request.messages_count {
            req_ctx = req_ctx.with_messages_count(messages_count);
        }
        if let Some(has_tools) = sim_request.has_tools {
            req_ctx = req_ctx.with_tools(has_tools);
        }
        if let Some(has_file_search) = sim_request.has_file_search {
            req_ctx = req_ctx.with_file_search(has_file_search);
        }
        if let Some(stream) = sim_request.stream {
            req_ctx = req_ctx.with_stream(stream);
        }
        if let Some(ref reasoning_effort) = sim_request.reasoning_effort {
            req_ctx = req_ctx.with_reasoning_effort(reasoning_effort);
        }
        if let Some(ref response_format) = sim_request.response_format {
            req_ctx = req_ctx.with_response_format(response_format);
        }
        if let Some(temperature) = sim_request.temperature {
            req_ctx = req_ctx.with_temperature(temperature);
        }
        if let Some(has_images) = sim_request.has_images {
            req_ctx = req_ctx.with_images(has_images);
        }
        if let Some(image_count) = sim_request.image_count {
            req_ctx = req_ctx.with_image_count(image_count);
        }
        if let Some(ref image_size) = sim_request.image_size {
            req_ctx = req_ctx.with_image_size(image_size);
        }
        if let Some(ref image_quality) = sim_request.image_quality {
            req_ctx = req_ctx.with_image_quality(image_quality);
        }
        if let Some(character_count) = sim_request.character_count {
            req_ctx = req_ctx.with_character_count(character_count);
        }
        if let Some(ref voice) = sim_request.voice {
            req_ctx = req_ctx.with_voice(voice);
        }
        if let Some(ref language) = sim_request.language {
            req_ctx = req_ctx.with_language(language);
        }
        policy_context = policy_context.with_request(req_ctx);
    }

    // Helper to convert config PolicyEffect to model RbacPolicyEffect
    let to_rbac_effect = |effect: PolicyEffect| -> RbacPolicyEffect {
        match effect {
            PolicyEffect::Allow => RbacPolicyEffect::Allow,
            PolicyEffect::Deny => RbacPolicyEffect::Deny,
        }
    };

    // Get the policy registry (contains system policies from config)
    let registry = state.policy_registry.as_ref();

    // Simulate system policies
    let (rbac_enabled, default_effect, system_policies_evaluated, system_matched) =
        if let Some(registry) = registry {
            let system_result = registry.engine().simulate(&subject, &policy_context);

            // Convert system policy results to response format
            let system_results: Vec<PolicyEvaluationResult> = system_result
                .policies_evaluated
                .into_iter()
                .map(|r| PolicyEvaluationResult {
                    name: r.name,
                    id: None, // System policies don't have UUIDs
                    source: PolicySource::System,
                    description: r.description,
                    pattern_matched: r.pattern_matched,
                    condition_matched: r.condition_matched,
                    skipped_reason: None, // System policies are always enabled
                    effect: to_rbac_effect(r.effect),
                    priority: r.priority,
                    error: r.error,
                })
                .collect();

            (
                system_result.rbac_enabled,
                system_result.default_effect,
                system_results,
                system_result.matched,
            )
        } else {
            // No registry configured - default to deny, no system policies
            (true, PolicyEffect::Deny, vec![], None)
        };

    // If RBAC is disabled, return early with allow
    if !rbac_enabled {
        return Ok(Json(SimulatePolicyResponse {
            rbac_enabled: false,
            allowed: true,
            matched_policy: None,
            matched_policy_source: None,
            reason: Some("RBAC is disabled, all requests are allowed".to_string()),
            system_policies_evaluated,
            org_policies_evaluated: vec![],
        }));
    }

    // Track the final decision
    let mut matched_policy: Option<String> = None;
    let mut matched_policy_source: Option<PolicySource> = None;
    let mut decision: Option<(bool, String)> = None;

    // If a system policy matched, use that decision
    if let Some((policy_name, allowed)) = system_matched {
        matched_policy = Some(policy_name.clone());
        matched_policy_source = Some(PolicySource::System);
        let effect = if allowed { "allow" } else { "deny" };
        decision = Some((
            allowed,
            format!(
                "Matched system policy '{}' with effect '{}'",
                policy_name, effect
            ),
        ));
    }

    // Get all org policies for this org
    let policies = services.org_rbac_policies.list_by_org(org.id).await?;

    // Filter to specific policy if requested
    let policies_to_evaluate: Vec<_> = if let Some(policy_id) = input.policy_id {
        policies.into_iter().filter(|p| p.id == policy_id).collect()
    } else {
        policies
    };

    if let Some(policy_id) = input.policy_id
        && policies_to_evaluate.is_empty()
    {
        return Err(AdminError::NotFound(format!(
            "RBAC policy '{}' not found",
            policy_id
        )));
    }

    // Evaluate org policies
    let mut org_evaluation_results = Vec::new();

    for policy in policies_to_evaluate {
        // Check if policy resource/action pattern matches (supports prefix wildcards like "team*")
        let resource_matches = pattern_matches(&policy.resource, &input.context.resource_type);
        let action_matches = pattern_matches(&policy.action, &input.context.action);
        let pattern_matched = resource_matches && action_matches;

        // Determine if policy was skipped and why
        let skipped_reason = if pattern_matched && !policy.enabled {
            Some("Policy is disabled".to_string())
        } else {
            None
        };

        let mut eval_result = PolicyEvaluationResult {
            name: policy.name.clone(),
            id: Some(policy.id),
            source: PolicySource::Organization,
            description: policy.description.clone(),
            pattern_matched,
            condition_matched: None,
            skipped_reason,
            effect: policy.effect,
            priority: policy.priority,
            error: None,
        };

        // Only evaluate condition if pattern matched, policy is enabled, and no system policy matched
        if pattern_matched && policy.enabled {
            #[cfg(feature = "cel")]
            {
                match evaluate_cel_condition(&policy.condition, &subject, &policy_context) {
                    Ok(result) => {
                        eval_result.condition_matched = Some(result);

                        // If condition matched and we haven't made a decision yet (no system policy matched)
                        if result && decision.is_none() {
                            matched_policy = Some(policy.name.clone());
                            matched_policy_source = Some(PolicySource::Organization);
                            let allowed = matches!(policy.effect, RbacPolicyEffect::Allow);
                            let reason = format!(
                                "Matched organization policy '{}' with effect '{}'",
                                policy.name, policy.effect
                            );
                            decision = Some((allowed, reason));
                        }
                    }
                    Err(e) => {
                        eval_result.error = Some(e);
                    }
                }
            }
            #[cfg(not(feature = "cel"))]
            {
                eval_result.error = Some(
                    "CEL policy evaluation requires the 'cel' feature to be enabled".to_string(),
                );
            }
        }

        org_evaluation_results.push(eval_result);
    }

    // If no policy matched, use configured default effect
    let (allowed, reason) = decision.unwrap_or_else(|| match default_effect {
        PolicyEffect::Allow => (true, "No policy matched (default allow)".to_string()),
        PolicyEffect::Deny => (false, "No policy matched (default deny)".to_string()),
    });

    Ok(Json(SimulatePolicyResponse {
        rbac_enabled,
        allowed,
        matched_policy,
        matched_policy_source,
        reason: Some(reason),
        system_policies_evaluated,
        org_policies_evaluated: org_evaluation_results,
    }))
}

/// Validate a CEL expression
///
/// Checks if a CEL expression is syntactically valid without creating a policy.
/// Useful for validating expressions before saving.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/rbac-policies/validate",
    tag = "rbac-policies",
    operation_id = "org_rbac_policy_validate",
    request_body = ValidateCelRequest,
    responses(
        (status = 200, description = "Validation result", body = ValidateCelResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.rbac_policies.validate", skip(state, authz, input))]
pub async fn validate(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Valid(Json(input)): Valid<Json<ValidateCelRequest>>,
) -> Result<Json<ValidateCelResponse>, AdminError> {
    // This is a global endpoint that doesn't require org context.
    // Require rbac_policy:read permission to reduce attack surface - only users
    // who can view RBAC policies should be able to validate CEL expressions.
    // This provides defense-in-depth since CEL parsing uses unsafe code in
    // the underlying antlr4rust dependency.
    authz.require("rbac_policy", "read", None, None, None, None)?;

    #[cfg(not(feature = "cel"))]
    {
        let _ = &input;
        let _ = &state;
        return Err(AdminError::BadRequest(
            "CEL policy evaluation requires the 'cel' feature to be enabled".to_string(),
        ));
    }

    // Validate the CEL expression (with length limit from the engine config)
    #[cfg(feature = "cel")]
    {
        let max_len = state
            .policy_registry
            .as_ref()
            .map(|r| r.engine().max_expression_length())
            .unwrap_or(0);
        match AuthzEngine::validate_expression_with_max_length(&input.condition, max_len) {
            Ok(()) => Ok(Json(ValidateCelResponse {
                valid: true,
                error: None,
            })),
            Err(e) => Ok(Json(ValidateCelResponse {
                valid: false,
                error: Some(e.to_string()),
            })),
        }
    }
}
