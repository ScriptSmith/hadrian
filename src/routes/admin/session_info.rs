//! Session information endpoint for debugging authentication and access.
//!
//! This module provides a comprehensive session debugging endpoint that shows
//! users information about their current authentication state, memberships,
//! and access levels. Useful for troubleshooting SSO and RBAC issues.

use axum::{Extension, Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::error::AdminError;
use crate::{
    AppState,
    middleware::{AdminAuth, AuthzContext},
    services::Services,
};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Comprehensive session and access information for debugging.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SessionInfoResponse {
    /// Identity information from the authentication source
    pub identity: IdentityInfo,

    /// Database user information (if user exists in database)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserInfo>,

    /// Organization memberships with roles
    pub organizations: Vec<OrgMembershipInfo>,

    /// Team memberships with roles
    pub teams: Vec<TeamMembershipInfo>,

    /// Project access (direct memberships)
    pub projects: Vec<ProjectMembershipInfo>,

    /// SSO connection information (if authenticated via SSO)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sso_connection: Option<SsoConnectionInfo>,

    /// Authentication method used for this session
    pub auth_method: String,

    /// Server timestamp for debugging timezone issues
    pub server_time: DateTime<Utc>,
}

/// Identity information from the authentication source (IdP or proxy).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct IdentityInfo {
    /// External identity ID (from IdP)
    pub external_id: String,

    /// Email address (if provided by IdP)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Display name (if provided by IdP)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Roles from IdP (e.g., super_admin, org_admin, team_admin, user)
    pub roles: Vec<String>,

    /// Raw IdP groups before any mapping.
    /// These are the exact values from the IdP (e.g., OIDC groups claim).
    /// Useful for debugging SSO group mappings.
    pub idp_groups: Vec<String>,
}

/// Database user information.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UserInfo {
    /// Internal user ID
    pub id: Uuid,

    /// External identity ID (links to IdP)
    pub external_id: String,

    /// Email address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// When the user was created in the database
    pub created_at: DateTime<Utc>,

    /// When the user was last updated
    pub updated_at: DateTime<Utc>,
}

/// Organization membership information.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgMembershipInfo {
    /// Organization ID
    pub org_id: Uuid,

    /// Organization slug
    pub org_slug: String,

    /// Organization name
    pub org_name: String,

    /// User's role in this organization
    pub role: String,

    /// When the membership was created
    pub joined_at: DateTime<Utc>,
}

/// Team membership information.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TeamMembershipInfo {
    /// Team ID
    pub team_id: Uuid,

    /// Team slug
    pub team_slug: String,

    /// Team name
    pub team_name: String,

    /// Organization the team belongs to
    pub org_slug: String,

    /// User's role in this team
    pub role: String,

    /// When the membership was created
    pub joined_at: DateTime<Utc>,
}

/// Project membership information.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProjectMembershipInfo {
    /// Project ID
    pub project_id: Uuid,

    /// Project slug
    pub project_slug: String,

    /// Project name
    pub project_name: String,

    /// Organization the project belongs to
    pub org_slug: String,

    /// User's role in this project
    pub role: String,

    /// When the membership was created
    pub joined_at: DateTime<Utc>,
}

/// SSO connection information (from config).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SsoConnectionInfo {
    /// Connection name
    pub name: String,

    /// Connection type (oidc, proxy_auth)
    #[serde(rename = "type")]
    pub connection_type: String,

    /// OIDC issuer URL (if OIDC)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,

    /// Claim used for groups (if configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups_claim: Option<String>,

    /// Whether JIT provisioning is enabled
    pub jit_enabled: bool,
}

