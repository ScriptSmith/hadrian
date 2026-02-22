//! SCIM 2.0 Protocol Routes
//!
//! This module implements SCIM 2.0 (System for Cross-domain Identity Management)
//! protocol endpoints per RFC 7643 (Core Schema) and RFC 7644 (Protocol).
//!
//! SCIM enables automatic user provisioning and deprovisioning from identity
//! providers like Okta, Azure AD, Google Workspace, OneLogin, Keycloak, and Auth0.
//!
//! ## Endpoint Structure
//!
//! All SCIM endpoints are under `/scim/v2/`:
//!
//! **Discovery Endpoints:**
//! - `GET /scim/v2/ServiceProviderConfig` - Service capabilities
//! - `GET /scim/v2/ResourceTypes` - List supported resource types
//! - `GET /scim/v2/ResourceTypes/{id}` - Get specific resource type
//! - `GET /scim/v2/Schemas` - List supported schemas
//! - `GET /scim/v2/Schemas/{id}` - Get specific schema
//!
//! **Resource Endpoints:**
//! - `GET/POST /scim/v2/Users` - List/create users
//! - `GET/PUT/PATCH/DELETE /scim/v2/Users/{id}` - User operations
//! - `GET/POST /scim/v2/Groups` - List/create groups
//! - `GET/PUT/PATCH/DELETE /scim/v2/Groups/{id}` - Group operations

pub mod discovery;
pub mod groups;
pub mod middleware;
pub mod users;

use axum::{Router, routing::get};

use crate::AppState;

/// Build the SCIM routes.
///
/// Returns a router configured for `/scim/v2/` endpoints with bearer token
/// authentication middleware applied.
pub fn scim_routes(state: AppState) -> Router<AppState> {
    Router::new().nest("/v2", scim_v2_routes(state))
}

fn scim_v2_routes(state: AppState) -> Router<AppState> {
    Router::new()
        // Discovery endpoints
        .route(
            "/ServiceProviderConfig",
            get(discovery::service_provider_config),
        )
        .route("/ResourceTypes", get(discovery::resource_types))
        .route("/ResourceTypes/{id}", get(discovery::resource_type))
        .route("/Schemas", get(discovery::schemas))
        .route("/Schemas/{id}", get(discovery::schema))
        // User resource endpoints
        .route(
            "/Users",
            axum::routing::get(users::list_users).post(users::create_user),
        )
        .route(
            "/Users/{id}",
            axum::routing::get(users::get_user)
                .put(users::replace_user)
                .patch(users::patch_user)
                .delete(users::delete_user),
        )
        // Group resource endpoints
        .route(
            "/Groups",
            axum::routing::get(groups::list_groups).post(groups::create_group),
        )
        .route(
            "/Groups/{id}",
            axum::routing::get(groups::get_group)
                .put(groups::replace_group)
                .patch(groups::patch_group)
                .delete(groups::delete_group),
        )
        // Apply SCIM bearer token authentication to all routes
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::scim_auth_middleware,
        ))
        // Apply IP-based rate limiting (runs before auth)
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::middleware::rate_limit_middleware,
        ))
}
