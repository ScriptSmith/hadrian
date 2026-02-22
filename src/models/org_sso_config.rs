use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[cfg(feature = "saml")]
use crate::auth::derive_acs_url_from_entity_id;

/// SSO provider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum SsoProviderType {
    /// OpenID Connect (OIDC) provider
    #[default]
    Oidc,
    /// SAML 2.0 provider
    Saml,
}

impl std::fmt::Display for SsoProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SsoProviderType::Oidc => write!(f, "oidc"),
            SsoProviderType::Saml => write!(f, "saml"),
        }
    }
}

impl std::str::FromStr for SsoProviderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "oidc" => Ok(SsoProviderType::Oidc),
            "saml" => Ok(SsoProviderType::Saml),
            _ => Err(format!("Invalid SSO provider type: {}", s)),
        }
    }
}

/// SSO enforcement mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum SsoEnforcementMode {
    /// SSO is optional - users can authenticate via other methods
    #[default]
    Optional,
    /// SSO is required - users must authenticate via this SSO config
    Required,
    /// Test mode - SSO validation runs but doesn't block access (shadow mode)
    Test,
}

impl std::fmt::Display for SsoEnforcementMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SsoEnforcementMode::Optional => write!(f, "optional"),
            SsoEnforcementMode::Required => write!(f, "required"),
            SsoEnforcementMode::Test => write!(f, "test"),
        }
    }
}

impl std::str::FromStr for SsoEnforcementMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "optional" => Ok(SsoEnforcementMode::Optional),
            "required" => Ok(SsoEnforcementMode::Required),
            "test" => Ok(SsoEnforcementMode::Test),
            _ => Err(format!("Invalid SSO enforcement mode: {}", s)),
        }
    }
}

