//! SCIM 2.0 Resource and Protocol Types
//!
//! This module defines the core SCIM resource types (User, Group) and
//! protocol types (ListResponse, ServiceProviderConfig, etc.) per RFC 7643/7644.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// =============================================================================
// Schema URIs
// =============================================================================

/// SCIM Core User schema URI
pub const SCHEMA_USER: &str = "urn:ietf:params:scim:schemas:core:2.0:User";

/// SCIM Core Group schema URI
pub const SCHEMA_GROUP: &str = "urn:ietf:params:scim:schemas:core:2.0:Group";

/// SCIM Enterprise User extension schema URI
pub const SCHEMA_ENTERPRISE_USER: &str =
    "urn:ietf:params:scim:schemas:extension:enterprise:2.0:User";

/// SCIM ListResponse schema URI
pub const SCHEMA_LIST_RESPONSE: &str = "urn:ietf:params:scim:api:messages:2.0:ListResponse";

/// SCIM Error schema URI
pub const SCHEMA_ERROR: &str = "urn:ietf:params:scim:api:messages:2.0:Error";

/// SCIM PatchOp schema URI
pub const SCHEMA_PATCH_OP: &str = "urn:ietf:params:scim:api:messages:2.0:PatchOp";

/// SCIM ServiceProviderConfig schema URI
pub const SCHEMA_SERVICE_PROVIDER_CONFIG: &str =
    "urn:ietf:params:scim:schemas:core:2.0:ServiceProviderConfig";

/// SCIM ResourceType schema URI
pub const SCHEMA_RESOURCE_TYPE: &str = "urn:ietf:params:scim:schemas:core:2.0:ResourceType";

/// SCIM Schema schema URI
pub const SCHEMA_SCHEMA: &str = "urn:ietf:params:scim:schemas:core:2.0:Schema";

// =============================================================================
// Resource Metadata
// =============================================================================

/// Resource metadata common to all SCIM resources.
///
/// Contains server-assigned metadata about the resource lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimMeta {
    /// The resource type (e.g., "User", "Group")
    pub resource_type: String,

    /// When the resource was created
    pub created: DateTime<Utc>,

    /// When the resource was last modified
    pub last_modified: DateTime<Utc>,

    /// The absolute URI of the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,

    /// ETag for optimistic concurrency (e.g., "W/\"a330bc54f0671c9\"")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl ScimMeta {
    /// Create metadata for a User resource
    pub fn user(created: DateTime<Utc>, last_modified: DateTime<Utc>) -> Self {
        Self {
            resource_type: "User".to_string(),
            created,
            last_modified,
            location: None,
            version: None,
        }
    }

    /// Create metadata for a Group resource
    pub fn group(created: DateTime<Utc>, last_modified: DateTime<Utc>) -> Self {
        Self {
            resource_type: "Group".to_string(),
            created,
            last_modified,
            location: None,
            version: None,
        }
    }

    /// Set the location URI
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Set the ETag version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}

// =============================================================================
// User Resource (RFC 7643)
// =============================================================================

/// SCIM User resource.
///
/// Represents a user account with identity attributes. This is the core
/// resource type for user provisioning operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimUser {
    /// SCIM schema URIs for this resource
    pub schemas: Vec<String>,

    /// Server-assigned unique identifier
    pub id: String,

    /// Client-assigned identifier for correlation with IdP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,

    /// Unique identifier for the user (typically email)
    pub user_name: String,

    /// User's name components
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<ScimName>,

    /// Display name shown in UI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Email addresses
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<ScimEmail>,

    /// Phone numbers
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phone_numbers: Vec<ScimPhoneNumber>,

    /// Whether the user is active
    #[serde(default = "default_true")]
    pub active: bool,

    /// Groups the user belongs to (read-only, populated by server)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ScimGroupRef>,

    /// Resource metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ScimMeta>,
}

impl ScimUser {
    /// Create a new SCIM user with minimal required fields
    pub fn new(id: impl Into<String>, user_name: impl Into<String>) -> Self {
        Self {
            schemas: vec![SCHEMA_USER.to_string()],
            id: id.into(),
            external_id: None,
            user_name: user_name.into(),
            name: None,
            display_name: None,
            emails: Vec::new(),
            phone_numbers: Vec::new(),
            active: true,
            groups: Vec::new(),
            meta: None,
        }
    }

