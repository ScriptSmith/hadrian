//! JavaScript FFI bridge to wa-sqlite running in the browser.
//!
//! `WasmSqlitePool` holds no Rust state — the actual SQLite database lives in
//! JavaScript (wa-sqlite + OPFS). All queries are dispatched via `wasm_bindgen`
//! extern functions and results are deserialized back into Rust types.

use wasm_bindgen::prelude::*;

use super::types::{WasmDbError, WasmParam, WasmQueryResult, WasmRow, WasmValue};

// ─────────────────────────────────────────────────────────────────────────────
// JS FFI declarations
// ─────────────────────────────────────────────────────────────────────────────

#[wasm_bindgen]
extern "C" {
    /// Execute a SELECT query. Returns a JSON-encoded array of row objects.
    /// Each row is an object mapping column names to values.
    #[wasm_bindgen(js_namespace = ["globalThis", "__hadrian_sqlite"], catch)]
    async fn query(sql: &str, params: JsValue) -> Result<JsValue, JsValue>;

    /// Execute a write statement (INSERT/UPDATE/DELETE).
    /// Returns a JSON-encoded object with `{ changes: number, last_insert_rowid: number }`.
    #[wasm_bindgen(js_namespace = ["globalThis", "__hadrian_sqlite"], catch)]
    async fn execute(sql: &str, params: JsValue) -> Result<JsValue, JsValue>;

    /// Initialize the database (create tables, run migrations).
    #[wasm_bindgen(js_namespace = ["globalThis", "__hadrian_sqlite"], catch)]
    async fn init_database() -> Result<(), JsValue>;

    /// Execute a multi-statement SQL script (e.g. migrations).
    /// Uses sql.js `db.exec()` which handles multiple statements natively.
    #[wasm_bindgen(js_namespace = ["globalThis", "__hadrian_sqlite"], catch)]
    async fn execute_script(sql: &str) -> Result<(), JsValue>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Pool
// ─────────────────────────────────────────────────────────────────────────────

/// Handle to the wa-sqlite database running in JavaScript.
///
/// This is a zero-size type — the actual database is managed by JS. Multiple
/// clones share the same underlying JS database instance.
#[derive(Debug, Clone)]
pub struct WasmSqlitePool;

impl WasmSqlitePool {
    pub fn new() -> Self {
        Self
    }

    /// Initialize the database (called once at startup).
    pub async fn init(&self) -> Result<(), WasmDbError> {
        init_database()
            .await
            .map_err(|e| WasmDbError::Query(js_error_to_string(&e)))
    }

    /// Execute a SELECT query and return all matching rows.
    pub async fn execute_query(
        &self,
        sql: &str,
        params: &[WasmParam],
    ) -> Result<Vec<WasmRow>, WasmDbError> {
        let js_params =
            serde_wasm_bindgen::to_value(params).map_err(|e| WasmDbError::Query(e.to_string()))?;

        let result = query(sql, js_params)
            .await
            .map_err(|e| classify_js_error(&e))?;

        // The JS bridge returns an array of objects: [{ col1: val1, col2: val2, ... }, ...]
        // We deserialize into Vec<Vec<(String, WasmValue)>> via an intermediate representation.
        let rows: Vec<serde_json::Map<String, serde_json::Value>> =
            serde_wasm_bindgen::from_value(result)
                .map_err(|e| WasmDbError::Query(format!("Failed to deserialize rows: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|obj| WasmRow {
                columns: obj
                    .into_iter()
                    .map(|(k, v)| (k, json_to_wasm_value(v)))
                    .collect(),
            })
            .collect())
    }

    /// Execute a write statement (INSERT/UPDATE/DELETE) and return the result.
    pub async fn execute_statement(
        &self,
        sql: &str,
        params: &[WasmParam],
    ) -> Result<WasmQueryResult, WasmDbError> {
        let js_params =
            serde_wasm_bindgen::to_value(params).map_err(|e| WasmDbError::Query(e.to_string()))?;

        let result = execute(sql, js_params)
            .await
            .map_err(|e| classify_js_error(&e))?;

        #[derive(serde::Deserialize)]
        struct ExecResult {
            changes: u64,
            #[serde(default)]
            last_insert_rowid: i64,
        }

        let exec: ExecResult = serde_wasm_bindgen::from_value(result)
            .map_err(|e| WasmDbError::Query(format!("Failed to deserialize exec result: {e}")))?;

        Ok(WasmQueryResult {
            rows_affected: exec.changes,
            last_insert_rowid: exec.last_insert_rowid,
        })
    }

    /// Run the embedded SQLite migration SQL.
    pub async fn run_migrations(&self) -> Result<(), WasmDbError> {
        let migration_sql =
            include_str!("../../../migrations_sqlx/sqlite/20250101000000_initial.sql");

        // Create migrations tracking table
        self.execute_statement(
            "CREATE TABLE IF NOT EXISTS _wasm_migrations (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            &[],
        )
        .await?;

        // Check if migration already applied
        let rows = self
            .execute_query(
                "SELECT id FROM _wasm_migrations WHERE name = ?",
                &[WasmParam::Text("20250101000000_initial".to_string())],
            )
            .await?;

        if !rows.is_empty() {
            tracing::debug!("WASM SQLite migrations already applied");
            return Ok(());
        }

        // Use execute_script to run the entire migration as one batch.
        // sql.js's db.exec() handles multiple statements natively, avoiding
        // issues with semicolons inside SQL comments.
        execute_script(migration_sql)
            .await
            .map_err(|e| WasmDbError::Query(js_error_to_string(&e)))?;

        // Record migration
        self.execute_statement(
            "INSERT INTO _wasm_migrations (name) VALUES (?)",
            &[WasmParam::Text("20250101000000_initial".to_string())],
        )
        .await?;

        tracing::info!("WASM SQLite migrations applied successfully");
        Ok(())
    }
}

impl Default for WasmSqlitePool {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a serde_json::Value to a WasmValue.
fn json_to_wasm_value(v: serde_json::Value) -> WasmValue {
    match v {
        serde_json::Value::Null => WasmValue::Null,
        serde_json::Value::Bool(b) => WasmValue::Integer(b as i64),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                WasmValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                WasmValue::Real(f)
            } else {
                WasmValue::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => WasmValue::Text(s),
        other => WasmValue::Text(other.to_string()),
    }
}

/// Extract a human-readable error message from a JsValue.
fn js_error_to_string(e: &JsValue) -> String {
    if let Some(s) = e.as_string() {
        return s;
    }
    if let Some(err) = e.dyn_ref::<js_sys::Error>() {
        return err.message().into();
    }
    format!("{e:?}")
}

/// Classify a JS error into the appropriate WasmDbError variant.
fn classify_js_error(e: &JsValue) -> WasmDbError {
    let msg = js_error_to_string(e);

    // SQLite constraint error messages
    if msg.contains("UNIQUE constraint failed") {
        WasmDbError::UniqueViolation(msg)
    } else if msg.contains("FOREIGN KEY constraint failed") {
        WasmDbError::ForeignKeyViolation(msg)
    } else {
        WasmDbError::Query(msg)
    }
}
