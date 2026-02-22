//! CSV export utilities for access review reports
//!
//! This module provides CSV serialization for access review data,
//! enabling auditor-friendly exports for compliance reviews.

use axum::{
    body::Body,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use csv::Writer;

use crate::models::{
    AccessInventoryResponse, OrgAccessReportResponse, StaleAccessResponse,
    UserAccessInventoryEntry, UserAccessSummaryResponse,
};

/// Error type for CSV export operations
#[derive(Debug)]
pub struct CsvExportError(String);

impl std::fmt::Display for CsvExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSV export error: {}", self.0)
    }
}

impl std::error::Error for CsvExportError {}

impl IntoResponse for CsvExportError {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0).into_response()
    }
}

/// CSV response wrapper that sets appropriate headers
pub struct CsvResponse {
    pub data: Vec<u8>,
    pub filename: String,
}

impl IntoResponse for CsvResponse {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", self.filename),
            )
            .body(Body::from(self.data))
            .unwrap()
    }
}

/// Flattened row for access inventory CSV export
#[derive(Clone, serde::Serialize)]
struct AccessInventoryRow {
    user_id: String,
    external_id: String,
    email: String,
    name: String,
    created_at: String,
    org_id: String,
    org_slug: String,
    org_name: String,
    org_role: String,
    org_granted_at: String,
    project_id: String,
    project_slug: String,
    project_name: String,
    project_org_id: String,
    project_role: String,
    project_granted_at: String,
    api_keys_active: i64,
    api_keys_revoked: i64,
    api_keys_expired: i64,
    last_activity_at: String,
}

/// Export access inventory to CSV format
pub fn export_access_inventory_csv(
    response: &AccessInventoryResponse,
) -> Result<Vec<u8>, CsvExportError> {
    let mut wtr = Writer::from_writer(vec![]);

    // Flatten the hierarchical data into rows
    // Each user-org-project combination becomes a row
    for user in &response.users {
        let base_row = create_base_inventory_row(user);

        if user.organizations.is_empty() && user.projects.is_empty() {
            // User with no org/project memberships - output one row
            wtr.serialize(&base_row)
                .map_err(|e| CsvExportError(e.to_string()))?;
        } else {
            // Output a row for each org membership
            for org in &user.organizations {
                let mut row = base_row.clone();
                row.org_id = org.org_id.to_string();
                row.org_slug = org.org_slug.clone();
                row.org_name = org.org_name.clone();
                row.org_role = org.role.clone();
                row.org_granted_at = org.granted_at.to_rfc3339();
                wtr.serialize(&row)
                    .map_err(|e| CsvExportError(e.to_string()))?;
            }

            // Output a row for each project membership
            for project in &user.projects {
                let mut row = base_row.clone();
                row.project_id = project.project_id.to_string();
                row.project_slug = project.project_slug.clone();
                row.project_name = project.project_name.clone();
                row.project_org_id = project.org_id.to_string();
                row.project_role = project.role.clone();
                row.project_granted_at = project.granted_at.to_rfc3339();
                wtr.serialize(&row)
                    .map_err(|e| CsvExportError(e.to_string()))?;
            }
        }
    }

    wtr.into_inner().map_err(|e| CsvExportError(e.to_string()))
}

fn create_base_inventory_row(user: &UserAccessInventoryEntry) -> AccessInventoryRow {
    AccessInventoryRow {
        user_id: user.user_id.to_string(),
        external_id: user.external_id.clone(),
        email: user.email.clone().unwrap_or_default(),
        name: user.name.clone().unwrap_or_default(),
        created_at: user.created_at.to_rfc3339(),
        org_id: String::new(),
        org_slug: String::new(),
        org_name: String::new(),
        org_role: String::new(),
        org_granted_at: String::new(),
        project_id: String::new(),
        project_slug: String::new(),
        project_name: String::new(),
        project_org_id: String::new(),
        project_role: String::new(),
        project_granted_at: String::new(),
        api_keys_active: user.api_key_summary.active_count,
        api_keys_revoked: user.api_key_summary.revoked_count,
        api_keys_expired: user.api_key_summary.expired_count,
        last_activity_at: user
            .last_activity_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_default(),
    }
}

