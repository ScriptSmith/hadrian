//! Axum middleware layers for the Hadrian gateway.
//!
//! # Middleware Pipeline
//!
//! Middleware is applied in layers, with different middleware for different route groups:
//!
//! ## Global (all routes)
//! - [`request_id_middleware`] — Assigns a unique request ID to each request
//! - [`security_headers_middleware`] — Adds security response headers (CSP, HSTS, etc.)
//!
//! ## API routes (`/v1/*`)
//! Applied via [`get_api_routes()`](crate::routes::api::get_api_routes) in this order:
//! 1. [`rate_limit_middleware`] — IP-based rate limiting (rejects early before auth overhead)
//! 2. [`api_middleware`] — Authentication, budget enforcement, usage tracking
//! 3. [`api_authz_middleware`] — CEL-based authorization policy evaluation
//!
//! ## Admin routes (`/admin/v1/*`)
//! - [`admin_auth_middleware`] — Admin authentication (OIDC/cookie/API key)
//! - [`authz_middleware`] — System-level CEL policy evaluation
//!
//! ## Unprotected admin routes (login, session info)
//! - [`permissive_authz_middleware`] — Injects allow-all authz context

// ── Middleware layers ──────────────────────────────────────────────────────────
mod admin;
mod authz;
mod combined;
mod rate_limit;
mod request_id;
mod security_headers;

// ── Internal helpers (used only by combined.rs) ────────────────────────────────
mod budget;
mod scope;
mod usage;

// ── Middleware layer exports ───────────────────────────────────────────────────
pub use admin::{AdminAuth, admin_auth_middleware};
pub use authz::{
    AuthzContext, api_authz_middleware, authz_middleware, permissive_authz_middleware,
};
pub use combined::api_middleware;
#[cfg(feature = "sso")]
pub use rate_limit::extract_client_ip_from_parts;
pub use rate_limit::rate_limit_middleware;
pub use request_id::{RequestId, request_id_middleware};
pub use security_headers::security_headers_middleware;

// ── Types extracted by middleware (used by route handlers via Extension<T>) ────

/// Client connection metadata extracted by middleware for audit logging.
#[derive(Debug, Clone, Default)]
pub struct ClientInfo {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}
