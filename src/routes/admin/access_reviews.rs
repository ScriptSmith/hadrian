use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};
use uuid::Uuid;

#[cfg(feature = "csv-export")]
use super::csv_export::{
    CsvResponse, export_access_inventory_csv, export_org_access_report_csv,
    export_stale_access_csv, export_user_access_summary_csv,
};
use super::error::AdminError;
#[cfg(feature = "utoipa")]
use crate::models::{
    AccessInventoryResponse, OrgAccessReportResponse, StaleAccessResponse,
    UserAccessSummaryResponse,
};
use crate::{
    AppState,
    middleware::AuthzContext,
    models::{
        AccessInventoryQuery, ExportFormat, OrgAccessReportQuery, StaleAccessQuery,
        UserAccessSummaryQuery,
    },
    services::Services,
};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Get access inventory for compliance reviews
///
/// Returns a comprehensive view of all users and their access rights across
/// organizations and projects. This endpoint supports compliance requirements
/// for access reviews (SOC 2, ISO 27001).
///
/// The response includes:
/// - User details (id, email, name, external_id)
/// - Organization memberships with roles and grant dates
/// - Project memberships with roles and grant dates
/// - API key summary (active, revoked, expired counts)
/// - Last activity timestamp
/// - Summary statistics
///
/// Use `format=csv` for CSV export suitable for auditors and spreadsheets.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/access-reviews/inventory",
    tag = "access-reviews",
    operation_id = "access_review_inventory",
    params(AccessInventoryQuery),
    responses(
        (status = 200, description = "Access inventory (JSON or CSV based on format parameter)", body = AccessInventoryResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get_inventory(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Query(query): Query<AccessInventoryQuery>,
) -> Result<impl IntoResponse, AdminError> {
    // Requires access_review.read permission (admin-level access)
    authz.require("access_review", "read", None, None, None, None)?;

    let services = get_services(&state)?;

    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);
    let format = query.format;

    let inventory = services
        .access_reviews
        .get_access_inventory(query.org_id, limit, offset)
        .await?;

    match format {
        ExportFormat::Json => Ok(Json(inventory).into_response()),
        #[cfg(feature = "csv-export")]
        ExportFormat::Csv => {
            let csv_data = export_access_inventory_csv(&inventory)
                .map_err(|e| AdminError::Internal(e.to_string()))?;
            Ok(CsvResponse {
                data: csv_data,
                filename: "access-inventory.csv".to_string(),
            }
            .into_response())
        }
        #[cfg(not(feature = "csv-export"))]
        ExportFormat::Csv => Err(AdminError::Internal(
            "CSV export requires the 'csv-export' feature".into(),
        )),
    }
}

/// Get access report for a specific organization
///
/// Returns a comprehensive access report for the specified organization including:
/// - All org members with their roles and project access
/// - All API keys scoped to the organization or its projects
/// - Recent access grant history from audit logs
/// - Summary statistics
///
/// Use `format=csv` for CSV export suitable for auditors and spreadsheets.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/access-report",
    tag = "access-reviews",
    operation_id = "org_access_report",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        OrgAccessReportQuery
    ),
    responses(
        (status = 200, description = "Organization access report (JSON or CSV based on format parameter)", body = OrgAccessReportResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get_org_access_report(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<OrgAccessReportQuery>,
) -> Result<impl IntoResponse, AdminError> {
    let services = get_services(&state)?;

    // First get the org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Requires access_review.read permission scoped to the organization
    authz.require(
        "access_review",
        "read",
        Some(&org.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    let report = services.access_reviews.get_org_access_report(&org).await?;

    match query.format {
        ExportFormat::Json => Ok(Json(report).into_response()),
        #[cfg(feature = "csv-export")]
        ExportFormat::Csv => {
            let csv_data = export_org_access_report_csv(&report)
                .map_err(|e| AdminError::Internal(e.to_string()))?;
            Ok(CsvResponse {
                data: csv_data,
                filename: format!("{}-access-report.csv", org_slug),
            }
            .into_response())
        }
        #[cfg(not(feature = "csv-export"))]
        ExportFormat::Csv => Err(AdminError::Internal(
            "CSV export requires the 'csv-export' feature".into(),
        )),
    }
}

/// Get access summary for a specific user
///
/// Returns a comprehensive access summary for the specified user including:
/// - All organizations the user belongs to with roles and grant dates
/// - All projects the user belongs to with roles and grant dates
/// - All API keys owned by the user
/// - Last activity timestamps per resource
/// - Summary statistics
///
/// Use `format=csv` for CSV export suitable for auditors and spreadsheets.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/users/{user_id}/access-summary",
    tag = "access-reviews",
    operation_id = "user_access_summary",
    params(
        ("user_id" = Uuid, Path, description = "User ID"),
        UserAccessSummaryQuery
    ),
    responses(
        (status = 200, description = "User access summary (JSON or CSV based on format parameter)", body = UserAccessSummaryResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "User not found", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get_user_access_summary(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<UserAccessSummaryQuery>,
) -> Result<impl IntoResponse, AdminError> {
    let services = get_services(&state)?;

    // Get the user by ID
    let user = services
        .users
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("User '{}' not found", user_id)))?;

    // Requires access_review.read permission (admin-level access)
    authz.require(
        "access_review",
        "read",
        Some(&user_id.to_string()),
        None,
        None,
        None,
    )?;

    let summary = services
        .access_reviews
        .get_user_access_summary(&user)
        .await?;

    match query.format {
        ExportFormat::Json => Ok(Json(summary).into_response()),
        #[cfg(feature = "csv-export")]
        ExportFormat::Csv => {
            let csv_data = export_user_access_summary_csv(&summary)
                .map_err(|e| AdminError::Internal(e.to_string()))?;
            Ok(CsvResponse {
                data: csv_data,
                filename: format!("user-{}-access-summary.csv", user_id),
            }
            .into_response())
        }
        #[cfg(not(feature = "csv-export"))]
        ExportFormat::Csv => Err(AdminError::Internal(
            "CSV export requires the 'csv-export' feature".into(),
        )),
    }
}

/// Detect stale access across the system
///
/// Identifies users and API keys that haven't been active for a configurable
/// number of days. This helps identify:
/// - Users who haven't logged in or performed any actions
/// - API keys that haven't been used
/// - Users with access but no recorded activity
///
/// This endpoint supports compliance requirements for periodic access reviews
/// (SOC 2, ISO 27001) by identifying potentially orphaned or unused access.
///
/// Use `format=csv` for CSV export suitable for auditors and spreadsheets.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/access-reviews/stale",
    tag = "access-reviews",
    operation_id = "stale_access_detection",
    params(StaleAccessQuery),
    responses(
        (status = 200, description = "Stale access report (JSON or CSV based on format parameter)", body = StaleAccessResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
    )
))]
pub async fn get_stale_access(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Query(query): Query<StaleAccessQuery>,
) -> Result<impl IntoResponse, AdminError> {
    // Requires access_review.read permission (admin-level access)
    authz.require("access_review", "read", None, None, None, None)?;

    let services = get_services(&state)?;

    let inactive_days = query.inactive_days.unwrap_or(90).clamp(1, 365);
    let limit = query.limit.unwrap_or(100).min(1000);
    let format = query.format;

    let report = services
        .access_reviews
        .get_stale_access(inactive_days, query.org_id, limit)
        .await?;

    match format {
        ExportFormat::Json => Ok(Json(report).into_response()),
        #[cfg(feature = "csv-export")]
        ExportFormat::Csv => {
            let csv_data = export_stale_access_csv(&report)
                .map_err(|e| AdminError::Internal(e.to_string()))?;
            Ok(CsvResponse {
                data: csv_data,
                filename: "stale-access-report.csv".to_string(),
            }
            .into_response())
        }
        #[cfg(not(feature = "csv-export"))]
        ExportFormat::Csv => Err(AdminError::Internal(
            "CSV export requires the 'csv-export' feature".into(),
        )),
    }
}