    /// Get the primary email address if any
    pub fn primary_email(&self) -> Option<&str> {
        self.emails
            .iter()
            .find(|e| e.primary.unwrap_or(false))
            .or_else(|| self.emails.first())
            .map(|e| e.value.as_str())
    }

    /// Get the formatted display name, falling back to userName
    pub fn display_name_or_username(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.user_name)
    }
}

impl Default for ScimUser {
    fn default() -> Self {
        Self {
            schemas: vec![SCHEMA_USER.to_string()],
            id: String::new(),
            external_id: None,
            user_name: String::new(),
            name: None,
            display_name: None,
            emails: Vec::new(),
            phone_numbers: Vec::new(),
            active: true,
            groups: Vec::new(),
            meta: None,
        }
    }
}

/// User's name components
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimName {
    /// Full formatted name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatted: Option<String>,

    /// Family name (last name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,

    /// Given name (first name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,

    /// Middle name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middle_name: Option<String>,

    /// Honorific prefix (e.g., "Dr.", "Mr.")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub honorific_prefix: Option<String>,

    /// Honorific suffix (e.g., "PhD", "Jr.")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub honorific_suffix: Option<String>,
}

impl ScimName {
    /// Create a name from given and family names
    pub fn from_names(given: impl Into<String>, family: impl Into<String>) -> Self {
        let given = given.into();
        let family = family.into();
        Self {
            formatted: Some(format!("{} {}", given, family)),
            given_name: Some(given),
            family_name: Some(family),
            middle_name: None,
            honorific_prefix: None,
            honorific_suffix: None,
        }
    }
}

/// Email address with type and primary flag
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimEmail {
    /// Email address value
    pub value: String,

    /// Email type (e.g., "work", "home")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub email_type: Option<String>,

    /// Whether this is the primary email
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
}

impl ScimEmail {
    /// Create a primary work email
    pub fn work_primary(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            email_type: Some("work".to_string()),
            primary: Some(true),
        }
    }

    /// Create a non-primary email
    pub fn other(value: impl Into<String>, email_type: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            email_type: Some(email_type.into()),
            primary: None,
        }
    }
}

/// Phone number with type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimPhoneNumber {
    /// Phone number value
    pub value: String,

    /// Phone type (e.g., "work", "mobile", "home")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub phone_type: Option<String>,
}

/// Reference to a group the user belongs to (read-only)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimGroupRef {
    /// Group ID
    pub value: String,

    /// URI reference to the group
    #[serde(rename = "$ref")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_uri: Option<String>,

    /// Display name of the group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,

    /// Membership type ("direct" or "indirect")
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub membership_type: Option<String>,
}

// =============================================================================
// Group Resource (RFC 7643)
// =============================================================================

/// SCIM Group resource.
///
/// Represents a group of users for membership-based access control.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimGroup {
    /// SCIM schema URIs for this resource
    pub schemas: Vec<String>,

    /// Server-assigned unique identifier
    pub id: String,

    /// Client-assigned identifier for correlation with IdP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,

    /// Human-readable group name
    pub display_name: String,

    /// Group members
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<ScimGroupMember>,

    /// Resource metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ScimMeta>,
}

impl ScimGroup {
    /// Create a new SCIM group with minimal required fields
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            schemas: vec![SCHEMA_GROUP.to_string()],
            id: id.into(),
            external_id: None,
            display_name: display_name.into(),
            members: Vec::new(),
            meta: None,
        }
    }
}

impl Default for ScimGroup {
    fn default() -> Self {
        Self {
            schemas: vec![SCHEMA_GROUP.to_string()],
            id: String::new(),
            external_id: None,
            display_name: String::new(),
            members: Vec::new(),
            meta: None,
        }
    }
}

/// Group member reference
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimGroupMember {
    /// User ID
    pub value: String,

    /// URI reference to the user
    #[serde(rename = "$ref")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_uri: Option<String>,

    /// Display name of the member
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,

    /// Member type (typically "User")
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_type: Option<String>,
}

impl ScimGroupMember {
    /// Create a user member reference
    pub fn user(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            ref_uri: None,
            display: None,
            member_type: Some("User".to_string()),
        }
    }

    /// Create a user member with display name
    pub fn user_with_display(value: impl Into<String>, display: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            ref_uri: None,
            display: Some(display.into()),
            member_type: Some("User".to_string()),
        }
    }
}

