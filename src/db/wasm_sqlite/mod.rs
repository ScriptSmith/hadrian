//! WASM SQLite database layer.
//!
//! Provides a sqlx-compatible API surface backed by wa-sqlite running in the
//! browser via JavaScript FFI. The repository implementations live in
//! `src/db/sqlite/` and are shared with native SQLite via the backend
//! abstraction layer (`src/db/sqlite/backend.rs`).
//!
//! This module only exports the FFI bridge and core types.
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

pub(crate) mod bridge;
pub(crate) mod types;

pub use bridge::WasmSqlitePool;
pub use types::{
    WasmDbError, WasmDecode, WasmParam, WasmQuery, WasmQueryResult, WasmRow, WasmValue,
};

/// Create a query builder (analogous to `sqlx::query()`).
pub fn query(sql: &str) -> WasmQuery {
    WasmQuery::new(sql)
}
