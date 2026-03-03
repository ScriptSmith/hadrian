mod api_types;
mod app;
mod auth;
pub mod authz;
mod cache;
mod catalog;
mod cli;
mod config;
mod db;
mod dlq;
pub mod events;
mod guardrails;
mod init;
mod jobs;
mod middleware;
mod models;
pub mod observability;
mod ontology;
pub mod openapi;
mod pricing;
mod providers;
mod retention;
mod routes;
mod routing;
#[cfg(feature = "sso")]
pub mod scim;
mod secrets;
pub mod services;
mod streaming;
mod usage_buffer;
mod usage_sink;
mod validation;
#[cfg(feature = "wizard")]
mod wizard;

#[cfg(test)]
mod tests;

// Re-export items that other modules reference via `crate::`.
pub use app::{AppState, build_app};
use clap::Parser;
#[cfg(feature = "wizard")]
pub(crate) use cli::{default_config_path, default_data_dir};

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();
    cli::dispatch(args).await;
}
