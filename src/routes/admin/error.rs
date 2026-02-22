use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

#[cfg(feature = "sso")]
use crate::services::{DomainVerificationError, OrgScimConfigError, OrgSsoConfigError};
use crate::{
    auth::Identity, authz::AuthzError, db::DbError, middleware::AdminAuth, models::AuditActorType,
    observability::metrics, openapi::ErrorResponse, services::OrgRbacPolicyError,
};

/// Audit actor information extracted from admin authentication.
///
/// This struct provides the correct actor type and ID for audit logging
/// based on the authenticated admin user.
#[derive(Debug, Clone)]
pub struct AuditActor {
    /// The type of actor (User if user_id is present, System otherwise)
    pub actor_type: AuditActorType,
    /// The internal user ID if the authenticated user is linked to a user record
    pub actor_id: Option<Uuid>,
}

impl AuditActor {
    /// Create an audit actor from an Identity.
    ///
    /// If the identity has a linked internal user_id, the actor type is User.
    /// Otherwise, it falls back to System (authenticated but not linked to internal user).
    pub fn from_identity(identity: &Identity) -> Self {
        if let Some(user_id) = identity.user_id {
            Self {
                actor_type: AuditActorType::User,
                actor_id: Some(user_id),
            }
        } else {
            // Authenticated via OIDC/Zero Trust but not linked to internal user record
            Self {
                actor_type: AuditActorType::System,
                actor_id: None,
            }
        }
    }
}

impl From<&AdminAuth> for AuditActor {
    fn from(auth: &AdminAuth) -> Self {
        Self::from_identity(&auth.identity)
    }
}

#[derive(Debug)]
pub enum AdminError {
    NotFound(String),
    Conflict(String),
    Validation(String),
    BadRequest(String),
    DatabaseRequired,
    ServicesRequired,
    #[allow(dead_code)] // Error variant used via From impls
    NotConfigured(String),
    #[allow(dead_code)] // Error variant used via From impls
    Unauthorized,
    Forbidden(String),
    Database(DbError),
    #[allow(dead_code)] // Error variant used via From impls
    Internal(String),
    RateLimited {
        seconds_remaining: u64,
        message: String,
    },
    /// SAML configuration validation error (missing required fields, invalid cert format)
    SamlValidation(String),
    /// SAML metadata fetch/parse error
    SamlMetadata(String),
}

impl From<DbError> for AdminError {
    fn from(err: DbError) -> Self {
        match err {
            DbError::NotFound => AdminError::NotFound("Resource not found".to_string()),
            DbError::Conflict(msg) => AdminError::Conflict(msg),
            DbError::Validation(msg) => AdminError::Validation(msg),
            DbError::NotConfigured => AdminError::DatabaseRequired,
            _ => AdminError::Database(err),
        }
    }
}

impl From<AuthzError> for AdminError {
    fn from(err: AuthzError) -> Self {
        match err {
            AuthzError::AccessDenied(msg) => AdminError::Forbidden(msg),
            _ => {
                tracing::error!(error = %err, "Authorization error");
                AdminError::Internal("An internal error occurred".to_string())
            }
        }
    }
}

#[cfg(feature = "sso")]
impl From<OrgSsoConfigError> for AdminError {
    fn from(err: OrgSsoConfigError) -> Self {
        match err {
            OrgSsoConfigError::NotFound => AdminError::NotFound("SSO config not found".to_string()),
            OrgSsoConfigError::Database(db_err) => AdminError::Database(db_err),
            OrgSsoConfigError::SecretStorage(msg) => {
                tracing::error!(error = %msg, "Secret storage error");
                AdminError::Internal("An internal error occurred".to_string())
            }
            OrgSsoConfigError::SecretRetrieval(msg) => {
                tracing::error!(error = %msg, "Secret retrieval error");
                AdminError::Internal("An internal error occurred".to_string())
            }
        }
    }
}

#[cfg(feature = "sso")]
impl From<OrgScimConfigError> for AdminError {
    fn from(err: OrgScimConfigError) -> Self {
        match err {
            OrgScimConfigError::Database(db_err) => AdminError::Database(db_err),
        }
    }
}

