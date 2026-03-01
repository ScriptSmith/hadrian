//! Response schema validation against OpenAI OpenAPI specification.
//!
//! This module provides runtime validation of API responses against the OpenAI
//! OpenAPI spec to catch response format issues before they reach clients.
//!
//! # Configuration
//!
//! Enable validation in `hadrian.toml`:
//!
//! ```toml
//! [observability.response_validation]
//! enabled = true
//! mode = "warn"  # or "error"
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use gateway::validation::{SchemaId, validate_response};
//!
//! let response_json = serde_json::json!({...});
//! if let Err(errors) = validate_response(SchemaId::ChatCompletion, &response_json) {
//!     tracing::warn!("Response validation failed: {}", errors);
//! }
//! ```

mod schema;
pub mod stream;
pub mod url;

pub use schema::{ResponseType, SchemaId, validate_response};
#[cfg(feature = "saml")]
pub use url::require_https;
pub use url::{UrlValidationOptions, validate_base_url, validate_base_url_opts};
