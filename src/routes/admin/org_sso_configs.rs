//! Admin API endpoints for per-organization SSO configuration.
//!
//! Each organization can have at most one SSO configuration, enabling IT admins
//! to configure their own identity provider (OIDC or SAML) via the Admin UI.

#[cfg(feature = "saml")]
use axum::response::{IntoResponse, Response};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use axum_valid::Valid;
#[cfg(feature = "saml")]
use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(feature = "saml")]
use validator::Validate;

use super::{AuditActor, error::AdminError};
use crate::{
    AppState,
    middleware::{AdminAuth, AuthzContext, ClientInfo},
    models::{
        CreateAuditLog, CreateOrgSsoConfig, OrgSsoConfig, SsoProviderType, UpdateOrgSsoConfig,
    },
    secrets::SecretManager,
    services::Services,
};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

fn get_secret_manager(state: &AppState) -> Result<&dyn SecretManager, AdminError> {
    state
        .secrets
        .as_ref()
        .map(|s| s.as_ref())
        .ok_or(AdminError::NotConfigured(
            "Secret manager not configured".to_string(),
        ))
}

// ============================================================================
// SAML Validation Helpers
// ============================================================================

/// Validate SAML configuration fields.
///
/// For SAML provider type, either:
/// - `saml_metadata_url` must be provided (IdP metadata will be auto-fetched), OR
/// - All manual fields must be provided: `saml_idp_entity_id`, `saml_idp_sso_url`,
///   `saml_idp_certificate`, and `saml_sp_entity_id`
fn validate_saml_config(
    provider_type: SsoProviderType,
    saml_idp_entity_id: Option<&str>,
    saml_idp_sso_url: Option<&str>,
    saml_idp_certificate: Option<&str>,
    saml_sp_entity_id: Option<&str>,
    saml_metadata_url: Option<&str>,
) -> Result<(), AdminError> {
    if provider_type != SsoProviderType::Saml {
        return Ok(());
    }

    // If metadata URL is provided, we can auto-fetch IdP config at auth time
    if saml_metadata_url.is_some() {
        // SP entity ID is still required even with metadata URL
        if saml_sp_entity_id.is_none() || saml_sp_entity_id.map(|s| s.is_empty()).unwrap_or(true) {
            return Err(AdminError::SamlValidation(
                "SAML SP entity ID is required".to_string(),
            ));
        }
        return Ok(());
    }

    // Without metadata URL, all manual fields are required
    let mut missing_fields = Vec::new();

    if saml_idp_entity_id.is_none() || saml_idp_entity_id.map(|s| s.is_empty()).unwrap_or(true) {
        missing_fields.push("saml_idp_entity_id");
    }
    if saml_idp_sso_url.is_none() || saml_idp_sso_url.map(|s| s.is_empty()).unwrap_or(true) {
        missing_fields.push("saml_idp_sso_url");
    }
    if saml_idp_certificate.is_none() || saml_idp_certificate.map(|s| s.is_empty()).unwrap_or(true)
    {
        missing_fields.push("saml_idp_certificate");
    }
    if saml_sp_entity_id.is_none() || saml_sp_entity_id.map(|s| s.is_empty()).unwrap_or(true) {
        missing_fields.push("saml_sp_entity_id");
    }

    if !missing_fields.is_empty() {
        return Err(AdminError::SamlValidation(format!(
            "SAML configuration requires either saml_metadata_url OR all of: {}",
            missing_fields.join(", ")
        )));
    }

    // Validate certificate format
    if let Some(cert) = saml_idp_certificate {
        validate_x509_certificate(cert)?;
    }

    Ok(())
}

