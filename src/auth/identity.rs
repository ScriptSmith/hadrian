use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{
    AuthError,
    principal::{Principal, derive_principal},
};
use crate::models::{ApiKey, ApiKeyOwner};

/// Identity information from the request
#[derive(Debug, Clone)]
pub struct Identity {
    /// External identity ID (from IdP)
    pub external_id: String,

    /// Email (if provided by IdP)
    pub email: Option<String>,

    /// Display name (if provided by IdP)
    pub name: Option<String>,

    /// Internal user ID (if user exists in database)
    pub user_id: Option<Uuid>,

    /// Roles from IdP (e.g., from JWT claims or Zero Trust headers)
    pub roles: Vec<String>,

    /// Raw IdP groups from the authentication source (OIDC groups claim, proxy header, etc.)
    /// These are the exact values from the IdP, before any mapping or transformation.
    /// Useful for debugging SSO group mappings.
    pub idp_groups: Vec<String>,

    /// Organization IDs the user belongs to (from claims, if available)
    pub org_ids: Vec<String>,

    /// Team IDs the user belongs to (from claims, if available)
    pub team_ids: Vec<String>,

    /// Project IDs the user belongs to (from claims, if available)
    pub project_ids: Vec<String>,
}

/// Represents the kind of authentication used
#[derive(Debug, Clone)]
pub enum IdentityKind {
    /// Authenticated via API key
    ApiKey(ApiKeyAuth),

    /// Authenticated via identity headers (Zero Trust / SSO)
    Identity(Identity),

    /// Authenticated via both (API key on behalf of a user)
    Both {
        api_key: Box<ApiKeyAuth>,
        identity: Identity,
    },
}

/// API key authentication details
#[derive(Debug, Clone)]
pub struct ApiKeyAuth {
    pub key: ApiKey,
    pub org_id: Option<Uuid>,
    pub team_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub service_account_id: Option<Uuid>,
    /// Roles from the service account (pre-fetched for RBAC evaluation)
    pub service_account_roles: Option<Vec<String>>,
}

impl ApiKeyAuth {
    #[allow(dead_code)] // Public API for CEL evaluation
    pub fn owner(&self) -> &ApiKeyOwner {
        &self.key.owner
    }

    /// Check if the API key has a budget configured
    /// Note: Actual budget enforcement is handled by the budget middleware
    /// which has access to the database and cache for querying current spend.
    #[allow(dead_code)] // Public API for CEL evaluation
    pub fn has_budget(&self) -> bool {
        self.key.budget_limit_cents.is_some() && self.key.budget_period.is_some()
    }

    /// Check if the API key has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = &self.key.expires_at {
            return expires_at < &Utc::now();
        }
        false
    }

    /// Check if the API key is revoked or rotation grace period expired
    pub fn is_revoked(&self) -> bool {
        // Explicitly revoked by user
        if self.key.revoked_at.is_some() {
            return true;
        }
        // Rotated and grace period has expired
        if let Some(grace_until) = self.key.rotation_grace_until
            && grace_until <= Utc::now()
        {
            return true;
        }
        false
    }

    /// Check if the API key allows access to a specific model.
    ///
    /// Returns `Ok(())` if allowed, or `Err(AuthError::ModelNotAllowed)` if not.
    pub fn check_model_allowed(&self, model: &str) -> Result<(), AuthError> {
        if self.key.is_model_allowed(model) {
            Ok(())
        } else {
            Err(AuthError::ModelNotAllowed {
                model: model.to_string(),
                allowed_patterns: self.key.allowed_models.clone().unwrap_or_default(),
            })
        }
    }
}

/// Extension type that gets added to requests after authentication
#[derive(Debug, Clone)]
pub struct AuthenticatedRequest {
    pub kind: IdentityKind,
    #[allow(dead_code)] // Public API for CEL evaluation
    pub authenticated_at: DateTime<Utc>,
}

impl AuthenticatedRequest {
    pub fn new(kind: IdentityKind) -> Self {
        Self {
            kind,
            authenticated_at: Utc::now(),
        }
    }

    /// Get the API key if available
    pub fn api_key(&self) -> Option<&ApiKeyAuth> {
        match &self.kind {
            IdentityKind::ApiKey(key) => Some(key),
            IdentityKind::Both { api_key, .. } => Some(api_key),
            _ => None,
        }
    }

    /// Get the identity if available
    #[allow(dead_code)] // Public API for CEL evaluation
    pub fn identity(&self) -> Option<&Identity> {
        match &self.kind {
            IdentityKind::Identity(id) => Some(id),
            IdentityKind::Both { identity, .. } => Some(identity),
            _ => None,
        }
    }

    /// Get the user ID from either API key or identity
    #[allow(dead_code)] // Public API for CEL evaluation
    pub fn user_id(&self) -> Option<Uuid> {
        match &self.kind {
            IdentityKind::ApiKey(key) => key.user_id,
            IdentityKind::Identity(id) => id.user_id,
            IdentityKind::Both { identity, .. } => identity.user_id,
        }
    }

    /// Get the org ID from the API key
    #[allow(dead_code)] // Public API for CEL evaluation
    pub fn org_id(&self) -> Option<Uuid> {
        self.api_key().and_then(|k| k.org_id)
    }

    /// Get the project ID from the API key
    #[allow(dead_code)] // Public API for CEL evaluation
    pub fn project_id(&self) -> Option<Uuid> {
        self.api_key().and_then(|k| k.project_id)
    }

    /// Get the Principal for this authenticated request.
    ///
    /// The Principal represents "who is making the request" regardless of
    /// the credential type used for authentication.
    pub fn principal(&self) -> Principal {
        derive_principal(self)
    }
}