// =============================================================================
// Protocol Types (RFC 7644)
// =============================================================================

/// SCIM list response for paginated collections.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimListResponse<T> {
    /// SCIM schema URIs
    pub schemas: Vec<String>,

    /// Total number of results available
    pub total_results: u32,

    /// Number of results returned in this response
    pub items_per_page: u32,

    /// 1-based index of the first result in this response
    pub start_index: u32,

    /// The list of resources
    #[serde(rename = "Resources")]
    pub resources: Vec<T>,
}

impl<T> ScimListResponse<T> {
    /// Create a new list response
    pub fn new(resources: Vec<T>, total_results: u32, start_index: u32) -> Self {
        let items_per_page = resources.len() as u32;
        Self {
            schemas: vec![SCHEMA_LIST_RESPONSE.to_string()],
            total_results,
            items_per_page,
            start_index,
            resources,
        }
    }

    /// Create an empty list response
    pub fn empty() -> Self {
        Self {
            schemas: vec![SCHEMA_LIST_RESPONSE.to_string()],
            total_results: 0,
            items_per_page: 0,
            start_index: 1,
            resources: Vec::new(),
        }
    }
}

/// Query parameters for list operations
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimListParams {
    /// SCIM filter expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,

    /// 1-based start index (default: 1)
    #[serde(default = "default_start_index")]
    pub start_index: u32,

    /// Number of results per page (default: 100)
    #[serde(default = "default_count")]
    pub count: u32,

    /// Attribute to sort by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,

    /// Sort order ("ascending" or "descending")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<String>,

    /// Attributes to include in response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<String>,

    /// Attributes to exclude from response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excluded_attributes: Option<String>,
}

fn default_start_index() -> u32 {
    1
}

fn default_count() -> u32 {
    100
}

// =============================================================================
// Discovery Types (RFC 7644)
// =============================================================================

/// Service Provider Configuration.
///
/// Advertises the SCIM service's capabilities and supported features.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ServiceProviderConfig {
    /// SCIM schema URIs
    pub schemas: Vec<String>,

    /// Documentation URI for this SCIM implementation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_uri: Option<String>,

    /// PATCH operation support
    pub patch: FeatureSupport,

    /// Bulk operation support
    pub bulk: BulkSupport,

    /// Filter support
    pub filter: FilterSupport,

    /// Change password support
    pub change_password: FeatureSupport,

    /// Sort support
    pub sort: FeatureSupport,

    /// ETag support
    pub etag: FeatureSupport,

    /// Supported authentication schemes
    pub authentication_schemes: Vec<AuthenticationScheme>,

    /// Resource metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ScimMeta>,
}

impl Default for ServiceProviderConfig {
    fn default() -> Self {
        Self {
            schemas: vec![SCHEMA_SERVICE_PROVIDER_CONFIG.to_string()],
            documentation_uri: None,
            patch: FeatureSupport { supported: true },
            bulk: BulkSupport::unsupported(),
            filter: FilterSupport::default(),
            change_password: FeatureSupport { supported: false },
            sort: FeatureSupport { supported: true },
            etag: FeatureSupport { supported: false },
            authentication_schemes: vec![AuthenticationScheme::oauth_bearer()],
            meta: None,
        }
    }
}

/// Simple feature support flag
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FeatureSupport {
    pub supported: bool,
}

/// Bulk operation support configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BulkSupport {
    pub supported: bool,
    pub max_operations: u32,
    pub max_payload_size: u32,
}

impl BulkSupport {
    pub fn unsupported() -> Self {
        Self {
            supported: false,
            max_operations: 0,
            max_payload_size: 0,
        }
    }
}

/// Filter support configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct FilterSupport {
    pub supported: bool,
    pub max_results: u32,
}

impl Default for FilterSupport {
    fn default() -> Self {
        Self {
            supported: true,
            max_results: 200,
        }
    }
}

/// Authentication scheme definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationScheme {
    /// Display name
    pub name: String,

    /// Human-readable description
    pub description: String,

    /// URI to specification document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec_uri: Option<String>,

    /// Scheme type identifier
    #[serde(rename = "type")]
    pub scheme_type: String,

    /// Whether this is the primary authentication method
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
}

