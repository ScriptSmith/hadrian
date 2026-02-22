//! Principal abstraction for unified identity model.
//!
//! A Principal represents "who is making the request" - the authenticated actor
//! regardless of credential type (OIDC token, SAML assertion, API key, etc.).
//!
//! This provides a clean abstraction over the various authentication methods,
//! making it easy to:
//! - Determine what kind of actor is making a request
//! - Get consistent identity information for audit logging
//! - Convert to Subject for RBAC evaluation
//!
//! # Principal Types
//!
//! - **User**: A human user authenticated via OIDC, SAML, proxy headers, or user-owned API key
//! - **ServiceAccount**: A machine identity with explicit roles (service account-owned API key)
//! - **Machine**: A shared/organizational credential (org/team/project-owned API key)
//!
//! # Derivation Logic
//!
//! ```text
//! API key only:
//!   - SA owner → ServiceAccount principal
//!   - User owner → User principal (with user_id, no identity roles)
//!   - Org/Team/Project owner → Machine principal
//!
//! Identity only (OIDC/SAML/proxy):
//!   - Always User principal (with all identity fields)
//!
//! Both (API key + Identity):
//!   - SA owner → ServiceAccount principal (SA takes precedence)
//!   - Otherwise → User principal (identity provides fields)
//! ```

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AuthenticatedRequest, IdentityKind};
use crate::{
    authz::Subject,
    models::{ApiKeyOwner, AuditActorType},
};

/// The authenticated actor making a request.
///
/// Principal abstracts over the various authentication methods (OIDC, SAML,
/// API keys, etc.) to provide a unified view of "who is making this request".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Principal {
    /// A human user (from OIDC/SAML/proxy headers or user-owned API key).
    User {
        /// Internal user ID (None if external-only identity).
        user_id: Option<Uuid>,
        /// External ID from IdP (sub claim, etc.).
        external_id: Option<String>,
        /// Email address.
        email: Option<String>,
        /// Display name.
        name: Option<String>,
        /// Roles from IdP (after mapping).
        roles: Vec<String>,
        /// Organization IDs the user belongs to.
        org_ids: Vec<String>,
        /// Team IDs the user belongs to.
        team_ids: Vec<String>,
        /// Project IDs the user belongs to.
        project_ids: Vec<String>,
    },

    /// A service account (machine identity with explicit roles).
    ServiceAccount {
        /// Service account ID.
        id: Uuid,
        /// Organization the service account belongs to.
        org_id: Uuid,
        /// Roles assigned to the service account.
        roles: Vec<String>,
    },

    /// A machine/shared credential (org/team/project-owned API key).
    ///
    /// These credentials represent organizational access rather than
    /// individual identity. They only carry scope, not roles.
    Machine {
        /// The kind of machine credential.
        kind: MachineKind,
    },
}

/// The kind of machine (shared) credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum MachineKind {
    /// Organization-scoped API key.
    Organization { org_id: Uuid },
    /// Team-scoped API key.
    Team { org_id: Uuid, team_id: Uuid },
    /// Project-scoped API key.
    Project { org_id: Uuid, project_id: Uuid },
    /// Unknown/malformed credential - carries no scope and fails all authorization checks.
    /// This is used when API key ownership data is incomplete or corrupted.
    Unknown,
}

impl Principal {
    /// Get the principal's unique identifier (if available).
    ///
    /// - User: user_id (internal ID)
    /// - ServiceAccount: service account ID
    /// - Machine: None (no individual identity)
    pub fn id(&self) -> Option<Uuid> {
        match self {
            Self::User { user_id, .. } => *user_id,
            Self::ServiceAccount { id, .. } => Some(*id),
            Self::Machine { .. } => None,
        }
    }