/// Organization SSO Configuration.
///
/// Stores per-organization OIDC or SAML configuration for multi-tenant SSO.
/// Each organization can have its own IdP, enabling IT admins to configure
/// their own identity provider via the Admin UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct OrgSsoConfig {
    /// Unique identifier for this SSO configuration
    pub id: Uuid,
    /// Organization this SSO config belongs to (one config per org)
    pub org_id: Uuid,
    /// Provider type (oidc or saml)
    pub provider_type: SsoProviderType,

    // =========================================================================
    // OIDC Configuration (used when provider_type = 'oidc')
    // =========================================================================
    /// OIDC issuer URL (e.g., "https://accounts.google.com")
    /// Required for OIDC, not used for SAML
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    /// Discovery URL for OIDC metadata (defaults to issuer/.well-known/openid-configuration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_url: Option<String>,
    /// OAuth2 client ID (required for OIDC, not used for SAML)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    // Note: client_secret is NOT included in the model - it's stored in secret manager
    /// Redirect URI for OAuth2 callback (optional - uses global default if not set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
    /// OAuth2 scopes to request (e.g., ["openid", "email", "profile"])
    /// Empty for SAML configs
    pub scopes: Vec<String>,
    /// JWT claim to use as the user's identity (default: "sub")
    /// Required for OIDC, not used for SAML (use saml_identity_attribute instead)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_claim: Option<String>,
    /// JWT claim containing organization IDs (optional, OIDC only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_claim: Option<String>,
    /// JWT claim containing group memberships (optional, OIDC only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups_claim: Option<String>,

    // =========================================================================
    // SAML 2.0 Configuration (used when provider_type = 'saml')
    // =========================================================================
    /// IdP metadata URL for auto-configuration (alternative to manual config)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_metadata_url: Option<String>,
    /// IdP entity identifier (e.g., "https://idp.example.com/metadata")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_idp_entity_id: Option<String>,
    /// IdP Single Sign-On service URL (HTTP-Redirect or HTTP-POST binding)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_idp_sso_url: Option<String>,
    /// IdP Single Logout service URL (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_idp_slo_url: Option<String>,
    /// IdP X.509 certificate for signature validation (PEM format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_idp_certificate: Option<String>,
    /// Service Provider entity ID (Hadrian's identifier to the IdP)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_sp_entity_id: Option<String>,
    /// NameID format to request (e.g., 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_name_id_format: Option<String>,
    /// Whether to sign AuthnRequests
    pub saml_sign_requests: bool,
    // Note: saml_sp_private_key is NOT included - it's stored in secret manager
    /// SP X.509 certificate for SP metadata (PEM format)
    /// Include when request signing is enabled so IdPs can verify signatures
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_sp_certificate: Option<String>,
    /// Whether to force re-authentication at IdP
    pub saml_force_authn: bool,
    /// Requested authentication context class
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_authn_context_class_ref: Option<String>,
    /// SAML attribute name for user identity (like identity_claim for OIDC)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_identity_attribute: Option<String>,
    /// SAML attribute name for email
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_email_attribute: Option<String>,
    /// SAML attribute name for display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_name_attribute: Option<String>,
    /// SAML attribute name for groups
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_groups_attribute: Option<String>,

    // =========================================================================
    // JIT Provisioning Settings (shared by OIDC and SAML)
    // =========================================================================
    /// Whether JIT provisioning is enabled for this SSO config
    pub provisioning_enabled: bool,
    /// Whether to create new users on first login
    pub create_users: bool,
    /// Default team to add new users to (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_team_id: Option<Uuid>,
    /// Default role for new users in the organization
    pub default_org_role: String,
    /// Default role for new users in the default team
    pub default_team_role: String,
    /// Allowed email domains (empty = allow all)
    pub allowed_email_domains: Vec<String>,
    /// Whether to sync user attributes (email, name) on each login
    pub sync_attributes_on_login: bool,
    /// Whether to sync team memberships from IdP groups on each login
    pub sync_memberships_on_login: bool,

    // Enforcement
    /// SSO enforcement mode (optional, required, or test)
    pub enforcement_mode: SsoEnforcementMode,
    /// Whether this SSO config is active
    pub enabled: bool,

    // Timestamps
    /// When this config was created
    pub created_at: DateTime<Utc>,
    /// When this config was last updated
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new organization SSO configuration.
#[derive(Debug, Clone, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CreateOrgSsoConfig {
    /// Provider type (defaults to 'oidc')
    #[serde(default)]
    pub provider_type: SsoProviderType,

    // =========================================================================
    // OIDC Configuration (used when provider_type = 'oidc')
    // =========================================================================
    /// OIDC issuer URL (e.g., "https://accounts.google.com")
    /// Required for OIDC, not used for SAML
    #[validate(length(max = 512), url)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,

    /// Discovery URL for OIDC metadata (optional - defaults to issuer/.well-known/openid-configuration)
    #[validate(length(max = 512), url)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_url: Option<String>,

    /// OAuth2 client ID (required for OIDC, not used for SAML)
    #[validate(length(max = 256))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    /// OAuth2 client secret (will be stored in secret manager)
    /// Required for OIDC, not used for SAML
    #[validate(length(max = 1024))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// Redirect URI for OAuth2 callback (optional - uses global default if not set)
    #[validate(length(max = 512), url)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,

    /// OAuth2 scopes to request (defaults to ["openid", "email", "profile"])
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,

    /// JWT claim to use as the user's identity (default: "sub")
    #[validate(length(min = 1, max = 64))]
    #[serde(default = "default_identity_claim")]
    pub identity_claim: String,

    /// JWT claim containing organization IDs (optional)
    #[validate(length(max = 64))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_claim: Option<String>,

    /// JWT claim containing group memberships (optional)
    #[validate(length(max = 64))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups_claim: Option<String>,

    // =========================================================================
    // SAML 2.0 Configuration (used when provider_type = 'saml')
    // =========================================================================
    /// IdP metadata URL for auto-configuration (alternative to manual config)
    #[validate(length(max = 512), url)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_metadata_url: Option<String>,

    /// IdP entity identifier (e.g., "https://idp.example.com/metadata")
    #[validate(length(max = 512))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_idp_entity_id: Option<String>,

    /// IdP Single Sign-On service URL (HTTP-Redirect or HTTP-POST binding)
    #[validate(length(max = 512), url)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_idp_sso_url: Option<String>,

    /// IdP Single Logout service URL (optional)
    #[validate(length(max = 512), url)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_idp_slo_url: Option<String>,

    /// IdP X.509 certificate for signature validation (PEM format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_idp_certificate: Option<String>,

    /// Service Provider entity ID (Hadrian's identifier to the IdP)
    #[validate(length(max = 512))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_sp_entity_id: Option<String>,

    /// NameID format to request (e.g., 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress')
    #[validate(length(max = 256))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_name_id_format: Option<String>,

    /// Whether to sign AuthnRequests (default: false)
    #[serde(default)]
    pub saml_sign_requests: bool,

    /// SP private key for signing AuthnRequests (PEM format, will be stored in secret manager)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_sp_private_key: Option<String>,

    /// SP X.509 certificate for metadata (PEM format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_sp_certificate: Option<String>,

    /// Whether to force re-authentication at IdP (default: false)
    #[serde(default)]
    pub saml_force_authn: bool,

    /// Requested authentication context class
    #[validate(length(max = 256))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_authn_context_class_ref: Option<String>,

    /// SAML attribute name for user identity (like identity_claim for OIDC)
    #[validate(length(max = 256))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_identity_attribute: Option<String>,

    /// SAML attribute name for email
    #[validate(length(max = 256))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_email_attribute: Option<String>,

    /// SAML attribute name for display name
    #[validate(length(max = 256))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_name_attribute: Option<String>,

    /// SAML attribute name for groups
    #[validate(length(max = 256))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_groups_attribute: Option<String>,

    // =========================================================================
    // JIT Provisioning Settings (shared by OIDC and SAML)
    // =========================================================================
    /// Whether JIT provisioning is enabled (default: true)
    #[serde(default = "default_true")]
    pub provisioning_enabled: bool,

    /// Whether to create new users on first login (default: true)
    #[serde(default = "default_true")]
    pub create_users: bool,

    /// Default team to add new users to (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_team_id: Option<Uuid>,

    /// Default role for new users in the organization (default: "member")
    #[validate(length(min = 1, max = 32))]
    #[serde(default = "default_role")]
    pub default_org_role: String,

    /// Default role for new users in the default team (default: "member")
    #[validate(length(min = 1, max = 32))]
    #[serde(default = "default_role")]
    pub default_team_role: String,

    /// Allowed email domains (empty = allow all)
    #[serde(default)]
    pub allowed_email_domains: Vec<String>,

    /// Whether to sync user attributes on each login (default: false)
    #[serde(default)]
    pub sync_attributes_on_login: bool,

    /// Whether to sync team memberships from IdP groups on each login (default: true)
    #[serde(default = "default_true")]
    pub sync_memberships_on_login: bool,

    /// SSO enforcement mode (default: optional)
    #[serde(default)]
    pub enforcement_mode: SsoEnforcementMode,

    /// Whether this SSO config is active (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
    ]
}