/// Get comprehensive session and access information.
///
/// Returns detailed information about the current user's authentication state,
/// database profile, organization/team/project memberships, and SSO configuration.
/// Useful for debugging authentication and authorization issues.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/session-info",
    tag = "session",
    operation_id = "session_info_get",
    responses(
        (status = 200, description = "Session information", body = SessionInfoResponse),
        (status = 401, description = "Not authenticated", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.session_info.get", skip(state, admin_auth, authz))]
pub async fn get(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<SessionInfoResponse>, AdminError> {
    authz.require("session_info", "read", None, None, None, None)?;

    let services = get_services(&state)?;
    let identity = &admin_auth.identity;

    // Build identity info
    let identity_info = IdentityInfo {
        external_id: identity.external_id.clone(),
        email: identity.email.clone(),
        name: identity.name.clone(),
        roles: identity.roles.clone(),
        idp_groups: identity.idp_groups.clone(),
    };

    // Get user info from database if user exists
    let user_info = if let Some(user_id) = identity.user_id {
        services.users.get_by_id(user_id).await?.map(|u| UserInfo {
            id: u.id,
            external_id: u.external_id,
            email: u.email,
            name: u.name,
            created_at: u.created_at,
            updated_at: u.updated_at,
        })
    } else {
        None
    };

    // Get organization memberships (using the already-rich UserOrgMembership type)
    let organizations = if let Some(user_id) = identity.user_id {
        services
            .users
            .get_org_memberships_for_user(user_id)
            .await?
            .into_iter()
            .map(|m| OrgMembershipInfo {
                org_id: m.org_id,
                org_slug: m.org_slug,
                org_name: m.org_name,
                role: m.role,
                joined_at: m.joined_at,
            })
            .collect()
    } else {
        Vec::new()
    };

    // Get team memberships (TeamMembership already has org_id, need to get org slug)
    let mut teams = Vec::new();
    if let Some(user_id) = identity.user_id {
        let memberships = services
            .users
            .get_team_memberships_for_user(user_id)
            .await?;
        for m in memberships {
            // Get org slug for context
            let org_slug = if let Some(org) = services.organizations.get_by_id(m.org_id).await? {
                org.slug
            } else {
                m.org_id.to_string()
            };

            teams.push(TeamMembershipInfo {
                team_id: m.team_id,
                team_slug: m.team_slug,
                team_name: m.team_name,
                org_slug,
                role: m.role,
                joined_at: m.joined_at,
            });
        }
    }

    // Get project memberships (UserProjectMembership has org_id, need to get org slug)
    let mut projects = Vec::new();
    if let Some(user_id) = identity.user_id {
        let memberships = services
            .users
            .get_project_memberships_for_user(user_id)
            .await?;
        for m in memberships {
            // Get org slug for context
            let org_slug = if let Some(org) = services.organizations.get_by_id(m.org_id).await? {
                org.slug
            } else {
                m.org_id.to_string()
            };

            projects.push(ProjectMembershipInfo {
                project_id: m.project_id,
                project_slug: m.project_slug,
                project_name: m.project_name,
                org_slug,
                role: m.role,
                joined_at: m.joined_at,
            });
        }
    }

    // Get SSO connection info from config
    // SSO connections are per-org. For IAP mode, expose the connection type.
    let sso_connection = match &state.config.auth.mode {
        crate::config::AuthMode::Iap(_) => Some(SsoConnectionInfo {
            name: "default".to_string(),
            connection_type: "iap".to_string(),
            issuer: None,
            groups_claim: None,
            jit_enabled: false,
        }),
        _ => None,
    };

    // Determine auth method
    let auth_method = match &state.config.auth.mode {
        crate::config::AuthMode::None => "none".to_string(),
        crate::config::AuthMode::ApiKey => "api_key".to_string(),
        #[cfg(feature = "sso")]
        crate::config::AuthMode::Idp => "idp".to_string(),
        crate::config::AuthMode::Iap(_) => "iap".to_string(),
    };

    Ok(Json(SessionInfoResponse {
        identity: identity_info,
        user: user_info,
        organizations,
        teams,
        projects,
        sso_connection,
        auth_method,
        server_time: Utc::now(),
    }))
}