/// Validate that a certificate is in valid PEM-encoded X.509 format.
fn validate_x509_certificate(pem: &str) -> Result<(), AdminError> {
    let pem = pem.trim();

    // Check for PEM headers
    if !pem.contains("-----BEGIN CERTIFICATE-----") {
        return Err(AdminError::SamlValidation(
            "Certificate must be in PEM format (missing BEGIN CERTIFICATE header)".to_string(),
        ));
    }

    if !pem.contains("-----END CERTIFICATE-----") {
        return Err(AdminError::SamlValidation(
            "Certificate must be in PEM format (missing END CERTIFICATE header)".to_string(),
        ));
    }

    // Try to parse the certificate using x509-cert crate
    // Extract the base64 content between headers
    let cert_content: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----BEGIN") && !line.starts_with("-----END"))
        .collect::<Vec<_>>()
        .join("");

    // Validate base64 decoding
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.decode(&cert_content).map_err(|e| {
        AdminError::SamlValidation(format!("Certificate contains invalid base64: {}", e))
    })?;

    Ok(())
}

// ============================================================================
// SAML Metadata Types
// ============================================================================

#[cfg(feature = "saml")]
/// Request to parse SAML IdP metadata from a URL.
#[derive(Debug, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ParseSamlMetadataRequest {
    /// URL to fetch IdP metadata from (must be HTTPS for security)
    #[validate(length(min = 1, max = 512), url)]
    pub metadata_url: String,
}

#[cfg(feature = "saml")]
/// Parsed SAML IdP configuration extracted from metadata.
///
/// This is returned for admin review before saving - use the individual fields
/// to populate the SSO config.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ParsedSamlIdpConfig {
    /// IdP entity identifier
    pub entity_id: String,
    /// IdP Single Sign-On service URL
    pub sso_url: String,
    /// IdP Single Logout service URL (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slo_url: Option<String>,
    /// X.509 certificates for signature verification (PEM format)
    /// Multiple certificates may be present for key rollover
    pub certificates: Vec<String>,
    /// Supported NameID formats
    pub name_id_formats: Vec<String>,
}

// ============================================================================
// Organization SSO Config CRUD endpoints
// ============================================================================

/// Get the SSO configuration for an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/sso-config",
    tag = "sso",
    operation_id = "org_sso_config_get",
    params(("org_slug" = String, Path, description = "Organization slug")),
    responses(
        (status = 200, description = "SSO config found", body = OrgSsoConfig),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or SSO config not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.org_sso_configs.get", skip(state, authz), fields(%org_slug))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
) -> Result<Json<OrgSsoConfig>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on org SSO config
    authz.require(
        "org_sso_config",
        "read",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Get the SSO config for this org
    let config = services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SSO config not found for organization '{}'",
                org_slug
            ))
        })?;

    Ok(Json(config))
}

