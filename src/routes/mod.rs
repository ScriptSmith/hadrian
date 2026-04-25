pub mod admin;
pub mod api;
#[cfg(feature = "sso")]
pub mod auth;
pub mod execution;
pub mod health;
pub mod oauth_public;
#[cfg(feature = "sso")]
pub mod scim;
#[cfg(feature = "server")]
pub mod ws;

pub use api::*;
#[cfg(feature = "sso")]
pub use auth as auth_routes;
#[cfg(feature = "sso")]
pub use scim::scim_routes;
#[cfg(feature = "server")]
pub use ws::ws_handler;