    /// Get the primary organization ID associated with this principal.
    ///
    /// - User: first org_id (if any)
    /// - ServiceAccount: org_id
    /// - Machine: org_id from the credential scope (None for Unknown)
    pub fn org_id(&self) -> Option<Uuid> {
        match self {
            Self::User { org_ids, .. } => org_ids.first().and_then(|id| Uuid::parse_str(id).ok()),
            Self::ServiceAccount { org_id, .. } => Some(*org_id),
            Self::Machine { kind } => match kind {
                MachineKind::Organization { org_id } => Some(*org_id),
                MachineKind::Team { org_id, .. } => Some(*org_id),
                MachineKind::Project { org_id, .. } => Some(*org_id),
                MachineKind::Unknown => None,
            },
        }
    }

    /// Get the roles associated with this principal.
    ///
    /// - User: identity roles
    /// - ServiceAccount: service account roles
    /// - Machine: empty (no roles, only scope)
    pub fn roles(&self) -> &[String] {
        match self {
            Self::User { roles, .. } => roles,
            Self::ServiceAccount { roles, .. } => roles,
            Self::Machine { .. } => &[],
        }
    }

    /// Check if the principal has a specific role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles().iter().any(|r| r == role)
    }

    /// Get a display name for this principal (for UI/logging).
    pub fn display_name(&self) -> String {
        match self {
            Self::User {
                name,
                email,
                external_id,
                user_id,
                ..
            } => {
                if let Some(n) = name {
                    n.clone()
                } else if let Some(e) = email {
                    e.clone()
                } else if let Some(ext) = external_id {
                    ext.clone()
                } else if let Some(id) = user_id {
                    format!("user:{}", id)
                } else {
                    "unknown user".to_string()
                }
            }
            Self::ServiceAccount { id, .. } => format!("service_account:{}", id),
            Self::Machine { kind } => match kind {
                MachineKind::Organization { org_id } => format!("org:{}", org_id),
                MachineKind::Team { team_id, .. } => format!("team:{}", team_id),
                MachineKind::Project { project_id, .. } => format!("project:{}", project_id),
                MachineKind::Unknown => "unknown:malformed_credential".to_string(),
            },
        }
    }

    /// Get the audit actor type for this principal.
    pub fn actor_type(&self) -> AuditActorType {
        match self {
            Self::User {
                user_id,
                external_id,
                ..
            } => {
                if user_id.is_some() {
                    AuditActorType::User
                } else if external_id.is_some() {
                    // External identity without internal user record
                    AuditActorType::ExternalUser
                } else {
                    // No user_id or external_id - shouldn't happen but fall back to System
                    AuditActorType::System
                }
            }
            Self::ServiceAccount { .. } => AuditActorType::ServiceAccount,
            Self::Machine { .. } => AuditActorType::ApiKey,
        }
    }

    /// Convert this principal to a Subject for RBAC evaluation.
    pub fn to_subject(&self) -> Subject {
        match self {
            Self::User {
                user_id,
                external_id,
                email,
                roles,
                org_ids,
                team_ids,
                project_ids,
                ..
            } => {
                let mut subject = Subject::new()
                    .with_roles(roles.clone())
                    .with_org_ids(org_ids.clone())
                    .with_team_ids(team_ids.clone())
                    .with_project_ids(project_ids.clone());

                if let Some(id) = user_id {
                    subject = subject.with_user_id(id.to_string());
                }
                if let Some(ext) = external_id {
                    subject = subject.with_external_id(ext);
                }
                if let Some(e) = email {
                    subject = subject.with_email(e);
                }

                subject
            }
            Self::ServiceAccount { id, org_id, roles } => Subject::new()
                .with_service_account_id(id.to_string())
                .with_org_ids(vec![org_id.to_string()])
                .with_roles(roles.clone()),
            Self::Machine { kind } => match kind {
                MachineKind::Organization { org_id } => {
                    Subject::new().with_org_ids(vec![org_id.to_string()])
                }
                MachineKind::Team { org_id, team_id } => Subject::new()
                    .with_org_ids(vec![org_id.to_string()])
                    .with_team_ids(vec![team_id.to_string()]),
                MachineKind::Project { org_id, project_id } => Subject::new()
                    .with_org_ids(vec![org_id.to_string()])
                    .with_project_ids(vec![project_id.to_string()]),
                // Unknown carries no scope - will fail authorization checks that require org membership
                MachineKind::Unknown => Subject::new(),
            },
        }
    }
}

