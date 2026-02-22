use std::time::Duration;

use axum::{
    Extension, Json, Router,
    body::{Body, Bytes},
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, header},
    middleware::from_fn_with_state,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_valid::Valid;
use chrono::Utc;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use uuid::Uuid;

use super::execution::{
    ChatCompletionExecutor, CompletionExecutor, EmbeddingExecutor, ExecutionResult,
    ProviderExecutor, ResponsesExecutor, execute_with_fallback,
};
#[cfg(feature = "provider-azure")]
use crate::providers::azure_openai;
use crate::{
    AppState, api_types,
    auth::AuthenticatedRequest,
    authz::RequestContext,
    cache::{CacheLookupResult, SemanticLookupResult, StoreParams},
    config::ProviderConfig,
    db::{DbError, ListParams},
    middleware::{
        AuthzContext, FileSearchAuthContext, FileSearchContext, ProviderCallback, RequestId,
        wrap_streaming_with_file_search,
    },
    models::{
        AddFileToVectorStore, AttributeFilter, ChunkingStrategy, CreateVectorStore, File, FileId,
        FilePurpose, FileSearchRankingOptions, UpdateVectorStore, UsageLogEntry, VectorStore,
        VectorStoreFile, VectorStoreFileId, VectorStoreFileStatus, VectorStoreId, VectorStoreOwner,
        VectorStoreOwnerType, chunk_id_serde, file_id_serde, vector_store_id_serde,
    },
    openapi::PaginationMeta,
    providers::{Provider, open_ai, test},
    routing::{RoutingError, resolver, route_model_extended, route_models_extended},
    services::{FilesService, FilesServiceError, Services},
};

/// Check if cache should be bypassed based on request headers.
///
/// Respects:
/// - `Cache-Control: no-cache` or `Cache-Control: no-store`
/// - `X-Cache-Force-Refresh: true`
fn should_bypass_cache(headers: &HeaderMap) -> bool {
    // Check Cache-Control header
    if let Some(cache_control) = headers.get("Cache-Control")
        && let Ok(value) = cache_control.to_str()
        && (value.contains("no-cache") || value.contains("no-store"))
    {
        return true;
    }

    // Check X-Cache-Force-Refresh header
    if let Some(force_refresh) = headers.get("X-Cache-Force-Refresh")
        && let Ok(value) = force_refresh.to_str()
        && (value.eq_ignore_ascii_case("true") || value == "1")
    {
        return true;
    }

    false
}

/// Check if any messages contain image content (multimodal).
fn messages_contain_images(messages: &[api_types::Message]) -> bool {
    use api_types::{
        Message,
        chat_completion::{ContentPart, MessageContent},
    };
    messages.iter().any(|msg| {
        let content = match msg {
            Message::System { content, .. } => Some(content),
            Message::User { content, .. } => Some(content),
            Message::Assistant { content, .. } => content.as_ref(),
            Message::Tool { content, .. } => Some(content),
            Message::Developer { content, .. } => Some(content),
        };
        content.is_some_and(|c| match c {
            MessageContent::Text(_) => false,
            MessageContent::Parts(parts) => parts
                .iter()
                .any(|p| matches!(p, ContentPart::ImageUrl { .. })),
        })
    })
}

/// Convert ResponseFormat enum to string for CEL policies.
fn response_format_to_string(format: &api_types::chat_completion::ResponseFormat) -> &'static str {
    use api_types::chat_completion::ResponseFormat;
    match format {
        ResponseFormat::Text => "text",
        ResponseFormat::JsonObject => "json_object",
        ResponseFormat::JsonSchema { .. } => "json_schema",
        ResponseFormat::Grammar { .. } => "grammar",
        ResponseFormat::Python => "python",
    }
}

/// Convert ReasoningEffort enum to string for CEL policies.
fn reasoning_effort_to_string(effort: &api_types::ReasoningEffort) -> &'static str {
    use api_types::ReasoningEffort;
    match effort {
        ReasoningEffort::None => "none",
        ReasoningEffort::Minimal => "minimal",
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
    }
}

/// Convert ResponsesReasoningEffort enum to string for CEL policies.
fn responses_reasoning_effort_to_string(
    effort: &api_types::ResponsesReasoningEffort,
) -> &'static str {
    use api_types::ResponsesReasoningEffort;
    match effort {
        ResponsesReasoningEffort::None => "none",
        ResponsesReasoningEffort::Minimal => "minimal",
        ResponsesReasoningEffort::Low => "low",
        ResponsesReasoningEffort::Medium => "medium",
        ResponsesReasoningEffort::High => "high",
    }
}

/// Convert ImageSize enum to string for CEL policies.
fn image_size_to_string(size: &api_types::ImageSize) -> &'static str {
    use api_types::ImageSize;
    match size {
        ImageSize::Auto => "auto",
        ImageSize::Size256 => "256x256",
        ImageSize::Size512 => "512x512",
        ImageSize::Size1024 => "1024x1024",
        ImageSize::Size1536x1024 => "1536x1024",
        ImageSize::Size1024x1536 => "1024x1536",
        ImageSize::Size1792x1024 => "1792x1024",
        ImageSize::Size1024x1792 => "1024x1792",
    }
}

/// Convert ImageQuality enum to string for CEL policies.
fn image_quality_to_string(quality: &api_types::ImageQuality) -> &'static str {
    use api_types::ImageQuality;
    match quality {
        ImageQuality::Standard => "standard",
        ImageQuality::Hd => "hd",
        ImageQuality::Low => "low",
        ImageQuality::Medium => "medium",
        ImageQuality::High => "high",
        ImageQuality::Auto => "auto",
    }
}

/// Convert Voice enum to string for CEL policies.
fn voice_to_string(voice: &api_types::Voice) -> &'static str {
    use api_types::Voice;
    match voice {
        Voice::Alloy => "alloy",
        Voice::Ash => "ash",
        Voice::Ballad => "ballad",
        Voice::Coral => "coral",
        Voice::Echo => "echo",
        Voice::Fable => "fable",
        Voice::Nova => "nova",
        Voice::Onyx => "onyx",
        Voice::Sage => "sage",
        Voice::Shimmer => "shimmer",
        Voice::Verse => "verse",
        Voice::Marin => "marin",
        Voice::Cedar => "cedar",
    }
}

/// Error response for API requests.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    /// Create a new API error
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = crate::openapi::ErrorResponse::new(self.code, self.message);
        (self.status, Json(body)).into_response()
    }
}

impl From<RoutingError> for ApiError {
    fn from(err: RoutingError) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "routing_error", err.to_string())
    }
}

impl From<DbError> for ApiError {
    fn from(err: DbError) -> Self {
        match err {
            DbError::NotFound => {
                Self::new(StatusCode::NOT_FOUND, "not_found", "Resource not found")
            }
            DbError::Conflict(msg) => Self::new(StatusCode::CONFLICT, "conflict", msg),
            DbError::Validation(msg) => Self::new(StatusCode::BAD_REQUEST, "validation_error", msg),
            DbError::NotConfigured => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "database_required",
                "Database not configured",
            ),
            _ => {
                tracing::error!(error = %err, "Database error");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "An internal database error occurred",
                )
            }
        }
    }
}

impl From<FilesServiceError> for ApiError {
    fn from(err: FilesServiceError) -> Self {
        match err {
            FilesServiceError::Database(db_err) => db_err.into(),
            FilesServiceError::Storage(storage_err) => {
                tracing::error!(error = %storage_err, "File storage error");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "storage_error",
                    "An internal storage error occurred",
                )
            }
            FilesServiceError::NotFound(id) => Self::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("File '{}' not found", id),
            ),
        }
    }
}

/// Check if the authenticated request has access to a resource based on its owner.
///
/// This function enforces ownership-based access control for vector stores and files:
/// - User-owned resources: caller must be the owner user
/// - Organization-owned resources: caller must belong to the organization
/// - Project-owned resources: caller must belong to the project
///
/// Returns `Ok(())` if access is allowed, or an `ApiError` with status 403 if denied.
fn check_resource_access(
    auth: &AuthenticatedRequest,
    owner_type: VectorStoreOwnerType,
    owner_id: Uuid,
) -> Result<(), ApiError> {
    let allowed = match owner_type {
        VectorStoreOwnerType::User => auth.user_id() == Some(owner_id),
        VectorStoreOwnerType::Organization => {
            // Check identity org membership or API key org ownership
            auth.identity()
                .map(|i| i.org_ids.contains(&owner_id.to_string()))
                .unwrap_or(false)
                || auth
                    .api_key()
                    .and_then(|k| k.org_id)
                    .map(|id| id == owner_id)
                    .unwrap_or(false)
        }
        VectorStoreOwnerType::Team => {
            // Team membership check requires database lookup
            // For now, return false - team access must be verified via database
            false
        }
        VectorStoreOwnerType::Project => {
            // Check identity project membership or API key project ownership
            auth.identity()
                .map(|i| i.project_ids.contains(&owner_id.to_string()))
                .unwrap_or(false)
                || auth
                    .api_key()
                    .and_then(|k| k.project_id)
                    .map(|id| id == owner_id)
                    .unwrap_or(false)
        }
    };

    if allowed {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "access_denied",
            "You do not have permission to access this resource",
        ))
    }
}

/// Check resource access with optional authentication.
/// When auth is None (e.g., auth.gateway.type = "none"), access is allowed.
fn check_resource_access_optional(
    auth: Option<&AuthenticatedRequest>,
    owner_type: VectorStoreOwnerType,
    owner_id: Uuid,
) -> Result<(), ApiError> {
    match auth {
        Some(auth) => check_resource_access(auth, owner_type, owner_id),
        None => Ok(()), // No auth configured, allow access
    }
}

/// User's identity memberships: (user_id, org_ids, team_ids, project_ids)
type IdentityMemberships = (Option<Uuid>, Vec<Uuid>, Vec<Uuid>, Vec<Uuid>);

/// Extract identity memberships from authentication context.
///
/// Returns the user ID and lists of organization, team, and project IDs
/// that the authenticated user has access to. This is used to filter
/// resources like vector stores to only show those the user can access.
///
/// Returns an error if no authentication is provided (required for accessible listing).
fn extract_identity_memberships(
    auth: Option<&AuthenticatedRequest>,
) -> Result<IdentityMemberships, ApiError> {
    let auth = auth.ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication is required to list accessible vector stores. Provide owner_type and owner_id to list specific collections without authentication.",
        )
    })?;

    let mut user_id: Option<Uuid> = None;
    let mut org_ids: Vec<Uuid> = Vec::new();
    let mut team_ids: Vec<Uuid> = Vec::new();
    let mut project_ids: Vec<Uuid> = Vec::new();

    // Extract from API key if present
    if let Some(api_key) = auth.api_key() {
        if let Some(uid) = api_key.user_id {
            user_id = Some(uid);
        }
        if let Some(org_id) = api_key.org_id {
            org_ids.push(org_id);
        }
        if let Some(team_id) = api_key.team_id {
            team_ids.push(team_id);
        }
        if let Some(project_id) = api_key.project_id {
            project_ids.push(project_id);
        }
    }

    // Extract from identity if present (OIDC claims)
    if let Some(identity) = auth.identity() {
        if let Some(uid) = identity.user_id {
            user_id = Some(uid);
        }
        // Parse string IDs to UUIDs
        for org_id_str in &identity.org_ids {
            if let Ok(org_id) = org_id_str.parse::<Uuid>()
                && !org_ids.contains(&org_id)
            {
                org_ids.push(org_id);
            }
        }
        for team_id_str in &identity.team_ids {
            if let Ok(team_id) = team_id_str.parse::<Uuid>()
                && !team_ids.contains(&team_id)
            {
                team_ids.push(team_id);
            }
        }
        for project_id_str in &identity.project_ids {
            if let Ok(project_id) = project_id_str.parse::<Uuid>()
                && !project_ids.contains(&project_id)
            {
                project_ids.push(project_id);
            }
        }
    }

    Ok((user_id, org_ids, team_ids, project_ids))
}

/// Validate that the vector store's embedding configuration matches the configured embedding service.
///
/// Collections are created with a specific embedding model and dimensions. When adding files,
/// the embeddings must be generated with the same model to ensure search quality. This function
/// validates that the gateway's configured embedding service matches the vector store's settings.
///
/// Returns an error if:
/// - File search service is not configured (no embedding service available)
/// - The embedding model doesn't match
/// - The embedding dimensions don't match
fn validate_embedding_model_compatibility(
    state: &AppState,
    vector_store: &VectorStore,
) -> Result<(), ApiError> {
    let file_search_service = state.file_search_service.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "embedding_service_unavailable",
            "File search service is not configured. Cannot process files for vector stores.",
        )
    })?;

    let embedding_service = file_search_service.embedding_service();
    let configured_model = embedding_service.model();
    let configured_dimensions = embedding_service.dimensions();

    // Check model compatibility
    if vector_store.embedding_model != configured_model {
        tracing::warn!(
            vector_store_id = %vector_store.id,
            collection_model = %vector_store.embedding_model,
            configured_model = %configured_model,
            "Embedding model mismatch: vector store was created with a different model"
        );
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "embedding_model_mismatch",
            format!(
                "Vector store '{}' uses embedding model '{}', but the gateway is configured with '{}'. \
                Files must be processed with the same embedding model used when the vector store was created. \
                Either reconfigure the gateway to use '{}' or create a new vector store with model '{}'.",
                vector_store.name,
                vector_store.embedding_model,
                configured_model,
                vector_store.embedding_model,
                configured_model
            ),
        ));
    }

    // Check dimensions compatibility
    if vector_store.embedding_dimensions != configured_dimensions as i32 {
        tracing::warn!(
            vector_store_id = %vector_store.id,
            collection_dimensions = vector_store.embedding_dimensions,
            configured_dimensions = configured_dimensions,
            "Embedding dimensions mismatch: vector store was created with different dimensions"
        );
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "embedding_dimensions_mismatch",
            format!(
                "Vector store '{}' uses {} embedding dimensions, but the gateway is configured with {}. \
                Files must be processed with the same embedding dimensions used when the vector store was created.",
                vector_store.name, vector_store.embedding_dimensions, configured_dimensions
            ),
        ));
    }

    Ok(())
}

/// Cache status for tracking cache hits/misses in response headers.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CacheStatus {
    /// No caching (streaming request, non-deterministic, etc.)
    None,
    /// Cache miss - request is cacheable but not found
    Miss,
}

/// Apply output guardrails to a non-streaming response.
///
/// Extracts assistant content from the response body, evaluates it against guardrails,
/// and applies the configured action (block, warn, redact, etc.).
///
/// Returns the (potentially modified) response and headers to add.
async fn apply_output_guardrails(
    state: &AppState,
    response: Response,
    user_id: Option<String>,
    auth: Option<&Extension<AuthenticatedRequest>>,
) -> Result<(Response, Vec<(&'static str, String)>), ApiError> {
    let output_guardrails = state.output_guardrails.as_ref().unwrap();

    // Read the response body
    let (parts, body) = response.into_parts();
    let body_bytes =
        match axum::body::to_bytes(body, state.config.server.max_response_body_bytes).await {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read response body for output guardrails");
                return Err(ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "response_read_error",
                    "Failed to read response for guardrails evaluation",
                ));
            }
        };

    // Extract assistant content from the response
    let assistant_content = crate::guardrails::extract_assistant_content_from_response(&body_bytes);

    // If no content to evaluate, return the original response
    if assistant_content.is_empty() {
        let response = Response::from_parts(parts, Body::from(body_bytes.to_vec()));
        return Ok((response, Vec::new()));
    }

    // Evaluate the content
    let result = output_guardrails
        .evaluate_response(&assistant_content, None, user_id.as_deref())
        .await;

    match result {
        Ok(guardrails_result) => {
            let headers = guardrails_result.to_headers();

            // Log audit event for output guardrails evaluation
            log_output_guardrails_evaluation(
                state,
                auth,
                output_guardrails.provider_name(),
                &guardrails_result,
                None,
            );

            // Check if content should be blocked
            if guardrails_result.is_blocked() {
                let error = crate::guardrails::GuardrailsError::blocked_with_violations(
                    crate::guardrails::ContentSource::LlmOutput,
                    "Response blocked by output guardrails",
                    guardrails_result.violations().to_vec(),
                );
                return Err(ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "guardrails_output_blocked",
                    error.to_string(),
                ));
            }

            // Check if content should be redacted
            if let Some(modified_content) = guardrails_result.modified_content() {
                // Rebuild the response with the modified content
                let modified_body = modify_response_content(&body_bytes, modified_content)
                    .unwrap_or_else(|| {
                        // If we can't modify the JSON, return the original
                        body_bytes.to_vec()
                    });
                let response = Response::from_parts(parts, Body::from(modified_body));
                return Ok((response, headers));
            }

            // Log warnings if any violations were found but allowed
            if !guardrails_result.response.violations.is_empty() {
                tracing::info!(
                    violations = ?guardrails_result.response.violations.len(),
                    "Output guardrails found violations but allowed response"
                );
            }

            // Return the original response with headers
            let response = Response::from_parts(parts, Body::from(body_bytes.to_vec()));
            Ok((response, headers))
        }
        Err(e) => {
            // Guardrails evaluation failed
            let status = match e.error_code() {
                "guardrails_blocked" => StatusCode::INTERNAL_SERVER_ERROR,
                "guardrails_timeout" => StatusCode::GATEWAY_TIMEOUT,
                "guardrails_auth_error" => StatusCode::UNAUTHORIZED,
                "guardrails_rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                "guardrails_config_error" => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::BAD_GATEWAY,
            };
            Err(ApiError::new(status, e.error_code(), e.to_string()))
        }
    }
}

/// Modifies the assistant content in a chat completion response JSON.
///
/// Returns the modified response body, or None if modification failed.
fn modify_response_content(body: &[u8], new_content: &str) -> Option<Vec<u8>> {
    let mut json: serde_json::Value = serde_json::from_slice(body).ok()?;

    // Modify choices[0].message.content
    if let Some(choices) = json.get_mut("choices").and_then(|c| c.as_array_mut())
        && let Some(first_choice) = choices.first_mut()
        && let Some(message) = first_choice.get_mut("message")
    {
        message["content"] = serde_json::Value::String(new_content.to_string());
    }

    serde_json::to_vec(&json).ok()
}

/// Build a [`UsageLogEntry`] for streaming cost tracking.
///
/// When authenticated, attributes usage to the principal (user, org, project, etc.).
/// When anonymous (no auth configured), attributes to the default anonymous user/org
/// so that streaming requests are tracked the same way the middleware tracks
/// non-streaming anonymous requests.
fn build_streaming_usage_entry(
    auth: &Option<Extension<AuthenticatedRequest>>,
    state: &AppState,
    model: &str,
    provider: &str,
    header_project_id: Option<uuid::Uuid>,
) -> Option<UsageLogEntry> {
    if let Some(Extension(auth)) = auth {
        let api_key = auth.api_key();
        Some(UsageLogEntry {
            request_id: uuid::Uuid::new_v4().to_string(),
            api_key_id: api_key.map(|k| k.key.id),
            user_id: auth.user_id(),
            org_id: api_key
                .and_then(|k| k.org_id)
                .or_else(|| auth.principal().org_id()),
            project_id: api_key.and_then(|k| k.project_id).or(header_project_id),
            team_id: api_key.and_then(|k| k.team_id),
            service_account_id: api_key.and_then(|k| k.service_account_id),
            model: model.to_string(),
            provider: provider.to_string(),
            input_tokens: 0,
            output_tokens: 0,
            cost_microcents: None,
            http_referer: None,
            request_at: Utc::now(),
            streamed: true,
            cached_tokens: 0,
            reasoning_tokens: 0,
            finish_reason: None,
            latency_ms: None,
            cancelled: false,
            status_code: None,
            pricing_source: crate::pricing::CostPricingSource::None,
            image_count: None,
            audio_seconds: None,
            character_count: None,
            provider_source: None,
        })
    } else if state.default_user_id.is_some() || state.default_org_id.is_some() {
        // Anonymous mode: attribute to the default user/org so streaming usage
        // is tracked the same way middleware tracks non-streaming anonymous usage.
        Some(UsageLogEntry {
            request_id: uuid::Uuid::new_v4().to_string(),
            api_key_id: None,
            user_id: state.default_user_id,
            org_id: state.default_org_id,
            project_id: header_project_id,
            team_id: None,
            service_account_id: None,
            model: model.to_string(),
            provider: provider.to_string(),
            input_tokens: 0,
            output_tokens: 0,
            cost_microcents: None,
            http_referer: None,
            request_at: Utc::now(),
            streamed: true,
            cached_tokens: 0,
            reasoning_tokens: 0,
            finish_reason: None,
            latency_ms: None,
            cancelled: false,
            status_code: None,
            pricing_source: crate::pricing::CostPricingSource::None,
            image_count: None,
            audio_seconds: None,
            character_count: None,
            provider_source: None,
        })
    } else {
        None
    }
}

/// Wraps a streaming response with guardrails filtering.
///
/// This function intercepts the SSE stream, extracts content, and evaluates
/// it against guardrails policies. The behavior depends on the configured mode:
/// - FinalOnly: Pass chunks through, evaluate complete response at end
/// - Buffered: Evaluate periodically during streaming
/// - PerChunk: Evaluate each chunk individually
fn wrap_streaming_with_guardrails(
    response: Response,
    output_guardrails: &crate::guardrails::OutputGuardrails,
    user_id: Option<String>,
    request_id: Option<String>,
) -> Response {
    use futures_util::StreamExt;

    // Check if this is a streaming response
    let is_streaming = response
        .headers()
        .get("Transfer-Encoding")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("chunked"))
        .unwrap_or(false);

    if !is_streaming {
        return response;
    }

    let (parts, body) = response.into_parts();

    // Convert body to byte stream
    let stream = body.into_data_stream().map(
        |result: Result<bytes::Bytes, axum::Error>| -> Result<bytes::Bytes, std::io::Error> {
            result.map_err(std::io::Error::other)
        },
    );

    // Create streaming guardrails config
    let config = crate::guardrails::StreamingGuardrailsConfig {
        mode: output_guardrails.streaming_mode(),
        request_id,
        user_id,
        retry_config: crate::guardrails::GuardrailsRetryConfig::default(),
        on_error: output_guardrails.on_error(),
    };

    // Wrap with guardrails filter stream
    let guardrails_stream = crate::guardrails::GuardrailsFilterStream::new(
        stream,
        output_guardrails.provider(),
        output_guardrails.action_executor(),
        config,
    );

    let new_body = axum::body::Body::from_stream(guardrails_stream);
    tracing::debug!("Streaming response wrapped with guardrails filter");

    Response::from_parts(parts, new_body)
}

/// Create a chat completion
///
/// Creates a model response for the given chat conversation. Supports both streaming and
/// non-streaming responses. The model can be specified using provider prefixes (e.g.,
/// `openai/gpt-4o`) or with dynamic routing for multi-tenant configurations.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/chat/completions",
    tag = "chat",
    request_body(
        content = api_types::CreateChatCompletionPayload,
        examples(
            ("Simple" = (
                summary = "Simple text completion",
                value = json!({
                    "model": "openai/gpt-4o",
                    "messages": [
                        {"role": "user", "content": "Hello, how are you?"}
                    ]
                })
            )),
            ("With system prompt" = (
                summary = "Completion with system prompt and parameters",
                value = json!({
                    "model": "anthropic/claude-sonnet-4-20250514",
                    "messages": [
                        {"role": "system", "content": "You are a helpful assistant."},
                        {"role": "user", "content": "Explain quantum computing in simple terms."}
                    ],
                    "max_tokens": 500,
                    "temperature": 0.7
                })
            )),
            ("Streaming" = (
                summary = "Streaming completion",
                value = json!({
                    "model": "openai/gpt-4o",
                    "messages": [
                        {"role": "user", "content": "Write a short poem about coding."}
                    ],
                    "stream": true
                })
            )),
            ("With tools" = (
                summary = "Completion with function calling",
                value = json!({
                    "model": "openai/gpt-4o",
                    "messages": [
                        {"role": "user", "content": "What's the weather in San Francisco?"}
                    ],
                    "tools": [{
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "description": "Get the current weather for a location",
                            "parameters": {
                                "type": "object",
                                "properties": {
                                    "location": {"type": "string", "description": "City name"}
                                },
                                "required": ["location"]
                            }
                        }
                    }],
                    "tool_choice": "auto"
                })
            ))
        )
    ),
    responses(
        (status = 200, description = "Chat completion response (streaming returns SSE events)",
            example = json!({
                "id": "chatcmpl-abc123",
                "object": "chat.completion",
                "created": 1733580800,
                "model": "gpt-4o-2024-08-06",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello! I'm doing well, thank you for asking. How can I help you today?"
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 12,
                    "completion_tokens": 18,
                    "total_tokens": 30
                }
            })
        ),
        (status = 400, description = "Bad request - invalid model, missing fields, or validation error",
            body = crate::openapi::ErrorResponse,
            example = json!({
                "error": {
                    "code": "routing_error",
                    "message": "Model 'invalid-model' not found"
                }
            })
        ),
        (status = 401, description = "Unauthorized - invalid or missing API key",
            body = crate::openapi::ErrorResponse,
            example = json!({
                "error": {
                    "code": "invalid_api_key",
                    "message": "Invalid API key provided"
                }
            })
        ),
        (status = 429, description = "Rate limit exceeded",
            body = crate::openapi::ErrorResponse,
            example = json!({
                "error": {
                    "code": "rate_limit_exceeded",
                    "message": "Rate limit exceeded: 100 requests per minute",
                    "details": {
                        "limit": 100,
                        "window": "minute",
                        "retry_after_secs": 30
                    }
                }
            })
        ),
        (status = 502, description = "Provider error - upstream LLM provider returned an error",
            body = crate::openapi::ErrorResponse,
            example = json!({
                "error": {
                    "code": "provider_error",
                    "message": "Upstream provider returned error: Service temporarily unavailable"
                }
            })
        ),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(
    name = "api.chat_completions",
    skip(state, headers, auth, authz, request_id, payload),
    fields(
        model = %payload.model.as_deref().unwrap_or("default"),
        streaming = payload.stream,
    )
)]
pub async fn api_v1_chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    request_id: Option<Extension<RequestId>>,
    Valid(Json(mut payload)): Valid<Json<api_types::CreateChatCompletionPayload>>,
) -> Result<Response, ApiError> {
    // Route the model to a provider with dynamic support
    let model_clone = payload.model.clone();
    let is_streaming = payload.stream;
    let routed = route_model_extended(model_clone.as_deref(), &state.config.providers)?;

    // Resolve to concrete provider configuration
    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        auth.as_ref().map(|e| &e.0),
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "provider_resolution_error",
            format!("Failed to resolve provider: {}", e),
        )
    })?;
    let provider_source = resolved.source;
    let (provider_name, provider_config, model_name) = (
        resolved.provider_name,
        resolved.provider_config,
        resolved.model,
    );

    // Update the payload with the resolved model name (provider prefix stripped)
    payload.model = Some(model_name.clone());

    // Check model restrictions if API key auth is used
    // Use original model string (with provider prefix) for restriction check
    if let Some(Extension(ref auth)) = auth
        && let Some(api_key) = auth.api_key()
    {
        let model_to_check = model_clone.as_deref().unwrap_or(&model_name);
        api_key.check_model_allowed(model_to_check).map_err(|e| {
            ApiError::new(StatusCode::FORBIDDEN, "model_not_allowed", e.to_string())
        })?;
    }

    // Check authorization if authz context is available and API RBAC is enabled
    if let Some(Extension(ref authz)) = authz {
        // Build request context from payload
        let mut request_ctx = RequestContext::new()
            .with_messages_count(payload.messages.len() as u64)
            .with_tools(payload.tools.is_some())
            .with_file_search(false) // file_search is only in Responses API
            .with_stream(payload.stream)
            .with_images(messages_contain_images(&payload.messages));

        // Add optional fields
        if let Some(max_tokens) = payload.max_tokens {
            request_ctx = request_ctx.with_max_tokens(max_tokens);
        }
        if let Some(ref reasoning) = payload.reasoning
            && let Some(ref effort) = reasoning.effort
        {
            request_ctx = request_ctx.with_reasoning_effort(reasoning_effort_to_string(effort));
        }
        if let Some(ref format) = payload.response_format {
            request_ctx = request_ctx.with_response_format(response_format_to_string(format));
        }
        if let Some(temp) = payload.temperature {
            request_ctx = request_ctx.with_temperature(temp);
        }

        // Get org_id and project_id from auth context
        // Try API key first, then fall back to identity's first org_id
        let org_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.org_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.org_ids.first().cloned()))
        });
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.project_ids.first().cloned()))
        });

        // Check model access authorization
        // Use original model string (with provider prefix) for RBAC policy evaluation
        // so policies can match against user-facing model identifiers
        authz
            .require_api(
                "model",
                "use",
                model_clone.as_deref().or(Some(&model_name)),
                Some(request_ctx),
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    // Check if input guardrails are configured and what mode they're in
    let use_concurrent_guardrails = state
        .input_guardrails
        .as_ref()
        .map(|g| g.is_concurrent())
        .unwrap_or(false);

    // Apply input guardrails in blocking mode (concurrent mode is handled later with the LLM call)
    let mut guardrails_headers: Vec<(&'static str, String)> = Vec::new();
    if let Some(ref input_guardrails) = state.input_guardrails
        && !use_concurrent_guardrails
    {
        // Blocking mode: evaluate guardrails before proceeding
        let user_id = auth
            .as_ref()
            .and_then(|a| a.api_key().map(|k| k.key.id.to_string()));

        let result = input_guardrails
            .evaluate_payload(&payload, None, user_id.as_deref())
            .await;

        match result {
            Ok(guardrails_result) => {
                // Collect headers for later (can't add to response yet)
                guardrails_headers = guardrails_result.to_headers();

                // Log audit event for guardrails evaluation
                log_guardrails_evaluation(
                    &state,
                    auth.as_ref(),
                    input_guardrails.provider_name(),
                    "input",
                    &guardrails_result,
                    None,
                );

                // Check if content should be blocked
                if guardrails_result.is_blocked() {
                    // Return the guardrails error (which implements IntoResponse)
                    let error = crate::guardrails::GuardrailsError::blocked_with_violations(
                        crate::guardrails::ContentSource::UserInput,
                        "Content blocked by input guardrails",
                        guardrails_result.violations().to_vec(),
                    );
                    return Err(ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "guardrails_blocked",
                        error.to_string(),
                    ));
                }

                // Log warnings if any violations were found but allowed
                if !guardrails_result.response.violations.is_empty() {
                    tracing::info!(
                        violations = ?guardrails_result.response.violations.len(),
                        "Input guardrails found violations but allowed request"
                    );
                }
            }
            Err(e) => {
                // Guardrails evaluation failed - the error handling is already done
                // by the evaluator based on on_error config, so this is a hard error
                let status = match e.error_code() {
                    "guardrails_blocked" => StatusCode::BAD_REQUEST,
                    "guardrails_timeout" => StatusCode::GATEWAY_TIMEOUT,
                    "guardrails_auth_error" => StatusCode::UNAUTHORIZED,
                    "guardrails_rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                    "guardrails_config_error" => StatusCode::INTERNAL_SERVER_ERROR,
                    _ => StatusCode::BAD_GATEWAY,
                };
                return Err(ApiError::new(status, e.error_code(), e.to_string()));
            }
        }
        // If concurrent mode, guardrails will be evaluated alongside the LLM call later
    }

    // Check if cache should be bypassed based on request headers
    let force_refresh = should_bypass_cache(&headers);

    // Track cache status for response headers
    let mut cache_status = CacheStatus::None;

    // Get cache key components for cache operations
    let key_components = state
        .config
        .features
        .response_caching
        .as_ref()
        .map(|c| &c.key_components);

    // Check semantic cache first (if available), then fall back to simple response cache
    if let Some(ref semantic_cache) = state.semantic_cache {
        let key_components = key_components.cloned().unwrap_or_default();
        match semantic_cache
            .lookup(&payload, &model_name, &key_components, force_refresh)
            .await
        {
            SemanticLookupResult::ExactHit(cached) => {
                tracing::debug!(
                    model = %model_name,
                    provider = %cached.provider,
                    cached_at = cached.cached_at,
                    "Returning exact-match cached response (semantic cache)"
                );
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", &cached.content_type)
                    .header("X-Cache", "HIT")
                    .header("X-Cached-At", cached.cached_at.to_string())
                    .body(Body::from(cached.body))
                    .unwrap());
            }
            SemanticLookupResult::SemanticHit {
                response,
                similarity,
            } => {
                tracing::debug!(
                    model = %model_name,
                    provider = %response.provider,
                    cached_at = response.cached_at,
                    similarity = %similarity,
                    "Returning semantic-match cached response"
                );
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", &response.content_type)
                    .header("X-Cache", "SEMANTIC_HIT")
                    .header("X-Cache-Similarity", format!("{:.4}", similarity))
                    .header("X-Cached-At", response.cached_at.to_string())
                    .body(Body::from(response.body))
                    .unwrap());
            }
            SemanticLookupResult::Miss => {
                cache_status = CacheStatus::Miss;
            }
            SemanticLookupResult::Bypass => {
                // Request is not cacheable (streaming, non-deterministic, etc.)
            }
        }
    } else if let Some(ref response_cache) = state.response_cache {
        // Fall back to simple response cache if semantic cache is not configured
        match response_cache
            .lookup(&payload, &model_name, force_refresh)
            .await
        {
            CacheLookupResult::Hit(cached) => {
                tracing::debug!(
                    model = %model_name,
                    provider = %cached.provider,
                    cached_at = cached.cached_at,
                    "Returning cached response"
                );
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", &cached.content_type)
                    .header("X-Cache", "HIT")
                    .header("X-Cached-At", cached.cached_at.to_string())
                    .body(Body::from(cached.body))
                    .unwrap());
            }
            CacheLookupResult::Miss => {
                cache_status = CacheStatus::Miss;
            }
            CacheLookupResult::Bypass => {
                // Request is not cacheable (streaming, non-deterministic, etc.)
            }
        }
    }

    // Execute request with fallback support
    // In concurrent guardrails mode, we race the guardrails evaluation with the LLM call
    let (response, provider_name, model_name) = if use_concurrent_guardrails {
        // Concurrent mode: race guardrails with LLM
        let input_guardrails = state.input_guardrails.as_ref().unwrap();
        let user_id = auth
            .as_ref()
            .and_then(|a| a.api_key().map(|k| k.key.id.to_string()));

        // Create the guardrails evaluation future
        let guardrails_payload = payload.clone();
        let guardrails_user_id = user_id.clone();
        let guardrails_future = input_guardrails.evaluate_payload(
            &guardrails_payload,
            None,
            guardrails_user_id.as_deref(),
        );

        // Create the LLM call future
        let llm_state = state.clone();
        let llm_provider_name = provider_name.clone();
        let llm_provider_config = provider_config.clone();
        let llm_model_name = model_name.clone();
        let llm_payload = payload.clone();
        let llm_future = async move {
            execute_with_fallback::<ChatCompletionExecutor>(
                &llm_state,
                llm_provider_name,
                llm_provider_config,
                llm_model_name,
                llm_payload,
            )
            .await
        };

        // Run concurrent evaluation
        let outcome = crate::guardrails::run_concurrent_evaluation(
            input_guardrails,
            guardrails_future,
            llm_future,
        )
        .await
        .map_err(|e| {
            let status = match e.error_code() {
                "guardrails_blocked" => StatusCode::BAD_REQUEST,
                "guardrails_timeout" => StatusCode::GATEWAY_TIMEOUT,
                "guardrails_auth_error" => StatusCode::UNAUTHORIZED,
                "guardrails_rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                "guardrails_config_error" => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::BAD_GATEWAY,
            };
            ApiError::new(status, e.error_code(), e.to_string())
        })?;

        // Collect guardrails headers from concurrent evaluation
        guardrails_headers = outcome.to_headers();

        // Log audit event for guardrails evaluation (concurrent mode)
        if let Some(ref guardrails_result) = outcome.guardrails_result {
            log_guardrails_evaluation(
                &state,
                auth.as_ref(),
                input_guardrails.provider_name(),
                "input",
                guardrails_result,
                None,
            );
        }

        // Extract the LLM result
        // The llm_result is Option<ChatCompletionResult> since successful LLM results
        // are extracted from Result<ChatCompletionResult, ApiError>
        match outcome.llm_result {
            Some(result) => (result.response, result.provider_name, result.model_name),
            None => {
                // LLM didn't complete or failed (error was logged in run_concurrent_evaluation)
                return Err(ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    "llm_error",
                    "LLM request failed during concurrent guardrails evaluation".to_string(),
                ));
            }
        }
    } else {
        // Blocking mode: execute LLM after guardrails
        let ExecutionResult {
            response,
            provider_name,
            model_name,
        } = execute_with_fallback::<ChatCompletionExecutor>(
            &state,
            provider_name,
            provider_config,
            model_name,
            payload.clone(),
        )
        .await?;
        (response, provider_name, model_name)
    };

    // Apply output guardrails if configured
    let (response, output_guardrails_headers) = if let Some(ref output_guardrails) =
        state.output_guardrails
        && response.status().is_success()
    {
        let user_id = auth
            .as_ref()
            .and_then(|a| a.api_key().map(|k| k.key.id.to_string()));
        let req_id = request_id.as_ref().map(|r| r.0.0.clone());

        if is_streaming {
            // Wrap streaming response with guardrails filter
            let wrapped =
                wrap_streaming_with_guardrails(response, output_guardrails, user_id, req_id);
            // Note: For streaming, headers are not added here since evaluation happens asynchronously
            (wrapped, Vec::new())
        } else {
            // Apply guardrails to non-streaming response
            apply_output_guardrails(&state, response, user_id, auth.as_ref()).await?
        }
    } else {
        (response, Vec::new())
    };

    // Cache the RAW response BEFORE cost injection (if applicable)
    // This ensures cached responses don't have stale pricing and cost $0 on replay
    let response = if cache_status == CacheStatus::Miss && response.status().is_success() {
        // Extract content-type and body for caching
        let content_type = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json")
            .to_string();

        // Read the body bytes for caching
        let (parts, body) = response.into_parts();
        match axum::body::to_bytes(body, state.config.server.max_response_body_bytes).await {
            Ok(bytes) => {
                let body_vec = bytes.to_vec();

                // Store in semantic cache if available, otherwise in response cache
                if let Some(ref semantic_cache) = state.semantic_cache {
                    let cache = semantic_cache.clone();
                    let payload_clone = payload.clone();
                    let model_clone = model_name.clone();
                    let provider_clone = provider_name.clone();
                    let content_type_clone = content_type.clone();
                    let body_clone = body_vec.clone();
                    let key_components_clone = key_components.cloned().unwrap_or_default();
                    let ttl_secs = state
                        .config
                        .features
                        .response_caching
                        .as_ref()
                        .map(|c| c.ttl_secs)
                        .unwrap_or(3600);
                    let org_id = auth
                        .as_ref()
                        .and_then(|a| a.org_id())
                        .map(|id| id.to_string());
                    let project_id = auth
                        .as_ref()
                        .and_then(|a| a.project_id())
                        .map(|id| id.to_string());

                    state.task_tracker.spawn(async move {
                        let params = StoreParams {
                            payload: &payload_clone,
                            model: &model_clone,
                            provider: &provider_clone,
                            body: body_clone,
                            content_type: &content_type_clone,
                            key_components: &key_components_clone,
                            ttl: Duration::from_secs(ttl_secs),
                            organization_id: org_id,
                            project_id,
                        };
                        if !cache.store(params).await {
                            tracing::debug!(
                                "Semantic cache store returned false (caching bypassed or disabled)"
                            );
                        }
                    });
                } else if let Some(ref response_cache) = state.response_cache {
                    let cache = response_cache.clone();
                    let payload_clone = payload.clone();
                    let model_clone = model_name.clone();
                    let provider_clone = provider_name.clone();
                    let content_type_clone = content_type;
                    let body_clone = body_vec.clone();
                    state.task_tracker.spawn(async move {
                        cache
                            .store(
                                &payload_clone,
                                &model_clone,
                                &provider_clone,
                                body_clone,
                                &content_type_clone,
                            )
                            .await;
                    });
                }

                // Rebuild response for cost injection
                Response::from_parts(parts, Body::from(body_vec))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read response body for caching");
                // Return error - we've consumed the body
                return Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Failed to process response"))
                    .unwrap());
            }
        }
    } else {
        response
    };

    // Create usage entry for streaming cost tracking
    let usage_entry = if is_streaming {
        build_streaming_usage_entry(&auth, &state, &model_name, &provider_name, {
            headers
                .get("X-Hadrian-Project")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| uuid::Uuid::parse_str(v).ok())
        })
    } else {
        None
    };

    // Inject cost calculation into the response
    let mut final_response =
        crate::providers::inject_cost_into_response(crate::providers::CostInjectionParams {
            response,
            provider: &provider_name,
            model: &model_name,
            pricing: &state.pricing,
            db: state.db.as_ref(),
            usage_entry,
            task_tracker: Some(&state.task_tracker),
            max_response_body_bytes: state.config.server.max_response_body_bytes,
            streaming_idle_timeout_secs: state.config.server.streaming_idle_timeout_secs,
            validation_config: &state.config.observability.response_validation,
            response_type: if is_streaming {
                crate::validation::ResponseType::ChatCompletionStream
            } else {
                crate::validation::ResponseType::ChatCompletion
            },
        })
        .await;

    // Add X-Cache: MISS header if this was a cache miss
    if cache_status == CacheStatus::Miss {
        final_response
            .headers_mut()
            .insert("X-Cache", "MISS".parse().unwrap());
    }

    // Add X-Provider and X-Model headers to identify which provider served the request
    // This is especially useful when fallback was used
    if let Ok(header_val) = provider_name.parse() {
        final_response
            .headers_mut()
            .insert("X-Provider", header_val);
    }
    if let Ok(source_val) = provider_source.parse() {
        final_response
            .headers_mut()
            .insert("X-Provider-Source", source_val);
    }
    if let Ok(header_val) = model_name.parse() {
        final_response.headers_mut().insert("X-Model", header_val);
    }

    // Add input guardrails headers if any were collected
    for (key, value) in guardrails_headers {
        if let Ok(header_val) = value.parse() {
            final_response.headers_mut().insert(key, header_val);
        }
    }

    // Add output guardrails headers if any were collected
    for (key, value) in output_guardrails_headers {
        if let Ok(header_val) = value.parse() {
            final_response.headers_mut().insert(key, header_val);
        }
    }

    Ok(final_response)
}

/// Create a response
///
/// Creates a model response using the Responses API format.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/responses",
    tag = "chat",
    request_body = api_types::CreateResponsesPayload,
    responses(
        (status = 200, description = "Response object (streaming or non-streaming)"),
        (status = 400, description = "Bad request", body = crate::openapi::ErrorResponse),
        (status = 502, description = "Provider error", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(
    name = "api.responses",
    skip(state, headers, auth, authz, request_id, payload),
    fields(
        model = %payload.model.as_deref().unwrap_or("default"),
        streaming = payload.stream,
    )
)]
pub async fn api_v1_responses(
    State(state): State<AppState>,
    headers: HeaderMap,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    request_id: Option<Extension<RequestId>>,
    Valid(Json(mut payload)): Valid<Json<api_types::CreateResponsesPayload>>,
) -> Result<Response, ApiError> {
    // Route the model to a provider with dynamic support
    let model_clone = payload.model.clone();
    let models_clone = payload.models.clone();
    let is_streaming = payload.stream;
    let routed = route_models_extended(
        model_clone.as_deref(),
        models_clone.as_deref(),
        &state.config.providers,
    )?;

    // Resolve to concrete provider configuration
    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        auth.as_ref().map(|e| &e.0),
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "provider_resolution_error",
            format!("Failed to resolve provider: {}", e),
        )
    })?;
    let provider_source = resolved.source;
    let (provider_name, provider_config, model_name) = (
        resolved.provider_name,
        resolved.provider_config,
        resolved.model,
    );

    // Update the payload with the resolved model name (provider prefix stripped)
    payload.model = Some(model_name.clone());

    // Check model restrictions if API key auth is used
    // Use original model string (with provider prefix) for restriction check
    if let Some(Extension(ref auth)) = auth
        && let Some(api_key) = auth.api_key()
    {
        let model_to_check = model_clone.as_deref().unwrap_or(&model_name);
        api_key.check_model_allowed(model_to_check).map_err(|e| {
            ApiError::new(StatusCode::FORBIDDEN, "model_not_allowed", e.to_string())
        })?;
    }

    // Check authorization if authz context is available and API RBAC is enabled
    if let Some(Extension(ref authz)) = authz {
        // Check if file_search tool is present
        let has_file_search = payload
            .tools
            .as_ref()
            .map(|tools| tools.iter().any(|t| t.is_file_search()))
            .unwrap_or(false);

        // Build request context from payload
        let mut request_ctx = RequestContext::new()
            .with_tools(payload.tools.is_some())
            .with_file_search(has_file_search)
            .with_stream(payload.stream);

        // Add optional fields
        if let Some(max_tokens) = payload.max_output_tokens {
            request_ctx = request_ctx.with_max_tokens(max_tokens as u64);
        }
        if let Some(ref reasoning) = payload.reasoning
            && let Some(ref effort) = reasoning.effort
        {
            request_ctx =
                request_ctx.with_reasoning_effort(responses_reasoning_effort_to_string(effort));
        }
        if let Some(temp) = payload.temperature {
            request_ctx = request_ctx.with_temperature(temp);
        }

        // Get org_id and project_id from auth context
        // Try API key first, then fall back to identity's first org_id
        let org_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.org_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.org_ids.first().cloned()))
        });
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.project_ids.first().cloned()))
        });

        // Check model access authorization
        // Use original model string (with provider prefix) for RBAC policy evaluation
        authz
            .require_api(
                "model",
                "use",
                model_clone.as_deref().or(Some(&model_name)),
                Some(request_ctx),
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    // Check if cache should be bypassed based on request headers
    let force_refresh = should_bypass_cache(&headers);

    // Track cache status for response headers
    let mut cache_status = CacheStatus::None;

    // Check response cache (simple cache only for now - semantic cache not yet supported for responses)
    if let Some(ref response_cache) = state.response_cache {
        match response_cache
            .lookup_responses(&payload, &model_name, force_refresh)
            .await
        {
            CacheLookupResult::Hit(cached) => {
                tracing::debug!(
                    model = %model_name,
                    provider = %cached.provider,
                    cached_at = cached.cached_at,
                    "Returning cached response (responses API)"
                );
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", &cached.content_type)
                    .header("X-Cache", "HIT")
                    .header("X-Cached-At", cached.cached_at.to_string())
                    .header("X-Provider", &cached.provider)
                    .header("X-Model", &cached.model)
                    .body(Body::from(cached.body))
                    .unwrap());
            }
            CacheLookupResult::Miss => {
                cache_status = CacheStatus::Miss;
            }
            CacheLookupResult::Bypass => {
                // Request is not cacheable (streaming, non-deterministic, etc.)
            }
        }
    }

    // Check if input guardrails are configured and what mode they're in
    let use_concurrent_guardrails = state
        .input_guardrails
        .as_ref()
        .map(|g| g.is_concurrent())
        .unwrap_or(false);

    // Apply input guardrails in blocking mode (concurrent mode is handled later with the LLM call)
    let mut guardrails_headers: Vec<(&'static str, String)> = Vec::new();
    if let Some(ref input_guardrails) = state.input_guardrails
        && !use_concurrent_guardrails
    {
        // Blocking mode: evaluate guardrails before proceeding
        let user_id = auth
            .as_ref()
            .and_then(|a| a.api_key().map(|k| k.key.id.to_string()));

        let result = input_guardrails
            .evaluate_responses_payload(&payload, None, user_id.as_deref())
            .await;

        match result {
            Ok(guardrails_result) => {
                guardrails_headers = guardrails_result.to_headers();

                // Log audit event for guardrails evaluation
                log_guardrails_evaluation(
                    &state,
                    auth.as_ref(),
                    input_guardrails.provider_name(),
                    "input",
                    &guardrails_result,
                    None,
                );

                if guardrails_result.is_blocked() {
                    let error = crate::guardrails::GuardrailsError::blocked_with_violations(
                        crate::guardrails::ContentSource::UserInput,
                        "Content blocked by input guardrails",
                        guardrails_result.violations().to_vec(),
                    );
                    return Err(ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "guardrails_blocked",
                        error.to_string(),
                    ));
                }

                if !guardrails_result.response.violations.is_empty() {
                    tracing::info!(
                        violations = ?guardrails_result.response.violations.len(),
                        "Input guardrails found violations but allowed request"
                    );
                }
            }
            Err(e) => {
                let status = match e.error_code() {
                    "guardrails_blocked" => StatusCode::BAD_REQUEST,
                    "guardrails_timeout" => StatusCode::GATEWAY_TIMEOUT,
                    "guardrails_auth_error" => StatusCode::UNAUTHORIZED,
                    "guardrails_rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                    "guardrails_config_error" => StatusCode::INTERNAL_SERVER_ERROR,
                    _ => StatusCode::BAD_GATEWAY,
                };
                return Err(ApiError::new(status, e.error_code(), e.to_string()));
            }
        }
        // If concurrent mode, guardrails will be evaluated alongside the LLM call below
    }

    // Create a provider from config and make a request
    // In concurrent mode, we race guardrails with the LLM call
    // Clone provider_config early - we need it later for file_search callback
    let saved_provider_config = provider_config.clone();
    let (response, provider_name, model_name, provider_config) = if use_concurrent_guardrails {
        let input_guardrails = state.input_guardrails.as_ref().unwrap();
        let user_id = auth
            .as_ref()
            .and_then(|a| a.api_key().map(|k| k.key.id.to_string()));

        // Create guardrails evaluation future
        let guardrails_payload = payload.clone();
        let guardrails_user_id = user_id.clone();
        let guardrails_future = input_guardrails.evaluate_responses_payload(
            &guardrails_payload,
            None,
            guardrails_user_id.as_deref(),
        );

        // Create LLM call future with fallback support
        let llm_state = state.clone();
        let llm_provider_name = provider_name.clone();
        let llm_provider_config = provider_config.clone();
        let llm_model_name = model_name.clone();
        let llm_payload = payload.clone();
        let llm_future = async move {
            execute_with_fallback::<ResponsesExecutor>(
                &llm_state,
                llm_provider_name,
                llm_provider_config,
                llm_model_name,
                llm_payload,
            )
            .await
        };

        // Run concurrent evaluation
        let outcome = crate::guardrails::run_concurrent_evaluation(
            input_guardrails,
            guardrails_future,
            llm_future,
        )
        .await
        .map_err(|e| {
            let status = match e.error_code() {
                "guardrails_blocked" => StatusCode::BAD_REQUEST,
                "guardrails_timeout" => StatusCode::GATEWAY_TIMEOUT,
                "guardrails_auth_error" => StatusCode::UNAUTHORIZED,
                "guardrails_rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                "guardrails_config_error" => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::BAD_GATEWAY,
            };
            ApiError::new(status, e.error_code(), e.to_string())
        })?;

        // Collect guardrails headers
        guardrails_headers = outcome.to_headers();

        // Log audit event for guardrails evaluation (concurrent mode)
        if let Some(ref guardrails_result) = outcome.guardrails_result {
            log_guardrails_evaluation(
                &state,
                auth.as_ref(),
                input_guardrails.provider_name(),
                "input",
                guardrails_result,
                None,
            );
        }

        // Extract LLM result
        match outcome.llm_result {
            Some(result) => (
                result.response,
                result.provider_name,
                result.model_name,
                saved_provider_config,
            ),
            None => {
                return Err(ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    "llm_error",
                    "LLM request failed during concurrent guardrails evaluation".to_string(),
                ));
            }
        }
    } else {
        // Blocking mode: execute LLM with fallback support
        let ExecutionResult {
            response,
            provider_name,
            model_name,
        } = execute_with_fallback::<ResponsesExecutor>(
            &state,
            provider_name,
            provider_config,
            model_name,
            payload.clone(),
        )
        .await?;
        (response, provider_name, model_name, saved_provider_config)
    };

    // Apply output guardrails if configured
    let (final_response, output_guardrails_headers) = if let Some(ref output_guardrails) =
        state.output_guardrails
        && response.status().is_success()
    {
        let user_id = auth
            .as_ref()
            .and_then(|a| a.api_key().map(|k| k.key.id.to_string()));
        let req_id = request_id.as_ref().map(|r| r.0.0.clone());

        if is_streaming {
            // Wrap streaming response with guardrails filter
            let wrapped =
                wrap_streaming_with_guardrails(response, output_guardrails, user_id, req_id);
            (wrapped, Vec::new())
        } else {
            // Apply guardrails to non-streaming response
            apply_output_guardrails_responses(&state, response, user_id, auth.as_ref()).await?
        }
    } else {
        (response, Vec::new())
    };

    // Apply file_search tool interception for streaming responses
    // This wraps the stream to detect and execute file_search tool calls
    let mut final_response = if is_streaming
        && final_response.status().is_success()
        && let Some(ref file_search_service) = state.file_search_service
        && let Some(ref file_search_config) = state.config.features.file_search
        && file_search_config.enabled
    {
        // Extract file_search tool definitions from the request
        let file_search_tools: Vec<_> = payload
            .tools
            .as_ref()
            .map(|tools| {
                tools
                    .iter()
                    .filter_map(|t| t.as_file_search().cloned())
                    .collect()
            })
            .unwrap_or_default();

        if !file_search_tools.is_empty() {
            // Extract full auth context for access control
            let file_search_auth =
                FileSearchAuthContext::from_auth_optional(auth.as_ref().map(|e| &e.0));

            // Create the provider callback for continuation requests
            let callback_state = state.clone();
            let callback_provider_name = provider_name.clone();
            let callback_provider_config = provider_config.clone();
            let callback_model_name = model_name.clone();

            let provider_callback: ProviderCallback = std::sync::Arc::new(move |payload| {
                let state = callback_state.clone();
                let provider_name = callback_provider_name.clone();
                let provider_config = callback_provider_config.clone();
                let model_name = callback_model_name.clone();

                Box::pin(async move {
                    // Set the model on the payload
                    let mut payload = payload;
                    payload.model = Some(model_name);

                    // Execute using the same provider
                    ResponsesExecutor::execute(&state, &provider_name, &provider_config, payload)
                        .await
                })
            });

            let context = FileSearchContext::new(
                file_search_service.clone(),
                file_search_config.clone(),
                file_search_auth,
                file_search_tools,
                payload.clone(),
            )
            .with_provider_callback(provider_callback);

            tracing::debug!(
                vector_store_ids = ?context.get_vector_store_ids(),
                "File search middleware enabled for request with multi-turn support"
            );

            wrap_streaming_with_file_search(final_response, context)
        } else {
            final_response
        }
    } else {
        final_response
    };

    // Add input guardrails headers
    for (key, value) in guardrails_headers {
        if let Ok(header_val) = value.parse() {
            final_response.headers_mut().insert(key, header_val);
        }
    }

    // Add output guardrails headers
    for (key, value) in output_guardrails_headers {
        if let Ok(header_val) = value.parse() {
            final_response.headers_mut().insert(key, header_val);
        }
    }

    // Cache successful responses (non-streaming only)
    let final_response = if cache_status == CacheStatus::Miss
        && final_response.status().is_success()
        && !is_streaming
    {
        // Extract content-type and body for caching
        let content_type = final_response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json")
            .to_string();

        // Read the body bytes for caching
        let (parts, body) = final_response.into_parts();
        match axum::body::to_bytes(body, state.config.server.max_response_body_bytes).await {
            Ok(bytes) => {
                let body_vec = bytes.to_vec();

                // Store in response cache (semantic cache not yet supported for responses API)
                if let Some(ref response_cache) = state.response_cache {
                    let cache = response_cache.clone();
                    let payload_clone = payload.clone();
                    let model_clone = model_name.clone();
                    let provider_clone = provider_name.clone();
                    let content_type_clone = content_type;
                    let body_clone = body_vec.clone();
                    state.task_tracker.spawn(async move {
                        cache
                            .store_responses(
                                &payload_clone,
                                &model_clone,
                                &provider_clone,
                                body_clone,
                                &content_type_clone,
                            )
                            .await;
                    });
                }

                // Rebuild response
                Response::from_parts(parts, Body::from(body_vec))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read response body for caching");
                // Return error - we've consumed the body
                return Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Failed to process response"))
                    .unwrap());
            }
        }
    } else {
        final_response
    };

    // Create usage entry for streaming cost tracking
    let usage_entry = if is_streaming {
        build_streaming_usage_entry(&auth, &state, &model_name, &provider_name, {
            headers
                .get("X-Hadrian-Project")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| uuid::Uuid::parse_str(v).ok())
        })
    } else {
        None
    };

    // Inject cost calculation into the response
    let mut final_response =
        crate::providers::inject_cost_into_response(crate::providers::CostInjectionParams {
            response: final_response,
            provider: &provider_name,
            model: &model_name,
            pricing: &state.pricing,
            db: state.db.as_ref(),
            usage_entry,
            task_tracker: Some(&state.task_tracker),
            max_response_body_bytes: state.config.server.max_response_body_bytes,
            streaming_idle_timeout_secs: state.config.server.streaming_idle_timeout_secs,
            validation_config: &state.config.observability.response_validation,
            response_type: if is_streaming {
                crate::validation::ResponseType::ResponseStream
            } else {
                crate::validation::ResponseType::Response
            },
        })
        .await;

    // Add X-Cache: MISS header if this was a cache miss
    if cache_status == CacheStatus::Miss {
        final_response
            .headers_mut()
            .insert("X-Cache", "MISS".parse().unwrap());
    }

    // Add X-Provider and X-Model headers to identify which provider served the request
    // This is especially useful when fallback was used
    if let Ok(header_val) = provider_name.parse() {
        final_response
            .headers_mut()
            .insert("X-Provider", header_val);
    }
    if let Ok(source_val) = provider_source.parse() {
        final_response
            .headers_mut()
            .insert("X-Provider-Source", source_val);
    }
    if let Ok(header_val) = model_name.parse() {
        final_response.headers_mut().insert("X-Model", header_val);
    }

    Ok(final_response)
}

/// Apply output guardrails to a non-streaming responses API response.
///
/// Similar to `apply_output_guardrails` but uses responses-specific content extraction.
async fn apply_output_guardrails_responses(
    state: &AppState,
    response: Response,
    user_id: Option<String>,
    auth: Option<&Extension<AuthenticatedRequest>>,
) -> Result<(Response, Vec<(&'static str, String)>), ApiError> {
    let output_guardrails = state.output_guardrails.as_ref().unwrap();

    // Read the response body
    let (parts, body) = response.into_parts();
    let body_bytes =
        match axum::body::to_bytes(body, state.config.server.max_response_body_bytes).await {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read response body for output guardrails");
                return Err(ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "response_read_error",
                    "Failed to read response for guardrails evaluation",
                ));
            }
        };

    // Extract content from the responses format
    let content = crate::guardrails::extract_text_from_responses_response(&body_bytes);

    // If no content to evaluate, return the original response
    if content.is_empty() {
        let response = Response::from_parts(parts, Body::from(body_bytes.to_vec()));
        return Ok((response, Vec::new()));
    }

    // Evaluate the content
    let result = output_guardrails
        .evaluate_response(&content, None, user_id.as_deref())
        .await;

    match result {
        Ok(guardrails_result) => {
            let headers = guardrails_result.to_headers();

            // Log audit event for output guardrails evaluation
            log_output_guardrails_evaluation(
                state,
                auth,
                output_guardrails.provider_name(),
                &guardrails_result,
                None,
            );

            // Check if content should be blocked
            if guardrails_result.is_blocked() {
                let error = crate::guardrails::GuardrailsError::blocked_with_violations(
                    crate::guardrails::ContentSource::LlmOutput,
                    "Response blocked by output guardrails",
                    guardrails_result.violations().to_vec(),
                );
                return Err(ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "guardrails_output_blocked",
                    error.to_string(),
                ));
            }

            // Check if content should be redacted
            if let Some(modified_content) = guardrails_result.modified_content() {
                // For responses API, rebuild with modified output_text
                let modified_body = modify_responses_content(&body_bytes, modified_content)
                    .unwrap_or_else(|| body_bytes.to_vec());
                let response = Response::from_parts(parts, Body::from(modified_body));
                return Ok((response, headers));
            }

            // Log warnings if any violations were found but allowed
            if !guardrails_result.response.violations.is_empty() {
                tracing::info!(
                    violations = ?guardrails_result.response.violations.len(),
                    "Output guardrails found violations but allowed response"
                );
            }

            // Return the original response with headers
            let response = Response::from_parts(parts, Body::from(body_bytes.to_vec()));
            Ok((response, headers))
        }
        Err(e) => {
            let status = match e.error_code() {
                "guardrails_blocked" => StatusCode::INTERNAL_SERVER_ERROR,
                "guardrails_timeout" => StatusCode::GATEWAY_TIMEOUT,
                "guardrails_auth_error" => StatusCode::UNAUTHORIZED,
                "guardrails_rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                "guardrails_config_error" => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::BAD_GATEWAY,
            };
            Err(ApiError::new(status, e.error_code(), e.to_string()))
        }
    }
}

/// Modifies the output_text in a responses API response JSON.
///
/// Returns the modified response body, or None if modification failed.
fn modify_responses_content(body: &[u8], new_content: &str) -> Option<Vec<u8>> {
    let mut json: serde_json::Value = serde_json::from_slice(body).ok()?;

    // Modify output_text field
    json["output_text"] = serde_json::Value::String(new_content.to_string());

    // Also modify content in output[0].content if it's a message
    if let Some(output) = json.get_mut("output").and_then(|o| o.as_array_mut()) {
        for item in output {
            if item.get("type").and_then(|t| t.as_str()) == Some("message")
                && let Some(content) = item.get_mut("content").and_then(|c| c.as_array_mut())
            {
                for content_item in content {
                    if content_item.get("type").and_then(|t| t.as_str()) == Some("output_text") {
                        content_item["text"] = serde_json::Value::String(new_content.to_string());
                    }
                }
            }
        }
    }

    serde_json::to_vec(&json).ok()
}

/// Create a text completion
///
/// Creates a completion for the provided prompt and parameters.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/completions",
    tag = "completions",
    request_body = api_types::CreateCompletionPayload,
    responses(
        (status = 200, description = "Completion response (streaming or non-streaming)"),
        (status = 400, description = "Bad request", body = crate::openapi::ErrorResponse),
        (status = 502, description = "Provider error", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(
    name = "api.completions",
    skip(state, headers, auth, request_id, payload),
    fields(
        model = %payload.model.as_deref().unwrap_or("default"),
        streaming = payload.stream,
    )
)]
pub async fn api_v1_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    auth: Option<Extension<AuthenticatedRequest>>,
    request_id: Option<Extension<RequestId>>,
    Valid(Json(mut payload)): Valid<Json<api_types::CreateCompletionPayload>>,
) -> Result<Response, ApiError> {
    // Route the model to a provider with dynamic support
    let model_clone = payload.model.clone();
    let models_clone = payload.models.clone();
    let is_streaming = payload.stream;
    let routed = route_models_extended(
        model_clone.as_deref(),
        models_clone.as_deref(),
        &state.config.providers,
    )?;

    // Resolve to concrete provider configuration
    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        auth.as_ref().map(|e| &e.0),
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "provider_resolution_error",
            format!("Failed to resolve provider: {}", e),
        )
    })?;
    let provider_source = resolved.source;
    let (provider_name, provider_config, model_name) = (
        resolved.provider_name,
        resolved.provider_config,
        resolved.model,
    );

    // Update the payload with the resolved model name (provider prefix stripped)
    payload.model = Some(model_name.clone());

    // Check model restrictions if API key auth is used
    // Use original model string (with provider prefix) for restriction check
    if let Some(Extension(ref auth)) = auth
        && let Some(api_key) = auth.api_key()
    {
        let model_to_check = model_clone.as_deref().unwrap_or(&model_name);
        api_key.check_model_allowed(model_to_check).map_err(|e| {
            ApiError::new(StatusCode::FORBIDDEN, "model_not_allowed", e.to_string())
        })?;
    }

    // Check if cache should be bypassed based on request headers
    let force_refresh = should_bypass_cache(&headers);

    // Track cache status for response headers
    let mut cache_status = CacheStatus::None;

    // Check response cache (simple cache only - semantic cache not yet supported for completions)
    if let Some(ref response_cache) = state.response_cache {
        match response_cache
            .lookup_completions(&payload, &model_name, force_refresh)
            .await
        {
            CacheLookupResult::Hit(cached) => {
                tracing::debug!(
                    model = %model_name,
                    provider = %cached.provider,
                    cached_at = cached.cached_at,
                    "Returning cached response (completions API)"
                );
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", &cached.content_type)
                    .header("X-Cache", "HIT")
                    .header("X-Cached-At", cached.cached_at.to_string())
                    .header("X-Provider", &cached.provider)
                    .header("X-Model", &cached.model)
                    .body(Body::from(cached.body))
                    .unwrap());
            }
            CacheLookupResult::Miss => {
                cache_status = CacheStatus::Miss;
            }
            CacheLookupResult::Bypass => {
                // Request is not cacheable (streaming, non-deterministic, etc.)
            }
        }
    }

    // Check if input guardrails are configured and what mode they're in
    let use_concurrent_guardrails = state
        .input_guardrails
        .as_ref()
        .map(|g| g.is_concurrent())
        .unwrap_or(false);

    // Apply input guardrails in blocking mode (concurrent mode is handled later with the LLM call)
    let mut guardrails_headers: Vec<(&'static str, String)> = Vec::new();
    if let Some(ref input_guardrails) = state.input_guardrails
        && !use_concurrent_guardrails
    {
        // Blocking mode: evaluate guardrails before proceeding
        let user_id = auth
            .as_ref()
            .and_then(|a| a.api_key().map(|k| k.key.id.to_string()));

        let result = input_guardrails
            .evaluate_completion_payload(&payload, None, user_id.as_deref())
            .await;

        match result {
            Ok(guardrails_result) => {
                guardrails_headers = guardrails_result.to_headers();

                // Log audit event for guardrails evaluation
                log_guardrails_evaluation(
                    &state,
                    auth.as_ref(),
                    input_guardrails.provider_name(),
                    "input",
                    &guardrails_result,
                    None,
                );

                if guardrails_result.is_blocked() {
                    let error = crate::guardrails::GuardrailsError::blocked_with_violations(
                        crate::guardrails::ContentSource::UserInput,
                        "Content blocked by input guardrails",
                        guardrails_result.violations().to_vec(),
                    );
                    return Err(ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "guardrails_blocked",
                        error.to_string(),
                    ));
                }

                if !guardrails_result.response.violations.is_empty() {
                    tracing::info!(
                        violations = ?guardrails_result.response.violations.len(),
                        "Input guardrails found violations but allowed request"
                    );
                }
            }
            Err(e) => {
                let status = match e.error_code() {
                    "guardrails_blocked" => StatusCode::BAD_REQUEST,
                    "guardrails_timeout" => StatusCode::GATEWAY_TIMEOUT,
                    "guardrails_auth_error" => StatusCode::UNAUTHORIZED,
                    "guardrails_rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                    "guardrails_config_error" => StatusCode::INTERNAL_SERVER_ERROR,
                    _ => StatusCode::BAD_GATEWAY,
                };
                return Err(ApiError::new(status, e.error_code(), e.to_string()));
            }
        }
        // If concurrent mode, guardrails will be evaluated alongside the LLM call below
    }

    // Create a provider from config and make a request
    // In concurrent mode, we race guardrails with the LLM call
    let (response, provider_name, model_name) = if use_concurrent_guardrails {
        // SAFETY: use_concurrent_guardrails is only true when input_guardrails is Some
        let input_guardrails = state.input_guardrails.as_ref().unwrap();
        let user_id = auth
            .as_ref()
            .and_then(|a| a.api_key().map(|k| k.key.id.to_string()));

        // Create guardrails evaluation future
        let guardrails_payload = payload.clone();
        let guardrails_user_id = user_id.clone();
        let guardrails_future = input_guardrails.evaluate_completion_payload(
            &guardrails_payload,
            None,
            guardrails_user_id.as_deref(),
        );

        // Create LLM call future with fallback support
        let llm_state = state.clone();
        let llm_provider_name = provider_name.clone();
        let llm_provider_config = provider_config.clone();
        let llm_model_name = model_name.clone();
        let llm_payload = payload.clone();
        let llm_future = async move {
            execute_with_fallback::<CompletionExecutor>(
                &llm_state,
                llm_provider_name,
                llm_provider_config,
                llm_model_name,
                llm_payload,
            )
            .await
        };

        // Run concurrent evaluation
        let outcome = crate::guardrails::run_concurrent_evaluation(
            input_guardrails,
            guardrails_future,
            llm_future,
        )
        .await
        .map_err(|e| {
            let status = match e.error_code() {
                "guardrails_blocked" => StatusCode::BAD_REQUEST,
                "guardrails_timeout" => StatusCode::GATEWAY_TIMEOUT,
                "guardrails_auth_error" => StatusCode::UNAUTHORIZED,
                "guardrails_rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                "guardrails_config_error" => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::BAD_GATEWAY,
            };
            ApiError::new(status, e.error_code(), e.to_string())
        })?;

        // Collect guardrails headers
        guardrails_headers = outcome.to_headers();

        // Log audit event for guardrails evaluation (concurrent mode)
        if let Some(ref guardrails_result) = outcome.guardrails_result {
            log_guardrails_evaluation(
                &state,
                auth.as_ref(),
                input_guardrails.provider_name(),
                "input",
                guardrails_result,
                None,
            );
        }

        // Extract LLM result
        match outcome.llm_result {
            Some(result) => (result.response, result.provider_name, result.model_name),
            None => {
                // LLM didn't complete or failed (error was logged in run_concurrent_evaluation)
                return Err(ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    "llm_error",
                    "LLM request failed during concurrent guardrails evaluation".to_string(),
                ));
            }
        }
    } else {
        // Blocking mode: execute LLM with fallback support
        let ExecutionResult {
            response,
            provider_name,
            model_name,
        } = execute_with_fallback::<CompletionExecutor>(
            &state,
            provider_name,
            provider_config,
            model_name,
            payload.clone(),
        )
        .await?;
        (response, provider_name, model_name)
    };

    // Apply output guardrails if configured
    let (mut final_response, output_guardrails_headers) = if let Some(ref output_guardrails) =
        state.output_guardrails
        && response.status().is_success()
    {
        let user_id = auth
            .as_ref()
            .and_then(|a| a.api_key().map(|k| k.key.id.to_string()));
        let req_id = request_id.as_ref().map(|r| r.0.0.clone());

        if is_streaming {
            // Wrap streaming response with guardrails filter
            // Note: For completions, we reuse the same streaming filter
            let wrapped =
                wrap_streaming_with_guardrails(response, output_guardrails, user_id, req_id);
            (wrapped, Vec::new())
        } else {
            // Apply guardrails to non-streaming response
            apply_output_guardrails_completions(&state, response, user_id, auth.as_ref()).await?
        }
    } else {
        (response, Vec::new())
    };

    // Add input guardrails headers
    for (key, value) in guardrails_headers {
        if let Ok(header_val) = value.parse() {
            final_response.headers_mut().insert(key, header_val);
        }
    }

    // Add output guardrails headers
    for (key, value) in output_guardrails_headers {
        if let Ok(header_val) = value.parse() {
            final_response.headers_mut().insert(key, header_val);
        }
    }

    // Cache successful responses (non-streaming only)
    let final_response = if cache_status == CacheStatus::Miss
        && final_response.status().is_success()
        && !is_streaming
    {
        // Extract content-type and body for caching
        let content_type = final_response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json")
            .to_string();

        // Read the body bytes for caching
        let (parts, body) = final_response.into_parts();
        match axum::body::to_bytes(body, state.config.server.max_response_body_bytes).await {
            Ok(bytes) => {
                let body_vec = bytes.to_vec();

                // Store in response cache
                if let Some(ref response_cache) = state.response_cache {
                    let cache = response_cache.clone();
                    let payload_clone = payload.clone();
                    let model_clone = model_name.clone();
                    let provider_clone = provider_name.clone();
                    let content_type_clone = content_type;
                    let body_clone = body_vec.clone();
                    state.task_tracker.spawn(async move {
                        cache
                            .store_completions(
                                &payload_clone,
                                &model_clone,
                                &provider_clone,
                                body_clone,
                                &content_type_clone,
                            )
                            .await;
                    });
                }

                // Rebuild response
                Response::from_parts(parts, Body::from(body_vec))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read response body for caching");
                // Return error - we've consumed the body
                return Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Failed to process response"))
                    .unwrap());
            }
        }
    } else {
        final_response
    };

    // Create usage entry for streaming cost tracking
    let usage_entry = if is_streaming {
        build_streaming_usage_entry(&auth, &state, &model_name, &provider_name, {
            headers
                .get("X-Hadrian-Project")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| uuid::Uuid::parse_str(v).ok())
        })
    } else {
        None
    };

    // Inject cost calculation into the response
    let mut final_response =
        crate::providers::inject_cost_into_response(crate::providers::CostInjectionParams {
            response: final_response,
            provider: &provider_name,
            model: &model_name,
            pricing: &state.pricing,
            db: state.db.as_ref(),
            usage_entry,
            task_tracker: Some(&state.task_tracker),
            max_response_body_bytes: state.config.server.max_response_body_bytes,
            streaming_idle_timeout_secs: state.config.server.streaming_idle_timeout_secs,
            validation_config: &state.config.observability.response_validation,
            response_type: if is_streaming {
                crate::validation::ResponseType::ChatCompletionStream // Legacy completions use same schema
            } else {
                crate::validation::ResponseType::Completion
            },
        })
        .await;

    // Add X-Cache: MISS header if this was a cache miss
    if cache_status == CacheStatus::Miss {
        final_response
            .headers_mut()
            .insert("X-Cache", "MISS".parse().unwrap());
    }

    // Add X-Provider and X-Model headers to identify which provider served the request
    // This is especially useful when fallback was used
    if let Ok(header_val) = provider_name.parse() {
        final_response
            .headers_mut()
            .insert("X-Provider", header_val);
    }
    if let Ok(source_val) = provider_source.parse() {
        final_response
            .headers_mut()
            .insert("X-Provider-Source", source_val);
    }
    if let Ok(header_val) = model_name.parse() {
        final_response.headers_mut().insert("X-Model", header_val);
    }

    Ok(final_response)
}

/// Apply output guardrails to a non-streaming completions API response.
///
/// Similar to `apply_output_guardrails` but uses completions-specific content extraction.
async fn apply_output_guardrails_completions(
    state: &AppState,
    response: Response,
    user_id: Option<String>,
    auth: Option<&Extension<AuthenticatedRequest>>,
) -> Result<(Response, Vec<(&'static str, String)>), ApiError> {
    let output_guardrails = state.output_guardrails.as_ref().unwrap();

    // Read the response body
    let (parts, body) = response.into_parts();
    let body_bytes =
        match axum::body::to_bytes(body, state.config.server.max_response_body_bytes).await {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read response body for output guardrails");
                return Err(ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "response_read_error",
                    "Failed to read response for guardrails evaluation",
                ));
            }
        };

    // Extract content from the completions format
    let content = crate::guardrails::extract_text_from_completion_response(&body_bytes);

    // If no content to evaluate, return the original response
    if content.is_empty() {
        let response = Response::from_parts(parts, Body::from(body_bytes.to_vec()));
        return Ok((response, Vec::new()));
    }

    // Evaluate the content
    let result = output_guardrails
        .evaluate_response(&content, None, user_id.as_deref())
        .await;

    match result {
        Ok(guardrails_result) => {
            let headers = guardrails_result.to_headers();

            // Log audit event for output guardrails evaluation
            log_output_guardrails_evaluation(
                state,
                auth,
                output_guardrails.provider_name(),
                &guardrails_result,
                None,
            );

            // Check if content should be blocked
            if guardrails_result.is_blocked() {
                let error = crate::guardrails::GuardrailsError::blocked_with_violations(
                    crate::guardrails::ContentSource::LlmOutput,
                    "Response blocked by output guardrails",
                    guardrails_result.violations().to_vec(),
                );
                return Err(ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "guardrails_output_blocked",
                    error.to_string(),
                ));
            }

            // Check if content should be redacted
            if let Some(modified_content) = guardrails_result.modified_content() {
                // For completions API, rebuild with modified text
                let modified_body = modify_completion_content(&body_bytes, modified_content)
                    .unwrap_or_else(|| body_bytes.to_vec());
                let response = Response::from_parts(parts, Body::from(modified_body));
                return Ok((response, headers));
            }

            // Log warnings if any violations were found but allowed
            if !guardrails_result.response.violations.is_empty() {
                tracing::info!(
                    violations = ?guardrails_result.response.violations.len(),
                    "Output guardrails found violations but allowed response"
                );
            }

            // Return the original response with headers
            let response = Response::from_parts(parts, Body::from(body_bytes.to_vec()));
            Ok((response, headers))
        }
        Err(e) => {
            let status = match e.error_code() {
                "guardrails_blocked" => StatusCode::INTERNAL_SERVER_ERROR,
                "guardrails_timeout" => StatusCode::GATEWAY_TIMEOUT,
                "guardrails_auth_error" => StatusCode::UNAUTHORIZED,
                "guardrails_rate_limited" => StatusCode::TOO_MANY_REQUESTS,
                "guardrails_config_error" => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::BAD_GATEWAY,
            };
            Err(ApiError::new(status, e.error_code(), e.to_string()))
        }
    }
}

/// Modifies the text in a completions API response JSON.
///
/// Returns the modified response body, or None if modification failed.
fn modify_completion_content(body: &[u8], new_content: &str) -> Option<Vec<u8>> {
    let mut json: serde_json::Value = serde_json::from_slice(body).ok()?;

    // Modify choices[].text
    if let Some(choices) = json.get_mut("choices").and_then(|c| c.as_array_mut()) {
        for choice in choices {
            choice["text"] = serde_json::Value::String(new_content.to_string());
        }
    }

    serde_json::to_vec(&json).ok()
}

/// Create embeddings
///
/// Creates an embedding vector representing the input text. Embeddings are useful for
/// semantic search, clustering, classification, and similarity comparisons.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/embeddings",
    tag = "embeddings",
    request_body(
        content = api_types::CreateEmbeddingPayload,
        examples(
            ("Single text" = (
                summary = "Embed a single text string",
                value = json!({
                    "model": "openai/text-embedding-3-small",
                    "input": "Hello world"
                })
            )),
            ("Multiple texts" = (
                summary = "Embed multiple texts in one request",
                value = json!({
                    "model": "openai/text-embedding-3-large",
                    "input": [
                        "First document to embed",
                        "Second document to embed",
                        "Third document to embed"
                    ],
                    "dimensions": 1024
                })
            ))
        )
    ),
    responses(
        (status = 200, description = "Embedding vectors for the input text(s)",
            example = json!({
                "object": "list",
                "data": [{
                    "object": "embedding",
                    "index": 0,
                    "embedding": [0.0023064255, -0.009327292, 0.015797347]
                }],
                "model": "text-embedding-3-small",
                "usage": {
                    "prompt_tokens": 2,
                    "total_tokens": 2
                }
            })
        ),
        (status = 400, description = "Bad request - invalid model or input",
            body = crate::openapi::ErrorResponse,
            example = json!({
                "error": {
                    "code": "routing_error",
                    "message": "Model 'invalid-embedding-model' not found"
                }
            })
        ),
        (status = 502, description = "Provider error",
            body = crate::openapi::ErrorResponse,
            example = json!({
                "error": {
                    "code": "provider_error",
                    "message": "Upstream provider returned error"
                }
            })
        ),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(
    name = "api.embeddings",
    skip(state, headers, auth, authz, payload),
    fields(model = %payload.model)
)]
pub async fn api_v1_embeddings(
    State(state): State<AppState>,
    headers: HeaderMap,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Valid(Json(payload)): Valid<Json<api_types::CreateEmbeddingPayload>>,
) -> Result<Response, ApiError> {
    // Route the model to a provider with dynamic support
    let model = payload.model.clone();
    let routed = route_model_extended(Some(&model), &state.config.providers)?;

    // Resolve to concrete provider configuration
    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        auth.as_ref().map(|e| &e.0),
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "provider_resolution_error",
            format!("Failed to resolve provider: {}", e),
        )
    })?;
    let provider_source = resolved.source;
    let (provider_name, provider_config, model_name) = (
        resolved.provider_name,
        resolved.provider_config,
        resolved.model,
    );

    // Check model restrictions if API key auth is used
    // Use original model string (with provider prefix) for restriction check
    if let Some(Extension(ref auth)) = auth
        && let Some(api_key) = auth.api_key()
    {
        api_key.check_model_allowed(&model).map_err(|e| {
            ApiError::new(StatusCode::FORBIDDEN, "model_not_allowed", e.to_string())
        })?;
    }

    // Check authorization if authz context is available and API RBAC is enabled
    if let Some(Extension(ref authz)) = authz {
        // Get org_id and project_id from auth context
        // Try API key first, then fall back to identity's first org_id
        let org_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.org_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.org_ids.first().cloned()))
        });
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.project_ids.first().cloned()))
        });

        // Check model access authorization (embeddings have no special request context)
        // Use original model string (with provider prefix) for RBAC policy evaluation
        authz
            .require_api(
                "model",
                "use",
                Some(&model),
                None, // No request context needed for embeddings
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    // Check if cache should be bypassed based on request headers
    let force_refresh = should_bypass_cache(&headers);

    // Track cache status for response headers
    let mut cache_status = CacheStatus::None;

    // Check response cache (embeddings are fully deterministic - excellent for caching)
    if let Some(ref response_cache) = state.response_cache {
        match response_cache
            .lookup_embeddings(&payload, &model_name, force_refresh)
            .await
        {
            CacheLookupResult::Hit(cached) => {
                tracing::debug!(
                    model = %model_name,
                    provider = %cached.provider,
                    cached_at = cached.cached_at,
                    "Returning cached response (embeddings API)"
                );
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", &cached.content_type)
                    .header("X-Cache", "HIT")
                    .header("X-Cached-At", cached.cached_at.to_string())
                    .header("X-Provider", &cached.provider)
                    .header("X-Model", &cached.model)
                    .body(Body::from(cached.body))
                    .unwrap());
            }
            CacheLookupResult::Miss => {
                cache_status = CacheStatus::Miss;
            }
            CacheLookupResult::Bypass => {
                // Caching disabled
            }
        }
    }

    // Execute embedding with fallback support
    let ExecutionResult {
        response,
        provider_name,
        model_name,
    } = execute_with_fallback::<EmbeddingExecutor>(
        &state,
        provider_name,
        provider_config,
        model_name,
        payload.clone(),
    )
    .await?;

    // Cache successful responses
    let final_response = if cache_status == CacheStatus::Miss && response.status().is_success() {
        // Extract content-type and body for caching
        let content_type = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json")
            .to_string();

        // Read the body bytes for caching
        let (parts, body) = response.into_parts();
        match axum::body::to_bytes(body, state.config.server.max_response_body_bytes).await {
            Ok(bytes) => {
                let body_vec = bytes.to_vec();

                // Store in response cache
                if let Some(ref response_cache) = state.response_cache {
                    let cache = response_cache.clone();
                    let payload_clone = payload.clone();
                    let model_clone = model_name.clone();
                    let provider_clone = provider_name.clone();
                    let content_type_clone = content_type;
                    let body_clone = body_vec.clone();
                    state.task_tracker.spawn(async move {
                        cache
                            .store_embeddings(
                                &payload_clone,
                                &model_clone,
                                &provider_clone,
                                body_clone,
                                &content_type_clone,
                            )
                            .await;
                    });
                }

                // Rebuild response
                Response::from_parts(parts, Body::from(body_vec))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to read response body for caching");
                // Return error - we've consumed the body
                return Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from("Failed to process response"))
                    .unwrap());
            }
        }
    } else {
        response
    };

    // Inject cost calculation into the response
    // Note: Embeddings don't stream, so no usage_entry or streaming_idle_timeout needed
    let mut final_response =
        crate::providers::inject_cost_into_response(crate::providers::CostInjectionParams {
            response: final_response,
            provider: &provider_name,
            model: &model_name,
            pricing: &state.pricing,
            db: state.db.as_ref(),
            usage_entry: None,
            task_tracker: Some(&state.task_tracker),
            max_response_body_bytes: state.config.server.max_response_body_bytes,
            streaming_idle_timeout_secs: 0, // Embeddings don't stream
            validation_config: &state.config.observability.response_validation,
            response_type: crate::validation::ResponseType::Embedding,
        })
        .await;

    // Add X-Cache: MISS header if this was a cache miss
    if cache_status == CacheStatus::Miss {
        final_response
            .headers_mut()
            .insert("X-Cache", "MISS".parse().unwrap());
    }

    // Add X-Provider and X-Model headers to identify which provider served the request
    // This is especially useful when fallback was used
    if let Ok(header_val) = provider_name.parse() {
        final_response
            .headers_mut()
            .insert("X-Provider", header_val);
    }
    if let Ok(source_val) = provider_source.parse() {
        final_response
            .headers_mut()
            .insert("X-Provider-Source", source_val);
    }
    if let Ok(header_val) = model_name.parse() {
        final_response.headers_mut().insert("X-Model", header_val);
    }

    Ok(final_response)
}

/// Combined models response with provider-prefixed model IDs.
#[derive(Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CombinedModelsResponse {
    /// List of available models
    #[cfg_attr(feature = "utoipa", schema(value_type = Vec<Object>))]
    data: Vec<serde_json::Value>,
}

/// List available models
///
/// Lists all models available from all configured providers.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/models",
    tag = "models",
    responses(
        (status = 200, description = "List of available models", body = CombinedModelsResponse),
        (status = 400, description = "Bad request", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(name = "api.models", skip(state, auth))]
pub async fn api_v1_models(
    State(state): State<AppState>,
    auth: Option<Extension<crate::auth::AuthenticatedRequest>>,
) -> Result<Json<CombinedModelsResponse>, ApiError> {
    use futures::future::join_all;

    // Create futures for fetching models from all providers in parallel
    let fetch_futures: Vec<_> = state
        .config
        .providers
        .iter()
        .map(|(provider_name, provider_config)| {
            let provider_name = provider_name.to_owned();
            let http_client = state.http_client.clone();
            let circuit_breakers = state.circuit_breakers.clone();

            async move {
                let models_result = crate::providers::list_models_for_config(
                    provider_config,
                    &provider_name,
                    &http_client,
                    &circuit_breakers,
                )
                .await;
                (provider_name, models_result)
            }
        })
        .collect();

    // Fetch from all providers in parallel
    let results = join_all(fetch_futures).await;

    // Collect successful results and enrich with catalog data
    let mut all_models = Vec::new();
    for (provider_name, models_result) in results {
        if let Ok(models_response) = models_result {
            // Get the provider config for catalog lookup
            let provider_config = state.config.providers.get(&provider_name);

            // Resolve the catalog provider ID for this provider
            let catalog_provider_id = provider_config.and_then(|pc| {
                crate::catalog::resolve_catalog_provider_id(
                    pc.provider_type_name(),
                    pc.base_url(),
                    pc.catalog_provider(),
                )
            });

            // Prefix each model ID with the provider name and enrich with catalog + config data
            for model in models_response.data {
                let prefixed_id = format!("{}/{}", provider_name, model.id);
                let mut model_json = model.extra;
                if let Some(obj) = model_json.as_object_mut() {
                    obj.insert("id".to_string(), serde_json::Value::String(prefixed_id));

                    // Look up catalog enrichment and config override
                    let enrichment = catalog_provider_id
                        .as_ref()
                        .and_then(|pid| state.model_catalog.lookup(pid, &model.id));
                    let model_config =
                        provider_config.and_then(|pc| pc.get_model_config(&model.id));

                    // Merge metadata: config wins if present, else catalog, else omit.
                    // Only enrich if at least one source has data.
                    if enrichment.is_some() || model_config.is_some() {
                        // Capabilities: config overrides catalog
                        if let Some(ref caps) = model_config.and_then(|mc| mc.capabilities.as_ref())
                        {
                            obj.insert(
                                "capabilities".to_string(),
                                serde_json::to_value(caps).unwrap_or_default(),
                            );
                        } else if let Some(ref e) = enrichment {
                            obj.insert(
                                "capabilities".to_string(),
                                serde_json::to_value(&e.capabilities).unwrap_or_default(),
                            );
                        }

                        // Context length: config > provider response > catalog
                        if let Some(ctx_len) = model_config.and_then(|mc| mc.context_length) {
                            obj.insert(
                                "context_length".to_string(),
                                serde_json::Value::Number(ctx_len.into()),
                            );
                        } else if !obj.contains_key("context_length")
                            && let Some(ctx_len) =
                                enrichment.as_ref().and_then(|e| e.limits.context_length)
                        {
                            obj.insert(
                                "context_length".to_string(),
                                serde_json::Value::Number(ctx_len.into()),
                            );
                        }

                        // Max output tokens
                        if let Some(max_out) = model_config.and_then(|mc| mc.max_output_tokens) {
                            obj.insert(
                                "max_output_tokens".to_string(),
                                serde_json::Value::Number(max_out.into()),
                            );
                        } else if let Some(max_out) =
                            enrichment.as_ref().and_then(|e| e.limits.max_output_tokens)
                        {
                            obj.insert(
                                "max_output_tokens".to_string(),
                                serde_json::Value::Number(max_out.into()),
                            );
                        }

                        // Modalities: config overrides catalog
                        if let Some(ref mods) = model_config.and_then(|mc| mc.modalities.as_ref()) {
                            obj.insert(
                                "modalities".to_string(),
                                serde_json::to_value(mods).unwrap_or_default(),
                            );
                        } else if let Some(ref e) = enrichment {
                            obj.insert(
                                "modalities".to_string(),
                                serde_json::to_value(&e.modalities).unwrap_or_default(),
                            );
                        }

                        // Tasks: config overrides catalog
                        let tasks = model_config
                            .filter(|mc| !mc.tasks.is_empty())
                            .map(|mc| &mc.tasks)
                            .or(enrichment
                                .as_ref()
                                .filter(|e| !e.tasks.is_empty())
                                .map(|e| &e.tasks));
                        if let Some(tasks) = tasks {
                            obj.insert(
                                "tasks".to_string(),
                                serde_json::to_value(tasks).unwrap_or_default(),
                            );
                        }

                        // Catalog pricing for display (from catalog only)
                        if let Some(ref e) = enrichment {
                            obj.insert(
                                "catalog_pricing".to_string(),
                                serde_json::to_value(&e.catalog_pricing).unwrap_or_default(),
                            );
                        }

                        // Family: config overrides catalog
                        if let Some(family) = model_config
                            .and_then(|mc| mc.family.as_ref())
                            .or(enrichment.as_ref().and_then(|e| e.family.as_ref()))
                        {
                            obj.insert(
                                "family".to_string(),
                                serde_json::Value::String(family.clone()),
                            );
                        }

                        // Open weights: config overrides catalog
                        if let Some(ow) = model_config.and_then(|mc| mc.open_weights) {
                            obj.insert("open_weights".to_string(), serde_json::Value::Bool(ow));
                        } else if let Some(ref e) = enrichment {
                            obj.insert(
                                "open_weights".to_string(),
                                serde_json::Value::Bool(e.open_weights),
                            );
                        }

                        // Image generation metadata (config only)
                        if let Some(mc) = model_config {
                            if !mc.image_sizes.is_empty() {
                                obj.insert(
                                    "image_sizes".to_string(),
                                    serde_json::to_value(&mc.image_sizes).unwrap_or_default(),
                                );
                            }
                            if !mc.image_qualities.is_empty() {
                                obj.insert(
                                    "image_qualities".to_string(),
                                    serde_json::to_value(&mc.image_qualities).unwrap_or_default(),
                                );
                            }
                            if let Some(max) = mc.max_images {
                                obj.insert(
                                    "max_images".to_string(),
                                    serde_json::Value::Number(max.into()),
                                );
                            }
                            if !mc.voices.is_empty() {
                                obj.insert(
                                    "voices".to_string(),
                                    serde_json::to_value(&mc.voices).unwrap_or_default(),
                                );
                            }
                        }
                    }
                } else {
                    model_json = serde_json::json!({ "id": prefixed_id });
                }
                all_models.push(model_json);
            }
        }
        // Skip providers that fail to return models
    }

    // Mark all static models with source
    for model in &mut all_models {
        if let Some(obj) = model.as_object_mut() {
            obj.insert(
                "source".to_string(),
                serde_json::Value::String("static".to_string()),
            );
        }
    }

    // Include dynamic models from the authenticated user's and org's providers (if any).
    // Falls back to the default anonymous user when API auth is disabled.
    let user_id_for_models = auth
        .as_ref()
        .and_then(|Extension(a)| a.user_id())
        .or(state.default_user_id);

    if let (Some(user_id), Some(services)) = (user_id_for_models, state.services.as_ref()) {
        // Look up the user's org membership for building scoped model IDs
        let org_membership = services
            .users
            .get_org_memberships_for_user(user_id)
            .await
            .ok()
            .and_then(|m| m.into_iter().next());

        let org_slug = org_membership.as_ref().map(|m| m.org_slug.as_str());

        // Helper: resolve models for a dynamic provider (with 5-minute cache)
        let resolve_models = |provider: &crate::models::DynamicProvider| {
            let provider = provider.clone();
            let http_client = state.http_client.clone();
            let circuit_breakers = state.circuit_breakers.clone();
            let secrets = state.secrets.clone();
            let cache = state.cache.clone();
            async move {
                if !provider.models.is_empty() {
                    return provider.models;
                }

                // Check cache for previously discovered models
                let cache_key = format!("gw:provider:models:{}", provider.id);
                if let Some(ref cache) = cache
                    && let Ok(Some(bytes)) = cache.get_bytes(&cache_key).await
                    && let Ok(models) = serde_json::from_slice::<Vec<String>>(&bytes)
                {
                    return models;
                }

                let Ok(config) = crate::routing::resolver::dynamic_provider_to_config(
                    &provider,
                    secrets.as_ref(),
                )
                .await
                else {
                    return Vec::new();
                };
                let models: Vec<String> = crate::providers::list_models_for_config(
                    &config,
                    &provider.name,
                    &http_client,
                    &circuit_breakers,
                )
                .await
                .map(|r| r.data.into_iter().map(|m| m.id).collect())
                .unwrap_or_default();

                // Cache the discovered models for 5 minutes
                if !models.is_empty()
                    && let Some(ref cache) = cache
                    && let Ok(bytes) = serde_json::to_vec(&models)
                {
                    let _ = cache
                        .set_bytes(&cache_key, &bytes, std::time::Duration::from_secs(300))
                        .await;
                }

                models
            }
        };

        // Collect all enabled providers across scopes, auto-paginating through cursor pages
        type ProviderPageFn = Box<
            dyn Fn(
                    crate::db::repos::ListParams,
                ) -> std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = crate::db::DbResult<
                                    crate::db::repos::ListResult<crate::models::DynamicProvider>,
                                >,
                            > + Send,
                    >,
                > + Send,
        >;
        let collect_all_enabled = |fetch_page: ProviderPageFn| async move {
            let mut all = Vec::new();
            let mut params = crate::db::repos::ListParams {
                limit: Some(100),
                ..Default::default()
            };
            loop {
                let Ok(page) = fetch_page(params.clone()).await else {
                    break;
                };
                all.extend(page.items);
                if !page.has_more {
                    break;
                }
                match page.cursors.next {
                    Some(cursor) => {
                        params.cursor = Some(cursor);
                    }
                    None => break,
                }
            }
            all
        };

        // Fetch user and org providers concurrently
        let user_providers_fut = {
            let services = services.clone();
            collect_all_enabled(Box::new(move |params| {
                let services = services.clone();
                Box::pin(async move {
                    services
                        .providers
                        .list_enabled_by_user(user_id, params)
                        .await
                })
            }))
        };

        let org_providers_fut = {
            let services = services.clone();
            let org_membership = org_membership.clone();
            async move {
                if let Some(ref membership) = org_membership {
                    let org_id = membership.org_id;
                    collect_all_enabled(Box::new(move |params| {
                        let services = services.clone();
                        Box::pin(async move {
                            services.providers.list_enabled_by_org(org_id, params).await
                        })
                    }))
                    .await
                } else {
                    Vec::new()
                }
            }
        };

        let project_providers_fut = {
            let services = services.clone();
            async move {
                let Ok(project_memberships) = services
                    .users
                    .get_project_memberships_for_user(user_id)
                    .await
                else {
                    return Vec::new();
                };
                let futs: Vec<_> = project_memberships
                    .iter()
                    .map(|m| {
                        let services = services.clone();
                        let project_id = m.project_id;
                        let project_slug = m.project_slug.clone();
                        async move {
                            let providers = collect_all_enabled(Box::new(move |params| {
                                let services = services.clone();
                                Box::pin(async move {
                                    services
                                        .providers
                                        .list_enabled_by_project(project_id, params)
                                        .await
                                })
                            }))
                            .await;
                            (project_slug, providers)
                        }
                    })
                    .collect();
                futures::future::join_all(futs).await
            }
        };

        let team_providers_fut = {
            let services = services.clone();
            async move {
                let Ok(team_memberships) =
                    services.users.get_team_memberships_for_user(user_id).await
                else {
                    return Vec::new();
                };
                let futs: Vec<_> = team_memberships
                    .iter()
                    .map(|m| {
                        let services = services.clone();
                        let team_id = m.team_id;
                        let team_slug = m.team_slug.clone();
                        let org_id = m.org_id;
                        async move {
                            let org_slug = services
                                .organizations
                                .get_by_id(org_id)
                                .await
                                .ok()
                                .flatten()
                                .map(|o| o.slug)
                                .unwrap_or_default();
                            let providers = collect_all_enabled(Box::new(move |params| {
                                let services = services.clone();
                                Box::pin(async move {
                                    services
                                        .providers
                                        .list_enabled_by_team(team_id, params)
                                        .await
                                })
                            }))
                            .await;
                            (org_slug, team_slug, providers)
                        }
                    })
                    .collect();
                futures::future::join_all(futs).await
            }
        };

        let (user_providers, org_providers, project_groups, team_groups) = tokio::join!(
            user_providers_fut,
            org_providers_fut,
            project_providers_fut,
            team_providers_fut,
        );

        // Resolve models for all providers concurrently within each scope
        let user_futs: Vec<_> = user_providers
            .iter()
            .map(|p| async move { (p, resolve_models(p).await) })
            .collect();
        let org_futs: Vec<_> = org_providers
            .iter()
            .map(|p| async move { (p, resolve_models(p).await) })
            .collect();
        let project_futs: Vec<_> = project_groups
            .iter()
            .flat_map(|(slug, providers)| {
                providers
                    .iter()
                    .map(move |p| async move { (slug.as_str(), p, resolve_models(p).await) })
            })
            .collect();

        let team_futs: Vec<_> = team_groups
            .iter()
            .flat_map(|(org_slug, team_slug, providers)| {
                providers.iter().map(move |p| async move {
                    (
                        org_slug.as_str(),
                        team_slug.as_str(),
                        p,
                        resolve_models(p).await,
                    )
                })
            })
            .collect();

        let (user_results, org_results, project_results, team_results) = tokio::join!(
            futures::future::join_all(user_futs),
            futures::future::join_all(org_futs),
            futures::future::join_all(project_futs),
            futures::future::join_all(team_futs),
        );

        // User-owned dynamic providers
        for (provider, model_names) in &user_results {
            let provider_name = &provider.name;
            for model_name in model_names {
                let scoped_id = if let Some(slug) = org_slug {
                    format!(":org/{slug}/:user/{user_id}/{provider_name}/{model_name}")
                } else {
                    format!(":user/{user_id}/{provider_name}/{model_name}")
                };
                all_models.push(serde_json::json!({
                    "id": scoped_id,
                    "object": "model",
                    "owned_by": provider_name,
                    "source": "dynamic",
                    "provider_name": provider_name,
                }));
            }
        }

        // Organization-owned dynamic providers
        if let Some(ref membership) = org_membership {
            for (provider, model_names) in &org_results {
                let provider_name = &provider.name;
                for model_name in model_names {
                    let scoped_id =
                        format!(":org/{}/{provider_name}/{model_name}", membership.org_slug);
                    all_models.push(serde_json::json!({
                        "id": scoped_id,
                        "object": "model",
                        "owned_by": provider_name,
                        "source": "dynamic",
                        "provider_name": provider_name,
                    }));
                }
            }
        }

        // Project-owned dynamic providers
        {
            let org_slug_for_project = org_membership
                .as_ref()
                .map(|m| m.org_slug.as_str())
                .unwrap_or("unknown");

            for (project_slug, provider, model_names) in &project_results {
                let provider_name = &provider.name;
                for model_name in model_names {
                    let scoped_id = format!(
                        ":org/{org_slug_for_project}/:project/{project_slug}/{provider_name}/{model_name}"
                    );
                    all_models.push(serde_json::json!({
                        "id": scoped_id,
                        "object": "model",
                        "owned_by": provider_name,
                        "source": "dynamic",
                        "provider_name": provider_name,
                    }));
                }
            }
        }

        // Team-owned dynamic providers
        for (org_slug, team_slug, provider, model_names) in &team_results {
            if org_slug.is_empty() {
                continue;
            }
            let provider_name = &provider.name;
            for model_name in model_names {
                let scoped_id =
                    format!(":org/{org_slug}/:team/{team_slug}/{provider_name}/{model_name}");
                all_models.push(serde_json::json!({
                    "id": scoped_id,
                    "object": "model",
                    "owned_by": provider_name,
                    "source": "dynamic",
                    "provider_name": provider_name,
                }));
            }
        }
    }

    Ok(Json(CombinedModelsResponse { data: all_models }))
}

// ============================================================================
// Image Generation Endpoints
// ============================================================================

/// Create image from text prompt
///
/// POST /v1/images/generations
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/images/generations",
    tag = "Images",
    request_body = api_types::CreateImageRequest,
    responses(
        (status = 200, description = "Image generated successfully", body = api_types::ImagesResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(
    name = "api.images.generations",
    skip(state, auth, authz, payload),
    fields(model = %payload.model.as_deref().unwrap_or("dall-e-2"))
)]
pub async fn api_v1_images_generations(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Valid(Json(payload)): Valid<Json<api_types::CreateImageRequest>>,
) -> Result<Response, ApiError> {
    // Route the model to a provider
    let model = payload.model.clone();
    let routed = route_model_extended(model.as_deref(), &state.config.providers)?;

    // Resolve to concrete provider configuration
    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        auth.as_ref().map(|e| &e.0),
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "provider_resolution_error",
            format!("Failed to resolve provider: {}", e),
        )
    })?;
    let provider_source = resolved.source;
    let (provider_name, provider_config, model_name) = (
        resolved.provider_name,
        resolved.provider_config,
        resolved.model,
    );

    // Check model restrictions if API key auth is used
    // Use original model string (with provider prefix) for restriction check
    if let Some(Extension(ref auth)) = auth
        && let Some(api_key) = auth.api_key()
    {
        let model_to_check = model.as_deref().unwrap_or(&model_name);
        api_key.check_model_allowed(model_to_check).map_err(|e| {
            ApiError::new(StatusCode::FORBIDDEN, "model_not_allowed", e.to_string())
        })?;
    }

    // Check authorization if authz context is available and API RBAC is enabled
    if let Some(Extension(ref authz)) = authz {
        // Build request context with image-specific fields
        let mut request_ctx = RequestContext::new().with_image_count(payload.n.unwrap_or(1) as u32);

        if let Some(ref size) = payload.size {
            request_ctx = request_ctx.with_image_size(image_size_to_string(size));
        }
        if let Some(ref quality) = payload.quality {
            request_ctx = request_ctx.with_image_quality(image_quality_to_string(quality));
        }

        // Get org_id and project_id from auth context
        // Try API key first, then fall back to identity's first org_id
        let org_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.org_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.org_ids.first().cloned()))
        });
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.project_ids.first().cloned()))
        });

        // Check model access authorization
        // Use original model string (with provider prefix) for RBAC policy evaluation
        authz
            .require_api(
                "model",
                "use",
                model.as_deref().or(Some(&model_name)),
                Some(request_ctx),
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    // Replace model with resolved name (strip provider prefix like "openai/dall-e-3" → "dall-e-3")
    let mut payload = payload;
    payload.model = Some(model_name.clone());

    // Strip parameters unsupported by this model's family (e.g. response_format for gpt-image)
    let model_family = provider_config
        .get_model_config(&model_name)
        .and_then(|mc| mc.family.as_deref());
    payload.normalize_for_family(model_family);

    // Capture size/quality for pricing before payload is consumed
    let pricing_size = payload.size.as_ref().map(image_size_to_string);
    let pricing_quality = payload.quality.as_ref().map(image_quality_to_string);

    // Execute the image generation request
    let response = match provider_config {
        ProviderConfig::OpenAi(config) => {
            open_ai::OpenAICompatibleProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_image(&state.http_client, payload)
            .await
        }
        #[cfg(feature = "provider-azure")]
        ProviderConfig::AzureOpenAi(config) => {
            azure_openai::AzureOpenAIProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_image(&state.http_client, payload)
            .await
        }
        ProviderConfig::Test(config) => {
            test::TestProvider::new(&config.model_name)
                .create_image(&state.http_client, payload)
                .await
        }
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "unsupported_provider",
                "This provider does not support image generation",
            ));
        }
    };

    let images_response = response.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_GATEWAY,
            "provider_error",
            format!("Image generation failed: {}", e),
        )
    })?;

    // Count images and log usage
    let image_count = images_response.data.as_ref().map(|d| d.len()).unwrap_or(0) as i64;
    let api_key_id = auth.as_ref().and_then(|a| a.api_key().map(|k| k.key.id));

    let (cost_microcents, _) =
        crate::providers::log_media_usage(crate::providers::MediaUsageParams {
            provider: &provider_name,
            model: &model_name,
            pricing: &state.pricing,
            db: state.db.as_ref(),
            api_key_id,
            task_tracker: &state.task_tracker,
            usage: crate::pricing::TokenUsage::for_images(
                image_count,
                pricing_size,
                pricing_quality,
            ),
        })
        .await;

    // Build response with cost headers
    let mut response = Json(&images_response).into_response();

    if let Some(cost) = cost_microcents
        && let Ok(value) = cost.to_string().parse()
    {
        response.headers_mut().insert("X-Cost-Microcents", value);
    }
    if let Ok(value) = image_count.to_string().parse() {
        response.headers_mut().insert("X-Image-Count", value);
    }
    if let Ok(value) = provider_name.parse() {
        response.headers_mut().insert("X-Provider", value);
    }
    if let Ok(source_val) = provider_source.parse() {
        response
            .headers_mut()
            .insert("X-Provider-Source", source_val);
    }
    if let Ok(value) = model_name.parse() {
        response.headers_mut().insert("X-Model", value);
    }

    Ok(response)
}

/// Edit image with text instructions
///
/// POST /v1/images/edits
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/images/edits",
    tag = "Images",
    request_body(content_type = "multipart/form-data", content = api_types::CreateImageEditRequest),
    responses(
        (status = 200, description = "Image edited successfully", body = api_types::ImagesResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(name = "api.images.edits", skip(state, auth, authz, multipart))]
pub async fn api_v1_images_edits(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    // Parse multipart form data
    let mut image_data: Option<Bytes> = None;
    let mut mask_data: Option<Bytes> = None;
    let mut prompt: Option<String> = None;
    let mut model: Option<String> = None;
    let mut n: Option<i32> = None;
    let mut size: Option<api_types::images::ImageSize> = None;
    let mut response_format: Option<api_types::images::ImageResponseFormat> = None;
    let mut user: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "multipart_error",
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let field_name = field.name().unwrap_or_default().to_string();

        match field_name.as_str() {
            "image" => {
                image_data = Some(field.bytes().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "image_read_error",
                        format!("Failed to read image: {}", e),
                    )
                })?);
            }
            "mask" => {
                mask_data = Some(field.bytes().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "mask_read_error",
                        format!("Failed to read mask: {}", e),
                    )
                })?);
            }
            "prompt" => {
                prompt = Some(field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "prompt_read_error",
                        format!("Failed to read prompt: {}", e),
                    )
                })?);
            }
            "model" => {
                model = Some(field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "model_read_error",
                        format!("Failed to read model: {}", e),
                    )
                })?);
            }
            "n" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "n_read_error",
                        format!("Failed to read n: {}", e),
                    )
                })?;
                n = Some(value.parse().map_err(|_| {
                    ApiError::new(StatusCode::BAD_REQUEST, "invalid_n", "Invalid value for n")
                })?);
            }
            "size" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "size_read_error",
                        format!("Failed to read size: {}", e),
                    )
                })?;
                size = Some(
                    serde_json::from_str(&format!("\"{}\"", value)).map_err(|_| {
                        ApiError::new(
                            StatusCode::BAD_REQUEST,
                            "invalid_size",
                            format!("Invalid size: {}", value),
                        )
                    })?,
                );
            }
            "response_format" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "response_format_read_error",
                        format!("Failed to read response_format: {}", e),
                    )
                })?;
                response_format = Some(serde_json::from_str(&format!("\"{}\"", value)).map_err(
                    |_| {
                        ApiError::new(
                            StatusCode::BAD_REQUEST,
                            "invalid_response_format",
                            format!("Invalid response_format: {}", value),
                        )
                    },
                )?);
            }
            "user" => {
                user = Some(field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "user_read_error",
                        format!("Failed to read user: {}", e),
                    )
                })?);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate required fields
    let image_data = image_data.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_image",
            "Missing required field: image",
        )
    })?;
    let prompt = prompt.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_prompt",
            "Missing required field: prompt",
        )
    })?;

    // Capture size for pricing before it's moved into the request
    let pricing_size = size.as_ref().map(image_size_to_string);

    // Build the request
    let request = api_types::CreateImageEditRequest {
        prompt,
        model: model.clone(),
        n,
        size,
        response_format,
        output_format: None,
        output_compression: None,
        background: None,
        quality: None,
        stream: None,
        partial_images: None,
        user,
    };

    // Route the model to a provider
    let routed = route_model_extended(model.as_deref(), &state.config.providers)?;

    // Resolve to concrete provider configuration
    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        auth.as_ref().map(|e| &e.0),
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "provider_resolution_error",
            format!("Failed to resolve provider: {}", e),
        )
    })?;
    let provider_source = resolved.source;
    let (provider_name, provider_config, model_name) = (
        resolved.provider_name,
        resolved.provider_config,
        resolved.model,
    );

    // Check authorization if authz context is available and API RBAC is enabled
    if let Some(Extension(ref authz)) = authz {
        // Build request context with image-specific fields
        let mut request_ctx = RequestContext::new().with_image_count(request.n.unwrap_or(1) as u32);

        if let Some(ref size) = request.size {
            request_ctx = request_ctx.with_image_size(image_size_to_string(size));
        }

        // Get org_id and project_id from auth context
        // Try API key first, then fall back to identity's first org_id
        let org_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.org_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.org_ids.first().cloned()))
        });
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.project_ids.first().cloned()))
        });

        // Check model access authorization
        // Use original model string (with provider prefix) for RBAC policy evaluation
        authz
            .require_api(
                "model",
                "use",
                model.as_deref().or(Some(&model_name)),
                Some(request_ctx),
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    // Replace model with resolved name (strip provider prefix)
    let mut request = request;
    request.model = Some(model_name.clone());

    // Strip parameters unsupported by this model's family (e.g. response_format for gpt-image)
    let model_family = provider_config
        .get_model_config(&model_name)
        .and_then(|mc| mc.family.as_deref());
    request.normalize_for_family(model_family);

    // Execute the image edit request
    let response = match provider_config {
        ProviderConfig::OpenAi(config) => {
            open_ai::OpenAICompatibleProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_image_edit(&state.http_client, image_data, mask_data, request)
            .await
        }
        #[cfg(feature = "provider-azure")]
        ProviderConfig::AzureOpenAi(config) => {
            azure_openai::AzureOpenAIProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_image_edit(&state.http_client, image_data, mask_data, request)
            .await
        }
        ProviderConfig::Test(config) => {
            test::TestProvider::new(&config.model_name)
                .create_image_edit(&state.http_client, image_data, mask_data, request)
                .await
        }
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "unsupported_provider",
                "This provider does not support image editing",
            ));
        }
    };

    let images_response = response.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_GATEWAY,
            "provider_error",
            format!("Image editing failed: {}", e),
        )
    })?;

    // Count images and log usage
    let image_count = images_response.data.as_ref().map(|d| d.len()).unwrap_or(0) as i64;
    let api_key_id = auth.as_ref().and_then(|a| a.api_key().map(|k| k.key.id));

    let (cost_microcents, _) =
        crate::providers::log_media_usage(crate::providers::MediaUsageParams {
            provider: &provider_name,
            model: &model_name,
            pricing: &state.pricing,
            db: state.db.as_ref(),
            api_key_id,
            task_tracker: &state.task_tracker,
            usage: crate::pricing::TokenUsage::for_images(
                image_count,
                pricing_size,
                None, // edits don't have a quality parameter
            ),
        })
        .await;

    // Build response with cost headers
    let mut response = Json(&images_response).into_response();

    if let Some(cost) = cost_microcents
        && let Ok(value) = cost.to_string().parse()
    {
        response.headers_mut().insert("X-Cost-Microcents", value);
    }
    if let Ok(value) = image_count.to_string().parse() {
        response.headers_mut().insert("X-Image-Count", value);
    }
    if let Ok(value) = provider_name.parse() {
        response.headers_mut().insert("X-Provider", value);
    }
    if let Ok(source_val) = provider_source.parse() {
        response
            .headers_mut()
            .insert("X-Provider-Source", source_val);
    }
    if let Ok(value) = model_name.parse() {
        response.headers_mut().insert("X-Model", value);
    }

    Ok(response)
}

/// Create image variations
///
/// POST /v1/images/variations
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/images/variations",
    tag = "Images",
    request_body(content_type = "multipart/form-data", content = api_types::CreateImageVariationRequest),
    responses(
        (status = 200, description = "Image variations created successfully", body = api_types::ImagesResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(name = "api.images.variations", skip(state, auth, authz, multipart))]
pub async fn api_v1_images_variations(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    // Parse multipart form data
    let mut image_data: Option<Bytes> = None;
    let mut model: Option<String> = None;
    let mut n: Option<i32> = None;
    let mut size: Option<api_types::images::ImageSize> = None;
    let mut response_format: Option<api_types::images::ImageResponseFormat> = None;
    let mut user: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "multipart_error",
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let field_name = field.name().unwrap_or_default().to_string();

        match field_name.as_str() {
            "image" => {
                image_data = Some(field.bytes().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "image_read_error",
                        format!("Failed to read image: {}", e),
                    )
                })?);
            }
            "model" => {
                model = Some(field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "model_read_error",
                        format!("Failed to read model: {}", e),
                    )
                })?);
            }
            "n" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "n_read_error",
                        format!("Failed to read n: {}", e),
                    )
                })?;
                n = Some(value.parse().map_err(|_| {
                    ApiError::new(StatusCode::BAD_REQUEST, "invalid_n", "Invalid value for n")
                })?);
            }
            "size" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "size_read_error",
                        format!("Failed to read size: {}", e),
                    )
                })?;
                size = Some(
                    serde_json::from_str(&format!("\"{}\"", value)).map_err(|_| {
                        ApiError::new(
                            StatusCode::BAD_REQUEST,
                            "invalid_size",
                            format!("Invalid size: {}", value),
                        )
                    })?,
                );
            }
            "response_format" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "response_format_read_error",
                        format!("Failed to read response_format: {}", e),
                    )
                })?;
                response_format = Some(serde_json::from_str(&format!("\"{}\"", value)).map_err(
                    |_| {
                        ApiError::new(
                            StatusCode::BAD_REQUEST,
                            "invalid_response_format",
                            format!("Invalid response_format: {}", value),
                        )
                    },
                )?);
            }
            "user" => {
                user = Some(field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "user_read_error",
                        format!("Failed to read user: {}", e),
                    )
                })?);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate required fields
    let image_data = image_data.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_image",
            "Missing required field: image",
        )
    })?;

    // Capture size for pricing before it's moved into the request
    let pricing_size = size.as_ref().map(image_size_to_string);

    // Build the request
    let request = api_types::CreateImageVariationRequest {
        model: model.clone(),
        n,
        size,
        response_format,
        user,
    };

    // Route the model to a provider
    let routed = route_model_extended(model.as_deref(), &state.config.providers)?;

    // Resolve to concrete provider configuration
    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        auth.as_ref().map(|e| &e.0),
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "provider_resolution_error",
            format!("Failed to resolve provider: {}", e),
        )
    })?;
    let provider_source = resolved.source;
    let (provider_name, provider_config, model_name) = (
        resolved.provider_name,
        resolved.provider_config,
        resolved.model,
    );

    // Check authorization if authz context is available and API RBAC is enabled
    if let Some(Extension(ref authz)) = authz {
        // Build request context with image-specific fields
        let mut request_ctx = RequestContext::new().with_image_count(request.n.unwrap_or(1) as u32);

        if let Some(ref size) = request.size {
            request_ctx = request_ctx.with_image_size(image_size_to_string(size));
        }

        // Get org_id and project_id from auth context
        // Try API key first, then fall back to identity's first org_id
        let org_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.org_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.org_ids.first().cloned()))
        });
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.project_ids.first().cloned()))
        });

        // Check model access authorization
        // Use original model string (with provider prefix) for RBAC policy evaluation
        authz
            .require_api(
                "model",
                "use",
                model.as_deref().or(Some(&model_name)),
                Some(request_ctx),
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    // Replace model with resolved name (strip provider prefix)
    let mut request = request;
    request.model = Some(model_name.clone());

    // Execute the image variation request
    let response = match provider_config {
        ProviderConfig::OpenAi(config) => {
            open_ai::OpenAICompatibleProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_image_variation(&state.http_client, image_data, request)
            .await
        }
        #[cfg(feature = "provider-azure")]
        ProviderConfig::AzureOpenAi(config) => {
            azure_openai::AzureOpenAIProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_image_variation(&state.http_client, image_data, request)
            .await
        }
        ProviderConfig::Test(config) => {
            test::TestProvider::new(&config.model_name)
                .create_image_variation(&state.http_client, image_data, request)
                .await
        }
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "unsupported_provider",
                "This provider does not support image variations",
            ));
        }
    };

    let images_response = response.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_GATEWAY,
            "provider_error",
            format!("Image variation failed: {}", e),
        )
    })?;

    // Count images and log usage
    let image_count = images_response.data.as_ref().map(|d| d.len()).unwrap_or(0) as i64;
    let api_key_id = auth.as_ref().and_then(|a| a.api_key().map(|k| k.key.id));

    let (cost_microcents, _) =
        crate::providers::log_media_usage(crate::providers::MediaUsageParams {
            provider: &provider_name,
            model: &model_name,
            pricing: &state.pricing,
            db: state.db.as_ref(),
            api_key_id,
            task_tracker: &state.task_tracker,
            usage: crate::pricing::TokenUsage::for_images(
                image_count,
                pricing_size,
                None, // variations don't have a quality parameter
            ),
        })
        .await;

    // Build response with cost headers
    let mut response = Json(&images_response).into_response();

    if let Some(cost) = cost_microcents
        && let Ok(value) = cost.to_string().parse()
    {
        response.headers_mut().insert("X-Cost-Microcents", value);
    }
    if let Ok(value) = image_count.to_string().parse() {
        response.headers_mut().insert("X-Image-Count", value);
    }
    if let Ok(value) = provider_name.parse() {
        response.headers_mut().insert("X-Provider", value);
    }
    if let Ok(source_val) = provider_source.parse() {
        response
            .headers_mut()
            .insert("X-Provider-Source", source_val);
    }
    if let Ok(value) = model_name.parse() {
        response.headers_mut().insert("X-Model", value);
    }

    Ok(response)
}

// ============================================================================
// Audio Endpoints
// ============================================================================

/// Generate speech from text
///
/// POST /v1/audio/speech
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/audio/speech",
    tag = "Audio",
    request_body = api_types::CreateSpeechRequest,
    responses(
        (status = 200, description = "Audio generated successfully", content_type = "audio/mpeg"),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(
    name = "api.audio.speech",
    skip(state, auth, authz, payload),
    fields(model = %payload.model, voice = ?payload.voice)
)]
pub async fn api_v1_audio_speech(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Valid(Json(payload)): Valid<Json<api_types::CreateSpeechRequest>>,
) -> Result<Response, ApiError> {
    // Count characters for usage tracking (before consuming payload)
    let character_count = payload.input.chars().count() as i64;

    // Route the model to a provider
    let model = Some(payload.model.clone());
    let routed = route_model_extended(model.as_deref(), &state.config.providers)?;

    // Resolve to concrete provider configuration
    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        auth.as_ref().map(|e| &e.0),
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "provider_resolution_error",
            format!("Failed to resolve provider: {}", e),
        )
    })?;
    let provider_source = resolved.source;
    let (provider_name, provider_config, model_name) = (
        resolved.provider_name,
        resolved.provider_config,
        resolved.model,
    );

    // Check model restrictions if API key auth is used
    // Use original model string (with provider prefix) for restriction check
    if let Some(Extension(ref auth)) = auth
        && let Some(api_key) = auth.api_key()
    {
        let model_to_check = model.as_deref().unwrap_or(&model_name);
        api_key.check_model_allowed(model_to_check).map_err(|e| {
            ApiError::new(StatusCode::FORBIDDEN, "model_not_allowed", e.to_string())
        })?;
    }

    // Check authorization if authz context is available and API RBAC is enabled
    if let Some(Extension(ref authz)) = authz {
        // Build request context with audio TTS-specific fields
        let request_ctx = RequestContext::new()
            .with_character_count(character_count as u64)
            .with_voice(voice_to_string(&payload.voice));

        // Get org_id and project_id from auth context
        // Try API key first, then fall back to identity's first org_id
        let org_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.org_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.org_ids.first().cloned()))
        });
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.project_ids.first().cloned()))
        });

        // Check model access authorization
        // Use original model string (with provider prefix) for RBAC policy evaluation
        authz
            .require_api(
                "model",
                "use",
                model.as_deref().or(Some(&model_name)),
                Some(request_ctx),
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    // Replace model with resolved name (strip provider prefix)
    let mut payload = payload;
    payload.model = model_name.clone();

    // Execute the speech request
    let response = match provider_config {
        ProviderConfig::OpenAi(config) => {
            open_ai::OpenAICompatibleProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_speech(&state.http_client, payload)
            .await
        }
        #[cfg(feature = "provider-azure")]
        ProviderConfig::AzureOpenAi(config) => {
            azure_openai::AzureOpenAIProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_speech(&state.http_client, payload)
            .await
        }
        ProviderConfig::Test(config) => {
            test::TestProvider::new(&config.model_name)
                .create_speech(&state.http_client, payload)
                .await
        }
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "unsupported_provider",
                "This provider does not support text-to-speech",
            ));
        }
    };

    let mut response = response.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_GATEWAY,
            "provider_error",
            format!("Speech generation failed: {}", e),
        )
    })?;

    // Log usage for TTS (character-based pricing)
    let api_key_id = auth.as_ref().and_then(|a| a.api_key().map(|k| k.key.id));

    let (cost_microcents, _) =
        crate::providers::log_media_usage(crate::providers::MediaUsageParams {
            provider: &provider_name,
            model: &model_name,
            pricing: &state.pricing,
            db: state.db.as_ref(),
            api_key_id,
            task_tracker: &state.task_tracker,
            usage: crate::pricing::TokenUsage::for_tts_characters(character_count),
        })
        .await;

    // Add cost headers to audio response
    if let Some(cost) = cost_microcents
        && let Ok(value) = cost.to_string().parse()
    {
        response.headers_mut().insert("X-Cost-Microcents", value);
    }
    if let Ok(value) = character_count.to_string().parse() {
        response.headers_mut().insert("X-Character-Count", value);
    }
    if let Ok(value) = provider_name.parse() {
        response.headers_mut().insert("X-Provider", value);
    }
    if let Ok(source_val) = provider_source.parse() {
        response
            .headers_mut()
            .insert("X-Provider-Source", source_val);
    }
    if let Ok(value) = model_name.parse() {
        response.headers_mut().insert("X-Model", value);
    }

    Ok(response)
}

/// Transcribe audio to text
///
/// POST /v1/audio/transcriptions
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/audio/transcriptions",
    tag = "Audio",
    request_body(content_type = "multipart/form-data", content = api_types::CreateTranscriptionRequest),
    responses(
        (status = 200, description = "Audio transcribed successfully", body = api_types::audio::TranscriptionResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(name = "api.audio.transcriptions", skip(state, auth, authz, multipart))]
pub async fn api_v1_audio_transcriptions(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    // Parse multipart form data
    let mut file_data: Option<Bytes> = None;
    let mut filename: Option<String> = None;
    let mut model: Option<String> = None;
    let mut language: Option<String> = None;
    let mut prompt: Option<String> = None;
    let mut response_format: Option<api_types::audio::AudioResponseFormat> = None;
    let mut temperature: Option<f32> = None;
    let mut timestamp_granularities: Option<Vec<api_types::audio::TimestampGranularity>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "multipart_error",
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let field_name = field.name().unwrap_or_default().to_string();

        match field_name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                file_data = Some(field.bytes().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "file_read_error",
                        format!("Failed to read file: {}", e),
                    )
                })?);
            }
            "model" => {
                model = Some(field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "model_read_error",
                        format!("Failed to read model: {}", e),
                    )
                })?);
            }
            "language" => {
                language = Some(field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "language_read_error",
                        format!("Failed to read language: {}", e),
                    )
                })?);
            }
            "prompt" => {
                prompt = Some(field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "prompt_read_error",
                        format!("Failed to read prompt: {}", e),
                    )
                })?);
            }
            "response_format" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "response_format_read_error",
                        format!("Failed to read response_format: {}", e),
                    )
                })?;
                response_format = Some(serde_json::from_str(&format!("\"{}\"", value)).map_err(
                    |_| {
                        ApiError::new(
                            StatusCode::BAD_REQUEST,
                            "invalid_response_format",
                            format!("Invalid response_format: {}", value),
                        )
                    },
                )?);
            }
            "temperature" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "temperature_read_error",
                        format!("Failed to read temperature: {}", e),
                    )
                })?;
                temperature = Some(value.parse().map_err(|_| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid_temperature",
                        "Invalid value for temperature",
                    )
                })?);
            }
            "timestamp_granularities[]" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "timestamp_granularities_read_error",
                        format!("Failed to read timestamp_granularities: {}", e),
                    )
                })?;
                let granularity: api_types::audio::TimestampGranularity =
                    serde_json::from_str(&format!("\"{}\"", value)).map_err(|_| {
                        ApiError::new(
                            StatusCode::BAD_REQUEST,
                            "invalid_timestamp_granularity",
                            format!("Invalid timestamp_granularity: {}", value),
                        )
                    })?;
                timestamp_granularities
                    .get_or_insert_with(Vec::new)
                    .push(granularity);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate required fields
    let file_data = file_data.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_file",
            "Missing required field: file",
        )
    })?;
    let model = model.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_model",
            "Missing required field: model",
        )
    })?;
    let filename = filename.unwrap_or_else(|| "audio.mp3".to_string());

    // Estimate audio duration from file size for usage tracking
    // Average bitrate ~128 kbps = 16000 bytes/sec
    // This is approximate; actual duration may vary by codec
    let file_size = file_data.len();
    let estimated_seconds = (file_size as i64 / 16000).max(1);

    // Build the request
    let request = api_types::CreateTranscriptionRequest {
        model: model.clone(),
        language,
        prompt,
        response_format,
        temperature,
        timestamp_granularities,
        stream: None,
        include: None,
        chunking_strategy: None,
        known_speaker_names: None,
        known_speaker_references: None,
    };

    // Route the model to a provider
    let routed = route_model_extended(Some(&model), &state.config.providers)?;

    // Resolve to concrete provider configuration
    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        auth.as_ref().map(|e| &e.0),
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "provider_resolution_error",
            format!("Failed to resolve provider: {}", e),
        )
    })?;
    let provider_source = resolved.source;
    let (provider_name, provider_config, model_name) = (
        resolved.provider_name,
        resolved.provider_config,
        resolved.model,
    );

    // Check model restrictions if API key auth is used
    // Use original model string (with provider prefix) for restriction check
    if let Some(Extension(ref auth)) = auth
        && let Some(api_key) = auth.api_key()
    {
        api_key.check_model_allowed(&model).map_err(|e| {
            ApiError::new(StatusCode::FORBIDDEN, "model_not_allowed", e.to_string())
        })?;
    }

    // Check authorization if authz context is available and API RBAC is enabled
    if let Some(Extension(ref authz)) = authz {
        // Build request context with audio transcription-specific fields
        let mut request_ctx = RequestContext::new();

        if let Some(ref lang) = request.language {
            request_ctx = request_ctx.with_language(lang.clone());
        }

        // Get org_id and project_id from auth context
        // Try API key first, then fall back to identity's first org_id
        let org_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.org_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.org_ids.first().cloned()))
        });
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.project_ids.first().cloned()))
        });

        // Check model access authorization
        // Use original model string (with provider prefix) for RBAC policy evaluation
        authz
            .require_api(
                "model",
                "use",
                Some(&model),
                Some(request_ctx),
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    // Replace model with resolved name (strip provider prefix)
    let mut request = request;
    request.model = model_name.clone();

    // Execute the transcription request
    let response = match provider_config {
        ProviderConfig::OpenAi(config) => {
            open_ai::OpenAICompatibleProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_transcription(&state.http_client, file_data, filename, request)
            .await
        }
        #[cfg(feature = "provider-azure")]
        ProviderConfig::AzureOpenAi(config) => {
            azure_openai::AzureOpenAIProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_transcription(&state.http_client, file_data, filename, request)
            .await
        }
        ProviderConfig::Test(config) => {
            test::TestProvider::new(&config.model_name)
                .create_transcription(&state.http_client, file_data, filename, request)
                .await
        }
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "unsupported_provider",
                "This provider does not support audio transcription",
            ));
        }
    };

    let mut response = response.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_GATEWAY,
            "provider_error",
            format!("Transcription failed: {}", e),
        )
    })?;

    // Log usage for audio transcription (per-second pricing)
    let api_key_id = auth.as_ref().and_then(|a| a.api_key().map(|k| k.key.id));

    let (cost_microcents, _) =
        crate::providers::log_media_usage(crate::providers::MediaUsageParams {
            provider: &provider_name,
            model: &model_name,
            pricing: &state.pricing,
            db: state.db.as_ref(),
            api_key_id,
            task_tracker: &state.task_tracker,
            usage: crate::pricing::TokenUsage::for_audio_seconds(estimated_seconds),
        })
        .await;

    // Add cost headers to response
    if let Some(cost) = cost_microcents
        && let Ok(value) = cost.to_string().parse()
    {
        response.headers_mut().insert("X-Cost-Microcents", value);
    }
    if let Ok(value) = estimated_seconds.to_string().parse() {
        response.headers_mut().insert("X-Audio-Seconds", value);
    }
    if let Ok(value) = provider_name.parse() {
        response.headers_mut().insert("X-Provider", value);
    }
    if let Ok(source_val) = provider_source.parse() {
        response
            .headers_mut()
            .insert("X-Provider-Source", source_val);
    }
    if let Ok(value) = model_name.parse() {
        response.headers_mut().insert("X-Model", value);
    }

    Ok(response)
}

/// Translate audio to English text
///
/// POST /v1/audio/translations
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/audio/translations",
    tag = "Audio",
    request_body(content_type = "multipart/form-data", content = api_types::CreateTranslationRequest),
    responses(
        (status = 200, description = "Audio translated successfully", body = api_types::audio::TranslationResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(name = "api.audio.translations", skip(state, auth, authz, multipart))]
pub async fn api_v1_audio_translations(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    // Parse multipart form data
    let mut file_data: Option<Bytes> = None;
    let mut filename: Option<String> = None;
    let mut model: Option<String> = None;
    let mut prompt: Option<String> = None;
    let mut response_format: Option<api_types::audio::AudioResponseFormat> = None;
    let mut temperature: Option<f32> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "multipart_error",
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let field_name = field.name().unwrap_or_default().to_string();

        match field_name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                file_data = Some(field.bytes().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "file_read_error",
                        format!("Failed to read file: {}", e),
                    )
                })?);
            }
            "model" => {
                model = Some(field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "model_read_error",
                        format!("Failed to read model: {}", e),
                    )
                })?);
            }
            "prompt" => {
                prompt = Some(field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "prompt_read_error",
                        format!("Failed to read prompt: {}", e),
                    )
                })?);
            }
            "response_format" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "response_format_read_error",
                        format!("Failed to read response_format: {}", e),
                    )
                })?;
                response_format = Some(serde_json::from_str(&format!("\"{}\"", value)).map_err(
                    |_| {
                        ApiError::new(
                            StatusCode::BAD_REQUEST,
                            "invalid_response_format",
                            format!("Invalid response_format: {}", value),
                        )
                    },
                )?);
            }
            "temperature" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "temperature_read_error",
                        format!("Failed to read temperature: {}", e),
                    )
                })?;
                temperature = Some(value.parse().map_err(|_| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid_temperature",
                        "Invalid value for temperature",
                    )
                })?);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate required fields
    let file_data = file_data.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_file",
            "Missing required field: file",
        )
    })?;
    let model = model.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_model",
            "Missing required field: model",
        )
    })?;
    let filename = filename.unwrap_or_else(|| "audio.mp3".to_string());

    // Estimate audio duration from file size for usage tracking
    // Average bitrate ~128 kbps = 16000 bytes/sec
    // This is approximate; actual duration may vary by codec
    let file_size = file_data.len();
    let estimated_seconds = (file_size as i64 / 16000).max(1);

    // Build the request
    let request = api_types::CreateTranslationRequest {
        model: model.clone(),
        prompt,
        response_format,
        temperature,
    };

    // Route the model to a provider
    let routed = route_model_extended(Some(&model), &state.config.providers)?;

    // Resolve to concrete provider configuration
    let resolved = resolver::resolve_to_provider(
        routed,
        state.db.as_ref(),
        state.cache.as_ref(),
        state.secrets.as_ref(),
        auth.as_ref().map(|e| &e.0),
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "provider_resolution_error",
            format!("Failed to resolve provider: {}", e),
        )
    })?;
    let provider_source = resolved.source;
    let (provider_name, provider_config, model_name) = (
        resolved.provider_name,
        resolved.provider_config,
        resolved.model,
    );

    // Check model restrictions if API key auth is used
    // Use original model string (with provider prefix) for restriction check
    if let Some(Extension(ref auth)) = auth
        && let Some(api_key) = auth.api_key()
    {
        api_key.check_model_allowed(&model).map_err(|e| {
            ApiError::new(StatusCode::FORBIDDEN, "model_not_allowed", e.to_string())
        })?;
    }

    // Check authorization if authz context is available and API RBAC is enabled
    if let Some(Extension(ref authz)) = authz {
        // Build request context (translation has minimal context - just model)
        let request_ctx = RequestContext::new();

        // Get org_id and project_id from auth context
        // Try API key first, then fall back to identity's first org_id
        let org_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.org_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.org_ids.first().cloned()))
        });
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
                .or_else(|| a.identity().and_then(|i| i.project_ids.first().cloned()))
        });

        // Check model access authorization
        // Use original model string (with provider prefix) for RBAC policy evaluation
        authz
            .require_api(
                "model",
                "use",
                Some(&model),
                Some(request_ctx),
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    // Replace model with resolved name (strip provider prefix)
    let mut request = request;
    request.model = model_name.clone();

    // Execute the translation request
    let response = match provider_config {
        ProviderConfig::OpenAi(config) => {
            open_ai::OpenAICompatibleProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_translation(&state.http_client, file_data, filename, request)
            .await
        }
        #[cfg(feature = "provider-azure")]
        ProviderConfig::AzureOpenAi(config) => {
            azure_openai::AzureOpenAIProvider::from_config_with_registry(
                &config,
                &provider_name,
                &state.circuit_breakers,
            )
            .create_translation(&state.http_client, file_data, filename, request)
            .await
        }
        ProviderConfig::Test(config) => {
            test::TestProvider::new(&config.model_name)
                .create_translation(&state.http_client, file_data, filename, request)
                .await
        }
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "unsupported_provider",
                "This provider does not support audio translation",
            ));
        }
    };

    let mut response = response.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_GATEWAY,
            "provider_error",
            format!("Translation failed: {}", e),
        )
    })?;

    // Log usage for audio translation (per-second pricing)
    let api_key_id = auth.as_ref().and_then(|a| a.api_key().map(|k| k.key.id));

    let (cost_microcents, _) =
        crate::providers::log_media_usage(crate::providers::MediaUsageParams {
            provider: &provider_name,
            model: &model_name,
            pricing: &state.pricing,
            db: state.db.as_ref(),
            api_key_id,
            task_tracker: &state.task_tracker,
            usage: crate::pricing::TokenUsage::for_audio_seconds(estimated_seconds),
        })
        .await;

    // Add cost headers to response
    if let Some(cost) = cost_microcents
        && let Ok(value) = cost.to_string().parse()
    {
        response.headers_mut().insert("X-Cost-Microcents", value);
    }
    if let Ok(value) = estimated_seconds.to_string().parse() {
        response.headers_mut().insert("X-Audio-Seconds", value);
    }
    if let Ok(value) = provider_name.parse() {
        response.headers_mut().insert("X-Provider", value);
    }
    if let Ok(source_val) = provider_source.parse() {
        response
            .headers_mut()
            .insert("X-Provider-Source", source_val);
    }
    if let Ok(value) = model_name.parse() {
        response.headers_mut().insert("X-Model", value);
    }

    Ok(response)
}

// ============================================================================
// Guardrails Audit Logging Helpers
// ============================================================================

/// Logs a guardrails evaluation event to the audit log.
///
/// This function spawns a background task to log the event, ensuring
/// request latency is not impacted by audit logging.
fn log_guardrails_evaluation(
    state: &AppState,
    auth: Option<&Extension<AuthenticatedRequest>>,
    provider: &str,
    stage: &str,
    result: &crate::guardrails::InputGuardrailsResult,
    request_id: Option<&str>,
) {
    // Get the audit config
    let Some(guardrails_config) = &state.config.features.guardrails else {
        return;
    };
    let audit_config = &guardrails_config.audit;

    // Check if we should log this evaluation
    if !audit_config.enabled {
        return;
    }

    // Only log if there are violations or if log_all_evaluations is true
    let has_violations = !result.response.violations.is_empty();
    if !has_violations && !audit_config.log_all_evaluations {
        return;
    }

    let Some(db) = &state.db else { return };

    // Determine what action was taken
    let (action_type, should_log) = match &result.action {
        crate::guardrails::ResolvedAction::Allow => ("allow", audit_config.log_all_evaluations),
        crate::guardrails::ResolvedAction::Block { .. } => ("block", audit_config.log_blocked),
        crate::guardrails::ResolvedAction::Warn { .. } => ("warn", audit_config.log_violations),
        crate::guardrails::ResolvedAction::Log { .. } => ("log", audit_config.log_violations),
        crate::guardrails::ResolvedAction::Redact { .. } => ("redact", audit_config.log_redacted),
    };

    if !should_log {
        return;
    }

    let db = db.clone();
    let api_key_id = auth.and_then(|a| a.0.api_key().map(|k| k.key.id));
    let org_id = auth.and_then(|a| a.0.api_key().and_then(|k| k.org_id));
    let project_id = auth.and_then(|a| a.0.api_key().and_then(|k| k.project_id));
    let provider = provider.to_string();
    let stage = stage.to_string();
    let request_id = request_id.map(String::from);
    let passed = result.response.passed;
    let latency_ms = result.response.latency_ms;
    let action_type = action_type.to_string();
    let violations: Vec<serde_json::Value> = result
        .response
        .violations
        .iter()
        .map(|v| {
            serde_json::json!({
                "category": v.category.to_string(),
                "severity": v.severity.to_string(),
                "confidence": v.confidence,
                "message": v.message,
            })
        })
        .collect();

    state.task_tracker.spawn(async move {
        let result = db
            .audit_logs()
            .create(crate::models::CreateAuditLog {
                actor_type: api_key_id
                    .map(|_| crate::models::AuditActorType::ApiKey)
                    .unwrap_or(crate::models::AuditActorType::System),
                actor_id: api_key_id,
                action: format!("guardrails.{}", action_type),
                resource_type: "guardrails".to_string(),
                resource_id: api_key_id.unwrap_or(uuid::Uuid::nil()),
                org_id,
                project_id,
                details: serde_json::json!({
                    "provider": provider,
                    "stage": stage,
                    "passed": passed,
                    "latency_ms": latency_ms,
                    "action": action_type,
                    "violations": violations,
                    "request_id": request_id,
                }),
                ip_address: None,
                user_agent: None,
            })
            .await;

        if let Err(e) = result {
            tracing::warn!(
                error = %e,
                provider = %provider,
                stage = %stage,
                "Failed to log guardrails audit event"
            );
        }
    });
}

/// Logs an output guardrails evaluation event to the audit log.
fn log_output_guardrails_evaluation(
    state: &AppState,
    auth: Option<&Extension<AuthenticatedRequest>>,
    provider: &str,
    result: &crate::guardrails::OutputGuardrailsResult,
    request_id: Option<&str>,
) {
    // Get the audit config
    let Some(guardrails_config) = &state.config.features.guardrails else {
        return;
    };
    let audit_config = &guardrails_config.audit;

    // Check if we should log this evaluation
    if !audit_config.enabled {
        return;
    }

    // Only log if there are violations or if log_all_evaluations is true
    let has_violations = !result.response.violations.is_empty();
    if !has_violations && !audit_config.log_all_evaluations {
        return;
    }

    let Some(db) = &state.db else { return };

    // Determine what action was taken
    let (action_type, should_log) = match &result.action {
        crate::guardrails::ResolvedAction::Allow => ("allow", audit_config.log_all_evaluations),
        crate::guardrails::ResolvedAction::Block { .. } => ("block", audit_config.log_blocked),
        crate::guardrails::ResolvedAction::Warn { .. } => ("warn", audit_config.log_violations),
        crate::guardrails::ResolvedAction::Log { .. } => ("log", audit_config.log_violations),
        crate::guardrails::ResolvedAction::Redact { .. } => ("redact", audit_config.log_redacted),
    };

    if !should_log {
        return;
    }

    let db = db.clone();
    let api_key_id = auth.and_then(|a| a.0.api_key().map(|k| k.key.id));
    let org_id = auth.and_then(|a| a.0.api_key().and_then(|k| k.org_id));
    let project_id = auth.and_then(|a| a.0.api_key().and_then(|k| k.project_id));
    let provider = provider.to_string();
    let request_id = request_id.map(String::from);
    let passed = result.response.passed;
    let latency_ms = result.response.latency_ms;
    let action_type = action_type.to_string();

    // For redacted content, include hashes instead of actual content
    let content_hashes = if let crate::guardrails::ResolvedAction::Redact {
        original_content,
        modified_content,
        ..
    } = &result.action
    {
        Some(serde_json::json!({
            "original_content_hash": crate::guardrails::audit::hash_content(original_content),
            "redacted_content_hash": crate::guardrails::audit::hash_content(modified_content),
        }))
    } else {
        None
    };

    let violations: Vec<serde_json::Value> = result
        .response
        .violations
        .iter()
        .map(|v| {
            serde_json::json!({
                "category": v.category.to_string(),
                "severity": v.severity.to_string(),
                "confidence": v.confidence,
                "message": v.message,
            })
        })
        .collect();

    state.task_tracker.spawn(async move {
        let mut details = serde_json::json!({
            "provider": provider,
            "stage": "output",
            "passed": passed,
            "latency_ms": latency_ms,
            "action": action_type,
            "violations": violations,
            "request_id": request_id,
        });

        // Add content hashes if this was a redaction
        if let Some(hashes) = content_hashes
            && let Some(obj) = details.as_object_mut()
        {
            obj.insert("content_hashes".to_string(), hashes);
        }

        let result = db
            .audit_logs()
            .create(crate::models::CreateAuditLog {
                actor_type: api_key_id
                    .map(|_| crate::models::AuditActorType::ApiKey)
                    .unwrap_or(crate::models::AuditActorType::System),
                actor_id: api_key_id,
                action: format!("guardrails.{}", action_type),
                resource_type: "guardrails".to_string(),
                resource_id: api_key_id.unwrap_or(uuid::Uuid::nil()),
                org_id,
                project_id,
                details,
                ip_address: None,
                user_agent: None,
            })
            .await;

        if let Err(e) = result {
            tracing::warn!(
                error = %e,
                provider = %provider,
                "Failed to log output guardrails audit event"
            );
        }
    });
}

// ============================================================================
// Files API (OpenAI-compatible)
// ============================================================================

/// Get services from app state for Files/Vector Stores APIs
fn get_services(state: &AppState) -> Result<&Services, ApiError> {
    state.services.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "feature_not_available",
            "This endpoint requires database support. Rebuild with --features database-sqlite or --features database-postgres.",
        )
    })
}

/// Query parameters for listing files (OpenAI-compatible).
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
pub struct ListFilesQuery {
    /// Maximum number of files to return (default: 20, max: 100)
    #[cfg_attr(feature = "utoipa", param(minimum = 1, maximum = 100))]
    pub limit: Option<i64>,
    /// Sort order by `created_at` timestamp (default: desc)
    #[serde(default)]
    pub order: Option<SortOrder>,
    /// Cursor for forward pagination. Returns results after this file ID.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "file-550e8400-e29b-41d4-a716-446655440000")
    )]
    pub after: Option<String>,
    /// **Hadrian Extension:** Cursor for backward pagination. Returns results before this file ID.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "file-550e8400-e29b-41d4-a716-446655440000")
    )]
    pub before: Option<String>,
    /// Filter by purpose
    #[cfg_attr(feature = "utoipa", param(example = "assistants"))]
    pub purpose: Option<String>,
    /// **Hadrian Extension:** Owner type for multi-tenancy (organization, project, or user)
    pub owner_type: String,
    /// **Hadrian Extension:** Owner ID for multi-tenancy
    pub owner_id: Uuid,
}

/// Paginated list of files response (OpenAI-compatible).
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileListResponse {
    /// Object type (always "list")
    pub object: String,
    /// List of files
    pub data: Vec<File>,
    /// ID of the first file in the list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    /// ID of the last file in the list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    /// Whether there are more results available
    pub has_more: bool,
}

/// Delete file response
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DeleteFileResponse {
    /// File ID that was deleted
    pub id: String,
    /// Object type (always "file")
    pub object: String,
    /// Whether the file was deleted
    pub deleted: bool,
}

/// Upload a file
///
/// Uploads a file that can be used with vector stores for RAG.
/// Files are uploaded as multipart/form-data with the following fields:
/// - `file`: The file to upload (required)
/// - `purpose`: The intended purpose of the file (default: "assistants")
/// - `owner_type`: Owner type - "organization", "project", or "user" (required)
/// - `owner_id`: Owner ID (required)
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/files",
    tag = "files",
    operation_id = "file_upload",
    request_body(content_type = "multipart/form-data", description = "File upload with metadata"),
    responses(
        (status = 200, description = "File uploaded successfully", body = File),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 413, description = "File too large", body = crate::openapi::ErrorResponse),
        (status = 422, description = "Virus detected in uploaded file", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz, multipart), fields(purpose))]
pub async fn api_v1_files_upload(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    mut multipart: Multipart,
) -> Result<Json<File>, ApiError> {
    // Check file upload permission via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "file",
                "upload",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let services = get_services(&state)?;

    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut purpose = FilePurpose::Assistants;
    let mut owner_type: Option<VectorStoreOwnerType> = None;
    let mut owner_id: Option<Uuid> = None;

    // Parse multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "multipart_error",
            format!("Failed to read multipart field: {}", e),
        )
    })? {
        let field_name = field.name().unwrap_or_default().to_string();

        match field_name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                content_type = field.content_type().map(|s| s.to_string());
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| {
                            ApiError::new(
                                StatusCode::BAD_REQUEST,
                                "file_read_error",
                                format!("Failed to read file: {}", e),
                            )
                        })?
                        .to_vec(),
                );
            }
            "purpose" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "purpose_read_error",
                        format!("Failed to read purpose: {}", e),
                    )
                })?;
                purpose = value.parse().map_err(|_| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid_purpose",
                        format!("Invalid purpose: {}", value),
                    )
                })?;
            }
            "owner_type" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "owner_type_read_error",
                        format!("Failed to read owner_type: {}", e),
                    )
                })?;
                owner_type = Some(value.parse().map_err(|_| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid_owner_type",
                        format!("Invalid owner_type: {}", value),
                    )
                })?);
            }
            "owner_id" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "owner_id_read_error",
                        format!("Failed to read owner_id: {}", e),
                    )
                })?;
                owner_id = Some(Uuid::parse_str(&value).map_err(|_| {
                    ApiError::new(
                        StatusCode::BAD_REQUEST,
                        "invalid_owner_id",
                        format!("Invalid owner_id: {}", value),
                    )
                })?);
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate required fields
    let file_data = file_data.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_file",
            "Missing required field: file",
        )
    })?;
    let filename = filename.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_filename",
            "Missing filename in file field",
        )
    })?;
    let owner_type = owner_type.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_owner_type",
            "Missing required field: owner_type",
        )
    })?;
    let owner_id = owner_id.ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_owner_id",
            "Missing required field: owner_id",
        )
    })?;

    // Validate file size against configured limit
    let max_file_size = state.config.features.file_processing.max_file_size_bytes();
    let file_size = file_data.len() as i64;
    if file_size > max_file_size {
        let max_mb = state.config.features.file_processing.max_file_size_mb;
        let file_mb = file_size as f64 / (1024.0 * 1024.0);
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "file_too_large",
            format!(
                "File size ({:.2} MB) exceeds maximum allowed size ({} MB)",
                file_mb, max_mb
            ),
        ));
    }

    // Validate file type based on purpose (extension check)
    if let Err(msg) = purpose.validate_file_extension(&filename) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_file_type",
            msg,
        ));
    }

    // Validate file content magic bytes match declared type
    if let Err(msg) = purpose.validate_file_content(&file_data) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_file_content",
            msg,
        ));
    }

    // Virus scan if enabled
    #[cfg(feature = "virus-scan")]
    {
        let virus_scan_config = &state.config.features.file_processing.virus_scan;
        if virus_scan_config.enabled {
            use crate::services::{ClamAvScanner, VirusScanner};

            let clamav_config = virus_scan_config.clamav.clone().unwrap_or_default();
            let scanner = ClamAvScanner::new(clamav_config).map_err(|e| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "virus_scan_config_error",
                    format!("Failed to initialize virus scanner: {}", e),
                )
            })?;

            let scan_result = scanner.scan(&file_data).await.map_err(|e| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "virus_scan_error",
                    format!("Virus scan failed: {}", e),
                )
            })?;

            if !scan_result.is_clean {
                let threat_name = scan_result
                    .threat_name
                    .unwrap_or_else(|| "Unknown".to_string());
                return Err(ApiError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "virus_detected",
                    format!("File rejected: malware detected ({})", threat_name),
                ));
            }
        }
    }

    // Validate that the owner exists
    let db = state.db.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database_not_configured",
            "Database not configured",
        )
    })?;

    let owner_type_name = match owner_type {
        VectorStoreOwnerType::User => "User",
        VectorStoreOwnerType::Organization => "Organization",
        VectorStoreOwnerType::Team => "Team",
        VectorStoreOwnerType::Project => "Project",
    };

    let owner_exists = match owner_type {
        VectorStoreOwnerType::User => {
            let result: Option<crate::models::User> =
                db.users().get_by_id(owner_id).await.unwrap_or(None);
            result.is_some()
        }
        VectorStoreOwnerType::Organization => {
            let result: Option<crate::models::Organization> =
                db.organizations().get_by_id(owner_id).await.unwrap_or(None);
            result.is_some()
        }
        VectorStoreOwnerType::Team => {
            let result: Option<crate::models::Team> =
                db.teams().get_by_id(owner_id).await.unwrap_or(None);
            result.is_some()
        }
        VectorStoreOwnerType::Project => {
            let result: Option<crate::models::Project> =
                db.projects().get_by_id(owner_id).await.unwrap_or(None);
            result.is_some()
        }
    };

    if !owner_exists {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "owner_not_found",
            format!("{} with ID {} not found", owner_type_name, owner_id),
        ));
    }

    // Create file with configured storage backend
    let storage_backend = services.files.configured_backend();
    let input = FilesService::create_file_input(
        owner_type,
        owner_id,
        filename,
        purpose,
        content_type,
        file_data,
        storage_backend,
    );

    let file = services.files.upload(input).await?;
    Ok(Json(file))
}

/// List files
///
/// Returns a list of files owned by the specified owner.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/files",
    tag = "files",
    operation_id = "file_list",
    params(ListFilesQuery),
    responses(
        (status = 200, description = "List of files", body = FileListResponse),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_files_list(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Query(query): Query<ListFilesQuery>,
) -> Result<Json<FileListResponse>, ApiError> {
    use crate::db::repos::{Cursor, CursorDirection};

    // Check file list permission via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "file",
                "list",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let services = get_services(&state)?;

    let owner_type: VectorStoreOwnerType = query.owner_type.parse().map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_owner_type",
            "Invalid owner_type",
        )
    })?;

    let purpose = query
        .purpose
        .map(|p| {
            p.parse::<FilePurpose>().map_err(|_| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_purpose",
                    format!("Invalid purpose: {}", p),
                )
            })
        })
        .transpose()?;

    // OpenAI defaults: limit=20
    let limit = query.limit.unwrap_or(20).min(100);

    // Parse cursor from `after` or `before` parameter
    let (cursor, direction) = if let Some(ref after_id) = query.after {
        let file_id: FileId = after_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'after' cursor: {}", after_id),
            )
        })?;

        let cursor_record = services
            .files
            .get(file_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("File '{}' not found for cursor", after_id),
                )
            })?;

        (
            Some(Cursor::new(cursor_record.created_at, cursor_record.id)),
            CursorDirection::Forward,
        )
    } else if let Some(ref before_id) = query.before {
        let file_id: FileId = before_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'before' cursor: {}", before_id),
            )
        })?;

        let cursor_record = services
            .files
            .get(file_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("File '{}' not found for cursor", before_id),
                )
            })?;

        (
            Some(Cursor::new(cursor_record.created_at, cursor_record.id)),
            CursorDirection::Backward,
        )
    } else {
        (None, CursorDirection::Forward)
    };

    let params = ListParams {
        limit: Some(limit),
        cursor,
        direction,
        sort_order: query.order.unwrap_or_default().into(),
        ..Default::default()
    };

    let result = services
        .files
        .list(owner_type, query.owner_id, purpose, params)
        .await?;

    // Build OpenAI-compatible response
    let first_id = result.items.first().map(|f| FileId::new(f.id).to_string());
    let last_id = result.items.last().map(|f| FileId::new(f.id).to_string());

    Ok(Json(FileListResponse {
        object: "list".to_string(),
        data: result.items,
        first_id,
        last_id,
        has_more: result.has_more,
    }))
}

/// Get file metadata
///
/// Returns information about a specific file.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/files/{file_id}",
    tag = "files",
    operation_id = "file_get",
    params(("file_id" = Uuid, Path, description = "File ID")),
    responses(
        (status = 200, description = "File metadata", body = File),
        (status = 404, description = "File not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_files_get(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(file_id): Path<FileId>,
) -> Result<Json<File>, ApiError> {
    // Check file read permission via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "file",
                "read",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    let file = services.files.get(file_id).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("File '{}' not found", file_id),
        )
    })?;

    // Check access permission
    check_resource_access_optional(auth.as_ref().map(|e| &e.0), file.owner_type, file.owner_id)?;

    Ok(Json(file))
}

/// Get file content
///
/// Returns the content of a file.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/files/{file_id}/content",
    tag = "files",
    operation_id = "file_get_content",
    params(("file_id" = Uuid, Path, description = "File ID")),
    responses(
        (status = 200, description = "File content", content_type = "application/octet-stream"),
        (status = 404, description = "File not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_files_get_content(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(file_id): Path<FileId>,
) -> Result<Response, ApiError> {
    // Check file read permission via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "file",
                "read",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    // Get file metadata first (for content-type and filename)
    let file = services.files.get(file_id).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("File '{}' not found", file_id),
        )
    })?;

    // Check access permission
    check_resource_access_optional(auth.as_ref().map(|e| &e.0), file.owner_type, file.owner_id)?;

    // Get content from the appropriate storage backend
    let content = services.files.get_content(file_id).await?;

    let content_type = file
        .content_type
        .unwrap_or_else(|| "application/octet-stream".to_string());

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", file.filename),
            ),
        ],
        Bytes::from(content),
    )
        .into_response())
}

/// Delete a file
///
/// Deletes a file. The file cannot be deleted if it is still referenced by any vector stores.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/api/v1/files/{file_id}",
    tag = "files",
    operation_id = "file_delete",
    params(("file_id" = Uuid, Path, description = "File ID")),
    responses(
        (status = 200, description = "File deleted", body = DeleteFileResponse),
        (status = 400, description = "File is still in use", body = crate::openapi::ErrorResponse),
        (status = 404, description = "File not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_files_delete(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(file_id): Path<FileId>,
) -> Result<Json<DeleteFileResponse>, ApiError> {
    // Check file delete permission via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "file",
                "delete",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    // Keep prefixed ID for response formatting
    let file_id_prefixed = file_id.to_string();
    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    // Check if file exists
    let file = services.files.get(file_id).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("File '{}' not found", file_id),
        )
    })?;

    // Check access permission
    check_resource_access_optional(auth.as_ref().map(|e| &e.0), file.owner_type, file.owner_id)?;

    // Check if file is still referenced (active references only, not soft-deleted)
    let ref_count = services.files.count_references(file_id).await?;
    if ref_count > 0 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "file_in_use",
            format!("File is still referenced by {} vector store(s)", ref_count),
        ));
    }

    // Clean up any soft-deleted references to avoid FK constraint violations
    services
        .vector_stores
        .cleanup_soft_deleted_references(file_id)
        .await?;

    // Delete the file
    services.files.delete(file_id).await?;

    Ok(Json(DeleteFileResponse {
        id: file_id_prefixed,
        object: "file".to_string(),
        deleted: true,
    }))
}

// ============================================================================
// Vector Stores API (OpenAI-compatible)
// ============================================================================

/// Sort order for list queries.
///
/// OpenAI-compatible sort order parameter for paginated list endpoints.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    /// Ascending order (oldest first)
    Asc,
    /// Descending order (newest first)
    #[default]
    Desc,
}

impl From<SortOrder> for crate::db::repos::SortOrder {
    fn from(order: SortOrder) -> Self {
        match order {
            SortOrder::Asc => crate::db::repos::SortOrder::Asc,
            SortOrder::Desc => crate::db::repos::SortOrder::Desc,
        }
    }
}

/// Query parameters for listing vector stores.
///
/// ## OpenAI Compatibility
///
/// This endpoint supports OpenAI-compatible cursor-based pagination:
/// - `limit`: Maximum number of results (1-100, default 20)
/// - `order`: Sort order by `created_at` timestamp (asc/desc, default desc)
/// - `after`: Cursor for forward pagination (object ID, e.g., `vs_abc123`)
/// - `before`: Cursor for backward pagination (object ID, e.g., `vs_abc123`)
///
/// ## Hadrian Extensions
///
/// - `owner_type`, `owner_id`: Optional for multi-tenancy scoping. When omitted, returns all
///   vector stores accessible to the authenticated user based on their memberships.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
pub struct ListVectorStoresQuery {
    /// **Hadrian Extension:** Owner type for multi-tenancy (organization, team, project, or user).
    /// When omitted along with `owner_id`, returns all accessible vector stores.
    pub owner_type: Option<String>,
    /// **Hadrian Extension:** Owner ID for multi-tenancy.
    /// When omitted along with `owner_type`, returns all accessible vector stores.
    pub owner_id: Option<Uuid>,
    /// Maximum number of vector stores to return (default: 20, max: 100)
    #[cfg_attr(feature = "utoipa", param(minimum = 1, maximum = 100))]
    pub limit: Option<i64>,
    /// Sort order by `created_at` timestamp (default: desc)
    #[serde(default)]
    pub order: Option<SortOrder>,
    /// Cursor for forward pagination. Returns results after this object ID.
    /// Use the `last_id` from a previous response to get the next page.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "vs_550e8400-e29b-41d4-a716-446655440000")
    )]
    pub after: Option<String>,
    /// Cursor for backward pagination. Returns results before this object ID.
    /// Use the `first_id` from a previous response to get the previous page.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "vs_550e8400-e29b-41d4-a716-446655440000")
    )]
    pub before: Option<String>,
}

/// Paginated list of vector stores response (OpenAI-compatible).
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStoreListResponse {
    /// Object type (always "list")
    pub object: String,
    /// List of vector stores
    pub data: Vec<VectorStore>,
    /// ID of the first object in the list (for backward pagination with `before`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    /// ID of the last object in the list (for forward pagination with `after`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    /// Whether there are more results available beyond this page
    pub has_more: bool,
}

/// Delete vector store response
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DeleteVectorStoreResponse {
    /// Vector store ID that was deleted
    pub id: String,
    /// Object type (always "vector_store.deleted")
    pub object: String,
    /// Whether the vector store was deleted
    pub deleted: bool,
}

/// Request to add a file to a vector store
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateVectorStoreFileRequest {
    /// The ID of the file to add (from the Files API)
    #[serde(with = "file_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "file-550e8400-e29b-41d4-a716-446655440000"))]
    pub file_id: Uuid,
    /// Chunking strategy for processing the file
    #[serde(default)]
    pub chunking_strategy: Option<ChunkingStrategy>,
}

/// Query parameters for listing vector store files (OpenAI-compatible).
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
pub struct ListVectorStoreFilesQuery {
    /// Maximum number of files to return (default: 20, max: 100)
    #[cfg_attr(feature = "utoipa", param(minimum = 1, maximum = 100))]
    pub limit: Option<i64>,
    /// Sort order by `created_at` timestamp (default: desc)
    #[serde(default)]
    pub order: Option<SortOrder>,
    /// Cursor for forward pagination. Returns results after this file ID.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "vsf_550e8400-e29b-41d4-a716-446655440000")
    )]
    pub after: Option<String>,
    /// Cursor for backward pagination. Returns results before this file ID.
    #[cfg_attr(
        feature = "utoipa",
        param(example = "vsf_550e8400-e29b-41d4-a716-446655440000")
    )]
    pub before: Option<String>,
    /// Filter by status (in_progress, completed, failed, cancelled)
    pub filter: Option<String>,
}

/// Paginated list of vector store files response (OpenAI-compatible).
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStoreFileListResponse {
    /// Object type (always "list")
    pub object: String,
    /// List of vector store files
    pub data: Vec<VectorStoreFile>,
    /// ID of the first file in the list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    /// ID of the last file in the list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    /// Whether there are more results available
    pub has_more: bool,
}

/// Delete vector store file response
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DeleteVectorStoreFileResponse {
    /// Vector store file ID that was deleted
    pub id: String,
    /// Object type (always "vector_store.file.deleted")
    pub object: String,
    /// Whether the file was deleted from the vector store
    pub deleted: bool,
}

/// Create a vector store
///
/// Creates a new vector store for storing file embeddings.
/// Optionally attaches files to the vector store at creation time via `file_ids`.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/vector_stores",
    tag = "vector-stores",
    operation_id = "vector_store_create",
    request_body = CreateVectorStore,
    responses(
        (status = 201, description = "Vector store created", body = VectorStore),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Owner not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_vector_stores_create(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Valid(Json(input)): Valid<Json<CreateVectorStore>>,
) -> Result<(StatusCode, Json<VectorStore>), ApiError> {
    // Check RAG feature access via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "vector_store",
                "create",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let services = get_services(&state)?;

    // Check caller has permission to create for this owner
    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        input.owner.owner_type(),
        input.owner.owner_id(),
    )?;

    // Verify the owner exists
    match &input.owner {
        VectorStoreOwner::Organization { organization_id } => {
            services
                .organizations
                .get_by_id(*organization_id)
                .await?
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::NOT_FOUND,
                        "not_found",
                        format!("Organization '{}' not found", organization_id),
                    )
                })?;
        }
        VectorStoreOwner::Team { team_id } => {
            services.teams.get_by_id(*team_id).await?.ok_or_else(|| {
                ApiError::new(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    format!("Team '{}' not found", team_id),
                )
            })?;
        }
        VectorStoreOwner::Project { project_id } => {
            services
                .projects
                .get_by_id(*project_id)
                .await?
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::NOT_FOUND,
                        "not_found",
                        format!("Project '{}' not found", project_id),
                    )
                })?;
        }
        VectorStoreOwner::User { user_id } => {
            services.users.get_by_id(*user_id).await?.ok_or_else(|| {
                ApiError::new(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    format!("User '{}' not found", user_id),
                )
            })?;
        }
    }

    // Extract file_ids and chunking_strategy before creating vector store
    let file_ids = input.file_ids.clone();
    let chunking_strategy = input.chunking_strategy.clone();

    // Create the vector store
    let vector_store = services.vector_stores.create(input).await?;

    // Attach files if file_ids were provided (OpenAI-compatible create-time file attachment)
    if !file_ids.is_empty() {
        for file_id in file_ids {
            // Verify the file exists
            if services.files.get(file_id).await?.is_none() {
                tracing::warn!(
                    file_id = %file_id,
                    vector_store_id = %vector_store.id,
                    "File not found when attaching to vector store at creation time"
                );
                continue;
            }

            let add_input = AddFileToVectorStore {
                vector_store_id: vector_store.id,
                file_id,
                chunking_strategy: chunking_strategy.clone(),
                attributes: None,
            };

            match services.vector_stores.add_file(add_input).await {
                Ok(_vector_store_file) => {
                    // Trigger file processing
                    #[cfg(any(
                        feature = "document-extraction-basic",
                        feature = "document-extraction-full"
                    ))]
                    if let Some(processor) = &state.document_processor {
                        let processor = processor.clone();
                        if let Err(e) = processor
                            .schedule_processing(_vector_store_file.internal_id)
                            .await
                        {
                            tracing::error!(
                                error = %e,
                                file_id = %_vector_store_file.internal_id,
                                "Failed to schedule file processing"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        file_id = %file_id,
                        vector_store_id = %vector_store.id,
                        "Failed to attach file to vector store at creation time"
                    );
                }
            }
        }

        // Refresh vector store to get updated file_counts
        if let Some(updated) = services.vector_stores.get_by_id(vector_store.id).await? {
            return Ok((StatusCode::CREATED, Json(updated)));
        }
    }

    Ok((StatusCode::CREATED, Json(vector_store)))
}

/// List vector stores
///
/// Returns a list of vector stores owned by the specified owner.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores",
    tag = "vector-stores",
    operation_id = "vector_store_list",
    params(ListVectorStoresQuery),
    responses(
        (status = 200, description = "List of vector stores", body = VectorStoreListResponse),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_vector_stores_list(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Query(query): Query<ListVectorStoresQuery>,
) -> Result<Json<VectorStoreListResponse>, ApiError> {
    use crate::db::repos::{Cursor, CursorDirection};

    // Check RAG feature access via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "vector_store",
                "list",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let services = get_services(&state)?;

    // OpenAI defaults: limit=20, order=desc
    let limit = query.limit.unwrap_or(20).min(100);

    // Parse cursor from `after` or `before` parameter
    // OpenAI uses object IDs as cursors (e.g., "vs_abc123")
    let (cursor, direction) = if let Some(ref after_id) = query.after {
        // `after` means get items after this ID (forward pagination)
        let vector_store_id: VectorStoreId = after_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'after' cursor: {}", after_id),
            )
        })?;

        // Look up the record to get its timestamp for keyset pagination
        let cursor_record = services
            .vector_stores
            .get_by_id(vector_store_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("Vector store '{}' not found for cursor", after_id),
                )
            })?;

        (
            Some(Cursor::new(cursor_record.updated_at, cursor_record.id)),
            CursorDirection::Forward,
        )
    } else if let Some(ref before_id) = query.before {
        // `before` means get items before this ID (backward pagination)
        let vector_store_id: VectorStoreId = before_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'before' cursor: {}", before_id),
            )
        })?;

        // Look up the record to get its timestamp for keyset pagination
        let cursor_record = services
            .vector_stores
            .get_by_id(vector_store_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("Vector store '{}' not found for cursor", before_id),
                )
            })?;

        (
            Some(Cursor::new(cursor_record.updated_at, cursor_record.id)),
            CursorDirection::Backward,
        )
    } else {
        (None, CursorDirection::Forward)
    };

    let params = ListParams {
        limit: Some(limit),
        cursor,
        direction,
        sort_order: query.order.unwrap_or_default().into(),
        ..Default::default()
    };

    // Determine whether to list by specific owner or by accessible collections
    let result = match (query.owner_type.as_ref(), query.owner_id) {
        // Both owner_type and owner_id provided - use single-owner listing
        (Some(owner_type_str), Some(owner_id)) => {
            let owner_type: VectorStoreOwnerType = owner_type_str.parse().map_err(|_| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_owner_type",
                    "Invalid owner_type. Must be one of: organization, team, project, user",
                )
            })?;

            // Check caller has permission to list for this owner
            check_resource_access_optional(auth.as_ref().map(|e| &e.0), owner_type, owner_id)?;

            services
                .vector_stores
                .list(owner_type, owner_id, params)
                .await?
        }

        // Neither provided - list all accessible collections based on identity
        (None, None) => {
            match auth.as_ref() {
                None => {
                    // No auth - list all vector stores (open access mode)
                    services.vector_stores.list_all(params).await?
                }
                Some(auth_ext) => {
                    let (user_id, org_ids, team_ids, project_ids) =
                        extract_identity_memberships(Some(&auth_ext.0))?;

                    services
                        .vector_stores
                        .list_accessible(user_id, &org_ids, &team_ids, &project_ids, params)
                        .await?
                }
            }
        }

        // Only one provided - invalid
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_parameters",
                "Both owner_type and owner_id must be provided together, or both omitted to list all accessible vector stores",
            ));
        }
    };

    // Build OpenAI-compatible response with first_id and last_id
    let first_id = result
        .items
        .first()
        .map(|c| VectorStoreId::new(c.id).to_string());
    let last_id = result
        .items
        .last()
        .map(|c| VectorStoreId::new(c.id).to_string());

    Ok(Json(VectorStoreListResponse {
        object: "list".to_string(),
        data: result.items,
        first_id,
        last_id,
        has_more: result.has_more,
    }))
}

/// Get a vector store
///
/// Retrieves a vector store by ID.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}",
    tag = "vector-stores",
    operation_id = "vector_store_get",
    params(("vector_store_id" = String, Path, description = "Vector store ID (e.g., vs_550e8400-e29b-41d4-a716-446655440000)")),
    responses(
        (status = 200, description = "Vector store details", body = VectorStore),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_vector_stores_get(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(vector_store_id): Path<VectorStoreId>,
) -> Result<Json<VectorStore>, ApiError> {
    // Check RAG feature access via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "vector_store",
                "read",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let services = get_services(&state)?;

    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id.into_inner())
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    // Check access permission
    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    Ok(Json(vector_store))
}

/// Modify a vector store
///
/// Modifies a vector store's metadata.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/vector_stores/{vector_store_id}",
    tag = "vector-stores",
    operation_id = "vector_store_modify",
    params(("vector_store_id" = Uuid, Path, description = "Vector store ID")),
    request_body = UpdateVectorStore,
    responses(
        (status = 200, description = "Vector store updated", body = VectorStore),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_modify(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(vector_store_id): Path<VectorStoreId>,
    Valid(Json(input)): Valid<Json<UpdateVectorStore>>,
) -> Result<Json<VectorStore>, ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let existing = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        existing.owner_type,
        existing.owner_id,
    )?;

    let vector_store = services
        .vector_stores
        .update(vector_store_id, input)
        .await?;
    Ok(Json(vector_store))
}

/// Delete a vector store
///
/// Deletes a vector store and all its files (soft delete).
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/api/v1/vector_stores/{vector_store_id}",
    tag = "vector-stores",
    operation_id = "vector_store_delete",
    params(("vector_store_id" = Uuid, Path, description = "Vector store ID")),
    responses(
        (status = 200, description = "Vector store deleted", body = DeleteVectorStoreResponse),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_vector_stores_delete(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(vector_store_id): Path<VectorStoreId>,
) -> Result<Json<DeleteVectorStoreResponse>, ApiError> {
    // Check RAG feature access via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "vector_store",
                "delete",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let vector_store_id_prefixed = vector_store_id.to_string();
    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    services.vector_stores.delete(vector_store_id).await?;

    Ok(Json(DeleteVectorStoreResponse {
        id: vector_store_id_prefixed,
        object: "vector_store.deleted".to_string(),
        deleted: true,
    }))
}

// ============================================================================
// Vector Store File Route Handlers
// ============================================================================

/// Create a vector store file
///
/// Adds a file to a vector store. The file must already exist in the Files API.
/// Processing will start automatically after the file is added.
///
/// ## Content Deduplication
///
/// Files are deduplicated by content hash (SHA-256). If a file with identical content
/// already exists in the vector store, the existing file is returned with status 200
/// instead of creating a duplicate. This is idempotent behavior—uploading the same
/// content multiple times has no additional effect.
///
/// ## Embedding Model Validation
///
/// The gateway validates that its configured embedding model matches the vector store's
/// embedding model before adding files. This prevents incompatible embeddings from being
/// stored together. If there's a mismatch, a 409 Conflict error is returned with details
/// about the expected vs. configured models.
///
/// - **201 Created**: New file added, processing started
/// - **200 OK**: Duplicate content detected, existing file returned (no re-processing)
/// - **409 Conflict**: Embedding model mismatch between gateway configuration and vector store
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/vector_stores/{vector_store_id}/files",
    tag = "vector-stores",
    operation_id = "vector_store_file_create",
    params(("vector_store_id" = Uuid, Path, description = "Vector store ID")),
    request_body = CreateVectorStoreFileRequest,
    responses(
        (status = 200, description = "Duplicate content detected, existing file returned", body = VectorStoreFile),
        (status = 201, description = "File added to vector store", body = VectorStoreFile),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Vector store or file not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Embedding model mismatch", body = crate::openapi::ErrorResponse),
        (status = 503, description = "File search service not configured", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_create_file(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(vector_store_id): Path<VectorStoreId>,
    Json(input): Json<CreateVectorStoreFileRequest>,
) -> Result<(StatusCode, Json<VectorStoreFile>), ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Verify the file exists and get its content hash for deduplication
    let file = services.files.get(input.file_id).await?.ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("File '{}' not found", input.file_id),
        )
    })?;

    // Verify the user has access to the file being added
    check_resource_access_optional(auth.as_ref().map(|e| &e.0), file.owner_type, file.owner_id)?;

    // Check if this file is already in the vector store (idempotency)
    if let Some(existing_file) = services
        .vector_stores
        .find_by_file_id(vector_store_id, input.file_id)
        .await?
    {
        // If the file previously failed, allow re-processing by resetting status
        if existing_file.status == VectorStoreFileStatus::Failed {
            tracing::info!(
                vector_store_id = %vector_store_id,
                file_id = %input.file_id,
                vector_store_file_internal_id = %existing_file.internal_id,
                previous_error = ?existing_file.last_error,
                "Re-processing previously failed file"
            );

            // Reset status to InProgress and clear error
            services
                .vector_stores
                .update_vector_store_file_status(
                    existing_file.internal_id,
                    VectorStoreFileStatus::InProgress,
                    None,
                )
                .await?;

            // Re-trigger processing (shadow-copy pattern ensures old partial chunks
            // are cleaned up after successful re-processing)
            #[cfg(any(
                feature = "document-extraction-basic",
                feature = "document-extraction-full"
            ))]
            if let Some(processor) = &state.document_processor {
                let processor = processor.clone();
                let internal_id = existing_file.internal_id;
                if let Err(e) = processor.schedule_processing(internal_id).await {
                    tracing::error!(
                        error = %e,
                        internal_id = %internal_id,
                        "Failed to schedule file re-processing"
                    );
                }
            }

            // Return updated file with 200 OK
            let updated_file = services
                .vector_stores
                .get_vector_store_file(existing_file.internal_id)
                .await?
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "file_not_found",
                        "File disappeared after status update",
                    )
                })?;
            return Ok((StatusCode::OK, Json(updated_file)));
        }

        // Check for stale InProgress files (stuck due to worker crash, etc.)
        if existing_file.status == VectorStoreFileStatus::InProgress {
            let stale_timeout_secs = state
                .config
                .features
                .file_processing
                .stale_processing_timeout_secs;

            // Only check for staleness if timeout is configured (> 0)
            if stale_timeout_secs > 0 {
                let age_secs = (Utc::now() - existing_file.updated_at).num_seconds();
                if age_secs > stale_timeout_secs as i64 {
                    tracing::info!(
                        vector_store_id = %vector_store_id,
                        file_id = %input.file_id,
                        vector_store_file_internal_id = %existing_file.internal_id,
                        age_secs = age_secs,
                        stale_timeout_secs = stale_timeout_secs,
                        "Re-processing stale in-progress file"
                    );

                    // Reset status to InProgress (to update timestamp) and clear any error
                    services
                        .vector_stores
                        .update_vector_store_file_status(
                            existing_file.internal_id,
                            VectorStoreFileStatus::InProgress,
                            None,
                        )
                        .await?;

                    // Re-trigger processing
                    #[cfg(any(
                        feature = "document-extraction-basic",
                        feature = "document-extraction-full"
                    ))]
                    if let Some(processor) = &state.document_processor {
                        let processor = processor.clone();
                        let internal_id = existing_file.internal_id;
                        if let Err(e) = processor.schedule_processing(internal_id).await {
                            tracing::error!(
                                error = %e,
                                internal_id = %internal_id,
                                "Failed to schedule stale file re-processing"
                            );
                        }
                    }

                    // Return updated file with 200 OK
                    let updated_file = services
                        .vector_stores
                        .get_vector_store_file(existing_file.internal_id)
                        .await?
                        .ok_or_else(|| {
                            ApiError::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "file_not_found",
                                "File disappeared after status update",
                            )
                        })?;
                    return Ok((StatusCode::OK, Json(updated_file)));
                }
            }
        }

        tracing::info!(
            vector_store_id = %vector_store_id,
            file_id = %input.file_id,
            vector_store_file_internal_id = %existing_file.internal_id,
            status = ?existing_file.status,
            "File already in vector_store, returning existing entry"
        );
        // Return existing entry with 200 OK (idempotent behavior)
        return Ok((StatusCode::OK, Json(existing_file)));
    }

    // Check for same-owner content deduplication (prevents accidental duplicates)
    if let Some(content_hash) = &file.content_hash
        && let Some(existing_file) = services
            .vector_stores
            .find_by_content_hash_and_owner(
                vector_store_id,
                content_hash,
                file.owner_type,
                file.owner_id,
            )
            .await?
    {
        tracing::info!(
            vector_store_id = %vector_store_id,
            file_id = %input.file_id,
            existing_file_id = %existing_file.file_id,
            vector_store_file_internal_id = %existing_file.internal_id,
            content_hash = %content_hash,
            "Same-owner duplicate content detected, returning existing file"
        );
        // Return existing file with 200 OK (deduplication)
        return Ok((StatusCode::OK, Json(existing_file)));
    }

    // Validate embedding model compatibility before adding new file.
    // This ensures the gateway's configured embedding model matches the vector store's model,
    // preventing incompatible vectors from being stored.
    validate_embedding_model_compatibility(&state, &vector_store)?;

    let add_input = AddFileToVectorStore {
        vector_store_id,
        file_id: input.file_id,
        chunking_strategy: input.chunking_strategy,
        attributes: None,
    };

    let vector_store_file = services.vector_stores.add_file(add_input).await?;

    // Trigger file processing (chunking + embedding)
    #[cfg(any(
        feature = "document-extraction-basic",
        feature = "document-extraction-full"
    ))]
    if let Some(processor) = &state.document_processor {
        let processor = processor.clone();
        let internal_id = vector_store_file.internal_id;
        if let Err(e) = processor.schedule_processing(internal_id).await {
            tracing::error!(
                error = %e,
                internal_id = %internal_id,
                "Failed to schedule file processing"
            );
        }
    } else {
        tracing::warn!(
            internal_id = %vector_store_file.internal_id,
            "Document processor not configured, file will remain in 'in_progress' status"
        );
    }
    #[cfg(not(any(
        feature = "document-extraction-basic",
        feature = "document-extraction-full"
    )))]
    tracing::warn!(
        internal_id = %vector_store_file.internal_id,
        "Document processor not configured (feature disabled), file will remain in 'in_progress' status"
    );

    Ok((StatusCode::CREATED, Json(vector_store_file)))
}

/// List vector store files
///
/// Returns a list of files in a vector store.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}/files",
    tag = "vector-stores",
    operation_id = "vector_store_file_list",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ListVectorStoreFilesQuery,
    ),
    responses(
        (status = 200, description = "List of files", body = VectorStoreFileListResponse),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_list_files(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(vector_store_id): Path<VectorStoreId>,
    Query(query): Query<ListVectorStoreFilesQuery>,
) -> Result<Json<VectorStoreFileListResponse>, ApiError> {
    use crate::db::repos::{Cursor, CursorDirection};

    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // OpenAI defaults: limit=20
    let limit = query.limit.unwrap_or(20).min(100);

    // Parse cursor from `after` or `before` parameter
    let (cursor, direction) = if let Some(ref after_id) = query.after {
        let file_id: VectorStoreFileId = after_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'after' cursor: {}", after_id),
            )
        })?;

        // Look up the record to get its timestamp for keyset pagination
        let cursor_record = services
            .vector_stores
            .get_vector_store_file(file_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("Vector store file '{}' not found for cursor", after_id),
                )
            })?;

        (
            Some(Cursor::new(
                cursor_record.updated_at,
                cursor_record.internal_id,
            )),
            CursorDirection::Forward,
        )
    } else if let Some(ref before_id) = query.before {
        let file_id: VectorStoreFileId = before_id.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                format!("Invalid 'before' cursor: {}", before_id),
            )
        })?;

        let cursor_record = services
            .vector_stores
            .get_vector_store_file(file_id.into_inner())
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_cursor",
                    format!("Vector store file '{}' not found for cursor", before_id),
                )
            })?;

        (
            Some(Cursor::new(
                cursor_record.updated_at,
                cursor_record.internal_id,
            )),
            CursorDirection::Backward,
        )
    } else {
        (None, CursorDirection::Forward)
    };

    let params = ListParams {
        limit: Some(limit),
        cursor,
        direction,
        sort_order: query.order.unwrap_or_default().into(),
        ..Default::default()
    };

    let result = services
        .vector_stores
        .list_vector_store_files(vector_store_id, params)
        .await?;

    // Filter by status if requested
    let items = if let Some(filter) = query.filter {
        let status: VectorStoreFileStatus = filter.parse().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_filter",
                format!("Invalid filter status: {}", filter),
            )
        })?;
        result
            .items
            .into_iter()
            .filter(|f| f.status == status)
            .collect()
    } else {
        result.items
    };

    // Build OpenAI-compatible response
    // Use file_id as the external ID (matches OpenAI behavior)
    let first_id = items.first().map(|f| FileId::new(f.file_id).to_string());
    let last_id = items.last().map(|f| FileId::new(f.file_id).to_string());

    Ok(Json(VectorStoreFileListResponse {
        object: "list".to_string(),
        data: items,
        first_id,
        last_id,
        has_more: result.has_more,
    }))
}

/// Get a vector store file
///
/// Retrieves a file from a vector store.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}/files/{file_id}",
    tag = "vector-stores",
    operation_id = "vector_store_file_get",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("file_id" = Uuid, Path, description = "Vector store file ID"),
    ),
    responses(
        (status = 200, description = "Vector store file details", body = VectorStoreFile),
        (status = 404, description = "Vector store or file not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_get_file(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path((vector_store_id, file_id)): Path<(VectorStoreId, FileId)>,
) -> Result<Json<VectorStoreFile>, ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Look up by file_id (Files API ID) + vector_store_id, not by vector_store_file.id
    let vector_store_file = services
        .vector_stores
        .find_by_file_id(vector_store_id, file_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!(
                    "File '{}' not found in vector store '{}'",
                    file_id, vector_store_id
                ),
            )
        })?;

    Ok(Json(vector_store_file))
}

/// Delete a vector store file
///
/// Removes a file from a vector store. This does not delete the underlying file
/// from the Files API.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/api/v1/vector_stores/{vector_store_id}/files/{file_id}",
    tag = "vector-stores",
    operation_id = "vector_store_file_delete",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("file_id" = Uuid, Path, description = "Vector store file ID"),
    ),
    responses(
        (status = 200, description = "File removed from vector store", body = DeleteVectorStoreFileResponse),
        (status = 404, description = "Vector store or file not found", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_delete_file(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path((vector_store_id, file_id)): Path<(VectorStoreId, FileId)>,
) -> Result<Json<DeleteVectorStoreFileResponse>, ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    // Keep prefixed form for response
    let file_id_prefixed = file_id.to_string();
    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Look up by file_id (Files API ID) + vector_store_id, not by vector_store_file.id
    let vector_store_file = services
        .vector_stores
        .find_by_file_id(vector_store_id, file_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!(
                    "File '{}' not found in vector store '{}'",
                    file_id, vector_store_id
                ),
            )
        })?;

    // Remove the file from the vector store using vector_store_file.internal_id
    services
        .vector_stores
        .remove_file(vector_store_file.internal_id)
        .await?;

    Ok(Json(DeleteVectorStoreFileResponse {
        id: file_id_prefixed,
        object: "vector_store.file.deleted".to_string(),
        deleted: true,
    }))
}

// ============================================================================
// Vector Store File Batch Route Handlers (Stub implementations)
// ============================================================================

/// File batch response
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileBatch {
    /// Batch ID
    pub id: String,
    /// Object type (always "vector_store.file_batch")
    pub object: String,
    /// Vector store ID
    pub vector_store_id: String,
    /// Batch status
    pub status: String,
    /// File counts by status
    pub file_counts: FileBatchCounts,
    /// Unix timestamp when batch was created
    pub created_at: i64,
}

/// File batch counts
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileBatchCounts {
    pub in_progress: i32,
    pub completed: i32,
    pub failed: i32,
    pub cancelled: i32,
    pub total: i32,
}

/// Create file batch request
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateFileBatchRequest {
    /// File IDs to add to the batch
    pub file_ids: Vec<Uuid>,
    /// Chunking strategy for all files in the batch
    #[serde(default)]
    pub chunking_strategy: Option<ChunkingStrategy>,
}

/// Create a file batch
///
/// Creates a batch of files to be added to a vector store.
/// Note: File batches are not yet fully implemented. This endpoint creates
/// files individually and returns a batch representation.
///
/// ## Content Deduplication
///
/// Files are deduplicated by content hash (SHA-256). If a file with identical content
/// already exists in the vector store, it is counted as "completed" in the batch
/// response but no re-processing occurs. This prevents duplicate chunks and wasted
/// compute while still reporting success for the file.
///
/// The `file_counts.completed` field in the response includes both newly processed
/// files and deduplicated files.
///
/// ## Embedding Model Validation
///
/// The gateway validates that its configured embedding model matches the vector store's
/// embedding model before processing any files in the batch. This prevents incompatible
/// embeddings from being stored together. If there's a mismatch, a 409 Conflict error
/// is returned with details about the expected vs. configured models.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/vector_stores/{vector_store_id}/file_batches",
    tag = "vector-stores",
    operation_id = "vector_store_file_batch_create",
    params(("vector_store_id" = Uuid, Path, description = "Vector store ID")),
    request_body = CreateFileBatchRequest,
    responses(
        (status = 201, description = "File batch created", body = FileBatch),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Embedding model mismatch", body = crate::openapi::ErrorResponse),
        (status = 503, description = "File search service not configured", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_create_file_batch(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path(vector_store_id): Path<VectorStoreId>,
    Json(input): Json<CreateFileBatchRequest>,
) -> Result<(StatusCode, Json<FileBatch>), ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Validate embedding model compatibility before processing any files.
    // This ensures the gateway's configured embedding model matches the vector store's model,
    // preventing incompatible vectors from being stored.
    validate_embedding_model_compatibility(&state, &vector_store)?;

    // Add each file to the vector store
    let mut completed = 0;
    let mut failed = 0;
    let mut duplicates = 0;

    for file_id in &input.file_ids {
        // Verify the file exists and get its content hash
        let file = match services.files.get(*file_id).await? {
            Some(f) => f,
            None => {
                failed += 1;
                continue;
            }
        };

        // Verify the user has access to the file being added
        if check_resource_access_optional(
            auth.as_ref().map(|e| &e.0),
            file.owner_type,
            file.owner_id,
        )
        .is_err()
        {
            tracing::warn!(
                file_id = %file_id,
                "Access denied to file in batch, skipping"
            );
            failed += 1;
            continue;
        }

        // Check if this file is already in the vector store (idempotency)
        if let Some(existing_file) = services
            .vector_stores
            .find_by_file_id(vector_store_id, *file_id)
            .await?
        {
            tracing::info!(
                vector_store_id = %vector_store_id,
                file_id = %file_id,
                vector_store_file_internal_id = %existing_file.internal_id,
                "File already in vector store in batch, skipping"
            );
            // Count as completed since the file is already in the vector store
            completed += 1;
            duplicates += 1;
            continue;
        }

        // Check for same-owner content deduplication (prevents accidental duplicates)
        if let Some(content_hash) = &file.content_hash
            && let Some(existing_file) = services
                .vector_stores
                .find_by_content_hash_and_owner(
                    vector_store_id,
                    content_hash,
                    file.owner_type,
                    file.owner_id,
                )
                .await?
        {
            tracing::info!(
                vector_store_id = %vector_store_id,
                file_id = %file_id,
                existing_file_id = %existing_file.file_id,
                vector_store_file_internal_id = %existing_file.internal_id,
                content_hash = %content_hash,
                "Same-owner duplicate content in batch, skipping"
            );
            // Count as completed since equivalent content is already in the vector store
            completed += 1;
            duplicates += 1;
            continue;
        }

        let add_input = AddFileToVectorStore {
            vector_store_id,
            file_id: *file_id,
            chunking_strategy: input.chunking_strategy.clone(),
            attributes: None,
        };

        match services.vector_stores.add_file(add_input).await {
            Ok(_vector_store_file) => {
                completed += 1;
                // Trigger file processing
                #[cfg(any(
                    feature = "document-extraction-basic",
                    feature = "document-extraction-full"
                ))]
                if let Some(processor) = &state.document_processor {
                    let processor = processor.clone();
                    if let Err(e) = processor
                        .schedule_processing(_vector_store_file.internal_id)
                        .await
                    {
                        tracing::error!(
                            error = %e,
                            internal_id = %_vector_store_file.internal_id,
                            "Failed to schedule file processing in batch"
                        );
                    }
                }
            }
            Err(_) => failed += 1,
        }
    }

    if duplicates > 0 {
        tracing::info!(
            vector_store_id = %vector_store_id,
            duplicates = duplicates,
            "Batch contained duplicate files that were skipped"
        );
    }

    let total = input.file_ids.len() as i32;
    let batch_id = Uuid::new_v4();

    Ok((
        StatusCode::CREATED,
        Json(FileBatch {
            id: format!("vsfb_{}", batch_id),
            object: "vector_store.file_batch".to_string(),
            vector_store_id: vector_store_id.to_string(),
            status: if failed == 0 { "completed" } else { "failed" }.to_string(),
            file_counts: FileBatchCounts {
                in_progress: 0,
                completed,
                failed,
                cancelled: 0,
                total,
            },
            created_at: vector_store.created_at.timestamp(),
        }),
    ))
}

/// Get a file batch
///
/// Retrieves a file batch. Note: File batches are executed synchronously,
/// so this endpoint returns a "completed" or "failed" status.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}/file_batches/{batch_id}",
    tag = "vector-stores",
    operation_id = "vector_store_file_batch_get",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("batch_id" = String, Path, description = "File batch ID"),
    ),
    responses(
        (status = 404, description = "File batches are not persisted", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(_state))]
pub async fn api_v1_vector_stores_get_file_batch(
    State(_state): State<AppState>,
    Path((_vector_store_id, _batch_id)): Path<(VectorStoreId, String)>,
) -> Result<Json<FileBatch>, ApiError> {
    // File batches are executed synchronously and not persisted
    Err(ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        "File batches are not persisted. Use the create endpoint which returns the final status.",
    ))
}

/// Cancel a file batch
///
/// Cancels a file batch. Note: File batches are executed synchronously,
/// so cancellation is not supported.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/api/v1/vector_stores/{vector_store_id}/file_batches/{batch_id}",
    tag = "vector-stores",
    operation_id = "vector_store_file_batch_cancel",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("batch_id" = String, Path, description = "File batch ID"),
    ),
    responses(
        (status = 400, description = "File batches cannot be cancelled", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(_state))]
pub async fn api_v1_vector_stores_cancel_file_batch(
    State(_state): State<AppState>,
    Path((_vector_store_id, _batch_id)): Path<(VectorStoreId, String)>,
) -> Result<Json<FileBatch>, ApiError> {
    Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        "not_supported",
        "File batches are executed synchronously and cannot be cancelled.",
    ))
}

/// List files in a batch
///
/// Lists files in a file batch. Note: File batches are not persisted.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}/file_batches/{batch_id}/files",
    tag = "vector-stores",
    operation_id = "vector_store_file_batch_list_files",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("batch_id" = String, Path, description = "File batch ID"),
    ),
    responses(
        (status = 404, description = "File batches are not persisted", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(_state))]
pub async fn api_v1_vector_stores_list_batch_files(
    State(_state): State<AppState>,
    Path((_vector_store_id, _batch_id)): Path<(VectorStoreId, String)>,
) -> Result<Json<VectorStoreFileListResponse>, ApiError> {
    Err(ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        "File batches are not persisted. List the vector store files directly using GET /v1/vector_stores/{id}/files",
    ))
}

// ============================================================================
// Hadrian Extensions - Chunk and Search Endpoints
// ============================================================================

/// A stored chunk as returned by the chunks endpoint.
///
/// ## OpenAI Compatibility Notes
///
/// - `id` is serialized with `chunk_` prefix (e.g., `chunk_550e8400-e29b-41d4-a716-446655440000`)
/// - `vector_store_id` is serialized with `vs_` prefix
/// - `file_id` is serialized with `file-` prefix
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ChunkResponse {
    /// Unique identifier for this chunk (serialized with `chunk_` prefix)
    #[serde(with = "chunk_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "chunk_550e8400-e29b-41d4-a716-446655440000"))]
    pub id: Uuid,
    /// Object type (always "vector_store.file.chunk")
    pub object: String,
    /// The vector store this chunk belongs to (serialized with `vs_` prefix)
    #[serde(with = "vector_store_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "vs_550e8400-e29b-41d4-a716-446655440000"))]
    pub vector_store_id: Uuid,
    /// The file this chunk was extracted from (serialized with `file-` prefix)
    #[serde(with = "file_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "file-550e8400-e29b-41d4-a716-446655440000"))]
    pub file_id: Uuid,
    /// Sequential index within the file (0-based)
    pub chunk_index: i32,
    /// The actual text content of the chunk
    pub content: String,
    /// Number of tokens in this chunk
    pub token_count: i32,
    /// Character offset where this chunk starts in the original file
    pub char_start: i32,
    /// Character offset where this chunk ends in the original file
    pub char_end: i32,
    /// Optional additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Unix timestamp when the chunk was created
    pub created_at: i64,
}

/// Paginated list of chunks response
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ChunkListResponse {
    /// Object type (always "list")
    pub object: String,
    /// List of chunks
    pub data: Vec<ChunkResponse>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

/// Search request for a vector store.
///
/// ## Ranking Options
///
/// Use `ranking_options` to control result scoring and filtering:
/// - `ranker`: Algorithm for ranking results
///   - `auto` (default): Automatically selects best ranker; supports hybrid search
///   - `vector`: Vector-only cosine similarity search
///   - `hybrid`: Combines vector and keyword search with RRF fusion
///   - `llm`: LLM-based re-ranking for highest quality results
///   - `none`: No re-ranking, raw similarity order
/// - `score_threshold`: Minimum similarity score (0.0-1.0, default: 0.0)
/// - `hybrid_search`: Enable hybrid search combining vector and keyword search
///   - `embedding_weight`: Weight for semantic (vector) search (default: 1.0)
///   - `text_weight`: Weight for keyword (full-text) search (default: 1.0)
///
/// ## Hybrid Search Example
///
/// ```json
/// {
///   "query": "API authentication",
///   "ranking_options": {
///     "ranker": "hybrid",
///     "score_threshold": 0.5,
///     "hybrid_search": {
///       "embedding_weight": 0.7,
///       "text_weight": 0.3
///     }
///   }
/// }
/// ```
///
/// ## LLM Re-ranking Example
///
/// ```json
/// {
///   "query": "How to authenticate API requests",
///   "ranking_options": {
///     "ranker": "llm",
///     "score_threshold": 0.5
///   }
/// }
/// ```
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStoreSearchRequest {
    /// The search query text.
    pub query: String,

    /// Maximum number of results to return (default: 10, max: 50).
    #[serde(default)]
    pub max_num_results: Option<usize>,

    /// Ranking options for controlling result scoring and filtering.
    ///
    /// If not specified, uses default ranking with score_threshold of 0.0 (return all results).
    #[serde(default)]
    pub ranking_options: Option<FileSearchRankingOptions>,

    /// A filter to apply based on file attributes. Supports comparison operators
    /// (eq, ne, gt, gte, lt, lte) and logical operators (and, or) for combining filters.
    ///
    /// Example: `{"type": "eq", "key": "category", "value": "documentation"}`
    #[serde(default)]
    pub filters: Option<AttributeFilter>,
}

/// A single search result.
///
/// ## Hadrian Extensions
///
/// The following fields are **Hadrian extensions** not present in the standard OpenAI API:
/// - `chunk_id`: Unique identifier for the matched chunk
/// - `object`: Object type identifier
/// - `vector_store_id`: Vector store ID the chunk belongs to
/// - `chunk_index`: Position of chunk within the source file
/// - `metadata`: Arbitrary metadata (OpenAI uses `attributes`)
///
/// ## OpenAI Compatibility Notes
///
/// - `chunk_id` is serialized with `chunk_` prefix
/// - `vector_store_id` is serialized with `vs_` prefix
/// - `file_id` is serialized with `file-` prefix
/// - `content` is a string; OpenAI uses `content: [{type: "text", text: "..."}]` array format
/// - `filename` is optional; OpenAI requires it
/// - `metadata` maps to OpenAI's `attributes` field
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SearchResultItem {
    /// **Hadrian Extension:** The chunk ID in the vector store (serialized with `chunk_` prefix)
    #[serde(with = "chunk_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "chunk_550e8400-e29b-41d4-a716-446655440000"))]
    pub chunk_id: Uuid,
    /// **Hadrian Extension:** Object type (always "vector_store.search_result")
    pub object: String,
    /// **Hadrian Extension:** The vector store this chunk belongs to (serialized with `vs_` prefix)
    #[serde(with = "vector_store_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "vs_550e8400-e29b-41d4-a716-446655440000"))]
    pub vector_store_id: Uuid,
    /// The file this chunk was extracted from (serialized with `file-` prefix)
    #[serde(with = "file_id_serde")]
    #[cfg_attr(feature = "utoipa", schema(value_type = String, example = "file-550e8400-e29b-41d4-a716-446655440000"))]
    pub file_id: Uuid,
    /// **Hadrian Extension:** Index of this chunk within the file
    pub chunk_index: i32,
    /// The actual text content of the chunk. Note: OpenAI uses array format `[{type, text}]`.
    pub content: String,
    /// Similarity score (0.0 to 1.0, higher is more similar)
    pub score: f64,
    /// Filename of the source file. Note: Required in OpenAI, optional in Hadrian.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// **Hadrian Extension:** Optional additional metadata. Note: OpenAI uses `attributes`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Search response from a vector store.
///
/// ## OpenAI Compatibility Notes
///
/// - `object` is "vector_store.search_results"; OpenAI uses "vector_store.search_results.page"
/// - `query` is a string; OpenAI uses `search_query` as an array of strings
/// - `has_more` and `next_page` pagination fields are not yet supported
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct VectorStoreSearchResponse {
    /// Object type. Note: OpenAI uses "vector_store.search_results.page".
    pub object: String,
    /// **Hadrian Extension:** The search query that was used. Note: OpenAI uses `search_query` as an array.
    pub query: String,
    /// Search results ordered by relevance (highest first)
    pub data: Vec<SearchResultItem>,
}

/// List chunks for a file
///
/// **Hadrian Extension** - This endpoint is not part of the OpenAI API.
///
/// Returns all chunks that have been extracted and embedded from a file.
/// This is useful for debugging chunking behavior and verifying embeddings.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/api/v1/vector_stores/{vector_store_id}/files/{file_id}/chunks",
    tag = "vector-stores",
    operation_id = "vector_store_file_chunks_list",
    summary = "List chunks for a file [Hadrian Extension]",
    description = "**Hadrian Extension** - This endpoint is not part of the standard OpenAI API.\n\nReturns all chunks that have been extracted and embedded from a file. Useful for debugging chunking behavior and verifying embeddings.",
    params(
        ("vector_store_id" = Uuid, Path, description = "Vector store ID"),
        ("file_id" = Uuid, Path, description = "Vector store file ID"),
    ),
    responses(
        (status = 200, description = "List of chunks for the file", body = ChunkListResponse),
        (status = 404, description = "Vector store or file not found", body = crate::openapi::ErrorResponse),
        (status = 503, description = "File search not configured", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth))]
pub async fn api_v1_vector_stores_list_file_chunks(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    Path((vector_store_id, file_id)): Path<(VectorStoreId, FileId)>,
) -> Result<Json<ChunkListResponse>, ApiError> {
    let vector_store_id = vector_store_id.into_inner();
    let file_id = file_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Look up by file_id (Files API ID) + vector_store_id, not by vector_store_file.id
    let vector_store_file = services
        .vector_stores
        .find_by_file_id(vector_store_id, file_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!(
                    "File '{}' not found in vector store '{}'",
                    file_id, vector_store_id
                ),
            )
        })?;

    // Get the file search service (which has access to the vector store)
    let file_search_service = state.file_search_service.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "not_configured",
            "File search is not configured. Enable [features.file_search] in configuration.",
        )
    })?;

    // Get chunks from the vector store
    // Note: chunks are stored by the underlying file_id, not the vector_store_file ID
    let chunks = file_search_service
        .get_chunks_by_file(vector_store_file.file_id)
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                format!("Failed to retrieve chunks: {}", e),
            )
        })?;

    let data: Vec<ChunkResponse> = chunks
        .into_iter()
        .map(|c| ChunkResponse {
            id: c.id,
            object: "vector_store.file.chunk".to_string(),
            vector_store_id: c.vector_store_id,
            file_id: c.file_id,
            chunk_index: c.chunk_index,
            content: c.content,
            token_count: c.token_count,
            char_start: c.char_start,
            char_end: c.char_end,
            metadata: c.metadata,
            created_at: c.created_at,
        })
        .collect();

    let total = data.len() as i64;
    let pagination = PaginationMeta::with_cursors(total, false, None, None);

    Ok(Json(ChunkListResponse {
        object: "list".to_string(),
        data,
        pagination,
    }))
}

/// Search a vector store
///
/// Performs a semantic search against a vector store (OpenAI-compatible endpoint).
/// Note: Request/response schema has Hadrian-specific extensions.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/api/v1/vector_stores/{vector_store_id}/search",
    tag = "vector-stores",
    operation_id = "vector_store_search",
    summary = "Search vector store",
    description = "Performs a semantic search against a vector store.\n\n**Hadrian Extensions:** The response schema includes additional fields not in the standard OpenAI API:\n- `chunk_id`, `vector_store_id`, `chunk_index` (debugging info)",
    params(("vector_store_id" = Uuid, Path, description = "Vector store ID")),
    request_body = VectorStoreSearchRequest,
    responses(
        (status = 200, description = "Search results", body = VectorStoreSearchResponse),
        (status = 400, description = "Invalid request", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Vector store not found", body = crate::openapi::ErrorResponse),
        (status = 503, description = "File search not configured", body = crate::openapi::ErrorResponse),
    ),
    security(("api_key" = []))
))]
#[tracing::instrument(skip(state, auth, authz))]
pub async fn api_v1_vector_stores_search(
    State(state): State<AppState>,
    auth: Option<Extension<AuthenticatedRequest>>,
    authz: Option<Extension<AuthzContext>>,
    Path(vector_store_id): Path<VectorStoreId>,
    Json(input): Json<VectorStoreSearchRequest>,
) -> Result<Json<VectorStoreSearchResponse>, ApiError> {
    // Check RAG feature access via CEL policies
    if let Some(Extension(ref authz)) = authz {
        let org_id = auth
            .as_ref()
            .and_then(|a| a.api_key().and_then(|k| k.org_id.map(|id| id.to_string())));
        let project_id = auth.as_ref().and_then(|a| {
            a.api_key()
                .and_then(|k| k.project_id.map(|id| id.to_string()))
        });

        authz
            .require_api(
                "vector_store",
                "search",
                None,
                None,
                org_id.as_deref(),
                project_id.as_deref(),
            )
            .await
            .map_err(|e| {
                ApiError::new(StatusCode::FORBIDDEN, "authorization_denied", e.to_string())
            })?;
    }

    let vector_store_id = vector_store_id.into_inner();
    let services = get_services(&state)?;

    // Verify the vector store exists and check access
    let vector_store = services
        .vector_stores
        .get_by_id(vector_store_id)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("Vector store '{}' not found", vector_store_id),
            )
        })?;

    check_resource_access_optional(
        auth.as_ref().map(|e| &e.0),
        vector_store.owner_type,
        vector_store.owner_id,
    )?;

    // Get the file search service
    let file_search_service = state.file_search_service.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "not_configured",
            "File search is not configured. Enable [features.file_search] in configuration.",
        )
    })?;

    // Extract and validate score_threshold
    let score_threshold = input.ranking_options.as_ref().map(|r| r.score_threshold);
    if let Some(threshold) = score_threshold
        && !(0.0..=1.0).contains(&threshold)
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_parameter",
            format!(
                "score_threshold must be between 0.0 and 1.0, got {}",
                threshold
            ),
        ));
    }

    let search_request = crate::services::FileSearchRequest {
        query: input.query.clone(),
        vector_store_ids: vec![vector_store_id],
        max_results: input.max_num_results,
        threshold: score_threshold,
        file_ids: None,
        filters: input.filters,
        ranking_options: input.ranking_options,
    };

    // Execute search
    let search_response = file_search_service
        .search(search_request, None)
        .await
        .map_err(|e| match e {
            crate::services::FileSearchError::VectorStoreNotFound(id) => ApiError::new(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("VectorStore '{}' not found", id),
            ),
            crate::services::FileSearchError::EmbeddingError(msg) => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "embedding_error",
                format!("Embedding error: {}", msg),
            ),
            crate::services::FileSearchError::SearchError(msg) => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "search_error",
                format!("Search error: {}", msg),
            ),
            crate::services::FileSearchError::NotConfigured => ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "not_configured",
                "File search is not configured",
            ),
            _ => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                e.to_string(),
            ),
        })?;

    let data: Vec<SearchResultItem> = search_response
        .results
        .into_iter()
        .map(|r| SearchResultItem {
            chunk_id: r.chunk_id,
            object: "vector_store.search_result".to_string(),
            vector_store_id: r.vector_store_id,
            file_id: r.file_id,
            chunk_index: r.chunk_index,
            content: r.content,
            score: r.score,
            filename: r.filename,
            metadata: r.metadata,
        })
        .collect();

    Ok(Json(VectorStoreSearchResponse {
        object: "vector_store.search_results".to_string(),
        query: input.query,
        data,
    }))
}

pub fn get_api_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/v1/chat/completions", post(api_v1_chat_completions))
        .route("/v1/responses", post(api_v1_responses))
        .route("/v1/completions", post(api_v1_completions))
        .route("/v1/embeddings", post(api_v1_embeddings))
        .route("/v1/models", get(api_v1_models))
        // Images API (OpenAI-compatible)
        .route("/v1/images/generations", post(api_v1_images_generations))
        .route("/v1/images/edits", post(api_v1_images_edits))
        .route("/v1/images/variations", post(api_v1_images_variations))
        // Audio API (OpenAI-compatible)
        .route("/v1/audio/speech", post(api_v1_audio_speech))
        .route(
            "/v1/audio/transcriptions",
            post(api_v1_audio_transcriptions),
        )
        .route("/v1/audio/translations", post(api_v1_audio_translations))
        // Files API (OpenAI-compatible)
        .route(
            "/v1/files",
            post(api_v1_files_upload).get(api_v1_files_list),
        )
        .route(
            "/v1/files/{file_id}",
            get(api_v1_files_get).delete(api_v1_files_delete),
        )
        .route("/v1/files/{file_id}/content", get(api_v1_files_get_content))
        // Vector Stores API (OpenAI-compatible)
        .route(
            "/v1/vector_stores",
            post(api_v1_vector_stores_create).get(api_v1_vector_stores_list),
        )
        .route(
            "/v1/vector_stores/{vector_store_id}",
            get(api_v1_vector_stores_get)
                .post(api_v1_vector_stores_modify)
                .delete(api_v1_vector_stores_delete),
        )
        .route(
            "/v1/vector_stores/{vector_store_id}/files",
            post(api_v1_vector_stores_create_file).get(api_v1_vector_stores_list_files),
        )
        .route(
            "/v1/vector_stores/{vector_store_id}/files/{file_id}",
            get(api_v1_vector_stores_get_file).delete(api_v1_vector_stores_delete_file),
        )
        // Hadrian extension: chunk inspection (not in OpenAI API)
        .route(
            "/v1/vector_stores/{vector_store_id}/files/{file_id}/chunks",
            get(api_v1_vector_stores_list_file_chunks),
        )
        // Search endpoint (OpenAI-compatible, but schema has Hadrian extensions)
        .route(
            "/v1/vector_stores/{vector_store_id}/search",
            post(api_v1_vector_stores_search),
        )
        // File batches
        .route(
            "/v1/vector_stores/{vector_store_id}/file_batches",
            post(api_v1_vector_stores_create_file_batch),
        )
        .route(
            "/v1/vector_stores/{vector_store_id}/file_batches/{batch_id}",
            get(api_v1_vector_stores_get_file_batch).delete(api_v1_vector_stores_cancel_file_batch),
        )
        .route(
            "/v1/vector_stores/{vector_store_id}/file_batches/{batch_id}/files",
            get(api_v1_vector_stores_list_batch_files),
        )
        // Apply middleware layers in order (ServiceBuilder runs top-to-bottom):
        // 1. Rate limiting - reject requests early before auth overhead
        // 2. Auth, budget, usage - authenticates and sets AuthenticatedRequest
        // 3. Authorization - policy checks (needs AuthenticatedRequest from step 2)
        .route_layer(
            ServiceBuilder::new()
                .layer(from_fn_with_state(
                    state.clone(),
                    crate::middleware::rate_limit_middleware,
                ))
                .layer(from_fn_with_state(
                    state.clone(),
                    crate::middleware::api_middleware,
                ))
                .layer(from_fn_with_state(
                    state,
                    crate::middleware::api_authz_middleware,
                )),
        )
}

#[cfg(all(test, feature = "database-sqlite"))]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    // ============================================================================
    // Test Infrastructure
    // ============================================================================

    /// Create a test application with an in-memory database and test provider
    async fn test_app() -> axum::Router {
        use std::sync::atomic::{AtomicU64, Ordering};

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let db_id = COUNTER.fetch_add(1, Ordering::SeqCst);

        let config_str = format!(
            r#"
[database]
type = "sqlite"
path = "file:api_test_db_{}?mode=memory&cache=shared"
create_if_missing = true
run_migrations = true
wal_mode = false
busy_timeout_ms = 5000

[providers]
default_provider = "test"

[providers.test]
type = "test"
model_name = "test-model"

[providers.secondary-test]
type = "test"
model_name = "secondary-model"
"#,
            db_id
        );

        let config = crate::config::GatewayConfig::from_str(&config_str)
            .expect("Failed to parse test config");
        let state = crate::AppState::new(config.clone())
            .await
            .expect("Failed to create AppState");
        crate::build_app(&config, state)
    }

    /// Helper to make a JSON POST request
    async fn post_json(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
        post_json_with_headers(app, uri, body, vec![]).await
    }

    /// Helper to make a JSON POST request with custom headers
    async fn post_json_with_headers(
        app: &axum::Router,
        uri: &str,
        body: Value,
        headers: Vec<(&str, &str)>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json");

        for (key, value) in headers {
            builder = builder.header(key, value);
        }

        let request = builder
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        (status, json)
    }

    /// Helper to make a JSON POST request and return raw body (for streaming)
    async fn post_json_raw(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, String) {
        let request = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&body).to_string())
    }

    /// Helper to make a GET request
    async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        (status, json)
    }

    /// Helper to make a GET request and return raw bytes with headers
    async fn get_raw(
        app: &axum::Router,
        uri: &str,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let request = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, headers, body.to_vec())
    }

    /// Helper to make a DELETE request
    async fn delete_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method("DELETE")
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        (status, json)
    }

    // ============================================================================
    // Chat Completions - Deep Response Validation
    // ============================================================================

    #[tokio::test]
    async fn test_chat_completions_response_content_validation() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        // Validate response structure thoroughly
        assert_eq!(body["object"], "chat.completion");
        assert!(body["id"].as_str().unwrap().starts_with("test-"));
        assert!(body["created"].is_number());

        // Validate choices array
        let choices = body["choices"].as_array().unwrap();
        assert_eq!(choices.len(), 1);

        let choice = &choices[0];
        assert_eq!(choice["index"], 0);
        assert_eq!(choice["finish_reason"], "stop");

        // Validate message content matches test provider output
        let message = &choice["message"];
        assert_eq!(message["role"], "assistant");
        assert_eq!(
            message["content"],
            "This is a test response from the test provider."
        );

        // Validate usage statistics
        let usage = &body["usage"];
        assert_eq!(usage["prompt_tokens"], 10);
        assert_eq!(usage["completion_tokens"], 10);
        assert_eq!(usage["total_tokens"], 20);
    }

    #[tokio::test]
    async fn test_chat_completions_streaming_content_validation() {
        let app = test_app().await;

        let (status, body) = post_json_raw(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [{"role": "user", "content": "Hello"}],
                "stream": true
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        // Validate SSE format
        assert!(body.starts_with("data:"), "Should start with 'data:'");
        assert!(body.ends_with("[DONE]\n\n"), "Should end with [DONE]");

        // Parse and validate individual chunks
        let chunks: Vec<&str> = body.split("data: ").filter(|s| !s.is_empty()).collect();
        assert!(chunks.len() >= 3, "Should have at least 3 chunks");

        // First chunk should have role
        let first_chunk: Value = serde_json::from_str(chunks[0].trim()).unwrap();
        assert_eq!(first_chunk["object"], "chat.completion.chunk");
        assert_eq!(first_chunk["choices"][0]["delta"]["role"], "assistant");

        // Second chunk should have content
        let second_chunk: Value = serde_json::from_str(chunks[1].trim()).unwrap();
        assert_eq!(
            second_chunk["choices"][0]["delta"]["content"],
            "This is a test response from the test provider."
        );

        // Third chunk should have finish_reason and usage
        let third_chunk: Value = serde_json::from_str(chunks[2].trim()).unwrap();
        assert_eq!(third_chunk["choices"][0]["finish_reason"], "stop");
        assert_eq!(third_chunk["usage"]["total_tokens"], 20);
    }

    #[tokio::test]
    async fn test_chat_completions_model_passthrough() {
        let app = test_app().await;

        // The model name should be passed through to the response
        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/custom-model-name",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        // Test provider uses the model name from the payload
        assert_eq!(body["model"], "custom-model-name");
    }

    #[tokio::test]
    async fn test_chat_completions_default_provider() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "any-model",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "chat.completion");
        // Model should be the unprefixed model name
        assert_eq!(body["model"], "any-model");
    }

    #[tokio::test]
    async fn test_chat_completions_specific_provider() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "secondary-test/my-model",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["model"], "my-model");
    }

    // ============================================================================
    // Chat Completions - Error Cases
    // ============================================================================

    #[tokio::test]
    async fn test_chat_completions_missing_model_error() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"]["code"].is_string());
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("No model")
        );
    }

    #[tokio::test]
    async fn test_chat_completions_unknown_provider_error() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "nonexistent-provider/model",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        let message = body["error"]["message"].as_str().unwrap();
        assert!(
            message.contains("not found"),
            "Error should mention provider not found: {}",
            message
        );
    }

    #[tokio::test]
    async fn test_chat_completions_missing_messages_validation() {
        let app = test_app().await;

        let (status, _body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model"
            }),
        )
        .await;

        // Missing messages field should fail validation (422)
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_chat_completions_empty_messages_array() {
        let app = test_app().await;

        let (status, _body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": []
            }),
        )
        .await;

        // Empty messages array fails validation (400 Bad Request)
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // ============================================================================
    // Chat Completions - Edge Cases
    // ============================================================================

    #[tokio::test]
    async fn test_chat_completions_unicode_content() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [
                    {"role": "user", "content": "Hello 你好 مرحبا 🎉 émojis and ümläuts"}
                ]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "chat.completion");
    }

    #[tokio::test]
    async fn test_chat_completions_very_long_content() {
        let app = test_app().await;

        // Create a message with 10KB of content
        let long_content = "x".repeat(10 * 1024);

        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [{"role": "user", "content": long_content}]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "chat.completion");
    }

    #[tokio::test]
    async fn test_chat_completions_special_characters() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [
                    {"role": "user", "content": "Test with \"quotes\", 'apostrophes', \n newlines, \t tabs, and \\backslashes\\"}
                ]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "chat.completion");
    }

    #[tokio::test]
    async fn test_chat_completions_multiple_messages() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [
                    {"role": "system", "content": "You are a helpful assistant"},
                    {"role": "user", "content": "First message"},
                    {"role": "assistant", "content": "First response"},
                    {"role": "user", "content": "Second message"}
                ]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "chat.completion");
    }

    #[tokio::test]
    async fn test_chat_completions_with_optional_parameters() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [{"role": "user", "content": "Hello"}],
                "temperature": 0.7,
                "max_tokens": 100,
                "top_p": 0.9,
                "frequency_penalty": 0.5,
                "presence_penalty": 0.5,
                "stop": ["\n"],
                "user": "test-user-123"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "chat.completion");
    }

    // ============================================================================
    // Responses API - Deep Validation
    // ============================================================================

    #[tokio::test]
    async fn test_responses_content_validation() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/responses",
            json!({
                "model": "test/test-model",
                "input": "Hello"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "response");
        assert!(body["id"].as_str().unwrap().starts_with("test-"));
        assert_eq!(body["status"], "completed");

        // Validate output structure
        let output = body["output"].as_array().unwrap();
        assert!(!output.is_empty());

        let first_output = &output[0];
        assert_eq!(first_output["type"], "message");
        assert_eq!(first_output["role"], "assistant");

        // Validate usage
        let usage = &body["usage"];
        assert_eq!(usage["input_tokens"], 10);
        assert_eq!(usage["output_tokens"], 10);
        assert_eq!(usage["total_tokens"], 20);
    }

    #[tokio::test]
    async fn test_responses_streaming_content_validation() {
        let app = test_app().await;

        let (status, body) = post_json_raw(
            &app,
            "/api/v1/responses",
            json!({
                "model": "test/test-model",
                "input": "Hello",
                "stream": true
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("response.created"));
        assert!(body.contains("response.completed"));
        assert!(body.contains("This is a test response"));
    }

    #[tokio::test]
    async fn test_responses_with_models_array() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/responses",
            json!({
                "models": ["test/test-model"],
                "input": "Hello"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "response");
    }

    // ============================================================================
    // Completions API - Deep Validation
    // ============================================================================

    #[tokio::test]
    async fn test_completions_content_validation() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/completions",
            json!({
                "model": "test/test-model",
                "prompt": "Once upon a time"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "text_completion");

        // Validate choices
        let choices = body["choices"].as_array().unwrap();
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0]["index"], 0);
        assert_eq!(choices[0]["finish_reason"], "stop");
        assert!(
            choices[0]["text"]
                .as_str()
                .unwrap()
                .contains("test completion")
        );

        // Validate usage
        assert_eq!(body["usage"]["prompt_tokens"], 5);
        assert_eq!(body["usage"]["completion_tokens"], 10);
        assert_eq!(body["usage"]["total_tokens"], 15);
    }

    #[tokio::test]
    async fn test_completions_streaming_content_validation() {
        let app = test_app().await;

        let (status, body) = post_json_raw(
            &app,
            "/api/v1/completions",
            json!({
                "model": "test/test-model",
                "prompt": "Once upon a time",
                "stream": true
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("text_completion"));
        assert!(body.contains("test completion"));
        assert!(body.contains("[DONE]"));
    }

    // ============================================================================
    // Embeddings API - Deep Validation
    // ============================================================================

    #[tokio::test]
    async fn test_embeddings_content_validation() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/embeddings",
            json!({
                "model": "test/test-model",
                "input": "Hello world"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "list");

        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);

        let embedding_obj = &data[0];
        assert_eq!(embedding_obj["object"], "embedding");
        assert_eq!(embedding_obj["index"], 0);

        // Validate embedding vector
        let embedding = embedding_obj["embedding"].as_array().unwrap();
        assert_eq!(embedding.len(), 1536, "Should have 1536 dimensions");

        // Validate usage
        assert_eq!(body["usage"]["prompt_tokens"], 8);
        assert_eq!(body["usage"]["total_tokens"], 8);
    }

    #[tokio::test]
    async fn test_embeddings_array_input() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/embeddings",
            json!({
                "model": "test/test-model",
                "input": ["Hello", "World"]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "list");
    }

    #[tokio::test]
    async fn test_embeddings_missing_input_error() {
        let app = test_app().await;

        let (status, _body) = post_json(
            &app,
            "/api/v1/embeddings",
            json!({
                "model": "test/test-model"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    // ============================================================================
    // Models API - Deep Validation
    // ============================================================================

    #[tokio::test]
    async fn test_list_models_content_validation() {
        let app = test_app().await;

        let (status, body) = get_json(&app, "/api/v1/models").await;

        assert_eq!(status, StatusCode::OK);
        let models = body["data"].as_array().unwrap();

        // Should have 4 models total (2 per test provider)
        assert_eq!(models.len(), 4);

        // Validate model structure
        for model in models {
            let id = model["id"].as_str().unwrap();
            assert!(id.contains('/'), "Model ID should be provider-prefixed");
            assert!(model["object"].is_string() || model["object"].is_null());
        }

        // Check for specific provider prefixes
        let ids: Vec<&str> = models.iter().map(|m| m["id"].as_str().unwrap()).collect();
        assert!(ids.iter().any(|id| id.starts_with("test/")));
        assert!(ids.iter().any(|id| id.starts_with("secondary-test/")));
    }

    // ============================================================================
    // Dynamic Provider Routing Tests
    // ============================================================================

    #[tokio::test]
    async fn test_dynamic_provider_org_scope_not_found() {
        let app = test_app().await;

        // Try to use a dynamic provider that doesn't exist
        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": ":org/nonexistent-org/my-provider/gpt-4",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

        // Should fail because org doesn't exist
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let message = body["error"]["message"].as_str().unwrap();
        assert!(
            message.contains("not found") || message.contains("Organization"),
            "Should indicate org/provider not found: {}",
            message
        );
    }

    #[tokio::test]
    async fn test_dynamic_provider_invalid_scope_format() {
        let app = test_app().await;

        // Invalid scope format - missing components
        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": ":org/incomplete",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        let message = body["error"]["message"].as_str().unwrap();
        assert!(
            message.contains("Missing") || message.contains("component"),
            "Should indicate missing components: {}",
            message
        );
    }

    // ============================================================================
    // Authenticated Request Tests
    // ============================================================================
    //
    // Note: The API middleware allows anonymous requests by default - auth is optional.
    // These tests verify that authenticated requests work correctly, not that auth is enforced.

    #[tokio::test]
    async fn test_chat_completions_with_valid_api_key() {
        let app = test_app().await;

        // First create an org
        let (status, org) = post_json(
            &app,
            "/admin/v1/organizations",
            json!({
                "slug": "test-org-auth",
                "name": "Test Org for Auth"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let org_id = org["id"].as_str().unwrap();

        // Create an API key for the org (correct format from admin tests)
        let (status, api_key_response) = post_json(
            &app,
            "/admin/v1/api-keys",
            json!({
                "name": "test-key",
                "owner": {"type": "organization", "org_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let api_key = api_key_response["key"].as_str().unwrap();

        // Make authenticated request using Authorization header
        let (status, body) = post_json_with_headers(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
            vec![("Authorization", &format!("Bearer {}", api_key))],
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "chat.completion");
    }

    #[tokio::test]
    async fn test_chat_completions_with_x_api_key_header() {
        let app = test_app().await;

        // Create org and API key
        let (_, org) = post_json(
            &app,
            "/admin/v1/organizations",
            json!({"slug": "test-org-x-api", "name": "Test"}),
        )
        .await;
        let org_id = org["id"].as_str().unwrap();

        let (_, api_key_response) = post_json(
            &app,
            "/admin/v1/api-keys",
            json!({"name": "x-api-key-test", "owner": {"type": "organization", "org_id": org_id}}),
        )
        .await;
        let api_key = api_key_response["key"].as_str().unwrap();

        // Make request using X-API-Key header
        let (status, body) = post_json_with_headers(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
            vec![("X-API-Key", api_key)],
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "chat.completion");
    }

    #[tokio::test]
    async fn test_request_with_invalid_api_key_format() {
        let app = test_app().await;

        // Providing an invalid API key returns 401 — the gateway does not
        // fall through to anonymous access when credentials are present but invalid
        let (status, body) = post_json_with_headers(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
            vec![(
                "Authorization",
                "Bearer malformed-key-without-proper-prefix",
            )],
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["type"], "authentication_error");
    }

    #[tokio::test]
    async fn test_anonymous_request_allowed_by_default() {
        let app = test_app().await;

        // Request without any auth headers
        let (status, body) = post_json(
            &app,
            "/api/v1/chat/completions",
            json!({
                "model": "test/test-model",
                "messages": [{"role": "user", "content": "Hello"}]
            }),
        )
        .await;

        // Anonymous requests are allowed by default
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "chat.completion");
    }

    // ============================================================================
    // Error Handling Tests
    // ============================================================================

    #[tokio::test]
    async fn test_invalid_json_body() {
        let app = test_app().await;

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/chat/completions")
            .header("content-type", "application/json")
            .body(Body::from("not valid json"))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert!(
            response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[tokio::test]
    async fn test_empty_body() {
        let app = test_app().await;

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/chat/completions")
            .header("content-type", "application/json")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert!(
            response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[tokio::test]
    async fn test_wrong_content_type() {
        let app = test_app().await;

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/chat/completions")
            .header("content-type", "text/plain")
            .body(Body::from(
                r#"{"model": "test/test-model", "messages": []}"#,
            ))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        // Should fail due to wrong content type
        assert!(
            response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[tokio::test]
    async fn test_method_not_allowed() {
        let app = test_app().await;

        let request = Request::builder()
            .method("GET")
            .uri("/api/v1/chat/completions")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    // ============================================================================
    // Unit Tests for ApiError
    // ============================================================================

    #[test]
    fn test_api_error_new() {
        use super::ApiError;

        let error = ApiError::new(StatusCode::BAD_REQUEST, "test_code", "Test message");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.code, "test_code");
        assert_eq!(error.message, "Test message");
    }

    #[test]
    fn test_api_error_into_response() {
        use axum::response::IntoResponse;

        use super::ApiError;

        let error = ApiError::new(StatusCode::NOT_FOUND, "not_found", "Resource not found");
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_routing_error_to_api_error() {
        use super::ApiError;
        use crate::routing::RoutingError;

        let test_cases = vec![
            (RoutingError::NoModel, "No model specified"),
            (
                RoutingError::ProviderNotFound("test".to_string()),
                "not found",
            ),
            (RoutingError::NoDefaultProvider, "No default provider"),
        ];

        for (routing_error, expected_msg_part) in test_cases {
            let api_error: ApiError = routing_error.into();
            assert_eq!(api_error.status, StatusCode::BAD_REQUEST);
            assert_eq!(api_error.code, "routing_error");
            assert!(
                api_error.message.contains(expected_msg_part),
                "Expected '{}' to contain '{}'",
                api_error.message,
                expected_msg_part
            );
        }
    }

    #[test]
    fn test_provider_error_to_api_error() {
        use crate::{providers::ProviderError, routes::execution::provider_error_to_api_error};

        let internal_error = ProviderError::Internal("test error".to_string());
        let api_error = provider_error_to_api_error(internal_error);
        assert_eq!(api_error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(api_error.code, "internal_error");
    }

    // ============================================================================
    // Unit Tests for check_resource_access
    // ============================================================================

    /// Helper to create an AuthenticatedRequest from an Identity for testing
    fn test_auth_from_identity(
        user_id: Option<uuid::Uuid>,
        org_ids: Vec<String>,
        project_ids: Vec<String>,
    ) -> crate::auth::AuthenticatedRequest {
        use crate::auth::{AuthenticatedRequest, Identity, IdentityKind};

        let identity = Identity {
            external_id: "test-external-id".to_string(),
            email: None,
            name: None,
            user_id,
            roles: vec![],
            idp_groups: vec![],
            org_ids,
            team_ids: vec![],
            project_ids,
        };
        AuthenticatedRequest::new(IdentityKind::Identity(identity))
    }

    #[test]
    fn test_check_resource_access_user_owner_allowed() {
        use super::VectorStoreOwnerType;

        let user_id = uuid::Uuid::new_v4();
        let auth = test_auth_from_identity(Some(user_id), vec![], vec![]);

        let result = super::check_resource_access(&auth, VectorStoreOwnerType::User, user_id);
        assert!(
            result.is_ok(),
            "User should have access to their own resources"
        );
    }

    #[test]
    fn test_check_resource_access_user_owner_denied() {
        use super::VectorStoreOwnerType;

        let user_a_id = uuid::Uuid::new_v4();
        let user_b_id = uuid::Uuid::new_v4();
        let auth = test_auth_from_identity(Some(user_a_id), vec![], vec![]);

        let result = super::check_resource_access(&auth, VectorStoreOwnerType::User, user_b_id);
        assert!(
            result.is_err(),
            "User A should NOT have access to User B's resources"
        );

        let err = result.unwrap_err();
        assert_eq!(err.status, StatusCode::FORBIDDEN);
        assert_eq!(err.code, "access_denied");
    }

    #[test]
    fn test_check_resource_access_org_member_allowed() {
        use super::VectorStoreOwnerType;

        let org_id = uuid::Uuid::new_v4();
        let auth =
            test_auth_from_identity(Some(uuid::Uuid::new_v4()), vec![org_id.to_string()], vec![]);

        let result =
            super::check_resource_access(&auth, VectorStoreOwnerType::Organization, org_id);
        assert!(
            result.is_ok(),
            "Org member should have access to org resources"
        );
    }

    #[test]
    fn test_check_resource_access_org_nonmember_denied() {
        use super::VectorStoreOwnerType;

        let org_a_id = uuid::Uuid::new_v4();
        let org_b_id = uuid::Uuid::new_v4();
        let auth = test_auth_from_identity(
            Some(uuid::Uuid::new_v4()),
            vec![org_a_id.to_string()],
            vec![],
        );

        let result =
            super::check_resource_access(&auth, VectorStoreOwnerType::Organization, org_b_id);
        assert!(
            result.is_err(),
            "Non-member should NOT have access to org resources"
        );

        let err = result.unwrap_err();
        assert_eq!(err.status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_check_resource_access_project_member_allowed() {
        use super::VectorStoreOwnerType;

        let project_id = uuid::Uuid::new_v4();
        let auth = test_auth_from_identity(
            Some(uuid::Uuid::new_v4()),
            vec![],
            vec![project_id.to_string()],
        );

        let result = super::check_resource_access(&auth, VectorStoreOwnerType::Project, project_id);
        assert!(
            result.is_ok(),
            "Project member should have access to project resources"
        );
    }

    #[test]
    fn test_check_resource_access_project_nonmember_denied() {
        use super::VectorStoreOwnerType;

        let project_a_id = uuid::Uuid::new_v4();
        let project_b_id = uuid::Uuid::new_v4();
        let auth = test_auth_from_identity(
            Some(uuid::Uuid::new_v4()),
            vec![],
            vec![project_a_id.to_string()],
        );

        let result =
            super::check_resource_access(&auth, VectorStoreOwnerType::Project, project_b_id);
        assert!(
            result.is_err(),
            "Non-member should NOT have access to project resources"
        );

        let err = result.unwrap_err();
        assert_eq!(err.status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_check_resource_access_optional_allows_when_no_auth() {
        use super::VectorStoreOwnerType;

        let owner_id = uuid::Uuid::new_v4();

        // When auth is None (no authentication configured), access should be allowed
        let result =
            super::check_resource_access_optional(None, VectorStoreOwnerType::User, owner_id);
        assert!(result.is_ok(), "Should allow access when auth is disabled");
    }

    #[test]
    fn test_check_resource_access_optional_delegates_when_auth_present() {
        use super::VectorStoreOwnerType;

        let user_a_id = uuid::Uuid::new_v4();
        let user_b_id = uuid::Uuid::new_v4();
        let auth = test_auth_from_identity(Some(user_a_id), vec![], vec![]);

        // Should deny when user tries to access another user's resource
        let result = super::check_resource_access_optional(
            Some(&auth),
            VectorStoreOwnerType::User,
            user_b_id,
        );
        assert!(
            result.is_err(),
            "Should deny access to another user's resources"
        );
    }
    fn create_file_upload_multipart(
        file_content: &[u8],
        filename: &str,
        owner_type: &str,
        owner_id: &str,
        purpose: Option<&str>,
    ) -> (String, Vec<u8>) {
        let boundary = "----FileUploadBoundary12345";
        let mut body = Vec::new();

        // File field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
                filename
            )
            .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(file_content);
        body.extend_from_slice(b"\r\n");

        // owner_type field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"owner_type\"\r\n\r\n");
        body.extend_from_slice(owner_type.as_bytes());
        body.extend_from_slice(b"\r\n");

        // owner_id field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"owner_id\"\r\n\r\n");
        body.extend_from_slice(owner_id.as_bytes());
        body.extend_from_slice(b"\r\n");

        // Optional purpose field
        if let Some(p) = purpose {
            body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
            body.extend_from_slice(b"Content-Disposition: form-data; name=\"purpose\"\r\n\r\n");
            body.extend_from_slice(p.as_bytes());
            body.extend_from_slice(b"\r\n");
        }

        // End boundary
        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let content_type = format!("multipart/form-data; boundary={}", boundary);
        (content_type, body)
    }

    /// Helper to create an organization and return its ID (for file upload tests)
    async fn create_org_for_files(app: &axum::Router, slug: &str) -> String {
        let (status, org) = post_json(
            app,
            "/admin/v1/organizations",
            json!({"slug": slug, "name": format!("Org {}", slug)}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        org["id"].as_str().unwrap().to_string()
    }

    /// Helper to create a user and return its ID (for file upload tests)
    async fn create_user_for_files(app: &axum::Router, external_id: &str) -> String {
        let (status, user) = post_json(
            app,
            "/admin/v1/users",
            json!({
                "external_id": external_id,
                "email": format!("{}@example.com", external_id),
                "name": format!("Test User {}", external_id)
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        user["id"].as_str().unwrap().to_string()
    }

    /// Helper to create a team and return its ID (for file upload tests)
    async fn create_team_for_files(app: &axum::Router, org_slug: &str, slug: &str) -> String {
        let (status, team) = post_json(
            app,
            &format!("/admin/v1/organizations/{}/teams", org_slug),
            json!({
                "slug": slug,
                "name": format!("Team {}", slug)
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        team["id"].as_str().unwrap().to_string()
    }

    /// Helper to create a project and return its ID (for file upload tests)
    async fn create_project_for_files(app: &axum::Router, org_slug: &str, slug: &str) -> String {
        let (status, project) = post_json(
            app,
            &format!("/admin/v1/organizations/{}/projects", org_slug),
            json!({
                "slug": slug,
                "name": format!("Project {}", slug)
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        project["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn test_file_upload_basic() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-upload-basic-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"Hello, this is test file content.",
            "test-document.txt",
            "user",
            &owner_id,
            None,
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["object"], "file");
        assert!(json["id"].as_str().unwrap().starts_with("file-"));
        assert_eq!(json["filename"], "test-document.txt");
        assert_eq!(json["purpose"], "assistants"); // Default purpose
        assert_eq!(json["bytes"], 33); // Length of test content
        assert!(json["created_at"].is_string()); // DateTime<Utc> serializes as ISO 8601 string
    }

    #[tokio::test]
    async fn test_file_upload_with_purpose_batch() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-batch-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"Batch file content",
            "batch-input.jsonl",
            "user",
            &owner_id,
            Some("batch"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["purpose"], "batch");
    }

    #[tokio::test]
    async fn test_file_upload_with_purpose_fine_tune() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-finetune-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"Fine-tuning training data",
            "training-data.jsonl",
            "user",
            &owner_id,
            Some("fine-tune"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        // Note: FilePurpose::FineTune serializes as "fine_tune" due to serde rename_all = "snake_case"
        assert_eq!(json["purpose"], "fine_tune");
    }

    #[tokio::test]
    async fn test_file_upload_with_purpose_vision() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-vision-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"\x89PNG\r\n\x1a\nimage data here",
            "image.png",
            "user",
            &owner_id,
            Some("vision"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["purpose"], "vision");
    }

    #[tokio::test]
    async fn test_file_upload_owner_type_organization() {
        let app = test_app().await;
        let owner_id = create_org_for_files(&app, "file-org-owner").await;
        let (content_type, body) = create_file_upload_multipart(
            b"Organization file",
            "org-doc.pdf",
            "organization",
            &owner_id,
            None,
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["object"], "file");
    }

    #[tokio::test]
    async fn test_file_upload_owner_type_project() {
        let app = test_app().await;
        let org_slug = "file-project-org";
        let _org_id = create_org_for_files(&app, org_slug).await;
        let owner_id = create_project_for_files(&app, org_slug, "file-project-owner").await;
        let (content_type, body) = create_file_upload_multipart(
            b"Project file",
            "project-doc.md",
            "project",
            &owner_id,
            None,
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["object"], "file");
    }

    #[tokio::test]
    async fn test_file_upload_owner_type_team() {
        let app = test_app().await;
        let org_slug = "file-team-org";
        let _org_id = create_org_for_files(&app, org_slug).await;
        let owner_id = create_team_for_files(&app, org_slug, "file-team-owner").await;
        let (content_type, body) = create_file_upload_multipart(
            b"Team shared file",
            "team-notes.txt",
            "team",
            &owner_id,
            None,
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["object"], "file");
    }

    #[tokio::test]
    async fn test_file_upload_unicode_filename() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-unicode-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"Unicode content test",
            "日本語ドキュメント_文档_документ.txt",
            "user",
            &owner_id,
            None,
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["filename"], "日本語ドキュメント_文档_документ.txt");
    }

    #[tokio::test]
    async fn test_file_upload_missing_file_field() {
        let app = test_app().await;
        let owner_id = uuid::Uuid::new_v4().to_string();
        let boundary = "----FileUploadBoundary12345";
        let mut body = Vec::new();

        // Only owner_type and owner_id, no file field
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"owner_type\"\r\n\r\n");
        body.extend_from_slice(b"user\r\n");
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"owner_id\"\r\n\r\n");
        body.extend_from_slice(owner_id.as_bytes());
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "missing_file");
    }

    #[tokio::test]
    async fn test_file_upload_missing_owner_type() {
        let app = test_app().await;
        let owner_id = uuid::Uuid::new_v4().to_string();
        let boundary = "----FileUploadBoundary12345";
        let mut body = Vec::new();

        // File and owner_id, but no owner_type
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: text/plain\r\n\r\n");
        body.extend_from_slice(b"Test content\r\n");
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"owner_id\"\r\n\r\n");
        body.extend_from_slice(owner_id.as_bytes());
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "missing_owner_type");
    }

    #[tokio::test]
    async fn test_file_upload_missing_owner_id() {
        let app = test_app().await;
        let boundary = "----FileUploadBoundary12345";
        let mut body = Vec::new();

        // File and owner_type, but no owner_id
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: text/plain\r\n\r\n");
        body.extend_from_slice(b"Test content\r\n");
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"owner_type\"\r\n\r\n");
        body.extend_from_slice(b"user\r\n");
        body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let content_type = format!("multipart/form-data; boundary={}", boundary);

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "missing_owner_id");
    }

    #[tokio::test]
    async fn test_file_upload_invalid_owner_type() {
        let app = test_app().await;
        let owner_id = uuid::Uuid::new_v4().to_string();
        let (content_type, body) = create_file_upload_multipart(
            b"Test content",
            "test.txt",
            "invalid_type",
            &owner_id,
            None,
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_owner_type");
    }

    #[tokio::test]
    async fn test_file_upload_invalid_owner_id() {
        let app = test_app().await;
        let (content_type, body) = create_file_upload_multipart(
            b"Test content",
            "test.txt",
            "user",
            "not-a-valid-uuid",
            None,
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_owner_id");
    }

    #[tokio::test]
    async fn test_file_upload_invalid_purpose() {
        let app = test_app().await;
        let owner_id = uuid::Uuid::new_v4().to_string();
        let (content_type, body) = create_file_upload_multipart(
            b"Test content",
            "test.txt",
            "user",
            &owner_id,
            Some("invalid_purpose"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_purpose");
    }

    #[tokio::test]
    async fn test_file_upload_owner_not_found() {
        let app = test_app().await;
        // Use a valid UUID format but for a non-existent user
        let owner_id = uuid::Uuid::new_v4().to_string();
        let (content_type, body) =
            create_file_upload_multipart(b"Test content", "test.txt", "user", &owner_id, None);

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"]["code"], "owner_not_found");
    }

    #[tokio::test]
    async fn test_file_upload_empty_file() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-empty-user").await;
        let (content_type, body) =
            create_file_upload_multipart(b"", "empty.txt", "user", &owner_id, None);

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        // Empty files should be allowed
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["bytes"], 0);
    }

    #[tokio::test]
    async fn test_file_upload_binary_content() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-binary-user").await;
        // Binary content with various byte values including null bytes
        // Use .png extension since binary files with .bin are not allowed for assistants purpose
        let binary_content: Vec<u8> = (0..=255).collect();
        let (content_type, body) =
            create_file_upload_multipart(&binary_content, "binary.png", "user", &owner_id, None);

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["bytes"], 256);
        assert_eq!(json["filename"], "binary.png");
    }

    /// Create a test application with a custom max file size limit
    async fn test_app_with_file_size_limit(max_file_size_mb: u64) -> axum::Router {
        use std::sync::atomic::{AtomicU64, Ordering};

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let db_id = COUNTER.fetch_add(1, Ordering::SeqCst);

        let config_str = format!(
            r#"
[database]
type = "sqlite"
path = "file:api_test_file_limit_db_{}?mode=memory&cache=shared"
create_if_missing = true
run_migrations = true
wal_mode = false
busy_timeout_ms = 5000

[providers]
default_provider = "test"

[providers.test]
type = "test"
model_name = "test-model"

[features.file_processing]
max_file_size_mb = {}
"#,
            db_id, max_file_size_mb
        );

        let config = crate::config::GatewayConfig::from_str(&config_str)
            .expect("Failed to parse test config");
        let state = crate::AppState::new(config.clone())
            .await
            .expect("Failed to create AppState");
        crate::build_app(&config, state)
    }

    /// Create a test application with file_search_service configured.
    ///
    /// This enables testing endpoints that require the file search service,
    /// such as the vector store file addition endpoint which validates
    /// embedding model compatibility.
    async fn test_app_with_file_search() -> axum::Router {
        let (app, _db) = test_app_with_file_search_and_db().await;
        app
    }

    /// Create a test application with file_search_service configured, returning
    /// both the app router and the database for direct manipulation in tests.
    async fn test_app_with_file_search_and_db() -> (axum::Router, std::sync::Arc<crate::db::DbPool>)
    {
        use std::sync::atomic::{AtomicU64, Ordering};

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let db_id = COUNTER.fetch_add(1, Ordering::SeqCst);

        let config_str = format!(
            r#"
[database]
type = "sqlite"
path = "file:api_test_file_search_db_{}?mode=memory&cache=shared"
create_if_missing = true
run_migrations = true
wal_mode = false
busy_timeout_ms = 5000

[providers]
default_provider = "test"

[providers.test]
type = "test"
model_name = "test-model"
"#,
            db_id
        );

        let config = crate::config::GatewayConfig::from_str(&config_str)
            .expect("Failed to parse test config");
        let mut state = crate::AppState::new(config.clone())
            .await
            .expect("Failed to create AppState");

        // Create EmbeddingService using the test provider
        // Use the default embedding model name that collections are created with
        let embedding_config = crate::config::EmbeddingConfig {
            provider: "test".to_string(),
            model: "text-embedding-3-small".to_string(), // Default vector store model
            dimensions: 1536,                            // Default vector store dimensions
        };

        let provider_config = config.providers.get("test").expect("test provider config");
        let embedding_service = std::sync::Arc::new(
            crate::cache::EmbeddingService::new(
                &embedding_config,
                provider_config,
                &state.circuit_breakers,
                state.http_client.clone(),
            )
            .expect("Failed to create embedding service"),
        );

        // Create TestVectorStore with matching dimensions
        let vector_store: std::sync::Arc<dyn crate::cache::vector_store::VectorBackend> =
            std::sync::Arc::new(crate::cache::vector_store::TestVectorStore::new(1536));

        let db = state.db.clone().expect("db should be configured");

        // Create FileSearchService
        let file_search_service = crate::services::FileSearchService::new(
            db.clone(),
            embedding_service,
            vector_store,
            None, // No reranker needed for tests
            crate::services::FileSearchServiceConfig {
                default_max_results: 10,
                default_threshold: 0.7,
                retry: crate::config::RetryConfig::default(),
                circuit_breaker: crate::config::CircuitBreakerConfig::default(),
                rerank: crate::config::RerankConfig::default(),
            },
        );

        state.file_search_service = Some(std::sync::Arc::new(file_search_service));

        (crate::build_app(&config, state), db)
    }

    /// Create a test application with MockableTestVectorStore for testing search results.
    ///
    /// Returns the app router, database, and a handle to set mock search results.
    async fn test_app_with_mockable_file_search() -> (
        axum::Router,
        std::sync::Arc<crate::db::DbPool>,
        std::sync::Arc<std::sync::Mutex<Vec<crate::cache::vector_store::ChunkSearchResult>>>,
    ) {
        use std::sync::atomic::{AtomicU64, Ordering};

        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let db_id = COUNTER.fetch_add(1, Ordering::SeqCst);

        let config_str = format!(
            r#"
[database]
type = "sqlite"
path = "file:api_test_mockable_fs_db_{}?mode=memory&cache=shared"
create_if_missing = true
run_migrations = true
wal_mode = false
busy_timeout_ms = 5000

[providers]
default_provider = "test"

[providers.test]
type = "test"
model_name = "test-model"
"#,
            db_id
        );

        let config = crate::config::GatewayConfig::from_str(&config_str)
            .expect("Failed to parse test config");
        let mut state = crate::AppState::new(config.clone())
            .await
            .expect("Failed to create AppState");

        // Create EmbeddingService using the test provider
        let embedding_config = crate::config::EmbeddingConfig {
            provider: "test".to_string(),
            model: "text-embedding-3-small".to_string(),
            dimensions: 1536,
        };

        let provider_config = config.providers.get("test").expect("test provider config");
        let embedding_service = std::sync::Arc::new(
            crate::cache::EmbeddingService::new(
                &embedding_config,
                provider_config,
                &state.circuit_breakers,
                state.http_client.clone(),
            )
            .expect("Failed to create embedding service"),
        );

        // Create MockableTestVectorStore with matching dimensions
        let mockable_store = crate::cache::vector_store::MockableTestVectorStore::new(1536);
        let mock_results_handle = mockable_store.mock_results_handle();
        let vector_store: std::sync::Arc<dyn crate::cache::vector_store::VectorBackend> =
            std::sync::Arc::new(mockable_store);

        let db = state.db.clone().expect("db should be configured");

        // Create FileSearchService
        let file_search_service = crate::services::FileSearchService::new(
            db.clone(),
            embedding_service,
            vector_store,
            None,
            crate::services::FileSearchServiceConfig {
                default_max_results: 10,
                default_threshold: 0.7,
                retry: crate::config::RetryConfig::default(),
                circuit_breaker: crate::config::CircuitBreakerConfig::default(),
                rerank: crate::config::RerankConfig::default(),
            },
        );

        state.file_search_service = Some(std::sync::Arc::new(file_search_service));

        (crate::build_app(&config, state), db, mock_results_handle)
    }

    #[tokio::test]
    async fn test_file_upload_file_size_limit_exceeded() {
        // Create app with 0 MB limit (any non-empty file will be rejected)
        let app = test_app_with_file_size_limit(0).await;
        let owner_id = create_user_for_files(&app, "file-size-limit-user").await;

        // Try to upload a small file (should be rejected since limit is 0)
        let (content_type, body) = create_file_upload_multipart(
            b"This file content exceeds the configured limit",
            "too-large.txt",
            "user",
            &owner_id,
            None,
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(json["error"]["code"], "file_too_large");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("exceeds maximum allowed size")
        );
    }

    #[tokio::test]
    async fn test_file_upload_file_size_within_limit() {
        // Create app with 1 MB limit
        let app = test_app_with_file_size_limit(1).await;
        let owner_id = create_user_for_files(&app, "file-size-ok-user").await;

        // Upload a small file (should succeed since it's under 1 MB)
        let (content_type, body) = create_file_upload_multipart(
            b"This file is small enough",
            "small-file.txt",
            "user",
            &owner_id,
            None,
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["filename"], "small-file.txt");
    }

    #[tokio::test]
    async fn test_file_upload_invalid_file_type_fine_tune_txt() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-type-finetune-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"This should fail - not jsonl",
            "training-data.txt", // Should be .jsonl for fine-tune
            "user",
            &owner_id,
            Some("fine-tune"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_file_type");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("fine-tune")
        );
    }

    #[tokio::test]
    async fn test_file_upload_invalid_file_type_batch_pdf() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-type-batch-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"This should fail - not jsonl",
            "batch-requests.pdf", // Should be .jsonl for batch
            "user",
            &owner_id,
            Some("batch"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_file_type");
        assert!(json["error"]["message"].as_str().unwrap().contains("batch"));
    }

    #[tokio::test]
    async fn test_file_upload_invalid_file_type_vision_txt() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-type-vision-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"This should fail - not an image",
            "document.txt", // Should be image for vision
            "user",
            &owner_id,
            Some("vision"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_file_type");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("vision")
        );
    }

    #[tokio::test]
    async fn test_file_upload_invalid_file_type_assistants_exe() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-type-assistants-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"\x4D\x5A\x90\x00", // PE header bytes
            "malware.exe",       // Executable files not allowed
            "user",
            &owner_id,
            None, // Default is assistants
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_file_type");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("assistants")
        );
    }

    #[tokio::test]
    async fn test_file_upload_invalid_file_type_no_extension() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-type-noext-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"No extension file",
            "README", // No extension
            "user",
            &owner_id,
            Some("fine-tune"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_file_type");
        // Message should indicate no extension
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("(no extension)")
        );
    }

    #[tokio::test]
    async fn test_file_upload_valid_file_type_assistants_pdf() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-type-valid-pdf-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"%PDF-1.4 fake pdf content",
            "document.pdf",
            "user",
            &owner_id,
            None, // Default is assistants
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["filename"], "document.pdf");
    }

    #[tokio::test]
    async fn test_file_upload_valid_file_type_vision_jpeg() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-type-valid-jpeg-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"\xFF\xD8\xFF\xE0", // JPEG magic bytes
            "photo.jpeg",
            "user",
            &owner_id,
            Some("vision"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["filename"], "photo.jpeg");
        assert_eq!(json["purpose"], "vision");
    }

    #[tokio::test]
    async fn test_file_upload_valid_file_type_assistants_code() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-type-valid-code-user").await;
        let (content_type, body) = create_file_upload_multipart(
            b"fn main() { println!(\"Hello\"); }",
            "main.rs",
            "user",
            &owner_id,
            None, // Default is assistants
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["filename"], "main.rs");
    }

    // ============================================================================
    // Vector Store API Tests
    // ============================================================================

    /// Helper to create an organization and return its ID
    async fn create_org_for_vector_store(app: &axum::Router, slug: &str) -> String {
        let (status, org) = post_json(
            app,
            "/admin/v1/organizations",
            json!({"slug": slug, "name": format!("Org {}", slug)}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        org["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn test_vector_store_create_basic() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-create-org").await;

        let (status, body) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Test Vector Store"
            }),
        )
        .await;

        if status != StatusCode::CREATED {
            eprintln!(
                "Error response: {}",
                serde_json::to_string_pretty(&body).unwrap()
            );
        }
        assert_eq!(status, StatusCode::CREATED);

        // Validate response structure
        assert!(body["id"].as_str().unwrap().starts_with("vs_"));
        assert_eq!(body["object"], "vector_store");
        assert_eq!(body["name"], "Test Vector Store");
        assert_eq!(body["owner_type"], "organization");
        assert_eq!(body["owner_id"], org_id);
        assert_eq!(body["status"], "completed");
        assert_eq!(body["embedding_model"], "text-embedding-3-small");
        assert_eq!(body["embedding_dimensions"], 1536);
        assert_eq!(body["usage_bytes"], 0);
        assert!(body["created_at"].is_string());
        assert!(body["updated_at"].is_string());

        // File counts should be zero initially
        assert_eq!(body["file_counts"]["in_progress"], 0);
        assert_eq!(body["file_counts"]["completed"], 0);
        assert_eq!(body["file_counts"]["failed"], 0);
        assert_eq!(body["file_counts"]["cancelled"], 0);
        assert_eq!(body["file_counts"]["total"], 0);
    }

    #[tokio::test]
    async fn test_vector_store_create_with_description() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-desc-org").await;

        let (status, body) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Described Store",
                "description": "A test vector store with a description"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["name"], "Described Store");
        assert_eq!(
            body["description"],
            "A test vector store with a description"
        );
    }

    #[tokio::test]
    async fn test_vector_store_create_with_metadata() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-meta-org").await;

        let (status, body) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Metadata Store",
                "metadata": {
                    "env": "test",
                    "version": "1.0"
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["metadata"]["env"], "test");
        assert_eq!(body["metadata"]["version"], "1.0");
    }

    #[tokio::test]
    async fn test_vector_store_create_with_custom_embedding() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-embed-org").await;

        let (status, body) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Custom Embedding Store",
                "embedding_model": "text-embedding-ada-002",
                "embedding_dimensions": 1024
            }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["embedding_model"], "text-embedding-ada-002");
        assert_eq!(body["embedding_dimensions"], 1024);
    }

    #[tokio::test]
    async fn test_vector_store_create_auto_generated_name() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-autoname-org").await;

        let (status, body) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        // Name should be auto-generated (not null/empty)
        assert!(body["name"].is_string());
        assert!(!body["name"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_vector_store_create_owner_not_found() {
        let app = test_app().await;
        let fake_org_id = uuid::Uuid::new_v4().to_string();

        let (status, body) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": fake_org_id},
                "name": "Orphan Store"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_vector_store_create_invalid_owner_type() {
        let app = test_app().await;

        let (status, _body) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "invalid_type", "organization_id": uuid::Uuid::new_v4().to_string()},
                "name": "Invalid Owner Store"
            }),
        )
        .await;

        // Should fail validation (422) or bad request (400)
        assert!(
            status == StatusCode::UNPROCESSABLE_ENTITY || status == StatusCode::BAD_REQUEST,
            "Expected 422 or 400, got {}",
            status
        );
    }

    #[tokio::test]
    async fn test_vector_store_create_missing_owner() {
        let app = test_app().await;

        let (status, _body) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "name": "No Owner Store"
            }),
        )
        .await;

        // Missing required field should fail validation
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_vector_store_create_with_expires_after() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-expires-org").await;

        let (status, body) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Expiring Store",
                "expires_after": {
                    "anchor": "last_active_at",
                    "days": 7
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["expires_after"]["anchor"], "last_active_at");
        assert_eq!(body["expires_after"]["days"], 7);
    }

    #[tokio::test]
    async fn test_vector_store_list_empty() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-list-empty-org").await;

        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/vector_stores?owner_type=organization&owner_id={}",
                org_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "list");
        assert!(body["data"].is_array());
        assert_eq!(body["data"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_vector_store_list_with_stores() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-list-stores-org").await;

        // Create two vector stores
        let (status, _) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Store One"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        let (status, _) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Store Two"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // List should return both
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/vector_stores?owner_type=organization&owner_id={}",
                org_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_vector_store_get_by_id() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-get-org").await;

        // Create a vector store
        let (status, created) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Get Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let store_id = created["id"].as_str().unwrap();

        // Get by ID
        let (status, body) = get_json(&app, &format!("/api/v1/vector_stores/{}", store_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], store_id);
        assert_eq!(body["name"], "Get Test Store");
    }

    #[tokio::test]
    async fn test_vector_store_get_not_found() {
        let app = test_app().await;
        let fake_id = format!("vs_{}", uuid::Uuid::new_v4());

        let (status, body) = get_json(&app, &format!("/api/v1/vector_stores/{}", fake_id)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_vector_store_modify() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-modify-org").await;

        // Create a vector store
        let (status, created) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Original Name"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let store_id = created["id"].as_str().unwrap();

        // Modify it
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}", store_id),
            json!({
                "name": "Updated Name",
                "description": "New description"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "Updated Name");
        assert_eq!(body["description"], "New description");
    }

    #[tokio::test]
    async fn test_vector_store_modify_not_found() {
        let app = test_app().await;
        let fake_id = format!("vs_{}", uuid::Uuid::new_v4());

        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}", fake_id),
            json!({"name": "New Name"}),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_vector_store_delete() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-delete-org").await;

        // Create a vector store
        let (status, created) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "To Be Deleted"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let store_id = created["id"].as_str().unwrap();

        // Delete it
        let (status, body) =
            delete_json(&app, &format!("/api/v1/vector_stores/{}", store_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], store_id);
        assert_eq!(body["object"], "vector_store.deleted");
        assert_eq!(body["deleted"], true);

        // Verify it's gone
        let (status, _) = get_json(&app, &format!("/api/v1/vector_stores/{}", store_id)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_vector_store_delete_not_found() {
        let app = test_app().await;
        let fake_id = format!("vs_{}", uuid::Uuid::new_v4());

        let (status, body) = delete_json(&app, &format!("/api/v1/vector_stores/{}", fake_id)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_vector_store_list_pagination() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vs-pagination-org").await;

        // Create 5 vector stores
        for i in 0..5 {
            let (status, _) = post_json(
                &app,
                "/api/v1/vector_stores",
                json!({
                    "owner": {"type": "organization", "organization_id": org_id},
                    "name": format!("Store {}", i)
                }),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED);
        }

        // Request with limit=2
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/vector_stores?owner_type=organization&owner_id={}&limit=2",
                org_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"].as_array().unwrap().len(), 2);
        assert!(body["has_more"].as_bool().unwrap());
    }

    // ============================================================================
    // Vector Store Files API Tests
    // ============================================================================

    /// Helper to upload a file and return its ID (for vector store file tests)
    async fn upload_file_for_vector_store(
        app: &axum::Router,
        owner_type: &str,
        owner_id: &str,
        filename: &str,
    ) -> String {
        let (content_type, body) = create_file_upload_multipart(
            b"Test file content for vector store",
            filename,
            owner_type,
            owner_id,
            Some("assistants"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK, "File upload failed: {:?}", json);
        json["id"].as_str().unwrap().to_string()
    }

    /// Helper to upload a file with unique content (avoids content deduplication)
    async fn upload_file_with_unique_content(
        app: &axum::Router,
        owner_type: &str,
        owner_id: &str,
        filename: &str,
    ) -> String {
        // Include filename and UUID in content to ensure uniqueness
        let content = format!("Unique content for {} - {}", filename, uuid::Uuid::new_v4());
        let (content_type, body) = create_file_upload_multipart(
            content.as_bytes(),
            filename,
            owner_type,
            owner_id,
            Some("assistants"),
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(status, StatusCode::OK, "File upload failed: {:?}", json);
        json["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn test_vector_store_file_create_vector_store_not_found() {
        let app = test_app().await;
        let fake_vs_id = format!("vs_{}", uuid::Uuid::new_v4());
        let fake_file_id = format!("file-{}", uuid::Uuid::new_v4());

        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", fake_vs_id),
            json!({"file_id": fake_file_id}),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_create_file_not_found() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vsf-file-not-found-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Test Store for File Not Found"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Try to add a non-existent file
        let fake_file_id = format!("file-{}", uuid::Uuid::new_v4());
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": fake_file_id}),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_create_service_unavailable() {
        // The default test_app() doesn't configure file_search_service,
        // so validate_embedding_model_compatibility returns 503
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vsf-service-unavail-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Test Store for Service Unavailable"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file
        let file_id = upload_file_for_vector_store(&app, "organization", &org_id, "test.txt").await;

        // Try to add the file to the vector store
        // This should fail with 503 because file_search_service is not configured
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"]["code"], "embedding_service_unavailable");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("File search service is not configured")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_list_vector_store_not_found() {
        let app = test_app().await;
        let fake_vs_id = format!("vs_{}", uuid::Uuid::new_v4());

        let (status, body) =
            get_json(&app, &format!("/api/v1/vector_stores/{}/files", fake_vs_id)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_list_empty() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vsf-list-empty-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Empty Vector Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // List files (should be empty)
        let (status, body) =
            get_json(&app, &format!("/api/v1/vector_stores/{}/files", vs_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "list");
        assert!(body["data"].as_array().unwrap().is_empty());
        assert_eq!(body["has_more"], false);
        assert!(body["first_id"].is_null());
        assert!(body["last_id"].is_null());
    }

    #[tokio::test]
    async fn test_vector_store_file_list_invalid_cursor() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vsf-list-cursor-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Cursor Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Try to list with invalid cursor format
        let (status, body) = get_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files?after=invalid-cursor", vs_id),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_cursor");
    }

    #[tokio::test]
    async fn test_vector_store_file_list_cursor_not_found() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vsf-list-cursor-nf-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Cursor Not Found Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Try to list with valid format but non-existent cursor
        let fake_file_id = format!("file-{}", uuid::Uuid::new_v4());
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/vector_stores/{}/files?after={}",
                vs_id, fake_file_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_cursor");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found for cursor")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_list_invalid_filter() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vsf-list-filter-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Filter Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Try to list with invalid filter
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/vector_stores/{}/files?filter=invalid_status",
                vs_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_filter");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Invalid filter status")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_list_with_limit() {
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vsf-list-limit-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Limit Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // List with limit parameter (should work even on empty store)
        let (status, body) = get_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files?limit=5", vs_id),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "list");
        assert!(body["data"].as_array().unwrap().is_empty());
    }

    // ============================================================================
    // Vector Store File Success Tests (POST /v1/vector_stores/{id}/files)
    // These tests use test_app_with_file_search() which has FileSearchService configured
    // ============================================================================

    #[tokio::test]
    async fn test_vector_store_file_create_success() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsf-create-success-org").await;

        // Create a vector store (uses default embedding model: text-embedding-3-small)
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Success Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file
        let file_id =
            upload_file_for_vector_store(&app, "organization", &org_id, "success.txt").await;

        // Add the file to the vector store
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["object"], "vector_store.file");
        assert_eq!(body["vector_store_id"], vs_id);
        // Note: file_id in response is the vector store_file's file_id, not the vector store file ID
        assert_eq!(body["status"], "in_progress"); // No document processor, so stays in_progress
        // VectorStoreFileId uses "file-" prefix per prefixed_id.rs
        assert!(body["id"].as_str().unwrap().starts_with("file-"));
    }

    #[tokio::test]
    async fn test_vector_store_file_create_idempotent() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsf-idempotent-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Idempotent Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file
        let file_id =
            upload_file_for_vector_store(&app, "organization", &org_id, "idempotent.txt").await;

        // Add the file to the vector store (first time)
        let (status1, body1) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;
        assert_eq!(status1, StatusCode::CREATED);
        let vector_store_file_id = body1["id"].as_str().unwrap();

        // Add the same file again (should be idempotent)
        let (status2, body2) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;

        // Should return 200 OK with the existing entry
        assert_eq!(status2, StatusCode::OK);
        // Note: After model change, id IS the file_id (file- prefix)
        assert_eq!(body2["id"], vector_store_file_id);
        assert_eq!(body2["vector_store_id"], vs_id);
    }

    #[tokio::test]
    async fn test_vector_store_file_create_reprocess_failed() {
        let (app, db) = test_app_with_file_search_and_db().await;
        let org_id = create_org_for_vector_store(&app, "vsf-reprocess-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Reprocess Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file
        let file_id =
            upload_file_for_vector_store(&app, "organization", &org_id, "reprocess.txt").await;

        // Add the file to the vector store
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let returned_file_id = body["id"].as_str().unwrap();

        // Manually mark the file as failed using the vector stores repo
        // After model change, body["id"] is the file_id (file- prefix).
        // We need to look up the internal junction record ID to update the status.
        let file_uuid: uuid::Uuid = returned_file_id
            .strip_prefix("file-")
            .unwrap()
            .parse()
            .unwrap();
        let vs_uuid: uuid::Uuid = vs_id.strip_prefix("vs_").unwrap().parse().unwrap();

        // Look up the vector store file to get the internal junction ID
        let vector_store_file = db
            .vector_stores()
            .find_vector_store_file_by_file_id(vs_uuid, file_uuid)
            .await
            .expect("Failed to find vector store file")
            .expect("VectorStore file not found");
        let internal_id = vector_store_file.internal_id;

        // Update the status using the vector stores repo
        use crate::models::{FileError, FileErrorCode, VectorStoreFileStatus};
        db.vector_stores()
            .update_vector_store_file_status(
                internal_id,
                VectorStoreFileStatus::Failed,
                Some(FileError {
                    code: FileErrorCode::ServerError,
                    message: "Test failure".to_string(),
                }),
            )
            .await
            .expect("Failed to update status");

        // Try to add the file again (should trigger re-processing)
        let (status2, body2) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;

        // Should return 200 OK with re-processing triggered
        assert_eq!(status2, StatusCode::OK);
        assert_eq!(body2["id"], returned_file_id);
        // Status will be "in_progress" (async processing) or "completed" (inline processing)
        // The test app uses inline processing, so file is processed before response returns
        assert!(
            body2["status"] == "in_progress" || body2["status"] == "completed",
            "Expected status 'in_progress' or 'completed', got '{}'",
            body2["status"]
        );
        // last_error should be cleared (re-processing was triggered successfully)
        assert!(body2["last_error"].is_null());
    }

    #[tokio::test]
    async fn test_vector_store_file_create_content_dedup() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsf-dedup-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Dedup Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload two files with identical content
        let content = b"Identical content for deduplication test";
        let (content_type1, body1) = create_file_upload_multipart(
            content,
            "file1.txt",
            "organization",
            &org_id,
            Some("assistants"),
        );
        let request1 = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type1)
            .body(Body::from(body1))
            .unwrap();
        let response1 = app.clone().oneshot(request1).await.unwrap();
        assert_eq!(response1.status(), StatusCode::OK);
        let body1 = axum::body::to_bytes(response1.into_body(), usize::MAX)
            .await
            .unwrap();
        let json1: Value = serde_json::from_slice(&body1).unwrap();
        let file1_id = json1["id"].as_str().unwrap();

        let (content_type2, body2) = create_file_upload_multipart(
            content,
            "file2.txt",
            "organization",
            &org_id,
            Some("assistants"),
        );
        let request2 = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type2)
            .body(Body::from(body2))
            .unwrap();
        let response2 = app.clone().oneshot(request2).await.unwrap();
        assert_eq!(response2.status(), StatusCode::OK);
        let body2 = axum::body::to_bytes(response2.into_body(), usize::MAX)
            .await
            .unwrap();
        let json2: Value = serde_json::from_slice(&body2).unwrap();
        let file2_id = json2["id"].as_str().unwrap();

        // File IDs should be different
        assert_ne!(file1_id, file2_id);

        // Add the first file to the vector store
        let (status1, body1) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file1_id}),
        )
        .await;
        assert_eq!(status1, StatusCode::CREATED);
        let vector_store_file_id = body1["id"].as_str().unwrap();

        // Add the second file (same content, same owner) - should detect duplicate
        let (status2, body2) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file2_id}),
        )
        .await;

        // Should return 200 OK with the existing vector store file
        assert_eq!(status2, StatusCode::OK);
        // Note: After model change, id IS the file_id (file- prefix)
        // The returned id should be the original file, not the duplicate
        assert_eq!(body2["id"], vector_store_file_id);
    }

    #[tokio::test]
    async fn test_vector_store_file_create_embedding_model_mismatch() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsf-mismatch-org").await;

        // Create a vector store with a DIFFERENT embedding model than the configured one
        // The test app uses text-embedding-3-small, so use a different model
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Mismatch Test Store",
                "embedding_model": "text-embedding-ada-002",
                "embedding_dimensions": 1536
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file
        let file_id =
            upload_file_for_vector_store(&app, "organization", &org_id, "mismatch.txt").await;

        // Try to add the file - should fail with embedding model mismatch
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "embedding_model_mismatch");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("text-embedding-ada-002")
        );
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("text-embedding-3-small")
        );
    }

    // ============================================================================
    // Vector Store File Delete Tests (DELETE /v1/vector_stores/{id}/files/{file_id})
    // ============================================================================

    #[tokio::test]
    async fn test_vector_store_file_delete_success() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsf-delete-success-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Delete Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file
        let file_id =
            upload_file_for_vector_store(&app, "organization", &org_id, "delete-test.txt").await;

        // Add the file to the vector store
        let (status, _) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // Delete the file from the vector store
        let (status, body) = delete_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files/{}", vs_id, file_id),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], file_id);
        assert_eq!(body["object"], "vector_store.file.deleted");
        assert_eq!(body["deleted"], true);
    }

    #[tokio::test]
    async fn test_vector_store_file_delete_vector_store_not_found() {
        let app = test_app().await;
        let fake_vs_id = format!("vs_{}", uuid::Uuid::new_v4());
        let fake_file_id = format!("file-{}", uuid::Uuid::new_v4());

        let (status, body) = delete_json(
            &app,
            &format!(
                "/api/v1/vector_stores/{}/files/{}",
                fake_vs_id, fake_file_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Vector store")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_delete_file_not_in_store() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsf-delete-not-in-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Delete Not In Store Test"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file but DON'T add it to the vector store
        let file_id =
            upload_file_for_vector_store(&app, "organization", &org_id, "not-in-store.txt").await;

        // Try to delete the file from the vector store (should fail - file not in store)
        let (status, body) = delete_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files/{}", vs_id, file_id),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found in vector store")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_delete_already_deleted() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsf-delete-twice-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Delete Twice Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file and add to vector store
        let file_id =
            upload_file_for_vector_store(&app, "organization", &org_id, "delete-twice.txt").await;
        let (status, _) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // Delete the file (first time - should succeed)
        let (status, _) = delete_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files/{}", vs_id, file_id),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Try to delete again (should fail - already deleted)
        let (status, body) = delete_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files/{}", vs_id, file_id),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
    }

    #[tokio::test]
    async fn test_vector_store_file_delete_preserves_original_file() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsf-delete-preserve-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Delete Preserve Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file and add to vector store
        let file_id =
            upload_file_for_vector_store(&app, "organization", &org_id, "preserve.txt").await;
        let (status, _) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // Delete the file from vector store
        let (status, _) = delete_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files/{}", vs_id, file_id),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Verify the original file still exists in Files API
        let (status, body) = get_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], file_id);
        assert_eq!(body["object"], "file");
    }

    #[tokio::test]
    async fn test_vector_store_file_delete_removes_from_list() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsf-delete-list-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Delete List Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload two files with unique content (to avoid deduplication) and add to vector store
        let file1_id =
            upload_file_with_unique_content(&app, "organization", &org_id, "list-file1.txt").await;
        let file2_id =
            upload_file_with_unique_content(&app, "organization", &org_id, "list-file2.txt").await;

        let (status, _) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file1_id}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        let (status, _) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file2_id}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // Verify both files are in the list
        let (status, body) =
            get_json(&app, &format!("/api/v1/vector_stores/{}/files", vs_id)).await;
        assert_eq!(status, StatusCode::OK);
        let files = body["data"].as_array().unwrap();
        assert_eq!(files.len(), 2);

        // Delete file1 from vector store
        let (status, _) = delete_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files/{}", vs_id, file1_id),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Verify only file2 remains in the list
        let (status, body) =
            get_json(&app, &format!("/api/v1/vector_stores/{}/files", vs_id)).await;
        assert_eq!(status, StatusCode::OK);
        let files = body["data"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        // Note: After model change, id IS the file_id (file- prefix)
        assert_eq!(files[0]["id"], file2_id);
    }

    // ============================================================================
    // Vector Store File Batch Tests (POST /v1/vector_stores/{id}/file_batches)
    // ============================================================================

    #[tokio::test]
    async fn test_vector_store_file_batch_create_vector_store_not_found() {
        let app = test_app_with_file_search().await;
        let fake_vs_id = format!("vs_{}", uuid::Uuid::new_v4());
        let fake_file_id = uuid::Uuid::new_v4();

        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/file_batches", fake_vs_id),
            json!({"file_ids": [fake_file_id]}),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_batch_create_embedding_model_mismatch() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsfb-mismatch-org").await;

        // Create a vector store with a DIFFERENT embedding model than the configured one
        // The test app uses text-embedding-3-small, so use a different model
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Mismatch Batch Test Store",
                "embedding_model": "text-embedding-ada-002",
                "embedding_dimensions": 1536
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file
        let file_id_prefixed =
            upload_file_for_vector_store(&app, "organization", &org_id, "batch-mismatch.txt").await;
        // Strip the "file-" prefix to get raw UUID for the request body
        let file_id = file_id_prefixed.strip_prefix("file-").unwrap();

        // Try to create a file batch - should fail with embedding model mismatch
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/file_batches", vs_id),
            json!({"file_ids": [file_id]}),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"]["code"], "embedding_model_mismatch");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("text-embedding-ada-002")
        );
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("text-embedding-3-small")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_batch_create_service_unavailable() {
        // The default test_app() doesn't configure file_search_service,
        // so validate_embedding_model_compatibility returns 503
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "vsfb-service-unavail-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Test Store for Batch Service Unavailable"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file
        let file_id_prefixed =
            upload_file_for_vector_store(&app, "organization", &org_id, "batch-unavail.txt").await;
        // Strip the "file-" prefix to get raw UUID for the request body
        let file_id = file_id_prefixed.strip_prefix("file-").unwrap();

        // Try to create a file batch
        // This should fail with 503 because file_search_service is not configured
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/file_batches", vs_id),
            json!({"file_ids": [file_id]}),
        )
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"]["code"], "embedding_service_unavailable");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("File search service is not configured")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_batch_create_basic() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsfb-basic-org").await;

        // Create a vector store (uses default embedding model: text-embedding-3-small)
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Basic Batch Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();
        // Response vector_store_id is raw UUID without prefix
        let vs_id_raw = vs_id.strip_prefix("vs_").unwrap();

        // Upload a file
        let file_id_prefixed =
            upload_file_for_vector_store(&app, "organization", &org_id, "batch-basic.txt").await;
        let file_id = file_id_prefixed.strip_prefix("file-").unwrap();

        // Create a file batch
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/file_batches", vs_id),
            json!({"file_ids": [file_id]}),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["object"], "vector_store.file_batch");
        assert_eq!(body["vector_store_id"], vs_id_raw);
        assert_eq!(body["status"], "completed");
        assert!(body["id"].as_str().unwrap().starts_with("vsfb_"));
        assert_eq!(body["file_counts"]["total"], 1);
        assert_eq!(body["file_counts"]["completed"], 1);
        assert_eq!(body["file_counts"]["failed"], 0);
        assert_eq!(body["file_counts"]["in_progress"], 0);
        assert_eq!(body["file_counts"]["cancelled"], 0);
    }

    #[tokio::test]
    async fn test_vector_store_file_batch_create_multiple_files() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsfb-multi-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Multi File Batch Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload multiple files
        let file1_prefixed =
            upload_file_for_vector_store(&app, "organization", &org_id, "batch-multi-1.txt").await;
        let file2_prefixed =
            upload_file_for_vector_store(&app, "organization", &org_id, "batch-multi-2.txt").await;
        let file3_prefixed =
            upload_file_for_vector_store(&app, "organization", &org_id, "batch-multi-3.txt").await;

        let file1_id = file1_prefixed.strip_prefix("file-").unwrap();
        let file2_id = file2_prefixed.strip_prefix("file-").unwrap();
        let file3_id = file3_prefixed.strip_prefix("file-").unwrap();

        // Create a file batch with multiple files
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/file_batches", vs_id),
            json!({"file_ids": [file1_id, file2_id, file3_id]}),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "completed");
        assert_eq!(body["file_counts"]["total"], 3);
        assert_eq!(body["file_counts"]["completed"], 3);
        assert_eq!(body["file_counts"]["failed"], 0);
    }

    #[tokio::test]
    async fn test_vector_store_file_batch_create_with_chunking() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsfb-chunk-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Chunking Batch Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file
        let file_id_prefixed =
            upload_file_for_vector_store(&app, "organization", &org_id, "batch-chunk.txt").await;
        let file_id = file_id_prefixed.strip_prefix("file-").unwrap();

        // Create a file batch with chunking strategy
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/file_batches", vs_id),
            json!({
                "file_ids": [file_id],
                "chunking_strategy": {
                    "type": "static",
                    "static": {
                        "max_chunk_size_tokens": 500,
                        "chunk_overlap_tokens": 100
                    }
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "completed");
        assert_eq!(body["file_counts"]["completed"], 1);
    }

    #[tokio::test]
    async fn test_vector_store_file_batch_create_idempotent() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsfb-idemp-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Idempotent Batch Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file
        let file_id_prefixed =
            upload_file_for_vector_store(&app, "organization", &org_id, "batch-idemp.txt").await;
        let file_id = file_id_prefixed.strip_prefix("file-").unwrap();

        // Add the file individually first
        let (status, _) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id_prefixed}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // Now create a batch with the same file - should still succeed (idempotent)
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/file_batches", vs_id),
            json!({"file_ids": [file_id]}),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "completed");
        // File was already in vector_store, so counts as completed
        assert_eq!(body["file_counts"]["total"], 1);
        assert_eq!(body["file_counts"]["completed"], 1);
        assert_eq!(body["file_counts"]["failed"], 0);
    }

    #[tokio::test]
    async fn test_vector_store_file_batch_create_partial_failure() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsfb-partial-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Partial Failure Batch Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload one real file
        let file1_prefixed =
            upload_file_for_vector_store(&app, "organization", &org_id, "batch-partial.txt").await;
        let file1_id = file1_prefixed.strip_prefix("file-").unwrap();

        // Use a fake file ID that doesn't exist
        let fake_file_id = uuid::Uuid::new_v4();

        // Create a batch with one real file and one fake file
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/file_batches", vs_id),
            json!({"file_ids": [file1_id, fake_file_id]}),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        // Status is "failed" because at least one file failed
        assert_eq!(body["status"], "failed");
        assert_eq!(body["file_counts"]["total"], 2);
        assert_eq!(body["file_counts"]["completed"], 1);
        assert_eq!(body["file_counts"]["failed"], 1);
    }

    #[tokio::test]
    async fn test_vector_store_file_batch_create_empty() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "vsfb-empty-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Empty Batch Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Create a batch with no files
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/file_batches", vs_id),
            json!({"file_ids": []}),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["status"], "completed");
        assert_eq!(body["file_counts"]["total"], 0);
        assert_eq!(body["file_counts"]["completed"], 0);
        assert_eq!(body["file_counts"]["failed"], 0);
    }

    // Vector Store File Batch Stub Endpoint Tests
    // These endpoints return errors because file batches are executed synchronously
    // and not persisted. The batch ID returned from create is for reference only.

    #[tokio::test]
    async fn test_vector_store_file_batch_get_not_persisted() {
        let app = test_app().await;
        let fake_vs_id = uuid::Uuid::new_v4();
        let fake_batch_id = "vsfb_12345";

        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/vector_stores/vs_{}/file_batches/{}",
                fake_vs_id, fake_batch_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not persisted")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_batch_cancel_not_supported() {
        let app = test_app().await;
        let fake_vs_id = uuid::Uuid::new_v4();
        let fake_batch_id = "vsfb_12345";

        let (status, body) = delete_json(
            &app,
            &format!(
                "/api/v1/vector_stores/vs_{}/file_batches/{}",
                fake_vs_id, fake_batch_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "not_supported");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("cannot be cancelled")
        );
    }

    #[tokio::test]
    async fn test_vector_store_file_batch_list_files_not_persisted() {
        let app = test_app().await;
        let fake_vs_id = uuid::Uuid::new_v4();
        let fake_batch_id = "vsfb_12345";

        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/vector_stores/vs_{}/file_batches/{}/files",
                fake_vs_id, fake_batch_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not persisted")
        );
    }

    // ============================================================================
    // Vector Store Search Tests (POST /v1/vector_stores/{id}/search)
    // ============================================================================

    #[tokio::test]
    async fn test_vector_store_search_vector_store_not_found() {
        let app = test_app_with_file_search().await;
        let fake_vs_id = uuid::Uuid::new_v4();

        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/vs_{}/search", fake_vs_id),
            json!({
                "query": "test query"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_vector_store_search_file_search_not_configured() {
        // Use test_app() which does NOT have file_search_service configured
        let app = test_app().await;
        let org_id = create_org_for_vector_store(&app, "search-no-fs-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Try to search - should fail because file_search_service is not configured
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"]["code"], "not_configured");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("File search is not configured")
        );
    }

    #[tokio::test]
    async fn test_vector_store_search_invalid_score_threshold_too_high() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-threshold-high-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Search with score_threshold > 1.0
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query",
                "ranking_options": {
                    "score_threshold": 1.5
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_parameter");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("score_threshold must be between 0.0 and 1.0")
        );
    }

    #[tokio::test]
    async fn test_vector_store_search_invalid_score_threshold_negative() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-threshold-neg-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Search with score_threshold < 0.0
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query",
                "ranking_options": {
                    "score_threshold": -0.5
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_parameter");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("score_threshold must be between 0.0 and 1.0")
        );
    }

    #[tokio::test]
    async fn test_vector_store_search_basic_empty_results() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-empty-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Search - should return empty results (TestVectorStore returns empty)
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "vector_store.search_results");
        assert_eq!(body["query"], "test query");
        assert!(body["data"].is_array());
        assert!(body["data"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_vector_store_search_with_max_num_results() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-max-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Search with max_num_results
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query",
                "max_num_results": 5
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "vector_store.search_results");
        assert_eq!(body["query"], "test query");
    }

    #[tokio::test]
    async fn test_vector_store_search_with_ranking_options() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-ranking-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Search with ranking options
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query",
                "ranking_options": {
                    "ranker": "vector",
                    "score_threshold": 0.5
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "vector_store.search_results");
    }

    #[tokio::test]
    async fn test_vector_store_search_with_filters() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-filters-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Search with filters
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query",
                "filters": {
                    "type": "eq",
                    "key": "category",
                    "value": "documentation"
                }
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "vector_store.search_results");
    }

    // Vector Store Search Tests with Mock Results
    // These tests use MockableTestVectorStore to inject mock search results

    #[tokio::test]
    async fn test_vector_store_search_returns_results() {
        let (app, _db, mock_handle) = test_app_with_mockable_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-results-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();
        // Extract the UUID from vs_XXX format
        let vs_uuid = uuid::Uuid::parse_str(&vs_id[3..]).unwrap();

        let chunk_id = uuid::Uuid::new_v4();
        let file_id = uuid::Uuid::new_v4();

        // Set up mock search results
        *mock_handle.lock().unwrap() = vec![crate::cache::vector_store::ChunkSearchResult {
            chunk_id,
            vector_store_id: vs_uuid,
            file_id,
            chunk_index: 0,
            content: "This is the matching content from the document.".to_string(),
            score: 0.95,
            metadata: Some(serde_json::json!({"source": "test.pdf"})),
        }];

        // Search
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "matching content"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "vector_store.search_results");
        assert_eq!(body["query"], "matching content");

        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);

        let result = &data[0];
        assert_eq!(result["object"], "vector_store.search_result");
        assert!(result["chunk_id"].as_str().unwrap().starts_with("chunk_"));
        assert_eq!(
            result["vector_store_id"].as_str().unwrap(),
            format!("vs_{}", vs_uuid)
        );
        assert!(result["file_id"].as_str().unwrap().starts_with("file-"));
        assert_eq!(result["chunk_index"], 0);
        assert_eq!(
            result["content"],
            "This is the matching content from the document."
        );
        assert_eq!(result["score"], 0.95);
        assert_eq!(result["metadata"]["source"], "test.pdf");
    }

    #[tokio::test]
    async fn test_vector_store_search_multiple_results() {
        let (app, _db, mock_handle) = test_app_with_mockable_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-multi-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();
        let vs_uuid = uuid::Uuid::parse_str(&vs_id[3..]).unwrap();

        let file_id = uuid::Uuid::new_v4();

        // Set up multiple mock search results
        *mock_handle.lock().unwrap() = vec![
            crate::cache::vector_store::ChunkSearchResult {
                chunk_id: uuid::Uuid::new_v4(),
                vector_store_id: vs_uuid,
                file_id,
                chunk_index: 0,
                content: "First result with highest score.".to_string(),
                score: 0.98,
                metadata: None,
            },
            crate::cache::vector_store::ChunkSearchResult {
                chunk_id: uuid::Uuid::new_v4(),
                vector_store_id: vs_uuid,
                file_id,
                chunk_index: 1,
                content: "Second result with medium score.".to_string(),
                score: 0.85,
                metadata: None,
            },
            crate::cache::vector_store::ChunkSearchResult {
                chunk_id: uuid::Uuid::new_v4(),
                vector_store_id: vs_uuid,
                file_id,
                chunk_index: 2,
                content: "Third result with lower score.".to_string(),
                score: 0.72,
                metadata: None,
            },
        ];

        // Search
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 3);

        // Verify order and scores
        assert_eq!(data[0]["score"], 0.98);
        assert_eq!(data[0]["chunk_index"], 0);
        assert_eq!(data[1]["score"], 0.85);
        assert_eq!(data[1]["chunk_index"], 1);
        assert_eq!(data[2]["score"], 0.72);
        assert_eq!(data[2]["chunk_index"], 2);
    }

    #[tokio::test]
    async fn test_vector_store_search_respects_max_num_results() {
        let (app, _db, mock_handle) = test_app_with_mockable_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-limit-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();
        let vs_uuid = uuid::Uuid::parse_str(&vs_id[3..]).unwrap();

        let file_id = uuid::Uuid::new_v4();

        // Set up more results than we'll request
        *mock_handle.lock().unwrap() = (0..10)
            .map(|i| crate::cache::vector_store::ChunkSearchResult {
                chunk_id: uuid::Uuid::new_v4(),
                vector_store_id: vs_uuid,
                file_id,
                chunk_index: i,
                content: format!("Result {}", i),
                score: 0.9 - (i as f64 * 0.05),
                metadata: None,
            })
            .collect();

        // Request only 3 results
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query",
                "max_num_results": 3
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 3);
    }

    #[tokio::test]
    async fn test_vector_store_search_with_metadata() {
        let (app, _db, mock_handle) = test_app_with_mockable_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-meta-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();
        let vs_uuid = uuid::Uuid::parse_str(&vs_id[3..]).unwrap();

        // Set up result with rich metadata
        *mock_handle.lock().unwrap() = vec![crate::cache::vector_store::ChunkSearchResult {
            chunk_id: uuid::Uuid::new_v4(),
            vector_store_id: vs_uuid,
            file_id: uuid::Uuid::new_v4(),
            chunk_index: 0,
            content: "Content with metadata".to_string(),
            score: 0.9,
            metadata: Some(serde_json::json!({
                "category": "documentation",
                "author": "test-author",
                "page": 42,
                "tags": ["api", "reference"]
            })),
        }];

        // Search
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);

        let metadata = &data[0]["metadata"];
        assert_eq!(metadata["category"], "documentation");
        assert_eq!(metadata["author"], "test-author");
        assert_eq!(metadata["page"], 42);
        assert!(metadata["tags"].as_array().unwrap().contains(&json!("api")));
    }

    #[tokio::test]
    async fn test_vector_store_search_without_metadata() {
        let (app, _db, mock_handle) = test_app_with_mockable_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-no-meta-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();
        let vs_uuid = uuid::Uuid::parse_str(&vs_id[3..]).unwrap();

        // Set up result without metadata
        *mock_handle.lock().unwrap() = vec![crate::cache::vector_store::ChunkSearchResult {
            chunk_id: uuid::Uuid::new_v4(),
            vector_store_id: vs_uuid,
            file_id: uuid::Uuid::new_v4(),
            chunk_index: 0,
            content: "Content without metadata".to_string(),
            score: 0.9,
            metadata: None,
        }];

        // Search
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);

        // metadata should be omitted when None (not present in JSON)
        assert!(data[0].get("metadata").is_none() || data[0]["metadata"].is_null());
    }

    #[tokio::test]
    async fn test_vector_store_search_id_prefixes() {
        let (app, _db, mock_handle) = test_app_with_mockable_file_search().await;
        let org_id = create_org_for_vector_store(&app, "search-prefix-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();
        let vs_uuid = uuid::Uuid::parse_str(&vs_id[3..]).unwrap();

        let chunk_uuid = uuid::Uuid::new_v4();
        let file_uuid = uuid::Uuid::new_v4();

        // Set up result
        *mock_handle.lock().unwrap() = vec![crate::cache::vector_store::ChunkSearchResult {
            chunk_id: chunk_uuid,
            vector_store_id: vs_uuid,
            file_id: file_uuid,
            chunk_index: 5,
            content: "Test content".to_string(),
            score: 0.88,
            metadata: None,
        }];

        // Search
        let (status, body) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/search", vs_id),
            json!({
                "query": "test query"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let result = &body["data"][0];

        // Verify ID prefixes are correctly applied
        assert_eq!(
            result["chunk_id"].as_str().unwrap(),
            format!("chunk_{}", chunk_uuid)
        );
        assert_eq!(
            result["vector_store_id"].as_str().unwrap(),
            format!("vs_{}", vs_uuid)
        );
        assert_eq!(
            result["file_id"].as_str().unwrap(),
            format!("file-{}", file_uuid)
        );
    }

    // ============================================================================
    // Files List API Tests (GET /v1/files)
    // ============================================================================

    /// Helper to upload a file and return its ID (for file list tests)
    async fn upload_file_for_list(
        app: &axum::Router,
        owner_type: &str,
        owner_id: &str,
        filename: &str,
        purpose: Option<&str>,
    ) -> String {
        let content = format!("Content for {}", filename);
        let (content_type, body) = create_file_upload_multipart(
            content.as_bytes(),
            filename,
            owner_type,
            owner_id,
            purpose,
        );

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();
        json["id"].as_str().unwrap().to_string()
    }

    /// Helper to upload a file with specific content and return its ID (for content download tests)
    async fn upload_file_with_content(
        app: &axum::Router,
        owner_type: &str,
        owner_id: &str,
        filename: &str,
        content: &[u8],
    ) -> String {
        let (content_type, body) =
            create_file_upload_multipart(content, filename, owner_type, owner_id, None);

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/files")
            .header("content-type", content_type)
            .body(Body::from(body))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap();
        json["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn test_file_list_empty() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-empty-user").await;

        let (status, body) = get_json(
            &app,
            &format!("/api/v1/files?owner_type=user&owner_id={}", owner_id),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "list");
        assert!(body["data"].as_array().unwrap().is_empty());
        assert_eq!(body["has_more"], false);
        assert!(body["first_id"].is_null());
        assert!(body["last_id"].is_null());
    }

    #[tokio::test]
    async fn test_file_list_with_files() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-with-files-user").await;

        // Upload two files
        let file1_id = upload_file_for_list(&app, "user", &owner_id, "document1.txt", None).await;
        let file2_id = upload_file_for_list(&app, "user", &owner_id, "document2.txt", None).await;

        let (status, body) = get_json(
            &app,
            &format!("/api/v1/files?owner_type=user&owner_id={}", owner_id),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "list");

        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);

        // Default order is desc, so file2 should be first
        assert_eq!(data[0]["id"], file2_id);
        assert_eq!(data[1]["id"], file1_id);

        assert_eq!(body["has_more"], false);
        assert_eq!(body["first_id"], file2_id);
        assert_eq!(body["last_id"], file1_id);
    }

    #[tokio::test]
    async fn test_file_list_with_limit() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-limit-user").await;

        // Upload three files
        let _file1_id = upload_file_for_list(&app, "user", &owner_id, "doc1.txt", None).await;
        let file2_id = upload_file_for_list(&app, "user", &owner_id, "doc2.txt", None).await;
        let file3_id = upload_file_for_list(&app, "user", &owner_id, "doc3.txt", None).await;

        // Request with limit=2
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&limit=2",
                owner_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(body["has_more"], true);

        // Default order is desc, so file3 and file2 should be returned
        assert_eq!(data[0]["id"], file3_id);
        assert_eq!(data[1]["id"], file2_id);
    }

    #[tokio::test]
    async fn test_file_list_pagination_after() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-after-user").await;

        // Upload three files
        let file1_id = upload_file_for_list(&app, "user", &owner_id, "doc1.txt", None).await;
        let file2_id = upload_file_for_list(&app, "user", &owner_id, "doc2.txt", None).await;
        let file3_id = upload_file_for_list(&app, "user", &owner_id, "doc3.txt", None).await;

        // Get files after file3 (most recent in desc order)
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&after={}",
                owner_id, file3_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0]["id"], file2_id);
        assert_eq!(data[1]["id"], file1_id);
        assert_eq!(body["has_more"], false);
    }

    #[tokio::test]
    async fn test_file_list_pagination_before() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-before-user").await;

        // Upload three files
        let file1_id = upload_file_for_list(&app, "user", &owner_id, "doc1.txt", None).await;
        let file2_id = upload_file_for_list(&app, "user", &owner_id, "doc2.txt", None).await;
        let file3_id = upload_file_for_list(&app, "user", &owner_id, "doc3.txt", None).await;

        // Get files before file1 (oldest in desc order)
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&before={}",
                owner_id, file1_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 2);
        // Before cursor returns items in same order direction
        assert_eq!(data[0]["id"], file3_id);
        assert_eq!(data[1]["id"], file2_id);
        assert_eq!(body["has_more"], false);
    }

    #[tokio::test]
    async fn test_file_list_filter_by_purpose() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-purpose-user").await;

        // Upload files with different purposes
        let _assistants_file =
            upload_file_for_list(&app, "user", &owner_id, "assistant.txt", Some("assistants"))
                .await;
        let batch_file =
            upload_file_for_list(&app, "user", &owner_id, "batch.jsonl", Some("batch")).await;
        let _fine_tune_file =
            upload_file_for_list(&app, "user", &owner_id, "train.jsonl", Some("fine-tune")).await;

        // Filter by batch purpose
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&purpose=batch",
                owner_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["id"], batch_file);
        assert_eq!(data[0]["purpose"], "batch");
    }

    #[tokio::test]
    async fn test_file_list_order_asc() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-asc-user").await;

        // Upload three files
        let file1_id = upload_file_for_list(&app, "user", &owner_id, "doc1.txt", None).await;
        let file2_id = upload_file_for_list(&app, "user", &owner_id, "doc2.txt", None).await;
        let file3_id = upload_file_for_list(&app, "user", &owner_id, "doc3.txt", None).await;

        // Request with ascending order
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&order=asc",
                owner_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 3);
        // Ascending order: oldest first
        assert_eq!(data[0]["id"], file1_id);
        assert_eq!(data[1]["id"], file2_id);
        assert_eq!(data[2]["id"], file3_id);
    }

    #[tokio::test]
    async fn test_file_list_order_desc() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-desc-user").await;

        // Upload three files
        let file1_id = upload_file_for_list(&app, "user", &owner_id, "doc1.txt", None).await;
        let file2_id = upload_file_for_list(&app, "user", &owner_id, "doc2.txt", None).await;
        let file3_id = upload_file_for_list(&app, "user", &owner_id, "doc3.txt", None).await;

        // Request with descending order (explicit)
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&order=desc",
                owner_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 3);
        // Descending order: newest first
        assert_eq!(data[0]["id"], file3_id);
        assert_eq!(data[1]["id"], file2_id);
        assert_eq!(data[2]["id"], file1_id);
    }

    #[tokio::test]
    async fn test_file_list_organization_owner() {
        let app = test_app().await;
        let org_id = create_org_for_files(&app, "file-list-org").await;

        // Upload file to organization
        let file_id =
            upload_file_for_list(&app, "organization", &org_id, "org-doc.txt", None).await;

        let (status, body) = get_json(
            &app,
            &format!("/api/v1/files?owner_type=organization&owner_id={}", org_id),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["id"], file_id);
    }

    #[tokio::test]
    async fn test_file_list_project_owner() {
        let app = test_app().await;
        let org_slug = "file-list-proj-org";
        let _org_id = create_org_for_files(&app, org_slug).await;
        let project_id = create_project_for_files(&app, org_slug, "file-list-project").await;

        // Upload file to project
        let file_id =
            upload_file_for_list(&app, "project", &project_id, "project-doc.txt", None).await;

        let (status, body) = get_json(
            &app,
            &format!("/api/v1/files?owner_type=project&owner_id={}", project_id),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["id"], file_id);
    }

    #[tokio::test]
    async fn test_file_list_team_owner() {
        let app = test_app().await;
        let org_slug = "file-list-team-org";
        let _org_id = create_org_for_files(&app, org_slug).await;
        let team_id = create_team_for_files(&app, org_slug, "file-list-team").await;

        // Upload file to team
        let file_id = upload_file_for_list(&app, "team", &team_id, "team-doc.txt", None).await;

        let (status, body) = get_json(
            &app,
            &format!("/api/v1/files?owner_type=team&owner_id={}", team_id),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["id"], file_id);
    }

    #[tokio::test]
    async fn test_file_list_invalid_owner_type() {
        let app = test_app().await;

        let (status, body) = get_json(
            &app,
            "/api/v1/files?owner_type=invalid&owner_id=00000000-0000-0000-0000-000000000000",
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_owner_type");
    }

    #[tokio::test]
    async fn test_file_list_invalid_owner_id() {
        let app = test_app().await;

        let (status, _body) =
            get_json(&app, "/api/v1/files?owner_type=user&owner_id=not-a-uuid").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_file_list_missing_owner_type() {
        let app = test_app().await;

        let (status, _body) = get_json(
            &app,
            "/api/v1/files?owner_id=00000000-0000-0000-0000-000000000000",
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_file_list_missing_owner_id() {
        let app = test_app().await;

        let (status, _body) = get_json(&app, "/api/v1/files?owner_type=user").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_file_list_invalid_after_cursor_format() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-invalid-after-user").await;

        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&after=not-a-valid-file-id",
                owner_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_cursor");
    }

    #[tokio::test]
    async fn test_file_list_invalid_before_cursor_format() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-invalid-before-user").await;

        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&before=invalid-cursor",
                owner_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_cursor");
    }

    #[tokio::test]
    async fn test_file_list_after_cursor_not_found() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-after-notfound-user").await;

        // Use a valid file ID format but non-existent file
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&after=file-00000000-0000-0000-0000-000000000000",
                owner_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_cursor");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_file_list_before_cursor_not_found() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-before-notfound-user").await;

        // Use a valid file ID format but non-existent file
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&before=file-00000000-0000-0000-0000-000000000000",
                owner_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_cursor");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_file_list_invalid_purpose() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-invalid-purpose-user").await;

        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&purpose=invalid-purpose",
                owner_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_purpose");
    }

    #[tokio::test]
    async fn test_file_list_limit_capped_at_100() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-limit-cap-user").await;

        // Upload one file
        let file_id = upload_file_for_list(&app, "user", &owner_id, "doc.txt", None).await;

        // Request with limit > 100 (should be capped)
        let (status, body) = get_json(
            &app,
            &format!(
                "/api/v1/files?owner_type=user&owner_id={}&limit=500",
                owner_id
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0]["id"], file_id);
    }

    #[tokio::test]
    async fn test_file_list_validates_file_metadata() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-list-metadata-user").await;

        // Upload a file
        let _file_id =
            upload_file_for_list(&app, "user", &owner_id, "metadata-test.txt", None).await;

        let (status, body) = get_json(
            &app,
            &format!("/api/v1/files?owner_type=user&owner_id={}", owner_id),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);

        let file = &data[0];
        assert_eq!(file["object"], "file");
        assert!(file["id"].as_str().unwrap().starts_with("file-"));
        assert_eq!(file["filename"], "metadata-test.txt");
        assert_eq!(file["purpose"], "assistants"); // Default purpose
        assert!(file["bytes"].is_number());
        assert!(file["created_at"].is_string());
    }

    // ============================================================================
    // File Get (GET /v1/files/{file_id})
    // ============================================================================

    #[tokio::test]
    async fn test_file_get_basic() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-get-basic-user").await;

        // Upload a file first
        let file_id = upload_file_for_list(&app, "user", &owner_id, "get-test.txt", None).await;

        // GET the file by ID
        let (status, body) = get_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["object"], "file");
        assert_eq!(body["id"], file_id);
        assert_eq!(body["filename"], "get-test.txt");
        assert_eq!(body["purpose"], "assistants");
        assert!(body["bytes"].is_number());
        assert!(body["created_at"].is_string());
        assert_eq!(body["owner_type"], "user");
        assert_eq!(body["owner_id"], owner_id);
    }

    #[tokio::test]
    async fn test_file_get_not_found() {
        let app = test_app().await;

        // Try to GET a non-existent file
        let non_existent_id = "file-00000000-0000-0000-0000-000000000000";
        let (status, body) = get_json(&app, &format!("/api/v1/files/{}", non_existent_id)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_file_get_invalid_id_format() {
        let app = test_app().await;

        // Try to GET with an invalid file ID format
        let (status, _body) = get_json(&app, "/api/v1/files/not-a-valid-uuid").await;

        // Invalid path parameter format returns 400 (Axum's default path rejection)
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_file_get_with_purpose() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-get-purpose-user").await;

        // Upload a file with a specific purpose
        let file_id =
            upload_file_for_list(&app, "user", &owner_id, "batch-file.jsonl", Some("batch")).await;

        // GET the file and verify purpose is preserved
        let (status, body) = get_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["purpose"], "batch");
        assert_eq!(body["filename"], "batch-file.jsonl");
    }

    #[tokio::test]
    async fn test_file_get_organization_owner() {
        let app = test_app().await;
        let org_id = create_org_for_files(&app, "file-get-org").await;

        // Upload a file owned by organization
        let file_id =
            upload_file_for_list(&app, "organization", &org_id, "org-file.txt", None).await;

        // GET the file and verify owner info
        let (status, body) = get_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["owner_type"], "organization");
        assert_eq!(body["owner_id"], org_id);
    }

    #[tokio::test]
    async fn test_file_get_project_owner() {
        let app = test_app().await;
        let _org_id = create_org_for_files(&app, "file-get-proj-org").await;
        let project_id = create_project_for_files(&app, "file-get-proj-org", "file-get-proj").await;

        // Upload a file owned by project
        let file_id =
            upload_file_for_list(&app, "project", &project_id, "project-file.txt", None).await;

        // GET the file and verify owner info
        let (status, body) = get_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["owner_type"], "project");
        assert_eq!(body["owner_id"], project_id);
    }

    #[tokio::test]
    async fn test_file_get_team_owner() {
        let app = test_app().await;
        let _org_id = create_org_for_files(&app, "file-get-team-org").await;
        let team_id = create_team_for_files(&app, "file-get-team-org", "file-get-team").await;

        // Upload a file owned by team
        let file_id = upload_file_for_list(&app, "team", &team_id, "team-file.txt", None).await;

        // GET the file and verify owner info
        let (status, body) = get_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["owner_type"], "team");
        assert_eq!(body["owner_id"], team_id);
    }

    #[tokio::test]
    async fn test_file_get_validates_all_response_fields() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-get-fields-user").await;

        // Upload a file
        let file_id = upload_file_for_list(&app, "user", &owner_id, "fields-test.txt", None).await;

        // GET the file
        let (status, body) = get_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::OK);

        // Validate all expected fields are present
        assert!(body["id"].is_string(), "id should be a string");
        assert!(body["object"].is_string(), "object should be a string");
        assert!(body["filename"].is_string(), "filename should be a string");
        assert!(body["purpose"].is_string(), "purpose should be a string");
        assert!(body["bytes"].is_number(), "bytes should be a number");
        assert!(
            body["created_at"].is_string(),
            "created_at should be a string"
        );
        assert!(
            body["owner_type"].is_string(),
            "owner_type should be a string"
        );
        assert!(body["owner_id"].is_string(), "owner_id should be a string");
        assert!(body["status"].is_string(), "status should be a string");

        // Verify specific values
        assert_eq!(body["object"], "file");
        assert_eq!(body["status"], "uploaded"); // Default status after upload
    }

    // ============================================================================
    // File Content Download Tests
    // ============================================================================

    #[tokio::test]
    async fn test_file_content_download_basic() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-content-basic-user").await;

        // Upload a file with known content
        let expected_content = b"Hello, this is test file content for download!";
        let file_id = upload_file_with_content(
            &app,
            "user",
            &owner_id,
            "download-test.txt",
            expected_content,
        )
        .await;

        // Download the content
        let (status, headers, body) =
            get_raw(&app, &format!("/api/v1/files/{}/content", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, expected_content);

        // Verify headers are present
        assert!(headers.contains_key("content-type"));
        assert!(headers.contains_key("content-disposition"));
    }

    #[tokio::test]
    async fn test_file_content_download_not_found() {
        let app = test_app().await;

        // Try to download content for non-existent file
        let non_existent_id = "file-00000000-0000-0000-0000-000000000000";
        let (status, _headers, body) =
            get_raw(&app, &format!("/api/v1/files/{}/content", non_existent_id)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "not_found");
    }

    #[tokio::test]
    async fn test_file_content_download_invalid_id_format() {
        let app = test_app().await;

        // Try to download with invalid file ID format
        let (status, _headers, _body) =
            get_raw(&app, "/api/v1/files/not-a-valid-uuid/content").await;

        // Invalid path parameter format returns 400 (Axum's default path rejection)
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_file_content_download_content_type_header() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-content-type-user").await;

        // Upload a text file
        let file_id =
            upload_file_with_content(&app, "user", &owner_id, "test.txt", b"text content").await;

        let (status, headers, _body) =
            get_raw(&app, &format!("/api/v1/files/{}/content", file_id)).await;

        assert_eq!(status, StatusCode::OK);

        // Content-Type should default to application/octet-stream (since we upload as binary)
        let content_type = headers
            .get("content-type")
            .expect("content-type header should be present")
            .to_str()
            .unwrap();
        assert_eq!(content_type, "application/octet-stream");
    }

    #[tokio::test]
    async fn test_file_content_download_content_disposition_header() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-content-disp-user").await;

        // Upload a file with a specific filename
        let file_id =
            upload_file_with_content(&app, "user", &owner_id, "my-document.pdf", b"PDF content")
                .await;

        let (status, headers, _body) =
            get_raw(&app, &format!("/api/v1/files/{}/content", file_id)).await;

        assert_eq!(status, StatusCode::OK);

        // Content-Disposition should include the filename
        let disposition = headers
            .get("content-disposition")
            .expect("content-disposition header should be present")
            .to_str()
            .unwrap();
        assert_eq!(disposition, "attachment; filename=\"my-document.pdf\"");
    }

    #[tokio::test]
    async fn test_file_content_download_binary_content() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-content-binary-user").await;

        // Upload binary content (non-UTF8) - use .png extension since .bin is not allowed
        let binary_content: Vec<u8> = (0..=255).collect();
        let file_id =
            upload_file_with_content(&app, "user", &owner_id, "binary.png", &binary_content).await;

        let (status, _headers, body) =
            get_raw(&app, &format!("/api/v1/files/{}/content", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, binary_content);
    }

    #[tokio::test]
    async fn test_file_content_download_unicode_filename() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-content-unicode-user").await;

        // Upload a file with unicode filename
        let file_id = upload_file_with_content(
            &app,
            "user",
            &owner_id,
            "文档-日本語-émojis-🎉.txt",
            b"Unicode filename test",
        )
        .await;

        let (status, headers, body) =
            get_raw(&app, &format!("/api/v1/files/{}/content", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"Unicode filename test");

        // Content-Disposition header contains unicode - check using raw bytes
        let disposition = headers
            .get("content-disposition")
            .expect("content-disposition header should be present");
        // Convert to bytes and check for presence of expected filename
        let disposition_bytes = disposition.as_bytes();
        assert!(disposition_bytes.starts_with(b"attachment; filename=\""));
        // The unicode filename should be present in the header value
        let expected_filename = "文档-日本語-émojis-🎉.txt".as_bytes();
        assert!(
            disposition_bytes
                .windows(expected_filename.len())
                .any(|window| window == expected_filename),
            "Content-Disposition should contain the unicode filename"
        );
    }

    #[tokio::test]
    async fn test_file_content_download_organization_owner() {
        let app = test_app().await;
        let org_id = create_org_for_files(&app, "file-content-org").await;

        // Upload a file owned by organization
        let file_id = upload_file_with_content(
            &app,
            "organization",
            &org_id,
            "org-file.txt",
            b"Org content",
        )
        .await;

        let (status, _headers, body) =
            get_raw(&app, &format!("/api/v1/files/{}/content", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"Org content");
    }

    #[tokio::test]
    async fn test_file_content_download_empty_file() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-content-empty-user").await;

        // Upload an empty file
        let file_id = upload_file_with_content(&app, "user", &owner_id, "empty.txt", b"").await;

        let (status, _headers, body) =
            get_raw(&app, &format!("/api/v1/files/{}/content", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.is_empty());
    }

    // ============================================================================
    // File Delete Tests
    // ============================================================================

    #[tokio::test]
    async fn test_file_delete_basic() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-delete-basic-user").await;

        // Upload a file
        let file_id = upload_file_for_list(&app, "user", &owner_id, "delete-me.txt", None).await;

        // Delete the file
        let (status, body) = delete_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], file_id);
        assert_eq!(body["object"], "file");
        assert_eq!(body["deleted"], true);
    }

    #[tokio::test]
    async fn test_file_delete_not_found() {
        let app = test_app().await;
        let fake_id = format!("file-{}", uuid::Uuid::new_v4());

        let (status, body) = delete_json(&app, &format!("/api/v1/files/{}", fake_id)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_file_delete_invalid_id_format() {
        let app = test_app().await;

        let (status, _body) = delete_json(&app, "/api/v1/files/not-a-valid-uuid").await;

        // Invalid UUID format returns 400 (bad request due to path parsing)
        // Axum path rejection may not include a JSON body
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_file_delete_file_in_use() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "file-delete-in-use-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "File In Use Test Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file and add it to the vector store
        let file_id =
            upload_file_for_vector_store(&app, "organization", &org_id, "in-use-file.txt").await;
        let (status, _) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // Try to delete the file (should fail - file is in use)
        let (status, body) = delete_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "file_in_use");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("referenced")
        );
    }

    #[tokio::test]
    async fn test_file_delete_after_removing_from_vector_store() {
        let app = test_app_with_file_search().await;
        let org_id = create_org_for_vector_store(&app, "file-delete-after-remove-org").await;

        // Create a vector store
        let (status, vs) = post_json(
            &app,
            "/api/v1/vector_stores",
            json!({
                "owner": {"type": "organization", "organization_id": org_id},
                "name": "Remove Then Delete Store"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let vs_id = vs["id"].as_str().unwrap();

        // Upload a file and add it to the vector store
        let file_id =
            upload_file_for_vector_store(&app, "organization", &org_id, "remove-then-delete.txt")
                .await;
        let (status, _) = post_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files", vs_id),
            json!({"file_id": file_id}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // Remove the file from the vector store
        let (status, _) = delete_json(
            &app,
            &format!("/api/v1/vector_stores/{}/files/{}", vs_id, file_id),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Now delete the file (should succeed)
        let (status, body) = delete_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["id"], file_id);
        assert_eq!(body["object"], "file");
        assert_eq!(body["deleted"], true);
    }

    #[tokio::test]
    async fn test_file_delete_verify_actually_deleted() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-delete-verify-user").await;

        // Upload a file
        let file_id =
            upload_file_for_list(&app, "user", &owner_id, "verify-delete.txt", None).await;

        // Verify file exists
        let (status, _) = get_json(&app, &format!("/api/v1/files/{}", file_id)).await;
        assert_eq!(status, StatusCode::OK);

        // Delete the file
        let (status, _) = delete_json(&app, &format!("/api/v1/files/{}", file_id)).await;
        assert_eq!(status, StatusCode::OK);

        // Verify file no longer exists
        let (status, body) = get_json(&app, &format!("/api/v1/files/{}", file_id)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
    }

    #[tokio::test]
    async fn test_file_delete_organization_owner() {
        let app = test_app().await;
        let org_id = create_org_for_files(&app, "file-delete-org-owner").await;

        // Upload a file owned by the organization
        let file_id =
            upload_file_for_list(&app, "organization", &org_id, "org-delete.txt", None).await;

        // Delete the file
        let (status, body) = delete_json(&app, &format!("/api/v1/files/{}", file_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["deleted"], true);
    }

    #[tokio::test]
    async fn test_file_delete_double_delete() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-delete-double-user").await;

        // Upload a file
        let file_id =
            upload_file_for_list(&app, "user", &owner_id, "double-delete.txt", None).await;

        // Delete the file (first time - should succeed)
        let (status, _) = delete_json(&app, &format!("/api/v1/files/{}", file_id)).await;
        assert_eq!(status, StatusCode::OK);

        // Try to delete again (should fail - file no longer exists)
        let (status, body) = delete_json(&app, &format!("/api/v1/files/{}", file_id)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
    }

    #[tokio::test]
    async fn test_file_delete_content_not_accessible() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-delete-content-user").await;

        // Upload a file with specific content
        let file_id = upload_file_with_content(
            &app,
            "user",
            &owner_id,
            "content-delete.txt",
            b"secret data",
        )
        .await;

        // Delete the file
        let (status, _) = delete_json(&app, &format!("/api/v1/files/{}", file_id)).await;
        assert_eq!(status, StatusCode::OK);

        // Verify content is not accessible
        let (status, _headers, _body) =
            get_raw(&app, &format!("/api/v1/files/{}/content", file_id)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_file_delete_not_in_list() {
        let app = test_app().await;
        let owner_id = create_user_for_files(&app, "file-delete-list-user").await;

        // Upload two files
        let file1_id = upload_file_for_list(&app, "user", &owner_id, "keep-me.txt", None).await;
        let file2_id = upload_file_for_list(&app, "user", &owner_id, "delete-me.txt", None).await;

        // Delete the second file
        let (status, _) = delete_json(&app, &format!("/api/v1/files/{}", file2_id)).await;
        assert_eq!(status, StatusCode::OK);

        // List files - should only contain the first file
        let (status, body) = get_json(
            &app,
            &format!("/api/v1/files?owner_type=user&owner_id={}", owner_id),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let files = body["data"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["id"], file1_id);
    }

    // ============================================================================
    // Image Generation Tests
    // ============================================================================

    #[tokio::test]
    async fn test_image_generation_basic() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "A cute baby sea otter",
                "model": "test/test-model"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["created"].is_number());
        assert!(body["data"].is_array());

        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 1);
        assert!(data[0]["url"].is_string());
        assert!(data[0]["revised_prompt"].is_string());
    }

    #[tokio::test]
    async fn test_image_generation_multiple_images() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "A sunset over mountains",
                "model": "test/test-model",
                "n": 3
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let data = body["data"].as_array().unwrap();
        assert_eq!(data.len(), 3);

        // Each image should have a unique URL
        let urls: Vec<&str> = data
            .iter()
            .map(|img| img["url"].as_str().unwrap())
            .collect();
        assert!(urls[0] != urls[1] && urls[1] != urls[2]);
    }

    #[tokio::test]
    async fn test_image_generation_with_size() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "A beautiful landscape",
                "model": "test/test-model",
                "size": "1024x1024"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["data"].is_array());
    }

    #[tokio::test]
    async fn test_image_generation_with_quality() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "A detailed portrait",
                "model": "test/test-model",
                "quality": "hd"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["data"].is_array());
    }

    #[tokio::test]
    async fn test_image_generation_with_style() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "An abstract painting",
                "model": "test/test-model",
                "style": "vivid"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["data"].is_array());
    }

    #[tokio::test]
    async fn test_image_generation_missing_prompt() {
        let app = test_app().await;

        let (status, body) = post_json_raw(
            &app,
            "/api/v1/images/generations",
            json!({
                "model": "test/test-model"
            }),
        )
        .await;

        // Validation errors return 422 UNPROCESSABLE_ENTITY
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            body.to_lowercase().contains("prompt"),
            "Expected error about 'prompt', got: {}",
            body
        );
    }

    #[tokio::test]
    async fn test_image_generation_invalid_n_value() {
        let app = test_app().await;

        let (status, body) = post_json_raw(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "Test image",
                "model": "test/test-model",
                "n": 0
            }),
        )
        .await;

        // Business logic validation returns 400 BAD_REQUEST for invalid n value
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Should contain error about n value
        assert!(!body.is_empty(), "Expected error response, got empty body");
    }

    #[tokio::test]
    async fn test_image_generation_n_exceeds_max() {
        let app = test_app().await;

        let (status, body) = post_json_raw(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "Test image",
                "model": "test/test-model",
                "n": 100
            }),
        )
        .await;

        // Business logic validation returns 400 BAD_REQUEST for n exceeding max
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Should contain error about n value
        assert!(!body.is_empty(), "Expected error response, got empty body");
    }

    #[tokio::test]
    async fn test_image_generation_unknown_provider() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "Test image",
                "model": "unknown-provider/model"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].is_object());
    }

    #[tokio::test]
    async fn test_image_generation_with_user_field() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "A test image",
                "model": "test/test-model",
                "user": "user-12345"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["data"].is_array());
    }

    #[tokio::test]
    async fn test_image_generation_response_format_url() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "A test image",
                "model": "test/test-model",
                "response_format": "url"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let data = body["data"].as_array().unwrap();
        assert!(data[0]["url"].is_string());
    }

    #[tokio::test]
    async fn test_image_generation_unicode_prompt() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/images/generations",
            json!({
                "prompt": "Un chat mignon avec des étoiles",
                "model": "test/test-model"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["data"].is_array());
    }

    #[tokio::test]
    async fn test_image_edit_basic() {
        let app = test_app().await;

        // Create a minimal PNG file (1x1 transparent pixel)
        let png_bytes: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
            0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49,
            0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D,
            0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60,
            0x82,
        ];

        // Build multipart form
        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        // Add image field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"image\"; filename=\"test.png\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
        body_bytes.extend_from_slice(&png_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        // Add prompt field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"prompt\"\r\n\r\n");
        body_bytes.extend_from_slice(b"Add a rainbow\r\n");

        // Add model field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        // End boundary
        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/images/edits")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

        assert_eq!(status, StatusCode::OK);
        assert!(json["data"].is_array());
    }

    #[tokio::test]
    async fn test_image_edit_with_mask() {
        let app = test_app().await;

        // Create a minimal PNG file (1x1 transparent pixel)
        let png_bytes: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        // Add image field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"image\"; filename=\"test.png\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
        body_bytes.extend_from_slice(&png_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        // Add mask field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"mask\"; filename=\"mask.png\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
        body_bytes.extend_from_slice(&png_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        // Add prompt field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"prompt\"\r\n\r\n");
        body_bytes.extend_from_slice(b"Replace masked area with a cat\r\n");

        // Add model field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/images/edits")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

        assert_eq!(status, StatusCode::OK);
        assert!(json["data"].is_array());
    }

    #[tokio::test]
    async fn test_image_variation_basic() {
        let app = test_app().await;

        // Create a minimal PNG file
        let png_bytes: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        // Add image field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"image\"; filename=\"test.png\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
        body_bytes.extend_from_slice(&png_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        // Add model field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/images/variations")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

        assert_eq!(status, StatusCode::OK);
        assert!(json["data"].is_array());
    }

    // ============================================================================
    // Audio Speech (TTS) Tests
    // ============================================================================

    #[tokio::test]
    async fn test_audio_speech_basic() {
        let app = test_app().await;

        let (status, body) = post_json_raw(
            &app,
            "/api/v1/audio/speech",
            json!({
                "model": "test/test-model",
                "input": "Hello, this is a test.",
                "voice": "alloy"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        // Response should be audio data (not JSON)
        assert!(!body.is_empty());
    }

    #[tokio::test]
    async fn test_audio_speech_with_response_format() {
        let app = test_app().await;

        // Test MP3 format (default)
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/speech")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({
                    "model": "test/test-model",
                    "input": "Testing different formats",
                    "voice": "nova",
                    "response_format": "mp3"
                }))
                .unwrap(),
            ))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "audio/mpeg"
        );
    }

    #[tokio::test]
    async fn test_audio_speech_opus_format() {
        let app = test_app().await;

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/speech")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({
                    "model": "test/test-model",
                    "input": "Testing opus format",
                    "voice": "echo",
                    "response_format": "opus"
                }))
                .unwrap(),
            ))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "audio/opus"
        );
    }

    #[tokio::test]
    async fn test_audio_speech_all_voices() {
        let app = test_app().await;
        let voices = ["alloy", "echo", "fable", "onyx", "nova", "shimmer"];

        for voice in voices {
            let (status, _) = post_json_raw(
                &app,
                "/api/v1/audio/speech",
                json!({
                    "model": "test/test-model",
                    "input": "Testing voice",
                    "voice": voice
                }),
            )
            .await;

            assert_eq!(status, StatusCode::OK, "Voice {} should work", voice);
        }
    }

    #[tokio::test]
    async fn test_audio_speech_with_speed() {
        let app = test_app().await;

        let (status, _) = post_json_raw(
            &app,
            "/api/v1/audio/speech",
            json!({
                "model": "test/test-model",
                "input": "Testing speed parameter",
                "voice": "alloy",
                "speed": 1.5
            }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_audio_speech_missing_input() {
        let app = test_app().await;

        let (status, body) = post_json_raw(
            &app,
            "/api/v1/audio/speech",
            json!({
                "model": "test/test-model",
                "voice": "alloy"
            }),
        )
        .await;

        // Validation errors return 422 UNPROCESSABLE_ENTITY
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        // The validation error message should mention the missing 'input' field
        assert!(
            body.to_lowercase().contains("input"),
            "Expected error about 'input', got: {}",
            body
        );
    }

    #[tokio::test]
    async fn test_audio_speech_missing_voice() {
        let app = test_app().await;

        let (status, body) = post_json_raw(
            &app,
            "/api/v1/audio/speech",
            json!({
                "model": "test/test-model",
                "input": "Hello"
            }),
        )
        .await;

        // Validation errors return 422 UNPROCESSABLE_ENTITY
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        // The validation error message should mention the missing 'voice' field
        assert!(
            body.to_lowercase().contains("voice"),
            "Expected error about 'voice', got: {}",
            body
        );
    }

    #[tokio::test]
    async fn test_audio_speech_invalid_speed() {
        let app = test_app().await;

        // Speed too low (must be between 0.25 and 4.0)
        let (status, body) = post_json_raw(
            &app,
            "/api/v1/audio/speech",
            json!({
                "model": "test/test-model",
                "input": "Testing invalid speed",
                "voice": "alloy",
                "speed": 0.1
            }),
        )
        .await;

        // Speed validation returns 400 BAD_REQUEST (range validation)
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // The error message should mention speed validation
        assert!(
            body.to_lowercase().contains("speed"),
            "Expected error about 'speed', got: {}",
            body
        );
    }

    #[tokio::test]
    async fn test_audio_speech_unknown_provider() {
        let app = test_app().await;

        let (status, body) = post_json(
            &app,
            "/api/v1/audio/speech",
            json!({
                "model": "unknown-provider/model",
                "input": "Test",
                "voice": "alloy"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].is_object());
    }

    // ============================================================================
    // Audio Transcription Tests
    // ============================================================================

    #[tokio::test]
    async fn test_audio_transcription_basic() {
        let app = test_app().await;

        // Create mock audio bytes (minimal valid structure)
        let audio_bytes: Vec<u8> = vec![
            0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        // Add file field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.mp3\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        body_bytes.extend_from_slice(&audio_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        // Add model field
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/transcriptions")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

        assert_eq!(status, StatusCode::OK);
        assert!(json["text"].is_string());
    }

    #[tokio::test]
    async fn test_audio_transcription_verbose_json() {
        let app = test_app().await;

        let audio_bytes: Vec<u8> = vec![
            0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.mp3\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        body_bytes.extend_from_slice(&audio_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes
            .extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
        body_bytes.extend_from_slice(b"verbose_json\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/transcriptions")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

        assert_eq!(status, StatusCode::OK);
        assert!(json["text"].is_string());
        assert!(json["duration"].is_number());
        assert!(json["words"].is_array());
    }

    #[tokio::test]
    async fn test_audio_transcription_text_format() {
        let app = test_app().await;

        let audio_bytes: Vec<u8> = vec![
            0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.mp3\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        body_bytes.extend_from_slice(&audio_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes
            .extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
        body_bytes.extend_from_slice(b"text\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/transcriptions")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/plain"
        );
    }

    #[tokio::test]
    async fn test_audio_transcription_srt_format() {
        let app = test_app().await;

        let audio_bytes: Vec<u8> = vec![
            0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.mp3\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        body_bytes.extend_from_slice(&audio_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes
            .extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
        body_bytes.extend_from_slice(b"srt\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/transcriptions")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&body);

        assert_eq!(status, StatusCode::OK);
        assert!(text.contains("-->"));
    }

    #[tokio::test]
    async fn test_audio_transcription_vtt_format() {
        let app = test_app().await;

        let audio_bytes: Vec<u8> = vec![
            0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.mp3\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        body_bytes.extend_from_slice(&audio_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes
            .extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
        body_bytes.extend_from_slice(b"vtt\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/transcriptions")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8_lossy(&body);

        assert_eq!(status, StatusCode::OK);
        assert!(text.contains("WEBVTT"));
    }

    #[tokio::test]
    async fn test_audio_transcription_missing_file() {
        let app = test_app().await;

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        // Only add model, no file
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/transcriptions")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_audio_transcription_missing_model() {
        let app = test_app().await;

        let audio_bytes: Vec<u8> = vec![
            0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        // Only add file, no model
        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.mp3\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        body_bytes.extend_from_slice(&audio_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/transcriptions")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // ============================================================================
    // Audio Translation Tests
    // ============================================================================

    #[tokio::test]
    async fn test_audio_translation_basic() {
        let app = test_app().await;

        let audio_bytes: Vec<u8> = vec![
            0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.mp3\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        body_bytes.extend_from_slice(&audio_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/translations")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

        assert_eq!(status, StatusCode::OK);
        assert!(json["text"].is_string());
    }

    #[tokio::test]
    async fn test_audio_translation_verbose_json() {
        let app = test_app().await;

        let audio_bytes: Vec<u8> = vec![
            0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.mp3\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        body_bytes.extend_from_slice(&audio_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes
            .extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
        body_bytes.extend_from_slice(b"verbose_json\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/translations")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

        assert_eq!(status, StatusCode::OK);
        assert!(json["text"].is_string());
        assert!(json["duration"].is_number());
    }

    #[tokio::test]
    async fn test_audio_translation_text_format() {
        let app = test_app().await;

        let audio_bytes: Vec<u8> = vec![
            0xFF, 0xFB, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.mp3\"\r\n",
        );
        body_bytes.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
        body_bytes.extend_from_slice(&audio_bytes);
        body_bytes.extend_from_slice(b"\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes
            .extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
        body_bytes.extend_from_slice(b"text\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/translations")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/plain"
        );
    }

    #[tokio::test]
    async fn test_audio_translation_missing_file() {
        let app = test_app().await;

        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let mut body_bytes = Vec::new();

        body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body_bytes.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
        body_bytes.extend_from_slice(b"test/test-model\r\n");

        body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/audio/translations")
            .header(
                "content-type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(Body::from(body_bytes))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