impl AuthenticationScheme {
    /// Create OAuth 2.0 Bearer Token scheme
    pub fn oauth_bearer() -> Self {
        Self {
            name: "OAuth 2.0 Bearer Token".to_string(),
            description: "Bearer token authentication using OAuth 2.0".to_string(),
            spec_uri: Some("https://tools.ietf.org/html/rfc6750".to_string()),
            scheme_type: "oauthbearertoken".to_string(),
            primary: Some(true),
        }
    }
}

/// Resource type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ResourceType {
    /// SCIM schema URIs
    pub schemas: Vec<String>,

    /// Resource type identifier (e.g., "User")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Endpoint path (e.g., "/Users")
    pub endpoint: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Primary schema URI for this resource type
    pub schema: String,

    /// Schema extensions supported
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schema_extensions: Vec<SchemaExtension>,

    /// Resource metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ScimMeta>,
}

impl ResourceType {
    /// Create User resource type
    pub fn user(base_url: &str) -> Self {
        Self {
            schemas: vec![SCHEMA_RESOURCE_TYPE.to_string()],
            id: "User".to_string(),
            name: "User".to_string(),
            endpoint: "/Users".to_string(),
            description: Some("User account".to_string()),
            schema: SCHEMA_USER.to_string(),
            schema_extensions: Vec::new(),
            meta: Some(
                ScimMeta::user(Utc::now(), Utc::now())
                    .with_location(format!("{}/ResourceTypes/User", base_url)),
            ),
        }
    }

    /// Create Group resource type
    pub fn group(base_url: &str) -> Self {
        Self {
            schemas: vec![SCHEMA_RESOURCE_TYPE.to_string()],
            id: "Group".to_string(),
            name: "Group".to_string(),
            endpoint: "/Groups".to_string(),
            description: Some("Group of users".to_string()),
            schema: SCHEMA_GROUP.to_string(),
            schema_extensions: Vec::new(),
            meta: Some(
                ScimMeta::group(Utc::now(), Utc::now())
                    .with_location(format!("{}/ResourceTypes/Group", base_url)),
            ),
        }
    }
}

/// Schema extension reference
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SchemaExtension {
    /// Schema URI of the extension
    pub schema: String,
    /// Whether the extension is required
    pub required: bool,
}

// =============================================================================
// Schema Definition Types (RFC 7643 Section 7)
// =============================================================================

/// SCIM Schema definition.
///
/// Describes the structure of a SCIM resource type, including its attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ScimSchema {
    /// Always contains SCHEMA_SCHEMA
    pub schemas: Vec<String>,

    /// Schema URI (e.g., "urn:ietf:params:scim:schemas:core:2.0:User")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Description of the schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Attribute definitions
    pub attributes: Vec<SchemaAttribute>,

    /// Resource metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ScimMeta>,
}

impl ScimSchema {
    /// Create the User schema definition
    pub fn user(base_url: &str) -> Self {
        Self {
            schemas: vec![SCHEMA_SCHEMA.to_string()],
            id: SCHEMA_USER.to_string(),
            name: "User".to_string(),
            description: Some("User account".to_string()),
            attributes: vec![
                SchemaAttribute::string(
                    "userName",
                    "Unique identifier for the user",
                    true,
                    Mutability::ReadWrite,
                ),
                SchemaAttribute::complex(
                    "name",
                    "Name components",
                    false,
                    vec![
                        SchemaAttribute::string(
                            "formatted",
                            "Full formatted name",
                            false,
                            Mutability::ReadWrite,
                        ),
                        SchemaAttribute::string(
                            "familyName",
                            "Family name (last name)",
                            false,
                            Mutability::ReadWrite,
                        ),
                        SchemaAttribute::string(
                            "givenName",
                            "Given name (first name)",
                            false,
                            Mutability::ReadWrite,
                        ),
                    ],
                ),
                SchemaAttribute::string(
                    "displayName",
                    "Display name shown in UI",
                    false,
                    Mutability::ReadWrite,
                ),
                SchemaAttribute::multi_valued(
                    "emails",
                    "Email addresses",
                    false,
                    vec![
                        SchemaAttribute::string(
                            "value",
                            "Email address",
                            false,
                            Mutability::ReadWrite,
                        ),
                        SchemaAttribute::string(
                            "type",
                            "Email type (work, home)",
                            false,
                            Mutability::ReadWrite,
                        ),
                        SchemaAttribute::boolean(
                            "primary",
                            "Is primary email",
                            false,
                            Mutability::ReadWrite,
                        ),
                    ],
                ),
                SchemaAttribute::boolean(
                    "active",
                    "Whether user is active",
                    false,
                    Mutability::ReadWrite,
                ),
                SchemaAttribute::string(
                    "externalId",
                    "External ID from IdP",
                    false,
                    Mutability::ReadWrite,
                ),
            ],
            meta: Some(
                ScimMeta::user(Utc::now(), Utc::now())
                    .with_location(format!("{}/Schemas/{}", base_url, SCHEMA_USER)),
            ),
        }
    }

