//! WASM SQLite database layer.
//!
//! Provides a sqlx-compatible API surface backed by wa-sqlite running in the
//! browser via JavaScript FFI. This allows the same repository SQL to work
//! on both native SQLite (via sqlx) and browser SQLite (via wa-sqlite + OPFS).
//!
//! # Architecture
//!
//! ```text
//! Rust repo code → WasmSqlitePool::query() → JS bridge → wa-sqlite → OPFS
//! ```
//!
//! The API mirrors sqlx's runtime query builder:
//! - `query(sql)` → `WasmQuery` with `.bind()` chaining
//! - `.fetch_all(pool)` / `.fetch_optional(pool)` / `.execute(pool)`
//! - `WasmRow` with `.get::<T>(column)` for type-safe column access

mod api_keys;
mod audit_logs;
mod bridge;
mod common;
mod conversations;
mod files;
mod model_pricing;
mod org_rbac_policies;
mod organizations;
mod projects;
mod prompts;
mod providers;
mod service_accounts;
mod teams;
mod types;
mod usage;
mod users;
mod vector_stores;

pub use api_keys::WasmSqliteApiKeyRepo;
pub use audit_logs::WasmSqliteAuditLogRepo;
pub use bridge::WasmSqlitePool;
pub use conversations::WasmSqliteConversationRepo;
pub use files::WasmSqliteFilesRepo;
pub use model_pricing::WasmSqliteModelPricingRepo;
pub use org_rbac_policies::WasmSqliteOrgRbacPolicyRepo;
pub use organizations::WasmSqliteOrganizationRepo;
pub use projects::WasmSqliteProjectRepo;
pub use prompts::WasmSqlitePromptRepo;
pub use providers::WasmSqliteDynamicProviderRepo;
pub use service_accounts::WasmSqliteServiceAccountRepo;
pub use teams::WasmSqliteTeamRepo;
pub use types::{WasmDbError, WasmParam, WasmQuery, WasmQueryResult, WasmRow, WasmValue};
pub use usage::WasmSqliteUsageRepo;
pub use users::WasmSqliteUserRepo;
pub use vector_stores::WasmSqliteVectorStoresRepo;

/// Create a query builder (analogous to `sqlx::query()`).
pub fn query(sql: &str) -> WasmQuery {
    WasmQuery::new(sql)
}
