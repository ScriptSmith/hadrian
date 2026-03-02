//! Admin endpoints for user session management.
//!
//! These endpoints enable the critical enterprise use case:
//! "An employee was terminated. Force logout all their sessions immediately."
//!
//! Sessions are nested under users: `/admin/v1/users/{user_id}/sessions`

use axum::{
    Extension, Json,
    extract::{Path, State},
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use super::{AuditActor, error::AdminError};
use crate::{
    AppState,
    auth::session_store::{DeviceInfo, SharedSessionStore},
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    models::CreateAuditLog,
    services::Services,
};

/// Information about an active user session.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SessionInfo {
    /// Unique session identifier
    pub id: Uuid,
    /// Device information (user agent, IP, description)
    pub device: Option<DeviceInfo>,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last active (if tracking enabled)
    pub last_activity: Option<DateTime<Utc>>,
    /// When the session will expire
    pub expires_at: DateTime<Utc>,
}

/// Response containing a list of user sessions.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SessionListResponse {
    /// List of active sessions for the user
    pub data: Vec<SessionInfo>,
    /// Whether enhanced session management is enabled.
    /// If false, the UI should show a message that session tracking is not enabled.
    pub enhanced_enabled: bool,
}

/// Response after revoking sessions.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SessionsRevokedResponse {
    /// Number of sessions that were revoked
    pub sessions_revoked: usize,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Get the shared session store from the app state.
///
/// The session store is shared across all authenticators (OIDC, SAML).
/// This function checks each potential source in order.
pub fn get_session_store(state: &AppState) -> Result<SharedSessionStore, AdminError> {
    // Try OIDC registry first (most common case for multi-tenant SSO)
    if let Some(ref registry) = state.oidc_registry {
        return Ok(registry.session_store().clone());
    }

    // Try SAML registry
    #[cfg(feature = "saml")]
    if let Some(ref registry) = state.saml_registry {
        return Ok(registry.session_store().clone());
    }

    Err(AdminError::BadRequest(
        "No session store configured. UI authentication (OIDC or SAML) must be enabled."
            .to_string(),
    ))
}

