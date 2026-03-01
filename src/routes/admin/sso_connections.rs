use axum::{Extension, Json, extract::State};
use serde::Serialize;

use super::error::AdminError;
use crate::{AppState, config::AuthMode, middleware::AuthzContext};

/// SSO connection info (read-only, from config)
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SsoConnection {
    /// Connection name (currently always "default")
    pub name: String,
    /// Type of SSO connection
    #[serde(rename = "type")]
    pub connection_type: String,
    /// OIDC issuer URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    /// OIDC client ID (not the secret)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// Configured scopes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    /// Claim used for user identity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_claim: Option<String>,
    /// Claim used for groups (if configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups_claim: Option<String>,
    /// Whether JIT provisioning is enabled
    pub jit_enabled: bool,
    /// Organization ID users are provisioned into (if JIT is enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    /// Default team ID for provisioned users (if configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_team_id: Option<String>,
    /// Default org role for provisioned users
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_org_role: Option<String>,
    /// Default team role for provisioned users
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_team_role: Option<String>,
    /// Whether to sync memberships on login
    pub sync_memberships_on_login: bool,
}

/// Response containing all configured SSO connections
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SsoConnectionsResponse {
    /// List of SSO connections (currently max 1)
    pub data: Vec<SsoConnection>,
}

/// List configured SSO connections
///
/// Returns read-only information about SSO connections configured in the gateway.
/// SSO connections are defined in the config file (hadrian.toml), not the database.
/// Currently only one OIDC connection is supported per deployment.
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/sso-connections",
    tag = "sso",
    operation_id = "sso_connections_list",
    responses(
        (status = 200, description = "List of SSO connections", body = SsoConnectionsResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sso_connections.list", skip(state, authz))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
) -> Result<Json<SsoConnectionsResponse>, AdminError> {
    // Require system-level read permission for SSO config
    // This is a global endpoint, not org-scoped
    authz.require("sso_connection", "list", None, None, None, None)?;

    let mut connections = Vec::new();

    // Extract SSO connection info from config
    // SSO connections are now per-org. This endpoint shows what auth mode is configured.
    match &state.config.auth.mode {
        #[cfg(feature = "sso")]
        AuthMode::Idp => {
            // IdP mode - SSO connections are per-org, not global
            // Return a placeholder - clients should use the org-specific SSO API
            connections.push(SsoConnection {
                name: "default".to_string(),
                connection_type: "idp".to_string(),
                issuer: None,
                client_id: None,
                scopes: None,
                identity_claim: None,
                groups_claim: None,
                jit_enabled: false,
                organization_id: None,
                default_team_id: None,
                default_org_role: None,
                default_team_role: None,
                sync_memberships_on_login: false,
            });
        }
        AuthMode::Iap(_) => {
            // IAP mode doesn't have SSO connections in the traditional sense
            connections.push(SsoConnection {
                name: "default".to_string(),
                connection_type: "iap".to_string(),
                issuer: None,
                client_id: None,
                scopes: None,
                identity_claim: None,
                groups_claim: None,
                jit_enabled: false,
                organization_id: None,
                default_team_id: None,
                default_org_role: None,
                default_team_role: None,
                sync_memberships_on_login: false,
            });
        }
        _ => {
            // None or ApiKey - no SSO configured
        }
    }

    Ok(Json(SsoConnectionsResponse { data: connections }))
}

/// Get a specific SSO connection by name
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/sso-connections/{name}",
    tag = "sso",
    operation_id = "sso_connection_get",
    params(("name" = String, Path, description = "SSO connection name")),
    responses(
        (status = 200, description = "SSO connection found", body = SsoConnection),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "SSO connection not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.sso_connections.get", skip(state, authz), fields(%name))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<SsoConnection>, AdminError> {
    // Require read permission
    authz.require("sso_connection", "read", Some(&name), None, None, None)?;

    // Currently only "default" is supported
    if name != "default" {
        return Err(AdminError::NotFound(format!(
            "SSO connection '{}' not found",
            name
        )));
    }

    // Extract SSO connection info from config
    // SSO connections are now per-org. This shows the gateway-level auth mode.
    match &state.config.auth.mode {
        #[cfg(feature = "sso")]
        AuthMode::Idp => {
            return Ok(Json(SsoConnection {
                name: "default".to_string(),
                connection_type: "idp".to_string(),
                issuer: None,
                client_id: None,
                scopes: None,
                identity_claim: None,
                groups_claim: None,
                jit_enabled: false,
                organization_id: None,
                default_team_id: None,
                default_org_role: None,
                default_team_role: None,
                sync_memberships_on_login: false,
            }));
        }
        AuthMode::Iap(_) => {
            return Ok(Json(SsoConnection {
                name: "default".to_string(),
                connection_type: "iap".to_string(),
                issuer: None,
                client_id: None,
                scopes: None,
                identity_claim: None,
                groups_claim: None,
                jit_enabled: false,
                organization_id: None,
                default_team_id: None,
                default_org_role: None,
                default_team_role: None,
                sync_memberships_on_login: false,
            }));
        }
        _ => {
            // None or ApiKey - fall through to not found
        }
    }

    Err(AdminError::NotFound(format!(
        "SSO connection '{}' not found",
        name
    )))
}