impl From<&Principal> for Subject {
    fn from(principal: &Principal) -> Self {
        principal.to_subject()
    }
}

impl From<Principal> for Subject {
    fn from(principal: Principal) -> Self {
        principal.to_subject()
    }
}

/// Derive a Principal from an AuthenticatedRequest.
///
/// This function examines the authentication context and determines
/// what kind of principal is making the request.
pub fn derive_principal(auth: &AuthenticatedRequest) -> Principal {
    match &auth.kind {
        IdentityKind::ApiKey(api_key) => {
            // Check if this is a service account-owned key
            if let Some(sa_id) = api_key.service_account_id {
                // Service account principal - org_id should always be present
                match api_key.org_id {
                    Some(org_id) => {
                        let roles = api_key.service_account_roles.clone().unwrap_or_default();
                        return Principal::ServiceAccount {
                            id: sa_id,
                            org_id,
                            roles,
                        };
                    }
                    None => {
                        // Service account should always have org_id; log warning and fall through
                        tracing::warn!(
                            service_account_id = %sa_id,
                            "Service account API key missing org_id, falling back to Machine principal"
                        );
                        // Fall through to owner-based principal derivation
                    }
                }
            }

            // Check owner type for user vs machine
            match &api_key.key.owner {
                ApiKeyOwner::User { .. } => {
                    // User-owned API key - treated as User principal
                    Principal::User {
                        user_id: api_key.user_id,
                        external_id: None,
                        email: None,
                        name: None,
                        roles: vec![],
                        org_ids: api_key
                            .org_id
                            .map(|id| vec![id.to_string()])
                            .unwrap_or_default(),
                        team_ids: api_key
                            .team_id
                            .map(|id| vec![id.to_string()])
                            .unwrap_or_default(),
                        project_ids: api_key
                            .project_id
                            .map(|id| vec![id.to_string()])
                            .unwrap_or_default(),
                    }
                }
                ApiKeyOwner::Organization { org_id } => Principal::Machine {
                    kind: MachineKind::Organization { org_id: *org_id },
                },
                ApiKeyOwner::Team { team_id } => {
                    // org_id comes from ApiKeyAuth, not the owner variant
                    match api_key.org_id {
                        Some(org_id) => Principal::Machine {
                            kind: MachineKind::Team {
                                org_id,
                                team_id: *team_id,
                            },
                        },
                        None => {
                            tracing::warn!(
                                team_id = %team_id,
                                "Team-owned API key missing org_id - credential is malformed"
                            );
                            // Unknown scope - will fail all authorization checks
                            Principal::Machine {
                                kind: MachineKind::Unknown,
                            }
                        }
                    }
                }
                ApiKeyOwner::Project { project_id } => {
                    // org_id comes from ApiKeyAuth, not the owner variant
                    match api_key.org_id {
                        Some(org_id) => Principal::Machine {
                            kind: MachineKind::Project {
                                org_id,
                                project_id: *project_id,
                            },
                        },
                        None => {
                            tracing::warn!(
                                project_id = %project_id,
                                "Project-owned API key missing org_id - credential is malformed"
                            );
                            // Unknown scope - will fail all authorization checks
                            Principal::Machine {
                                kind: MachineKind::Unknown,
                            }
                        }
                    }
                }
                ApiKeyOwner::ServiceAccount { service_account_id } => {
                    // This case handles the fallback when service_account_id wasn't
                    // set in ApiKeyAuth but the owner type is ServiceAccount.
                    // Fall back to a machine principal scoped to the org.
                    match api_key.org_id {
                        Some(org_id) => Principal::Machine {
                            kind: MachineKind::Organization { org_id },
                        },
                        None => {
                            tracing::warn!(
                                service_account_id = %service_account_id,
                                "Service account API key missing org_id - credential is malformed"
                            );
                            // Unknown scope - will fail all authorization checks
                            Principal::Machine {
                                kind: MachineKind::Unknown,
                            }
                        }
                    }
                }
            }
        }

        IdentityKind::Identity(identity) => {
            // Pure identity auth (OIDC/SAML/proxy)
            Principal::User {
                user_id: identity.user_id,
                external_id: Some(identity.external_id.clone()),
                email: identity.email.clone(),
                name: identity.name.clone(),
                roles: identity.roles.clone(),
                org_ids: identity.org_ids.clone(),
                team_ids: identity.team_ids.clone(),
                project_ids: identity.project_ids.clone(),
            }
        }

        IdentityKind::Both { api_key, identity } => {
            // Both API key and identity present

            // If service account-owned, SA takes precedence
            if let Some(sa_id) = api_key.service_account_id {
                match api_key.org_id {
                    Some(org_id) => {
                        let roles = api_key.service_account_roles.clone().unwrap_or_default();
                        return Principal::ServiceAccount {
                            id: sa_id,
                            org_id,
                            roles,
                        };
                    }
                    None => {
                        // Service account should have org_id; log warning and use identity instead
                        tracing::warn!(
                            service_account_id = %sa_id,
                            "Service account API key missing org_id in Both path, using identity"
                        );
                        // Fall through to identity-based principal
                    }
                }
            }

            // Otherwise, identity provides the user information
            // Use API key scopes if identity doesn't have them
            let org_ids = if !identity.org_ids.is_empty() {
                identity.org_ids.clone()
            } else {
                api_key
                    .org_id
                    .map(|id| vec![id.to_string()])
                    .unwrap_or_default()
            };

            let team_ids = if !identity.team_ids.is_empty() {
                identity.team_ids.clone()
            } else {
                api_key
                    .team_id
                    .map(|id| vec![id.to_string()])
                    .unwrap_or_default()
            };

            let project_ids = if !identity.project_ids.is_empty() {
                identity.project_ids.clone()
            } else {
                api_key
                    .project_id
                    .map(|id| vec![id.to_string()])
                    .unwrap_or_default()
            };

            Principal::User {
                user_id: identity.user_id.or(api_key.user_id),
                external_id: Some(identity.external_id.clone()),
                email: identity.email.clone(),
                name: identity.name.clone(),
                roles: identity.roles.clone(),
                org_ids,
                team_ids,
                project_ids,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::{
        auth::{ApiKeyAuth, Identity},
        models::ApiKey,
    };

    fn make_test_api_key(owner: ApiKeyOwner) -> ApiKey {
        ApiKey {
            id: Uuid::new_v4(),
            key_prefix: "test_".to_string(),
            name: "Test Key".to_string(),
            owner,
            budget_limit_cents: None,
            budget_period: None,
            created_at: Utc::now(),
            expires_at: None,
            revoked_at: None,
            last_used_at: None,
            scopes: None,
            allowed_models: None,
            ip_allowlist: None,
            rate_limit_rpm: None,
            rate_limit_tpm: None,
            rotated_from_key_id: None,
            rotation_grace_until: None,
        }
    }

    #[test]
    fn test_derive_principal_service_account() {
        let sa_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(ApiKeyOwner::ServiceAccount {
                service_account_id: sa_id,
            }),
            org_id: Some(org_id),
            team_id: None,
            project_id: None,
            user_id: None,
            service_account_id: Some(sa_id),
            service_account_roles: Some(vec!["deployer".to_string(), "viewer".to_string()]),
        };

        let auth = AuthenticatedRequest::new(IdentityKind::ApiKey(api_key_auth));
        let principal = derive_principal(&auth);

        match principal {
            Principal::ServiceAccount {
                id,
                org_id: o,
                roles,
            } => {
                assert_eq!(id, sa_id);
                assert_eq!(o, org_id);
                assert_eq!(roles, vec!["deployer", "viewer"]);
            }
            _ => panic!("Expected ServiceAccount principal"),
        }
    }

    #[test]
    fn test_derive_principal_user_owned_api_key() {
        let user_id = Uuid::new_v4();

        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(ApiKeyOwner::User { user_id }),
            org_id: None,
            team_id: None,
            project_id: None,
            user_id: Some(user_id),
            service_account_id: None,
            service_account_roles: None,
        };

        let auth = AuthenticatedRequest::new(IdentityKind::ApiKey(api_key_auth));
        let principal = derive_principal(&auth);

        match principal {
            Principal::User {
                user_id: uid,
                roles,
                ..
            } => {
                assert_eq!(uid, Some(user_id));
                assert!(roles.is_empty()); // API key doesn't carry roles
            }
            _ => panic!("Expected User principal"),
        }
    }

    #[test]
    fn test_derive_principal_org_owned_api_key() {
        let org_id = Uuid::new_v4();

        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(ApiKeyOwner::Organization { org_id }),
            org_id: Some(org_id),
            team_id: None,
            project_id: None,
            user_id: None,
            service_account_id: None,
            service_account_roles: None,
        };

        let auth = AuthenticatedRequest::new(IdentityKind::ApiKey(api_key_auth));
        let principal = derive_principal(&auth);

        match principal {
            Principal::Machine {
                kind: MachineKind::Organization { org_id: o },
            } => {
                assert_eq!(o, org_id);
            }
            _ => panic!("Expected Machine principal with Organization kind"),
        }
    }

    #[test]
    fn test_derive_principal_team_owned_api_key() {
        let org_id = Uuid::new_v4();
        let team_id = Uuid::new_v4();

        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(ApiKeyOwner::Team { team_id }),
            org_id: Some(org_id),
            team_id: Some(team_id),
            project_id: None,
            user_id: None,
            service_account_id: None,
            service_account_roles: None,
        };

        let auth = AuthenticatedRequest::new(IdentityKind::ApiKey(api_key_auth));
        let principal = derive_principal(&auth);

        match principal {
            Principal::Machine {
                kind:
                    MachineKind::Team {
                        org_id: o,
                        team_id: t,
                    },
            } => {
                assert_eq!(o, org_id);
                assert_eq!(t, team_id);
            }
            _ => panic!("Expected Machine principal with Team kind"),
        }
    }

    #[test]
    fn test_derive_principal_project_owned_api_key() {
        let org_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();

        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(ApiKeyOwner::Project { project_id }),
            org_id: Some(org_id),
            team_id: None,
            project_id: Some(project_id),
            user_id: None,
            service_account_id: None,
            service_account_roles: None,
        };

        let auth = AuthenticatedRequest::new(IdentityKind::ApiKey(api_key_auth));
        let principal = derive_principal(&auth);

        match principal {
            Principal::Machine {
                kind:
                    MachineKind::Project {
                        org_id: o,
                        project_id: p,
                    },
            } => {
                assert_eq!(o, org_id);
                assert_eq!(p, project_id);
            }
            _ => panic!("Expected Machine principal with Project kind"),
        }
    }

    #[test]
    fn test_derive_principal_identity_only() {
        let user_id = Uuid::new_v4();
        let identity = Identity {
            external_id: "user@example.com".to_string(),
            email: Some("user@example.com".to_string()),
            name: Some("Test User".to_string()),
            user_id: Some(user_id),
            roles: vec!["admin".to_string(), "viewer".to_string()],
            idp_groups: vec![],
            org_ids: vec!["org-123".to_string()],
            team_ids: vec!["team-456".to_string()],
            project_ids: vec![],
        };

        let auth = AuthenticatedRequest::new(IdentityKind::Identity(identity));
        let principal = derive_principal(&auth);

        match principal {
            Principal::User {
                user_id: uid,
                external_id,
                email,
                name,
                roles,
                org_ids,
                team_ids,
                ..
            } => {
                assert_eq!(uid, Some(user_id));
                assert_eq!(external_id, Some("user@example.com".to_string()));
                assert_eq!(email, Some("user@example.com".to_string()));
                assert_eq!(name, Some("Test User".to_string()));
                assert_eq!(roles, vec!["admin", "viewer"]);
                assert_eq!(org_ids, vec!["org-123"]);
                assert_eq!(team_ids, vec!["team-456"]);
            }
            _ => panic!("Expected User principal"),
        }
    }

    #[test]
    fn test_derive_principal_both_with_service_account() {
        let sa_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(ApiKeyOwner::ServiceAccount {
                service_account_id: sa_id,
            }),
            org_id: Some(org_id),
            team_id: None,
            project_id: None,
            user_id: None,
            service_account_id: Some(sa_id),
            service_account_roles: Some(vec!["sa_role".to_string()]),
        };

        let identity = Identity {
            external_id: "user@example.com".to_string(),
            email: Some("user@example.com".to_string()),
            name: Some("Test User".to_string()),
            user_id: None,
            roles: vec!["identity_role".to_string()],
            idp_groups: vec![],
            org_ids: vec![],
            team_ids: vec![],
            project_ids: vec![],
        };

        let auth = AuthenticatedRequest::new(IdentityKind::Both {
            api_key: Box::new(api_key_auth),
            identity,
        });
        let principal = derive_principal(&auth);

        // SA takes precedence
        match principal {
            Principal::ServiceAccount { id, roles, .. } => {
                assert_eq!(id, sa_id);
                assert_eq!(roles, vec!["sa_role"]);
            }
            _ => panic!("Expected ServiceAccount principal (SA takes precedence)"),
        }
    }

    #[test]
    fn test_derive_principal_both_identity_override() {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();

        let api_key_auth = ApiKeyAuth {
            key: make_test_api_key(ApiKeyOwner::Organization { org_id }),
            org_id: Some(org_id),
            team_id: None,
            project_id: None,
            user_id: None,
            service_account_id: None,
            service_account_roles: None,
        };

        let identity = Identity {
            external_id: "user@example.com".to_string(),
            email: Some("user@example.com".to_string()),
            name: Some("Test User".to_string()),
            user_id: Some(user_id),
            roles: vec!["identity_role".to_string()],
            idp_groups: vec![],
            org_ids: vec!["identity-org".to_string()],
            team_ids: vec![],
            project_ids: vec![],
        };

        let auth = AuthenticatedRequest::new(IdentityKind::Both {
            api_key: Box::new(api_key_auth),
            identity,
        });
        let principal = derive_principal(&auth);

        // Identity provides user info, overrides API key scope
        match principal {
            Principal::User {
                user_id: uid,
                email,
                roles,
                org_ids,
                ..
            } => {
                assert_eq!(uid, Some(user_id));
                assert_eq!(email, Some("user@example.com".to_string()));
                assert_eq!(roles, vec!["identity_role"]);
                assert_eq!(org_ids, vec!["identity-org"]); // Identity overrides API key org
            }
            _ => panic!("Expected User principal"),
        }
    }

    #[test]
    fn test_principal_to_subject_user() {
        let user_id = Uuid::new_v4();
        let principal = Principal::User {
            user_id: Some(user_id),
            external_id: Some("ext-123".to_string()),
            email: Some("test@example.com".to_string()),
            name: Some("Test User".to_string()),
            roles: vec!["admin".to_string()],
            org_ids: vec!["org-1".to_string()],
            team_ids: vec!["team-1".to_string()],
            project_ids: vec!["proj-1".to_string()],
        };

        let subject = principal.to_subject();

        assert_eq!(subject.user_id, Some(user_id.to_string()));
        assert_eq!(subject.external_id, Some("ext-123".to_string()));
        assert_eq!(subject.email, Some("test@example.com".to_string()));
        assert_eq!(subject.roles, vec!["admin"]);
        assert_eq!(subject.org_ids, vec!["org-1"]);
        assert_eq!(subject.team_ids, vec!["team-1"]);
        assert_eq!(subject.project_ids, vec!["proj-1"]);
    }

    #[test]
    fn test_principal_to_subject_service_account() {
        let sa_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let principal = Principal::ServiceAccount {
            id: sa_id,
            org_id,
            roles: vec!["deployer".to_string()],
        };

        let subject = principal.to_subject();

        assert_eq!(subject.service_account_id, Some(sa_id.to_string()));
        assert_eq!(subject.org_ids, vec![org_id.to_string()]);
        assert_eq!(subject.roles, vec!["deployer"]);
        assert!(subject.user_id.is_none());
    }

    #[test]
    fn test_principal_to_subject_machine() {
        let org_id = Uuid::new_v4();
        let team_id = Uuid::new_v4();
        let principal = Principal::Machine {
            kind: MachineKind::Team { org_id, team_id },
        };

        let subject = principal.to_subject();

        assert_eq!(subject.org_ids, vec![org_id.to_string()]);
        assert_eq!(subject.team_ids, vec![team_id.to_string()]);
        assert!(subject.roles.is_empty());
        assert!(subject.user_id.is_none());
    }

    #[test]
    fn test_principal_actor_type() {
        let user_id = Uuid::new_v4();

        // User with internal ID
        let user_principal = Principal::User {
            user_id: Some(user_id),
            external_id: None,
            email: None,
            name: None,
            roles: vec![],
            org_ids: vec![],
            team_ids: vec![],
            project_ids: vec![],
        };
        assert_eq!(user_principal.actor_type(), AuditActorType::User);

        // User without internal ID (external-only)
        let external_principal = Principal::User {
            user_id: None,
            external_id: Some("ext-123".to_string()),
            email: None,
            name: None,
            roles: vec![],
            org_ids: vec![],
            team_ids: vec![],
            project_ids: vec![],
        };
        assert_eq!(
            external_principal.actor_type(),
            AuditActorType::ExternalUser
        );

        // Service account
        let sa_principal = Principal::ServiceAccount {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            roles: vec![],
        };
        assert_eq!(sa_principal.actor_type(), AuditActorType::ServiceAccount);

        // Machine
        let machine_principal = Principal::Machine {
            kind: MachineKind::Organization {
                org_id: Uuid::new_v4(),
            },
        };
        assert_eq!(machine_principal.actor_type(), AuditActorType::ApiKey);
    }

    #[test]
    fn test_principal_display_name() {
        // User with name
        let principal = Principal::User {
            user_id: None,
            external_id: None,
            email: Some("test@example.com".to_string()),
            name: Some("Test User".to_string()),
            roles: vec![],
            org_ids: vec![],
            team_ids: vec![],
            project_ids: vec![],
        };
        assert_eq!(principal.display_name(), "Test User");

        // User with email only
        let principal = Principal::User {
            user_id: None,
            external_id: None,
            email: Some("test@example.com".to_string()),
            name: None,
            roles: vec![],
            org_ids: vec![],
            team_ids: vec![],
            project_ids: vec![],
        };
        assert_eq!(principal.display_name(), "test@example.com");

        // Service account
        let sa_id = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let principal = Principal::ServiceAccount {
            id: sa_id,
            org_id: Uuid::new_v4(),
            roles: vec![],
        };
        assert_eq!(
            principal.display_name(),
            "service_account:12345678-1234-1234-1234-123456789abc"
        );
    }

    #[test]
    fn test_principal_roles_and_has_role() {
        let principal = Principal::User {
            user_id: None,
            external_id: None,
            email: None,
            name: None,
            roles: vec!["admin".to_string(), "viewer".to_string()],
            org_ids: vec![],
            team_ids: vec![],
            project_ids: vec![],
        };

        assert_eq!(principal.roles(), &["admin", "viewer"]);
        assert!(principal.has_role("admin"));
        assert!(principal.has_role("viewer"));
        assert!(!principal.has_role("editor"));

        // Machine principals have no roles
        let machine = Principal::Machine {
            kind: MachineKind::Organization {
                org_id: Uuid::new_v4(),
            },
        };
        assert!(machine.roles().is_empty());
        assert!(!machine.has_role("admin"));
    }
}