/// List all active sessions for a user.
///
/// Returns an empty list if enhanced session management is not enabled.
/// The `enhanced_enabled` field indicates whether session tracking is active.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}/sessions",
    tag = "users",
    operation_id = "user_sessions_list",
    params(("user_id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "List of user sessions", body = SessionListResponse),
        (status = 400, description = "User has no external_id (API-only user)", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sessions.list", skip(state, authz), fields(%user_id))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<SessionListResponse>, AdminError> {
    // Require user:read permission
    authz.require("user", "read", Some(&user_id.to_string()), None, None, None)?;

    let services = get_services(&state)?;

    // Get user to verify they exist and get their external_id
    let user = services
        .users
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;

    // Check if user has an external_id (required for session tracking)
    // Users created via OIDC/SAML have external_id from IdP; API-only users might have empty external_id
    if user.external_id.is_empty() {
        return Err(AdminError::BadRequest(
            "User has no external_id and cannot have browser sessions".to_string(),
        ));
    }

    // Get session store from app state
    let session_store = get_session_store(&state)?;

    let enhanced_enabled = session_store.is_enhanced_enabled();

    // List sessions for the user
    let sessions = session_store
        .list_user_sessions(&user.external_id)
        .await
        .map_err(|e| AdminError::Internal(format!("Failed to list sessions: {}", e)))?;

    // Convert to SessionInfo
    let data: Vec<SessionInfo> = sessions
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

/// Revoke all sessions for a user (force logout).
///
/// This is the critical endpoint for the "terminated employee" use case.
/// Returns 0 sessions revoked if enhanced session management is not enabled.
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/users/{user_id}/sessions",
    tag = "users",
    operation_id = "user_sessions_delete_all",
    params(("user_id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "All sessions revoked", body = SessionsRevokedResponse),
        (status = 400, description = "User has no external_id (API-only user)", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sessions.delete_all", skip(state, admin_auth, authz), fields(%user_id))]
pub async fn delete_all(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<SessionsRevokedResponse>, AdminError> {
    // Require user:manage permission (higher privilege than read)
    authz.require(
        "user",
        "manage",
        Some(&user_id.to_string()),
        None,
        None,
        None,
    )?;

    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get user to verify they exist and get their external_id
    let user = services
        .users
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;

    // Check if user has an external_id (required for session tracking)
    if user.external_id.is_empty() {
        return Err(AdminError::BadRequest(
            "User has no external_id and cannot have browser sessions".to_string(),
        ));
    }

    // Get session store from app state
    let session_store = get_session_store(&state)?;

    // Delete all sessions for the user
    let sessions_revoked = session_store
        .delete_user_sessions(&user.external_id)
        .await
        .map_err(|e| AdminError::Internal(format!("Failed to delete sessions: {}", e)))?;

    // Audit log (fire-and-forget - don't fail the request if audit logging fails)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "session.delete_all".to_string(),
            resource_type: "user".to_string(),
            resource_id: user_id,
            org_id: None,
            project_id: None,
            details: json!({
                "user_email": user.email,
                "external_id": user.external_id,
                "sessions_revoked": sessions_revoked,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    tracing::info!(
        user_id = %user_id,
        external_id = %user.external_id,
        sessions_revoked = sessions_revoked,
        "Force logout: revoked all user sessions"
    );

    Ok(Json(SessionsRevokedResponse { sessions_revoked }))
}

/// Revoke a specific session for a user.
///
/// Returns success even if the session doesn't exist (idempotent).
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/users/{user_id}/sessions/{session_id}",
    tag = "users",
    operation_id = "user_sessions_delete_one",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        ("session_id" = Uuid, Path, description = "Session ID to revoke"),
    ),
    responses(
        (status = 200, description = "Session revoked", body = SessionsRevokedResponse),
        (status = 400, description = "User has no external_id or session doesn't belong to user", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sessions.delete_one", skip(state, admin_auth, authz), fields(%user_id, %session_id))]
pub async fn delete_one(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path((user_id, session_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SessionsRevokedResponse>, AdminError> {
    // Require user:manage permission
    authz.require(
        "user",
        "manage",
        Some(&user_id.to_string()),
        None,
        None,
        None,
    )?;

    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get user to verify they exist and get their external_id
    let user = services
        .users
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;

    // Check if user has an external_id
    if user.external_id.is_empty() {
        return Err(AdminError::BadRequest(
            "User has no external_id and cannot have browser sessions".to_string(),
        ));
    }

    // Get session store from app state
    let session_store = get_session_store(&state)?;

    // Check if session exists and verify it belongs to this user
    let session_existed = match session_store.get_session(session_id).await {
        Ok(Some(session)) => {
            if session.external_id != user.external_id {
                return Err(AdminError::BadRequest(
                    "Session does not belong to this user".to_string(),
                ));
            }
            true
        }
        Ok(None) => false,
        Err(_) => false,
    };

    // Delete the specific session (idempotent - succeeds even if not found)
    let result = session_store.delete_session(session_id).await;
    let sessions_revoked = if result.is_ok() && session_existed {
        1
    } else {
        0
    };

    // Audit log (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "session.delete_one".to_string(),
            resource_type: "user".to_string(),
            resource_id: user_id,
            org_id: None,
            project_id: None,
            details: json!({
                "user_email": user.email,
                "external_id": user.external_id,
                "session_id": session_id,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    tracing::info!(
        user_id = %user_id,
        session_id = %session_id,
        "Revoked specific user session"
    );

    Ok(Json(SessionsRevokedResponse { sessions_revoked }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_info_serialization() {
        let session = SessionInfo {
            id: Uuid::new_v4(),
            device: Some(DeviceInfo {
                user_agent: Some("Mozilla/5.0".to_string()),
                ip_address: Some("192.168.1.1".to_string()),
                device_id: Some("abc123".to_string()),
                device_description: Some("Chrome 120 on Windows".to_string()),
            }),
            created_at: Utc::now(),
            last_activity: Some(Utc::now()),
            expires_at: Utc::now() + chrono::Duration::hours(1),
        };

        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("device"));
        assert!(json.contains("Chrome 120 on Windows"));
    }

    #[test]
    fn test_session_list_response_serialization() {
        let response = SessionListResponse {
            data: vec![],
            enhanced_enabled: false,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"enhanced_enabled\":false"));
    }

    #[test]
    fn test_sessions_revoked_response() {
        let response = SessionsRevokedResponse {
            sessions_revoked: 3,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"sessions_revoked\":3"));
    }
}