    /// Create the Group schema definition
    pub fn group(base_url: &str) -> Self {
        Self {
            schemas: vec![SCHEMA_SCHEMA.to_string()],
            id: SCHEMA_GROUP.to_string(),
            name: "Group".to_string(),
            description: Some("Group of users".to_string()),
            attributes: vec![
                SchemaAttribute::string(
                    "displayName",
                    "Human-readable group name",
                    true,
                    Mutability::ReadWrite,
                ),
                SchemaAttribute::multi_valued(
                    "members",
                    "Group members",
                    false,
                    vec![
                        SchemaAttribute::string("value", "Member ID", false, Mutability::Immutable),
                        SchemaAttribute::string(
                            "$ref",
                            "Member URI reference",
                            false,
                            Mutability::Immutable,
                        ),
                        SchemaAttribute::string(
                            "display",
                            "Member display name",
                            false,
                            Mutability::Immutable,
                        ),
                    ],
                ),
                SchemaAttribute::string(
                    "externalId",
                    "External ID from IdP",
                    false,
                    Mutability::ReadWrite,
                ),
            ],
            meta: Some(
                ScimMeta::group(Utc::now(), Utc::now())
                    .with_location(format!("{}/Schemas/{}", base_url, SCHEMA_GROUP)),
            ),
        }
    }
}

/// SCIM attribute definition within a schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SchemaAttribute {
    /// Attribute name
    pub name: String,

    /// Attribute data type
    #[serde(rename = "type")]
    pub attr_type: AttributeType,

    /// Whether this is a multi-valued attribute
    pub multi_valued: bool,

    /// Description of the attribute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether the attribute is required
    pub required: bool,

    /// Whether the attribute is case-exact (only for strings)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_exact: Option<bool>,

    /// Mutability of the attribute
    pub mutability: Mutability,

    /// Whether the attribute is returned by default
    pub returned: Returned,

    /// Uniqueness constraint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uniqueness: Option<Uniqueness>,

    /// Sub-attributes for complex types
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_attributes: Vec<SchemaAttribute>,
}

impl SchemaAttribute {
    /// Create a string attribute
    pub fn string(name: &str, description: &str, required: bool, mutability: Mutability) -> Self {
        Self {
            name: name.to_string(),
            attr_type: AttributeType::String,
            multi_valued: false,
            description: Some(description.to_string()),
            required,
            case_exact: Some(false),
            mutability,
            returned: Returned::Default,
            uniqueness: None,
            sub_attributes: Vec::new(),
        }
    }

    /// Create a boolean attribute
    pub fn boolean(name: &str, description: &str, required: bool, mutability: Mutability) -> Self {
        Self {
            name: name.to_string(),
            attr_type: AttributeType::Boolean,
            multi_valued: false,
            description: Some(description.to_string()),
            required,
            case_exact: None,
            mutability,
            returned: Returned::Default,
            uniqueness: None,
            sub_attributes: Vec::new(),
        }
    }

    /// Create a complex attribute with sub-attributes
    pub fn complex(
        name: &str,
        description: &str,
        required: bool,
        sub_attributes: Vec<SchemaAttribute>,
    ) -> Self {
        Self {
            name: name.to_string(),
            attr_type: AttributeType::Complex,
            multi_valued: false,
            description: Some(description.to_string()),
            required,
            case_exact: None,
            mutability: Mutability::ReadWrite,
            returned: Returned::Default,
            uniqueness: None,
            sub_attributes,
        }
    }

    /// Create a multi-valued complex attribute
    pub fn multi_valued(
        name: &str,
        description: &str,
        required: bool,
        sub_attributes: Vec<SchemaAttribute>,
    ) -> Self {
        Self {
            name: name.to_string(),
            attr_type: AttributeType::Complex,
            multi_valued: true,
            description: Some(description.to_string()),
            required,
            case_exact: None,
            mutability: Mutability::ReadWrite,
            returned: Returned::Default,
            uniqueness: None,
            sub_attributes,
        }
    }
}