/// Flattened row for org access report CSV export
#[derive(Clone, serde::Serialize)]
struct OrgAccessReportRow {
    user_id: String,
    external_id: String,
    email: String,
    name: String,
    org_role: String,
    org_granted_at: String,
    project_id: String,
    project_slug: String,
    project_name: String,
    project_role: String,
    project_granted_at: String,
    api_keys_active: i64,
    api_keys_revoked: i64,
    api_keys_expired: i64,
    last_activity_at: String,
}

/// Export org access report to CSV format
pub fn export_org_access_report_csv(
    response: &OrgAccessReportResponse,
) -> Result<Vec<u8>, CsvExportError> {
    let mut wtr = Writer::from_writer(vec![]);

    for member in &response.members {
        let base_row = OrgAccessReportRow {
            user_id: member.user_id.to_string(),
            external_id: member.external_id.clone(),
            email: member.email.clone().unwrap_or_default(),
            name: member.name.clone().unwrap_or_default(),
            org_role: member.role.clone(),
            org_granted_at: member.granted_at.to_rfc3339(),
            project_id: String::new(),
            project_slug: String::new(),
            project_name: String::new(),
            project_role: String::new(),
            project_granted_at: String::new(),
            api_keys_active: member.api_key_summary.active_count,
            api_keys_revoked: member.api_key_summary.revoked_count,
            api_keys_expired: member.api_key_summary.expired_count,
            last_activity_at: member
                .last_activity_at
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
        };

        if member.project_access.is_empty() {
            wtr.serialize(&base_row)
                .map_err(|e| CsvExportError(e.to_string()))?;
        } else {
            for project in &member.project_access {
                let mut row = base_row.clone();
                row.project_id = project.project_id.to_string();
                row.project_slug = project.project_slug.clone();
                row.project_name = project.project_name.clone();
                row.project_role = project.role.clone();
                row.project_granted_at = project.granted_at.to_rfc3339();
                wtr.serialize(&row)
                    .map_err(|e| CsvExportError(e.to_string()))?;
            }
        }
    }

    wtr.into_inner().map_err(|e| CsvExportError(e.to_string()))
}

/// Flattened row for user access summary CSV export
#[derive(serde::Serialize)]
struct UserAccessSummaryRow {
    user_id: String,
    external_id: String,
    email: String,
    name: String,
    created_at: String,
    resource_type: String, // "organization", "project", or "api_key"
    resource_id: String,
    resource_slug: String,
    resource_name: String,
    role: String,
    granted_at: String,
    is_active: String, // For API keys
    last_used_at: String,
    last_activity_at: String,
}