/// Create a new SSO configuration for an organization
///
/// Each organization can have at most one SSO configuration. Creating a config
/// for an organization that already has one will result in a 409 Conflict error.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/sso-config",
    tag = "sso",
    operation_id = "org_sso_config_create",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = CreateOrgSsoConfig,
    responses(
        (status = 201, description = "SSO config created", body = OrgSsoConfig),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Organization already has an SSO config", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.org_sso_configs.create", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<CreateOrgSsoConfig>>,
) -> Result<(StatusCode, Json<OrgSsoConfig>), AdminError> {
    let services = get_services(&state)?;
    let secret_manager = get_secret_manager(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require create permission on org SSO config
    authz.require(
        "org_sso_config",
        "create",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Check if org already has an SSO config
    if services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .is_some()
    {
        return Err(AdminError::Conflict(format!(
            "Organization '{}' already has an SSO configuration",
            org_slug
        )));
    }

    // Validate default_team_id belongs to the org if provided
    if let Some(team_id) = input.default_team_id {
        let team = services
            .teams
            .get_by_id(team_id)
            .await?
            .ok_or_else(|| AdminError::NotFound(format!("Team '{}' not found", team_id)))?;
        if team.org_id != org.id {
            return Err(AdminError::BadRequest(
                "Team does not belong to this organization".to_string(),
            ));
        }
    }

    // Validate SAML configuration if provider type is SAML
    validate_saml_config(
        input.provider_type,
        input.saml_idp_entity_id.as_deref(),
        input.saml_idp_sso_url.as_deref(),
        input.saml_idp_certificate.as_deref(),
        input.saml_sp_entity_id.as_deref(),
        input.saml_metadata_url.as_deref(),
    )?;

    // SSRF-validate OIDC URLs at input time
    if input.provider_type == SsoProviderType::Oidc {
        let allow_loopback = state.config.server.allow_loopback_urls;
        if let Some(ref issuer) = input.issuer {
            crate::validation::validate_base_url(issuer, allow_loopback)
                .map_err(|e| AdminError::Validation(format!("Invalid issuer URL: {e}")))?;
        }
        if let Some(ref discovery_url) = input.discovery_url {
            crate::validation::validate_base_url(discovery_url, allow_loopback)
                .map_err(|e| AdminError::Validation(format!("Invalid discovery URL: {e}")))?;
        }
    }

    // Create the SSO config
    let config = services
        .org_sso_configs
        .create(org.id, input.clone(), secret_manager)
        .await?;

    // Auto-verify domains from bootstrap config
    // This enables E2E testing and initial setup without DNS verification
    if let Some(bootstrap) = &state.config.auth.bootstrap {
        let auto_verify_domains = &bootstrap.auto_verify_domains;
        if !auto_verify_domains.is_empty() {
            for domain in &input.allowed_email_domains {
                let domain_lower = domain.to_lowercase();
                let should_auto_verify = auto_verify_domains
                    .iter()
                    .any(|d| d.to_lowercase() == domain_lower);

                if should_auto_verify {
                    match services
                        .domain_verifications
                        .create_auto_verified(config.id, &domain_lower)
                        .await
                    {
                        Ok(verification) => {
                            tracing::info!(
                                domain = %domain_lower,
                                sso_config_id = %config.id,
                                verification_id = %verification.id,
                                "Auto-verified domain from bootstrap config"
                            );
                        }
                        Err(e) => {
                            // Log but don't fail the SSO config creation
                            // Domain might already exist from a previous config
                            tracing::warn!(
                                domain = %domain_lower,
                                sso_config_id = %config.id,
                                error = %e,
                                "Failed to auto-verify domain from bootstrap config"
                            );
                        }
                    }
                }
            }
        }
    }

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "org_sso_config.create".to_string(),
            resource_type: "org_sso_config".to_string(),
            resource_id: config.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "provider_type": config.provider_type.to_string(),
                "issuer": config.issuer,
                "enforcement_mode": config.enforcement_mode.to_string(),
                "enabled": config.enabled,
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    // Sync gateway JWT registry for per-org API JWT auth
    if config.enabled
        && config.provider_type == SsoProviderType::Oidc
        && let Some(registry) = &state.gateway_jwt_registry
    {
        // Clear any negative cache entry for this issuer so lazy-load can pick it up
        if let Some(ref issuer) = config.issuer {
            registry.invalidate_negative_cache(issuer).await;
        }
        if let Err(e) = registry
            .register_from_sso_config(
                &config,
                &state.http_client,
                state.config.server.allow_loopback_urls,
            )
            .await
        {
            tracing::warn!(
                org_id = %org.id,
                error = %e,
                "Failed to register gateway JWT validator (will lazy-load)"
            );
        }
    }

    Ok((StatusCode::CREATED, Json(config)))
}

/// Update the SSO configuration for an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    patch,
    path = "/admin/v1/organizations/{org_slug}/sso-config",
    tag = "sso",
    operation_id = "org_sso_config_update",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = UpdateOrgSsoConfig,
    responses(
        (status = 200, description = "SSO config updated", body = OrgSsoConfig),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or SSO config not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.org_sso_configs.update", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn update(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<UpdateOrgSsoConfig>>,
) -> Result<Json<OrgSsoConfig>, AdminError> {
    let services = get_services(&state)?;
    let secret_manager = get_secret_manager(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get existing config
    let existing = services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SSO config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Require update permission
    authz.require(
        "org_sso_config",
        "update",
        Some(&existing.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Validate default_team_id belongs to the org if being updated
    if let Some(Some(team_id)) = input.default_team_id {
        let team = services
            .teams
            .get_by_id(team_id)
            .await?
            .ok_or_else(|| AdminError::NotFound(format!("Team '{}' not found", team_id)))?;
        if team.org_id != org.id {
            return Err(AdminError::BadRequest(
                "Team does not belong to this organization".to_string(),
            ));
        }
    }

    // Validate SAML configuration if changing to SAML or updating SAML fields
    // Merge existing values with updates to determine final state
    let final_provider_type = input.provider_type.unwrap_or(existing.provider_type);
    let final_idp_entity_id = match &input.saml_idp_entity_id {
        Some(v) => v.as_deref(),
        None => existing.saml_idp_entity_id.as_deref(),
    };
    let final_idp_sso_url = match &input.saml_idp_sso_url {
        Some(v) => v.as_deref(),
        None => existing.saml_idp_sso_url.as_deref(),
    };
    let final_idp_certificate = match &input.saml_idp_certificate {
        Some(v) => v.as_deref(),
        None => existing.saml_idp_certificate.as_deref(),
    };
    let final_sp_entity_id = match &input.saml_sp_entity_id {
        Some(v) => v.as_deref(),
        None => existing.saml_sp_entity_id.as_deref(),
    };
    let final_metadata_url = match &input.saml_metadata_url {
        Some(v) => v.as_deref(),
        None => existing.saml_metadata_url.as_deref(),
    };
    validate_saml_config(
        final_provider_type,
        final_idp_entity_id,
        final_idp_sso_url,
        final_idp_certificate,
        final_sp_entity_id,
        final_metadata_url,
    )?;

    // SSRF-validate OIDC URLs at input time (only check fields being updated)
    if final_provider_type == SsoProviderType::Oidc {
        let allow_loopback = state.config.server.allow_loopback_urls;
        if let Some(ref issuer) = input.issuer {
            crate::validation::validate_base_url(issuer, allow_loopback)
                .map_err(|e| AdminError::Validation(format!("Invalid issuer URL: {e}")))?;
        }
        if let Some(Some(ref discovery_url)) = input.discovery_url {
            crate::validation::validate_base_url(discovery_url, allow_loopback)
                .map_err(|e| AdminError::Validation(format!("Invalid discovery URL: {e}")))?;
        }
    }

    // Update the SSO config
    let updated = services
        .org_sso_configs
        .update(existing.id, input.clone(), secret_manager)
        .await?;

    // Log audit event (fire-and-forget)
    // Note: We log what was requested to change, not what the final state is
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "org_sso_config.update".to_string(),
            resource_type: "org_sso_config".to_string(),
            resource_id: existing.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "issuer": input.issuer,
                "provider_type": input.provider_type.map(|p| p.to_string()),
                "enforcement_mode": input.enforcement_mode.map(|e| e.to_string()),
                "enabled": input.enabled,
                "client_secret_changed": input.client_secret.is_some(),
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    // Sync gateway JWT registry for per-org API JWT auth
    if let Some(registry) = &state.gateway_jwt_registry {
        if updated.enabled && updated.provider_type == SsoProviderType::Oidc {
            // Clear negative cache so the updated config can be found
            if let Some(ref issuer) = updated.issuer {
                registry.invalidate_negative_cache(issuer).await;
            }
            if let Err(e) = registry
                .register_from_sso_config(
                    &updated,
                    &state.http_client,
                    state.config.server.allow_loopback_urls,
                )
                .await
            {
                tracing::warn!(
                    org_id = %org.id,
                    error = %e,
                    "Failed to update gateway JWT validator (will lazy-load)"
                );
            }
        } else {
            // Config disabled or not OIDC — remove any existing validator
            registry.remove(org.id).await;
        }
    }

    Ok(Json(updated))
}

/// Delete the SSO configuration for an organization
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/sso-config",
    tag = "sso",
    operation_id = "org_sso_config_delete",
    params(("org_slug" = String, Path, description = "Organization slug")),
    responses(
        (status = 200, description = "SSO config deleted"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or SSO config not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.org_sso_configs.delete", skip(state, admin_auth, authz), fields(%org_slug))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Extension(client_info): Extension<ClientInfo>,
    Path(org_slug): Path<String>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let secret_manager = get_secret_manager(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get existing config
    let existing = services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SSO config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Require delete permission
    authz.require(
        "org_sso_config",
        "delete",
        Some(&existing.id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Capture details for audit log before deletion
    let issuer = existing.issuer.clone();
    let provider_type = existing.provider_type;

    // Delete the SSO config
    services
        .org_sso_configs
        .delete(existing.id, secret_manager)
        .await?;

    // Remove from gateway JWT registry
    if let Some(registry) = &state.gateway_jwt_registry {
        registry.remove(org.id).await;
    }

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "org_sso_config.delete".to_string(),
            resource_type: "org_sso_config".to_string(),
            resource_id: existing.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "issuer": issuer,
                "provider_type": provider_type.to_string(),
            }),
            ip_address: client_info.ip_address,
            user_agent: client_info.user_agent,
        })
        .await;

    Ok(Json(()))
}

// ============================================================================
// SAML Metadata Endpoints
// ============================================================================

/// Parse SAML IdP metadata from a URL
///
/// Fetches IdP metadata from the provided URL and extracts configuration fields.
/// This does NOT save the configuration - it returns the parsed data for admin review.
/// Use the returned values to populate the SSO config fields.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/sso-config/saml/parse-metadata",
    tag = "sso",
    operation_id = "org_sso_config_parse_saml_metadata",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = ParseSamlMetadataRequest,
    responses(
        (status = 200, description = "Metadata parsed successfully", body = ParsedSamlIdpConfig),
        (status = 400, description = "Invalid metadata URL or failed to parse", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization not found", body = crate::openapi::ErrorResponse),
    )
))]
#[cfg(feature = "saml")]
#[tracing::instrument(name = "admin.org_sso_configs.parse_saml_metadata", skip(state, authz, input), fields(%org_slug))]
pub async fn parse_saml_metadata(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<ParseSamlMetadataRequest>>,
) -> Result<Json<ParsedSamlIdpConfig>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on org SSO config (parsing metadata is a read-like operation)
    authz.require(
        "org_sso_config",
        "read",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Enforce HTTPS for metadata URLs (SSRF protection)
    crate::validation::require_https(&input.metadata_url)
        .map_err(|e| AdminError::Validation(format!("SAML metadata URL must use HTTPS: {e}")))?;

    // Fetch and parse the metadata
    let client = reqwest::Client::new();
    tracing::debug!(url = %input.metadata_url, "Fetching SAML IdP metadata");

    let response = client.get(&input.metadata_url).send().await.map_err(|e| {
        tracing::error!(error = %e, url = %input.metadata_url, "Failed to fetch SAML metadata");
        AdminError::SamlMetadata(format!("Failed to fetch metadata: {}", e))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        tracing::error!(status = %status, "SAML metadata endpoint returned error");
        return Err(AdminError::SamlMetadata(format!(
            "Metadata endpoint returned HTTP {}",
            status
        )));
    }

    let metadata_xml = response.text().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to read SAML metadata response");
        AdminError::SamlMetadata(format!("Failed to read metadata: {}", e))
    })?;

    // Parse the XML using samael
    let entity_descriptor: samael::metadata::EntityDescriptor =
        samael::metadata::de::from_str(&metadata_xml).map_err(|e| {
            tracing::error!(error = %e, "Failed to parse SAML metadata XML");
            AdminError::SamlMetadata(format!("Failed to parse metadata: {}", e))
        })?;

    // Extract IdP configuration from the parsed metadata
    let parsed = extract_idp_config_from_metadata(&entity_descriptor)?;

    Ok(Json(parsed))
}

#[cfg(feature = "saml")]
/// Extract IdP configuration from parsed SAML metadata.
fn extract_idp_config_from_metadata(
    entity: &samael::metadata::EntityDescriptor,
) -> Result<ParsedSamlIdpConfig, AdminError> {
    let entity_id = entity
        .entity_id
        .clone()
        .ok_or_else(|| AdminError::SamlMetadata("Metadata missing entityID".to_string()))?;

    // Find the IDPSSODescriptor
    let idp_descriptor = entity
        .idp_sso_descriptors
        .as_ref()
        .and_then(|d| d.first())
        .ok_or_else(|| AdminError::SamlMetadata("Metadata missing IDPSSODescriptor".to_string()))?;

    // Extract SSO URL (prefer HTTP-Redirect binding)
    let sso_url = idp_descriptor
        .single_sign_on_services
        .iter()
        .find(|s| s.binding.contains("HTTP-Redirect"))
        .or_else(|| idp_descriptor.single_sign_on_services.first())
        .map(|s| s.location.clone())
        .ok_or_else(|| {
            AdminError::SamlMetadata("Metadata missing SingleSignOnService".to_string())
        })?;

    // Extract SLO URL (optional)
    let slo_url = idp_descriptor
        .single_logout_services
        .iter()
        .find(|s| s.binding.contains("HTTP-Redirect"))
        .or_else(|| idp_descriptor.single_logout_services.first())
        .map(|s| s.location.clone());

    // Extract certificates
    let mut certificates = Vec::new();
    for kd in &idp_descriptor.key_descriptors {
        // Only include signing certificates (use = "signing" or no use specified)
        let is_signing = kd.key_use.as_ref().map(|u| u == "signing").unwrap_or(true);

        if is_signing && let Some(x509_data) = &kd.key_info.x509_data {
            for cert in &x509_data.certificates {
                // Convert to PEM format
                let pem = format!(
                    "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----",
                    cert
                );
                certificates.push(pem);
            }
        }
    }

    if certificates.is_empty() {
        return Err(AdminError::SamlMetadata(
            "Metadata missing signing certificate".to_string(),
        ));
    }

    // Extract NameID formats
    let name_id_formats = idp_descriptor.name_id_formats.clone();

    Ok(ParsedSamlIdpConfig {
        entity_id,
        sso_url,
        slo_url,
        certificates,
        name_id_formats,
    })
}

#[cfg(feature = "saml")]
/// Get SP metadata for IdP configuration
///
/// Returns Hadrian's Service Provider metadata XML that can be imported into
/// the IdP to configure the SAML integration automatically. The metadata includes
/// the SP entity ID, Assertion Consumer Service URL, and supported bindings.
///
/// This endpoint requires an existing SAML SSO configuration for the organization.
#[cfg(feature = "saml")]
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/sso-config/saml/sp-metadata",
    tag = "sso",
    operation_id = "org_sso_config_get_sp_metadata",
    params(("org_slug" = String, Path, description = "Organization slug")),
    responses(
        (status = 200, description = "SP metadata XML", content_type = "application/samlmetadata+xml"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or SAML config not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.org_sso_configs.get_sp_metadata", skip(state, authz), fields(%org_slug))]
pub async fn get_sp_metadata(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
) -> Result<Response, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on org SSO config
    authz.require(
        "org_sso_config",
        "read",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Get existing config
    let config = services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SSO config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Verify it's a SAML config
    if config.provider_type != SsoProviderType::Saml {
        return Err(AdminError::BadRequest(
            "SSO config is not a SAML configuration".to_string(),
        ));
    }

    // Get SP entity ID from config
    let sp_entity_id = config
        .saml_sp_entity_id
        .ok_or_else(|| AdminError::BadRequest("SAML SP entity ID not configured".to_string()))?;

    // Construct ACS URL from server config
    let protocol = if state.config.server.tls.is_some() {
        "https"
    } else {
        "http"
    };
    let acs_url = format!(
        "{}://{}:{}/auth/saml/acs",
        protocol, state.config.server.host, state.config.server.port
    );

    // Generate minimal SP metadata
    let metadata = generate_sp_metadata(&sp_entity_id, &acs_url);

    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "application/samlmetadata+xml",
        )],
        metadata,
    )
        .into_response())
}

#[cfg(feature = "saml")]
/// Generate SP metadata XML.
fn generate_sp_metadata(sp_entity_id: &str, acs_url: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<md:EntityDescriptor xmlns:md="urn:oasis:names:tc:SAML:2.0:metadata" entityID="{}">
  <md:SPSSODescriptor protocolSupportEnumeration="urn:oasis:names:tc:SAML:2.0:protocol">
    <md:NameIDFormat>urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress</md:NameIDFormat>
    <md:AssertionConsumerService
        Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"
        Location="{}"
        index="0"/>
  </md:SPSSODescriptor>
</md:EntityDescriptor>"#,
        sp_entity_id, acs_url
    )
}
