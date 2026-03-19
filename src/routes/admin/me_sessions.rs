//! Self-service session management endpoints.
//!
//! Allows users to view and revoke their own sessions at `/admin/v1/me/sessions`.

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde_json::json;
use uuid::Uuid;

use super::{AuditActor, error::AdminError, sessions::get_session_store};
use crate::{
    AppState,
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    models::CreateAuditLog,
    routes::admin::sessions::{SessionInfo, SessionListResponse, SessionsRevokedResponse},
    services::Services,
};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// List current user's active sessions.
///
/// Returns an empty list if enhanced session management is not enabled.
/// The `enhanced_enabled` field indicates whether session tracking is active.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/me/sessions",
    tag = "me",
    operation_id = "me_sessions_list",
    responses(
        (status = 200, description = "List of current user's sessions", body = SessionListResponse),
        (status = 401, description = "Not authenticated", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.me.sessions.list", skip(state, admin_auth, authz))]
pub async fn list(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<SessionListResponse>, AdminError> {
    authz.require("me", "read", None, None, None, None)?;

    let external_id = &admin_auth.identity.external_id;
    if external_id.is_empty() {
        return Ok(Json(SessionListResponse {
            data: vec![],
            enhanced_enabled: false,
        }));
    }

    let session_store = get_session_store(&state)?;
    let enhanced_enabled = session_store.is_enhanced_enabled();

    let sessions = session_store
        .list_user_sessions(external_id)
        .await
        .map_err(|e| AdminError::Internal(format!("Failed to list sessions: {}", e)))?;

    let data = sessions
        .into_iter()
        .map(|s| SessionInfo {
            id: s.id,
            device: s.device,
            created_at: s.created_at,
            last_activity: s.last_activity,
            expires_at: s.expires_at,
        })
        .collect();

    Ok(Json(SessionListResponse {
        data,
        enhanced_enabled,
    }))
}

/// Revoke a specific session belonging to the current user.
///
/// Returns success even if the session doesn't exist (idempotent).
/// Returns 400 if the session belongs to a different user.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/me/sessions/{session_id}",
    tag = "me",
    operation_id = "me_sessions_delete_one",
    params(("session_id" = Uuid, Path, description = "Session ID to revoke")),
    responses(
        (status = 200, description = "Session revoked", body = SessionsRevokedResponse),
        (status = 400, description = "Session does not belong to current user", body = crate::openapi::ErrorResponse),
        (status = 401, description = "Not authenticated", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.me.sessions.delete_one", skip(state, admin_auth, authz), fields(%session_id))]
pub async fn delete_one(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionsRevokedResponse>, AdminError> {
    authz.require("me", "delete", None, None, None, None)?;

    let external_id = &admin_auth.identity.external_id;
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    let session_store = get_session_store(&state)?;

    // Verify session belongs to the current user
    let session_existed = match session_store.get_session(session_id).await {
        Ok(Some(session)) => {
            if session.external_id != *external_id {
                return Err(AdminError::BadRequest(
                    "Session does not belong to current user".to_string(),
                ));
            }
            true
        }
        Ok(None) => false,
        Err(e) => {
            return Err(AdminError::Internal(format!(
                "Failed to look up session: {e}"
            )));
        }
    };

    let result = session_store.delete_session(session_id).await;
    let sessions_revoked = if result.is_ok() && session_existed {
        1
    } else {
        0
    };

    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "session.self_delete_one".to_string(),
            resource_type: "session".to_string(),
            resource_id: session_id,
            org_id: None,
            project_id: None,
            details: json!({
                "session_id": session_id,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(SessionsRevokedResponse { sessions_revoked }))
}