/// Export user access summary to CSV format
pub fn export_user_access_summary_csv(
    response: &UserAccessSummaryResponse,
) -> Result<Vec<u8>, CsvExportError> {
    let mut wtr = Writer::from_writer(vec![]);

    let base = |resource_type: &str| UserAccessSummaryRow {
        user_id: response.user_id.to_string(),
        external_id: response.external_id.clone(),
        email: response.email.clone().unwrap_or_default(),
        name: response.name.clone().unwrap_or_default(),
        created_at: response.created_at.to_rfc3339(),
        resource_type: resource_type.to_string(),
        resource_id: String::new(),
        resource_slug: String::new(),
        resource_name: String::new(),
        role: String::new(),
        granted_at: String::new(),
        is_active: String::new(),
        last_used_at: String::new(),
        last_activity_at: response
            .last_activity_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_default(),
    };

    // Organizations
    for org in &response.organizations {
        let mut row = base("organization");
        row.resource_id = org.org_id.to_string();
        row.resource_slug = org.org_slug.clone();
        row.resource_name = org.org_name.clone();
        row.role = org.role.clone();
        row.granted_at = org.granted_at.to_rfc3339();
        row.last_activity_at = org
            .last_activity_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_default();
        wtr.serialize(&row)
            .map_err(|e| CsvExportError(e.to_string()))?;
    }

    // Projects
    for project in &response.projects {
        let mut row = base("project");
        row.resource_id = project.project_id.to_string();
        row.resource_slug = project.project_slug.clone();
        row.resource_name = project.project_name.clone();
        row.role = project.role.clone();
        row.granted_at = project.granted_at.to_rfc3339();
        row.last_activity_at = project
            .last_activity_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_default();
        wtr.serialize(&row)
            .map_err(|e| CsvExportError(e.to_string()))?;
    }

    // API keys
    for api_key in &response.api_keys {
        let mut row = base("api_key");
        row.resource_id = api_key.key_id.to_string();
        row.resource_name = api_key.name.clone();
        row.is_active = api_key.is_active.to_string();
        row.granted_at = api_key.created_at.to_rfc3339();
        row.last_used_at = api_key
            .last_used_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_default();
        wtr.serialize(&row)
            .map_err(|e| CsvExportError(e.to_string()))?;
    }

    // If no resources, output a single row with user info
    if response.organizations.is_empty()
        && response.projects.is_empty()
        && response.api_keys.is_empty()
    {
        wtr.serialize(base(""))
            .map_err(|e| CsvExportError(e.to_string()))?;
    }

    wtr.into_inner().map_err(|e| CsvExportError(e.to_string()))
}

/// Flattened row for stale access CSV export
#[derive(serde::Serialize)]
struct StaleAccessRow {
    category: String, // "stale_user", "stale_api_key", "never_active_user"
    user_id: String,
    external_id: String,
    email: String,
    name: String,
    created_at: String,
    last_activity_at: String,
    days_inactive: i64,
    org_count: i64,
    project_count: i64,
    active_api_keys: i64,
    // For API keys
    key_id: String,
    key_name: String,
    key_prefix: String,
    owner_type: String,
    owner_id: String,
    never_used: String,
}