fn default_identity_claim() -> String {
    "sub".to_string()
}

fn default_role() -> String {
    "member".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for CreateOrgSsoConfig {
    fn default() -> Self {
        Self {
            provider_type: SsoProviderType::default(),
            // OIDC fields (None for SAML configs)
            issuer: None,
            discovery_url: None,
            client_id: None,
            client_secret: None,
            redirect_uri: None,
            scopes: default_scopes(),
            identity_claim: default_identity_claim(),
            org_claim: None,
            groups_claim: None,
            // SAML fields
            saml_metadata_url: None,
            saml_idp_entity_id: None,
            saml_idp_sso_url: None,
            saml_idp_slo_url: None,
            saml_idp_certificate: None,
            saml_sp_entity_id: None,
            saml_name_id_format: None,
            saml_sign_requests: false,
            saml_sp_private_key: None,
            saml_sp_certificate: None,
            saml_force_authn: false,
            saml_authn_context_class_ref: None,
            saml_identity_attribute: None,
            saml_email_attribute: None,
            saml_name_attribute: None,
            saml_groups_attribute: None,
            // JIT provisioning
            provisioning_enabled: true,
            create_users: true,
            default_team_id: None,
            default_org_role: default_role(),
            default_team_role: default_role(),
            allowed_email_domains: Vec::new(),
            sync_attributes_on_login: false,
            sync_memberships_on_login: true,
            enforcement_mode: SsoEnforcementMode::default(),
            enabled: true,
        }
    }
}

/// Request to update an existing organization SSO configuration.
///
/// All fields are optional - only provided fields will be updated.
#[derive(Debug, Clone, Default, Deserialize, Validate)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct UpdateOrgSsoConfig {
    /// Update provider type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_type: Option<SsoProviderType>,

    // =========================================================================
    // OIDC Configuration
    // =========================================================================
    /// Update OIDC issuer URL
    #[validate(length(min = 1, max = 512), url)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,

    /// Update discovery URL (set to null to use default)
    #[validate(length(max = 512))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub discovery_url: Option<Option<String>>,

    /// Update OAuth2 client ID
    #[validate(length(min = 1, max = 256))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    /// Update OAuth2 client secret (will be stored in secret manager)
    #[validate(length(min = 1, max = 1024))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// Update redirect URI (set to null to use global default)
    #[validate(length(max = 512))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub redirect_uri: Option<Option<String>>,

    /// Update OAuth2 scopes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,

    /// Update identity claim
    #[validate(length(min = 1, max = 64))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_claim: Option<String>,

    /// Update org claim (set to null to remove)
    #[validate(length(max = 64))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub org_claim: Option<Option<String>>,

    /// Update groups claim (set to null to remove)
    #[validate(length(max = 64))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub groups_claim: Option<Option<String>>,

    // =========================================================================
    // SAML 2.0 Configuration
    // =========================================================================
    /// Update IdP metadata URL (set to null to remove)
    #[validate(length(max = 512))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_metadata_url: Option<Option<String>>,

    /// Update IdP entity identifier (set to null to remove)
    #[validate(length(max = 512))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_idp_entity_id: Option<Option<String>>,

    /// Update IdP SSO URL (set to null to remove)
    #[validate(length(max = 512))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_idp_sso_url: Option<Option<String>>,

    /// Update IdP SLO URL (set to null to remove)
    #[validate(length(max = 512))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_idp_slo_url: Option<Option<String>>,

    /// Update IdP certificate (set to null to remove)
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_idp_certificate: Option<Option<String>>,

    /// Update SP entity ID (set to null to remove)
    #[validate(length(max = 512))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_sp_entity_id: Option<Option<String>>,

    /// Update NameID format (set to null to remove)
    #[validate(length(max = 256))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_name_id_format: Option<Option<String>>,

    /// Update sign requests flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_sign_requests: Option<bool>,

    /// Update SP private key (will be stored in secret manager)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_sp_private_key: Option<String>,

    /// Update SP certificate (set to null to remove)
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_sp_certificate: Option<Option<String>>,

    /// Update force re-authentication flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saml_force_authn: Option<bool>,

    /// Update authentication context class (set to null to remove)
    #[validate(length(max = 256))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_authn_context_class_ref: Option<Option<String>>,

    /// Update SAML identity attribute (set to null to remove)
    #[validate(length(max = 256))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_identity_attribute: Option<Option<String>>,

    /// Update SAML email attribute (set to null to remove)
    #[validate(length(max = 256))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_email_attribute: Option<Option<String>>,

    /// Update SAML name attribute (set to null to remove)
    #[validate(length(max = 256))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_name_attribute: Option<Option<String>>,

    /// Update SAML groups attribute (set to null to remove)
    #[validate(length(max = 256))]
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub saml_groups_attribute: Option<Option<String>>,

    // =========================================================================
    // JIT Provisioning Settings
    // =========================================================================
    /// Update provisioning enabled flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioning_enabled: Option<bool>,

    /// Update create users flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_users: Option<bool>,

    /// Update default team (set to null to remove)
    #[serde(default, deserialize_with = "deserialize_optional_uuid")]
    pub default_team_id: Option<Option<Uuid>>,

    /// Update default org role
    #[validate(length(min = 1, max = 32))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_org_role: Option<String>,

    /// Update default team role
    #[validate(length(min = 1, max = 32))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_team_role: Option<String>,

    /// Update allowed email domains
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_email_domains: Option<Vec<String>>,

    /// Update sync attributes on login flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_attributes_on_login: Option<bool>,

    /// Update sync memberships on login flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_memberships_on_login: Option<bool>,

    /// Update enforcement mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforcement_mode: Option<SsoEnforcementMode>,

    /// Update enabled flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Custom deserializer for Option<Option<Uuid>> to distinguish between:
/// - Field not present in JSON -> None (don't update)
/// - Field present as null -> Some(None) (set to NULL)
/// - Field present with value -> Some(Some(uuid)) (set to value)
fn deserialize_optional_uuid<'de, D>(deserializer: D) -> Result<Option<Option<Uuid>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

/// Custom deserializer for Option<Option<String>> to distinguish between:
/// - Field not present in JSON -> None (don't update)
/// - Field present as null -> Some(None) (set to NULL)
/// - Field present with value -> Some(Some(string)) (set to value)
fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

/// Internal struct for database operations that includes secret key references.
/// This is NOT exposed via the API - only used internally.
#[derive(Debug, Clone)]
pub struct OrgSsoConfigWithSecret {
    /// The public SSO config
    pub config: OrgSsoConfig,
    /// Key reference for the OIDC client secret in the secret manager (for OIDC configs)
    pub client_secret_key: Option<String>,
    /// Key reference for the SAML SP private key in the secret manager (for SAML configs)
    pub saml_sp_private_key_ref: Option<String>,
}

impl OrgSsoConfigWithSecret {
    /// Convert to a SAML auth config for use by the SAML authenticator.
    ///
    /// # Arguments
    /// * `default_acs_url` - Default ACS URL if not specified in config
    /// * `session_config` - Session configuration for cookies/timeouts
    /// * `sp_private_key` - The SP private key (already retrieved from secret manager)
    ///
    /// # Returns
    /// * `Ok(SamlAuthConfig)` if all required SAML fields are present
    /// * `Err(String)` if required fields are missing
    #[cfg(feature = "saml")]
    pub fn to_saml_auth_config(
        &self,
        default_acs_url: &str,
        session_config: crate::config::SessionConfig,
        sp_private_key: Option<String>,
    ) -> Result<crate::auth::saml::SamlAuthConfig, String> {
        use crate::auth::saml::SamlAuthConfig;

        // Validate required SAML fields
        let idp_entity_id = self
            .config
            .saml_idp_entity_id
            .clone()
            .ok_or("SAML IdP entity ID is required")?;

        let idp_sso_url = self
            .config
            .saml_idp_sso_url
            .clone()
            .ok_or("SAML IdP SSO URL is required")?;

        let idp_certificate = self
            .config
            .saml_idp_certificate
            .clone()
            .ok_or("SAML IdP certificate is required")?;

        let sp_entity_id = self
            .config
            .saml_sp_entity_id
            .clone()
            .ok_or("SAML SP entity ID is required")?;

        // Derive SP ACS URL from SP entity ID (see saml_registry.rs for details)
        let sp_acs_url = derive_acs_url_from_entity_id(&sp_entity_id)
            .unwrap_or_else(|| default_acs_url.to_string());

        Ok(SamlAuthConfig {
            idp_entity_id,
            idp_sso_url,
            idp_slo_url: self.config.saml_idp_slo_url.clone(),
            idp_certificate,
            sp_entity_id,
            sp_acs_url,
            name_id_format: self.config.saml_name_id_format.clone(),
            sign_requests: self.config.saml_sign_requests,
            force_authn: self.config.saml_force_authn,
            authn_context_class_ref: self.config.saml_authn_context_class_ref.clone(),
            identity_attribute: self.config.saml_identity_attribute.clone(),
            email_attribute: self.config.saml_email_attribute.clone(),
            name_attribute: self.config.saml_name_attribute.clone(),
            groups_attribute: self.config.saml_groups_attribute.clone(),
            sp_private_key,
            sp_certificate: self.config.saml_sp_certificate.clone(),
            session: session_config,
            metadata_url: self.config.saml_metadata_url.clone(),
        })
    }
}
