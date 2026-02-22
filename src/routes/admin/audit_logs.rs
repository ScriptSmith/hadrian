use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use serde::Serialize;
use uuid::Uuid;

use super::error::AdminError;
use crate::{
    AppState,
    middleware::AuthzContext,
    models::{AuditLog, AuditLogQuery},
    openapi::PaginationMeta,
    services::Services,
};

/// Paginated list of audit logs
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AuditLogListResponse {
    /// List of audit log entries
    pub data: Vec<AuditLog>,
    /// Pagination metadata
    pub pagination: PaginationMeta,
}

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// List audit logs
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/audit-logs",
    tag = "audit-logs",
    operation_id = "audit_log_list",
    params(AuditLogQuery),
    responses(
        (status = 200, description = "List of audit log entries", body = AuditLogListResponse),
        (status = 400, description = "Invalid cursor or direction", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Query(query): Query<AuditLogQuery>,
) -> Result<Json<AuditLogListResponse>, AdminError> {
    authz.require("audit_log", "list", None, None, None, None)?;
    let services = get_services(&state)?;

    let limit = query.limit.unwrap_or(100);

    // Validate direction if provided
    if let Some(ref dir) = query.direction
        && dir != "forward"
        && dir != "backward"
    {
        return Err(AdminError::BadRequest(format!(
            "Invalid direction '{}': must be 'forward' or 'backward'",
            dir
        )));
    }

    let result = services.audit_logs.list(query).await?;

    let pagination = PaginationMeta::with_cursors(
        limit,
        result.has_more,
        result.cursors.next.map(|c| c.encode()),
        result.cursors.prev.map(|c| c.encode()),
    );

    Ok(Json(AuditLogListResponse {
        data: result.items,
        pagination,
    }))
}

/// Get an audit log entry by ID
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/audit-logs/{id}",
    tag = "audit-logs",
    operation_id = "audit_log_get",
    params(("id" = Uuid, Path, description = "Audit log entry ID")),
    responses(
        (status = 200, description = "Audit log entry found", body = AuditLog),
        (status = 404, description = "Audit log entry not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<AuditLog>, AdminError> {
    authz.require("audit_log", "read", None, None, None, None)?;
    let services = get_services(&state)?;

    let entry = services
        .audit_logs
        .get_by_id(id)
        .await?
        .ok_or_else(|| AdminError::NotFound("Audit log entry not found".to_string()))?;

    Ok(Json(entry))
}
