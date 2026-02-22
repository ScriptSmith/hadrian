//! SCIM 2.0 Protocol Implementation
//!
//! This module provides types and utilities for implementing SCIM 2.0
//! (System for Cross-domain Identity Management) protocol support.
//!
//! SCIM enables automatic user provisioning and deprovisioning from identity
//! providers like Okta, Azure AD, Google Workspace, OneLogin, Keycloak, and Auth0.
//!
//! ## RFC References
//!
//! - RFC 7643: SCIM Core Schema
//! - RFC 7644: SCIM Protocol
//!
//! ## Module Structure
//!
//! - [`types`]: Core SCIM resource types (User, Group) and protocol types
//! - [`error`]: SCIM-specific error responses per RFC 7644
//! - [`filter`]: SCIM filter expression parser
//! - [`patch`]: SCIM PATCH operation parser and executor

pub mod error;
pub mod filter;
pub mod filter_to_sql;
pub mod patch;
pub mod types;

pub use error::*;
pub use filter::{AttrPath, CompareOp, Filter, FilterParseError, FilterValue, parse_filter};
pub use filter_to_sql::{ScimResourceType, SqlFilter, SqlValue, filter_to_sql};
pub use patch::{PatchError, PatchOp, PatchPath, PatchRequest, matches_filter, parse_path};
pub use types::*;
