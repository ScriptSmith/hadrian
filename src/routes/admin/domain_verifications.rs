//! Admin API endpoints for domain verification.
//!
//! Domain verification allows organizations to prove ownership of email domains
//! via DNS TXT records before SSO can be enforced for users with those domains.

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum_valid::Valid;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::{AuditActor, error::AdminError};
use crate::{
    AppState,
    db::ListParams,
    middleware::{AdminAuth, AuthzContext},
    models::{
        CreateAuditLog, CreateDomainVerification, DomainVerification,
        DomainVerificationInstructions, VerifyDomainResponse,
    },
    services::Services,
};

fn get_services(state: &AppState) -> Result<&Services, AdminError> {
    state.services.as_ref().ok_or(AdminError::ServicesRequired)
}

/// Path parameters for domain verification endpoints nested under an org.
#[derive(Debug, Deserialize)]
pub struct OrgDomainPath {
    pub org_slug: String,
    pub domain_id: Uuid,
}

/// Query parameters for listing domain verifications.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub limit: Option<i64>,
}

/// Response for list endpoint with total count.
#[derive(Debug, serde::Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ListDomainVerificationsResponse {
    pub items: Vec<DomainVerification>,
    pub total: i64,
}

// ============================================================================
// Domain Verification CRUD endpoints
// ============================================================================

/// List all domain verifications for an organization's SSO config
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/sso-config/domains",
    tag = "domain-verifications",
    operation_id = "domain_verifications_list",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("limit" = Option<i64>, Query, description = "Maximum number of items to return"),
    ),
    responses(
        (status = 200, description = "Domain verifications list", body = ListDomainVerificationsResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or SSO config not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.domain_verifications.list", skip(state, authz), fields(%org_slug))]
pub async fn list(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ListDomainVerificationsResponse>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on domain verification
    authz.require(
        "domain_verification",
        "read",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Get the SSO config for this org
    let sso_config = services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SSO config not found for organization '{}'",
                org_slug
            ))
        })?;

    // List domain verifications
    let params = ListParams {
        limit: query.limit.or(Some(100)),
        ..Default::default()
    };
    let items = services
        .domain_verifications
        .list_by_config(sso_config.id, params)
        .await?;
    let total = services
        .domain_verifications
        .count_by_config(sso_config.id)
        .await?;

    Ok(Json(ListDomainVerificationsResponse { items, total }))
}

/// Initiate domain verification for an organization's SSO config
///
/// Creates a new domain verification record with a unique verification token.
/// Returns the verification instructions with DNS record details.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/sso-config/domains",
    tag = "domain-verifications",
    operation_id = "domain_verifications_create",
    params(("org_slug" = String, Path, description = "Organization slug")),
    request_body = CreateDomainVerification,
    responses(
        (status = 201, description = "Domain verification initiated", body = DomainVerification),
        (status = 400, description = "Invalid domain or public domain blocked", body = crate::openapi::ErrorResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization or SSO config not found", body = crate::openapi::ErrorResponse),
        (status = 409, description = "Domain already being verified", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.domain_verifications.create", skip(state, admin_auth, authz, input), fields(%org_slug))]
pub async fn create(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(org_slug): Path<String>,
    Valid(Json(input)): Valid<Json<CreateDomainVerification>>,
) -> Result<(StatusCode, Json<DomainVerification>), AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require create permission on domain verification
    authz.require(
        "domain_verification",
        "create",
        None,
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Get the SSO config for this org
    let sso_config = services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SSO config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Create the domain verification
    let verification = services
        .domain_verifications
        .create(sso_config.id, input.clone())
        .await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "domain_verification.create".to_string(),
            resource_type: "domain_verification".to_string(),
            resource_id: verification.id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "domain": verification.domain,
                "org_sso_config_id": sso_config.id,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok((StatusCode::CREATED, Json(verification)))
}

/// Get a specific domain verification by ID
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/sso-config/domains/{domain_id}",
    tag = "domain-verifications",
    operation_id = "domain_verifications_get",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("domain_id" = Uuid, Path, description = "Domain verification ID"),
    ),
    responses(
        (status = 200, description = "Domain verification found", body = DomainVerification),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization, SSO config, or domain verification not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.domain_verifications.get", skip(state, authz), fields(%org_slug, %domain_id))]
pub async fn get(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(OrgDomainPath {
        org_slug,
        domain_id,
    }): Path<OrgDomainPath>,
) -> Result<Json<DomainVerification>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on domain verification
    authz.require(
        "domain_verification",
        "read",
        Some(&domain_id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Get the SSO config for this org
    let sso_config = services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SSO config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Get the domain verification
    let verification = services
        .domain_verifications
        .get_by_id(domain_id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!("Domain verification '{}' not found", domain_id))
        })?;

    // Verify the domain verification belongs to this SSO config
    if verification.org_sso_config_id != sso_config.id {
        return Err(AdminError::NotFound(format!(
            "Domain verification '{}' not found",
            domain_id
        )));
    }

    Ok(Json(verification))
}

/// Get verification instructions for a domain
#[cfg_attr(feature = "utoipa", utoipa::path(
    get,
    path = "/admin/v1/organizations/{org_slug}/sso-config/domains/{domain_id}/instructions",
    tag = "domain-verifications",
    operation_id = "domain_verifications_get_instructions",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("domain_id" = Uuid, Path, description = "Domain verification ID"),
    ),
    responses(
        (status = 200, description = "Verification instructions", body = DomainVerificationInstructions),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization, SSO config, or domain verification not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.domain_verifications.get_instructions", skip(state, authz), fields(%org_slug, %domain_id))]
pub async fn get_instructions(
    State(state): State<AppState>,
    Extension(authz): Extension<AuthzContext>,
    Path(OrgDomainPath {
        org_slug,
        domain_id,
    }): Path<OrgDomainPath>,
) -> Result<Json<DomainVerificationInstructions>, AdminError> {
    let services = get_services(&state)?;

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Require read permission on domain verification
    authz.require(
        "domain_verification",
        "read",
        Some(&domain_id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Get the SSO config for this org
    let sso_config = services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SSO config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Get the domain verification
    let verification = services
        .domain_verifications
        .get_by_id(domain_id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!("Domain verification '{}' not found", domain_id))
        })?;

    // Verify the domain verification belongs to this SSO config
    if verification.org_sso_config_id != sso_config.id {
        return Err(AdminError::NotFound(format!(
            "Domain verification '{}' not found",
            domain_id
        )));
    }

    Ok(Json(verification.instructions()))
}

/// Delete a domain verification
#[cfg_attr(feature = "utoipa", utoipa::path(
    delete,
    path = "/admin/v1/organizations/{org_slug}/sso-config/domains/{domain_id}",
    tag = "domain-verifications",
    operation_id = "domain_verifications_delete",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("domain_id" = Uuid, Path, description = "Domain verification ID"),
    ),
    responses(
        (status = 200, description = "Domain verification deleted"),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization, SSO config, or domain verification not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.domain_verifications.delete", skip(state, admin_auth, authz), fields(%org_slug, %domain_id))]
pub async fn delete(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(OrgDomainPath {
        org_slug,
        domain_id,
    }): Path<OrgDomainPath>,
) -> Result<Json<()>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get the SSO config for this org
    let sso_config = services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SSO config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Get the domain verification
    let verification = services
        .domain_verifications
        .get_by_id(domain_id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!("Domain verification '{}' not found", domain_id))
        })?;

    // Verify the domain verification belongs to this SSO config
    if verification.org_sso_config_id != sso_config.id {
        return Err(AdminError::NotFound(format!(
            "Domain verification '{}' not found",
            domain_id
        )));
    }

    // Require delete permission on domain verification
    authz.require(
        "domain_verification",
        "delete",
        Some(&domain_id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Capture details for audit log before deletion
    let domain = verification.domain.clone();
    let status = verification.status;

    // Delete the domain verification
    services.domain_verifications.delete(domain_id).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "domain_verification.delete".to_string(),
            resource_type: "domain_verification".to_string(),
            resource_id: domain_id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "domain": domain,
                "status": status.to_string(),
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(()))
}