/// SCIM attribute data type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum AttributeType {
    String,
    Boolean,
    Decimal,
    Integer,
    DateTime,
    Reference,
    Complex,
    Binary,
}

/// SCIM attribute mutability
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub enum Mutability {
    ReadOnly,
    ReadWrite,
    Immutable,
    WriteOnly,
}

/// SCIM attribute return behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Returned {
    Always,
    Never,
    Default,
    Request,
}

/// SCIM attribute uniqueness constraint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum Uniqueness {
    None,
    Server,
    Global,
}

// =============================================================================
// Helper functions
// =============================================================================

fn default_true() -> bool {
    true
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scim_user_serialization() {
        let user = ScimUser {
            schemas: vec![SCHEMA_USER.to_string()],
            id: "12345".to_string(),
            external_id: Some("ext-001".to_string()),
            user_name: "john.doe@example.com".to_string(),
            name: Some(ScimName::from_names("John", "Doe")),
            display_name: Some("John Doe".to_string()),
            emails: vec![ScimEmail::work_primary("john.doe@example.com")],
            phone_numbers: Vec::new(),
            active: true,
            groups: Vec::new(),
            meta: Some(ScimMeta::user(Utc::now(), Utc::now())),
        };

        let json = serde_json::to_string_pretty(&user).unwrap();
        assert!(json.contains("\"userName\""));
        assert!(json.contains("\"externalId\""));
        assert!(json.contains("john.doe@example.com"));

        // Roundtrip
        let parsed: ScimUser = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.user_name, "john.doe@example.com");
        assert_eq!(parsed.id, "12345");
    }

    #[test]
    fn test_scim_group_serialization() {
        let group = ScimGroup {
            schemas: vec![SCHEMA_GROUP.to_string()],
            id: "group-123".to_string(),
            external_id: Some("ext-group-001".to_string()),
            display_name: "Engineering".to_string(),
            members: vec![
                ScimGroupMember::user_with_display("user-1", "John Doe"),
                ScimGroupMember::user("user-2"),
            ],
            meta: Some(ScimMeta::group(Utc::now(), Utc::now())),
        };

        let json = serde_json::to_string_pretty(&group).unwrap();
        assert!(json.contains("\"displayName\""));
        assert!(json.contains("Engineering"));
        assert!(json.contains("\"members\""));

        // Roundtrip
        let parsed: ScimGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.display_name, "Engineering");
        assert_eq!(parsed.members.len(), 2);
    }

    #[test]
    fn test_scim_list_response() {
        let users = vec![
            ScimUser::new("1", "alice@example.com"),
            ScimUser::new("2", "bob@example.com"),
        ];

        let response = ScimListResponse::new(users, 100, 1);

        let json = serde_json::to_string_pretty(&response).unwrap();
        assert!(json.contains("\"totalResults\": 100"));
        assert!(json.contains("\"itemsPerPage\": 2"));
        assert!(json.contains("\"startIndex\": 1"));
        assert!(json.contains("\"Resources\""));
    }

    #[test]
    fn test_service_provider_config() {
        let config = ServiceProviderConfig::default();

        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("\"patch\""));
        assert!(json.contains("\"supported\": true"));
        assert!(json.contains("\"authenticationSchemes\""));
    }

    #[test]
    fn test_scim_user_primary_email() {
        let mut user = ScimUser::new("1", "user@example.com");

        // No emails
        assert!(user.primary_email().is_none());

        // Single email (becomes primary by default)
        user.emails
            .push(ScimEmail::other("work@example.com", "work"));
        assert_eq!(user.primary_email(), Some("work@example.com"));

        // Add primary email
        user.emails
            .push(ScimEmail::work_primary("primary@example.com"));
        assert_eq!(user.primary_email(), Some("primary@example.com"));
    }

    #[test]
    fn test_resource_types() {
        let user_type = ResourceType::user("https://example.com/scim");
        assert_eq!(user_type.id, "User");
        assert_eq!(user_type.endpoint, "/Users");

        let group_type = ResourceType::group("https://example.com/scim");
        assert_eq!(group_type.id, "Group");
        assert_eq!(group_type.endpoint, "/Groups");
    }
}