/// Export stale access report to CSV format
pub fn export_stale_access_csv(response: &StaleAccessResponse) -> Result<Vec<u8>, CsvExportError> {
    let mut wtr = Writer::from_writer(vec![]);

    // Stale users
    for user in &response.stale_users {
        let row = StaleAccessRow {
            category: "stale_user".to_string(),
            user_id: user.user_id.to_string(),
            external_id: user.external_id.clone(),
            email: user.email.clone().unwrap_or_default(),
            name: user.name.clone().unwrap_or_default(),
            created_at: user.created_at.to_rfc3339(),
            last_activity_at: user
                .last_activity_at
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
            days_inactive: user.days_inactive,
            org_count: user.org_count,
            project_count: user.project_count,
            active_api_keys: user.active_api_keys,
            key_id: String::new(),
            key_name: String::new(),
            key_prefix: String::new(),
            owner_type: String::new(),
            owner_id: String::new(),
            never_used: String::new(),
        };
        wtr.serialize(&row)
            .map_err(|e| CsvExportError(e.to_string()))?;
    }

    // Never active users
    for user in &response.never_active_users {
        let row = StaleAccessRow {
            category: "never_active_user".to_string(),
            user_id: user.user_id.to_string(),
            external_id: user.external_id.clone(),
            email: user.email.clone().unwrap_or_default(),
            name: user.name.clone().unwrap_or_default(),
            created_at: user.created_at.to_rfc3339(),
            last_activity_at: String::new(),
            days_inactive: user.days_since_creation,
            org_count: user.org_count,
            project_count: user.project_count,
            active_api_keys: user.active_api_keys,
            key_id: String::new(),
            key_name: String::new(),
            key_prefix: String::new(),
            owner_type: String::new(),
            owner_id: String::new(),
            never_used: String::new(),
        };
        wtr.serialize(&row)
            .map_err(|e| CsvExportError(e.to_string()))?;
    }

    // Stale API keys
    for key in &response.stale_api_keys {
        let row = StaleAccessRow {
            category: "stale_api_key".to_string(),
            user_id: String::new(),
            external_id: String::new(),
            email: String::new(),
            name: String::new(),
            created_at: key.created_at.to_rfc3339(),
            last_activity_at: key.last_used_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
            days_inactive: key.days_inactive,
            org_count: 0,
            project_count: 0,
            active_api_keys: 0,
            key_id: key.key_id.to_string(),
            key_name: key.name.clone(),
            key_prefix: key.key_prefix.clone(),
            owner_type: key.owner_type.clone(),
            owner_id: key.owner_id.to_string(),
            never_used: key.never_used.to_string(),
        };
        wtr.serialize(&row)
            .map_err(|e| CsvExportError(e.to_string()))?;
    }

    wtr.into_inner().map_err(|e| CsvExportError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::*;
    use crate::models::{
        AccessInventorySummary, ApiKeySummary, OrgAccessEntry, OrgAccessReportSummary,
        OrgMemberAccessEntry, StaleAccessSummary, StaleApiKeyEntry, StaleUserEntry,
        UserAccessApiKeyEntry, UserAccessInventoryEntry, UserAccessOrgEntry, UserAccessSummary,
    };

    #[test]
    fn test_export_access_inventory_csv_empty() {
        let response = AccessInventoryResponse {
            generated_at: Utc::now(),
            total_users: 0,
            users: vec![],
            summary: AccessInventorySummary {
                total_organizations: 0,
                total_projects: 0,
                total_org_memberships: 0,
                total_project_memberships: 0,
                total_active_api_keys: 0,
            },
        };

        let csv = export_access_inventory_csv(&response).unwrap();
        // Empty data produces empty CSV (no header without data when using csv crate)
        let csv_str = String::from_utf8(csv).unwrap();
        assert!(csv_str.is_empty());
    }

    #[test]
    fn test_export_access_inventory_csv_with_data() {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let now = Utc::now();

        let response = AccessInventoryResponse {
            generated_at: now,
            total_users: 1,
            users: vec![UserAccessInventoryEntry {
                user_id,
                external_id: "test-user".to_string(),
                email: Some("test@example.com".to_string()),
                name: Some("Test User".to_string()),
                created_at: now,
                organizations: vec![OrgAccessEntry {
                    org_id,
                    org_slug: "test-org".to_string(),
                    org_name: "Test Org".to_string(),
                    role: "admin".to_string(),
                    granted_at: now,
                }],
                projects: vec![],
                api_key_summary: ApiKeySummary {
                    active_count: 2,
                    revoked_count: 1,
                    expired_count: 0,
                    total_count: 3,
                },
                last_activity_at: Some(now),
            }],
            summary: AccessInventorySummary {
                total_organizations: 1,
                total_projects: 0,
                total_org_memberships: 1,
                total_project_memberships: 0,
                total_active_api_keys: 2,
            },
        };

        let csv = export_access_inventory_csv(&response).unwrap();
        let csv_str = String::from_utf8(csv).unwrap();

        assert!(csv_str.contains("test-user"));
        assert!(csv_str.contains("test@example.com"));
        assert!(csv_str.contains("test-org"));
        assert!(csv_str.contains("admin"));
    }

    #[test]
    fn test_export_org_access_report_csv() {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let now = Utc::now();

        let response = OrgAccessReportResponse {
            generated_at: now,
            org_id,
            org_slug: "test-org".to_string(),
            org_name: "Test Org".to_string(),
            members: vec![OrgMemberAccessEntry {
                user_id,
                external_id: "member-1".to_string(),
                email: Some("member@example.com".to_string()),
                name: Some("Member One".to_string()),
                role: "member".to_string(),
                granted_at: now,
                project_access: vec![],
                api_key_summary: ApiKeySummary {
                    active_count: 1,
                    revoked_count: 0,
                    expired_count: 0,
                    total_count: 1,
                },
                last_activity_at: None,
            }],
            api_keys: vec![],
            access_history: vec![],
            summary: OrgAccessReportSummary {
                total_members: 1,
                total_projects: 0,
                total_project_memberships: 0,
                active_api_keys: 1,
                revoked_api_keys: 0,
            },
        };

        let csv = export_org_access_report_csv(&response).unwrap();
        let csv_str = String::from_utf8(csv).unwrap();

        assert!(csv_str.contains("member-1"));
        assert!(csv_str.contains("member@example.com"));
        assert!(csv_str.contains("member"));
    }

    #[test]
    fn test_export_user_access_summary_csv() {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let key_id = Uuid::new_v4();
        let now = Utc::now();

        let response = UserAccessSummaryResponse {
            generated_at: now,
            user_id,
            external_id: "user-123".to_string(),
            email: Some("user@example.com".to_string()),
            name: Some("Test User".to_string()),
            created_at: now,
            organizations: vec![UserAccessOrgEntry {
                org_id,
                org_slug: "org-1".to_string(),
                org_name: "Org One".to_string(),
                role: "owner".to_string(),
                granted_at: now,
                last_activity_at: Some(now),
            }],
            projects: vec![],
            api_keys: vec![UserAccessApiKeyEntry {
                key_id,
                name: "My Key".to_string(),
                key_prefix: "gw_live_".to_string(),
                owner_type: "user".to_string(),
                owner_id: user_id,
                is_active: true,
                created_at: now,
                revoked_at: None,
                expires_at: None,
                last_used_at: Some(now),
            }],
            last_activity_at: Some(now),
            summary: UserAccessSummary {
                total_organizations: 1,
                total_projects: 0,
                active_api_keys: 1,
                revoked_api_keys: 0,
                expired_api_keys: 0,
            },
        };

        let csv = export_user_access_summary_csv(&response).unwrap();
        let csv_str = String::from_utf8(csv).unwrap();

        assert!(csv_str.contains("user-123"));
        assert!(csv_str.contains("organization"));
        assert!(csv_str.contains("api_key"));
        assert!(csv_str.contains("My Key"));
    }

    #[test]
    fn test_export_stale_access_csv() {
        let user_id = Uuid::new_v4();
        let key_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let now = Utc::now();

        let response = StaleAccessResponse {
            generated_at: now,
            inactive_days_threshold: 90,
            cutoff_date: now,
            stale_users: vec![StaleUserEntry {
                user_id,
                external_id: "stale-user".to_string(),
                email: Some("stale@example.com".to_string()),
                name: None,
                created_at: now,
                last_activity_at: Some(now),
                days_inactive: 100,
                org_count: 2,
                project_count: 3,
                active_api_keys: 1,
            }],
            stale_api_keys: vec![StaleApiKeyEntry {
                key_id,
                name: "Old Key".to_string(),
                key_prefix: "gw_test_".to_string(),
                owner_type: "organization".to_string(),
                owner_id,
                created_at: now,
                last_used_at: None,
                days_inactive: 120,
                never_used: true,
            }],
            never_active_users: vec![],
            summary: StaleAccessSummary {
                total_users_scanned: 10,
                stale_users_count: 1,
                never_active_users_count: 0,
                total_api_keys_scanned: 5,
                stale_api_keys_count: 1,
                never_used_api_keys_count: 1,
            },
        };

        let csv = export_stale_access_csv(&response).unwrap();
        let csv_str = String::from_utf8(csv).unwrap();

        assert!(csv_str.contains("stale_user"));
        assert!(csv_str.contains("stale-user"));
        assert!(csv_str.contains("stale_api_key"));
        assert!(csv_str.contains("Old Key"));
    }

    #[test]
    fn test_csv_response_headers() {
        let response = CsvResponse {
            data: b"test,data\n1,2".to_vec(),
            filename: "test-export.csv".to_string(),
        };

        let axum_response = response.into_response();
        let headers = axum_response.headers();

        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap(),
            "text/csv; charset=utf-8"
        );
        assert!(
            headers
                .get(header::CONTENT_DISPOSITION)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("test-export.csv")
        );
    }
}