/// Trigger DNS verification for a domain
///
/// Performs a DNS TXT record lookup to verify domain ownership.
#[cfg_attr(feature = "utoipa", utoipa::path(
    post,
    path = "/admin/v1/organizations/{org_slug}/sso-config/domains/{domain_id}/verify",
    tag = "domain-verifications",
    operation_id = "domain_verifications_verify",
    params(
        ("org_slug" = String, Path, description = "Organization slug"),
        ("domain_id" = Uuid, Path, description = "Domain verification ID"),
    ),
    responses(
        (status = 200, description = "Verification attempt result", body = VerifyDomainResponse),
        (status = 403, description = "Access denied", body = crate::openapi::ErrorResponse),
        (status = 404, description = "Organization, SSO config, or domain verification not found", body = crate::openapi::ErrorResponse),
    )
))]
#[tracing::instrument(name = "admin.domain_verifications.verify", skip(state, admin_auth, authz), fields(%org_slug, %domain_id))]
pub async fn verify(
    State(state): State<AppState>,
    Extension(admin_auth): Extension<AdminAuth>,
    Extension(authz): Extension<AuthzContext>,
    Path(OrgDomainPath {
        org_slug,
        domain_id,
    }): Path<OrgDomainPath>,
) -> Result<Json<VerifyDomainResponse>, AdminError> {
    let services = get_services(&state)?;
    let actor = AuditActor::from(&admin_auth);

    // Get org by slug
    let org = services
        .organizations
        .get_by_slug(&org_slug)
        .await?
        .ok_or_else(|| AdminError::NotFound(format!("Organization '{}' not found", org_slug)))?;

    // Get the SSO config for this org
    let sso_config = services
        .org_sso_configs
        .get_by_org_id(org.id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!(
                "SSO config not found for organization '{}'",
                org_slug
            ))
        })?;

    // Get the domain verification to verify it belongs to this config
    let verification = services
        .domain_verifications
        .get_by_id(domain_id)
        .await?
        .ok_or_else(|| {
            AdminError::NotFound(format!("Domain verification '{}' not found", domain_id))
        })?;

    // Verify the domain verification belongs to this SSO config
    if verification.org_sso_config_id != sso_config.id {
        return Err(AdminError::NotFound(format!(
            "Domain verification '{}' not found",
            domain_id
        )));
    }

    // Require verify permission
    authz.require(
        "domain_verification",
        "verify",
        Some(&domain_id.to_string()),
        Some(&org.id.to_string()),
        None,
        None,
    )?;

    // Capture previous status for audit log
    let previous_status = verification.status;

    // Perform the verification
    let result = services.domain_verifications.verify(domain_id).await?;

    // Log audit event (fire-and-forget)
    let _ = services
        .audit_logs
        .create(CreateAuditLog {
            actor_type: actor.actor_type,
            actor_id: actor.actor_id,
            action: "domain_verification.verify".to_string(),
            resource_type: "domain_verification".to_string(),
            resource_id: domain_id,
            org_id: Some(org.id),
            project_id: None,
            details: json!({
                "domain": result.verification.domain,
                "previous_status": previous_status.to_string(),
                "new_status": result.verification.status.to_string(),
                "verified": result.verified,
                "dns_record_found": result.dns_record_found,
            }),
            ip_address: None,
            user_agent: None,
        })
        .await;

    Ok(Json(result))
}