#[cfg(feature = "sso")]
impl From<DomainVerificationError> for AdminError {
    fn from(err: DomainVerificationError) -> Self {
        match err {
            DomainVerificationError::NotFound => {
                AdminError::NotFound("Domain verification not found".to_string())
            }
            DomainVerificationError::PublicDomainBlocked(msg) => AdminError::Validation(msg),
            DomainVerificationError::InvalidDomain(msg) => AdminError::Validation(msg),
            DomainVerificationError::DnsLookup(msg) => {
                tracing::error!(error = %msg, "DNS lookup failed");
                AdminError::Internal("An internal error occurred".to_string())
            }
            DomainVerificationError::Database(db_err) => AdminError::Database(db_err),
            DomainVerificationError::RateLimited {
                seconds_remaining,
                message,
            } => AdminError::RateLimited {
                seconds_remaining,
                message,
            },
        }
    }
}

impl From<OrgRbacPolicyError> for AdminError {
    fn from(err: OrgRbacPolicyError) -> Self {
        match err {
            OrgRbacPolicyError::NotFound => {
                AdminError::NotFound("RBAC policy not found".to_string())
            }
            OrgRbacPolicyError::InvalidCondition(msg) => AdminError::Validation(msg),
            OrgRbacPolicyError::Database(db_err) => AdminError::Database(db_err),
            OrgRbacPolicyError::RegistryRefresh(msg) => {
                tracing::error!(error = %msg, "Registry refresh failed");
                AdminError::Internal("An internal error occurred".to_string())
            }
        }
    }
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        // Handle RateLimited specially to add Retry-After header
        if let AdminError::RateLimited {
            seconds_remaining,
            message,
        } = self
        {
            metrics::record_gateway_error("rate_limited", "rate_limited", None);
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(
                    axum::http::header::RETRY_AFTER,
                    seconds_remaining.to_string(),
                )],
                Json(ErrorResponse::new("rate_limited", message)),
            )
                .into_response();
        }

        let (status, code, message, error_type) = match self {
            AdminError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg, "not_found"),
            AdminError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg, "conflict"),
            AdminError::Validation(msg) => (
                StatusCode::BAD_REQUEST,
                "validation_error",
                msg,
                "validation_error",
            ),
            AdminError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, "bad_request", msg, "bad_request")
            }
            AdminError::DatabaseRequired => (
                StatusCode::NOT_IMPLEMENTED,
                "feature_not_available",
                "This endpoint requires database support. Rebuild with --features database-sqlite or --features database-postgres.".to_string(),
                "internal_error",
            ),
            AdminError::ServicesRequired => (
                StatusCode::NOT_IMPLEMENTED,
                "feature_not_available",
                "This endpoint requires database support. Rebuild with --features database-sqlite or --features database-postgres.".to_string(),
                "internal_error",
            ),
            AdminError::NotConfigured(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "not_configured",
                msg,
                "internal_error",
            ),
            AdminError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Unauthorized".to_string(),
                "auth_failure",
            ),
            AdminError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg, "auth_failure"),
            AdminError::Database(err) => {
                tracing::error!(error = %err, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "An internal database error occurred".to_string(),
                    "internal_error",
                )
            }
            AdminError::Internal(msg) => {
                tracing::error!(error = %msg, "Internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An internal error occurred".to_string(),
                    "internal_error",
                )
            }
            AdminError::SamlValidation(msg) => (
                StatusCode::BAD_REQUEST,
                "saml_validation_error",
                msg,
                "validation_error",
            ),
            AdminError::SamlMetadata(msg) => (
                StatusCode::BAD_REQUEST,
                "saml_metadata_error",
                msg,
                "validation_error",
            ),
            AdminError::RateLimited { .. } => unreachable!("Handled above"),
        };

        // Record admin error metric
        metrics::record_gateway_error(error_type, code, None);

        (status, Json(ErrorResponse::new(code, message))).into_response()
    }
}
