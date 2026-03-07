mod api_types;
pub mod app;
pub mod auth;
pub mod authz;
pub mod cache;
pub mod catalog;
#[cfg(feature = "cli")]
pub mod cli;
pub mod compat;
pub mod config;
pub mod db;
pub mod dlq;
pub mod events;
pub mod guardrails;
pub mod init;
pub mod jobs;
pub mod middleware;
pub mod models;
pub mod observability;
pub mod ontology;
pub mod openapi;
pub mod pricing;
pub mod providers;
pub mod retention;
pub mod routes;
pub mod routing;
#[cfg(feature = "sso")]
pub mod scim;
pub mod secrets;
pub mod services;
pub mod streaming;
pub mod usage_buffer;
pub mod usage_sink;
pub mod validation;
#[cfg(feature = "wizard")]
pub mod wizard;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(test)]
mod tests;

// Re-export items that other modules reference via `crate::`.
pub use app::AppState;
#[cfg(feature = "server")]
pub use app::build_app;
#[cfg(feature = "wizard")]
pub(crate) use cli::{default_config_path, default_data_dir};
