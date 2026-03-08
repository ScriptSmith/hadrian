//! Core types for the WASM SQLite database layer.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use thiserror::Error;
use uuid::Uuid;

use super::WasmSqlitePool;

// ─────────────────────────────────────────────────────────────────────────────
// Error
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum WasmDbError {
    #[error("WASM SQLite error: {0}")]
    Query(String),
    #[error("Row not found")]
    RowNotFound,
    #[error("Column not found: {0}")]
    ColumnNotFound(String),
    #[error("Type mismatch for column {column}: expected {expected}, got {actual}")]
    TypeMismatch {
        column: String,
        expected: &'static str,
        actual: String,
    },
    #[error("Unique constraint violation: {0}")]
    UniqueViolation(String),
    #[error("Foreign key constraint violation: {0}")]
    ForeignKeyViolation(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl WasmDbError {
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, Self::UniqueViolation(_))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bind parameters
// ─────────────────────────────────────────────────────────────────────────────

/// A bindable parameter value for SQLite queries.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum WasmParam {
    Null,
    Text(String),
    Integer(i64),
    Real(f64),
    Bool(bool),
}

// Conversions into WasmParam
impl From<String> for WasmParam {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<&str> for WasmParam {
    fn from(v: &str) -> Self {
        Self::Text(v.to_string())
    }
}

impl From<&String> for WasmParam {
    fn from(v: &String) -> Self {
        Self::Text(v.clone())
    }
}

impl From<i64> for WasmParam {
    fn from(v: i64) -> Self {
        Self::Integer(v)
    }
}

impl From<i32> for WasmParam {
    fn from(v: i32) -> Self {
        Self::Integer(v as i64)
    }
}

impl From<u32> for WasmParam {
    fn from(v: u32) -> Self {
        Self::Integer(v as i64)
    }
}

impl From<u16> for WasmParam {
    fn from(v: u16) -> Self {
        Self::Integer(v as i64)
    }
}

impl From<i16> for WasmParam {
    fn from(v: i16) -> Self {
        Self::Integer(v as i64)
    }
}

impl From<&Option<i16>> for WasmParam {
    fn from(v: &Option<i16>) -> Self {
        match v {
            Some(i) => Self::Integer(*i as i64),
            None => Self::Null,
        }
    }
}

impl From<f64> for WasmParam {
    fn from(v: f64) -> Self {
        Self::Real(v)
    }
}

impl From<bool> for WasmParam {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<Uuid> for WasmParam {
    fn from(v: Uuid) -> Self {
        Self::Text(v.to_string())
    }
}

impl From<DateTime<Utc>> for WasmParam {
    fn from(v: DateTime<Utc>) -> Self {
        Self::Text(v.to_rfc3339())
    }
}

impl From<NaiveDateTime> for WasmParam {
    fn from(v: NaiveDateTime) -> Self {
        Self::Text(v.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
    }
}

impl From<NaiveDate> for WasmParam {
    fn from(v: NaiveDate) -> Self {
        Self::Text(v.format("%Y-%m-%d").to_string())
    }
}

impl From<Decimal> for WasmParam {
    fn from(v: Decimal) -> Self {
        Self::Text(v.to_string())
    }
}

impl<T: Into<WasmParam>> From<Option<T>> for WasmParam {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(inner) => inner.into(),
            None => Self::Null,
        }
    }
}

impl From<&Option<String>> for WasmParam {
    fn from(v: &Option<String>) -> Self {
        match v {
            Some(s) => Self::Text(s.clone()),
            None => Self::Null,
        }
    }
}

impl From<&Uuid> for WasmParam {
    fn from(v: &Uuid) -> Self {
        Self::Text(v.to_string())
    }
}

impl From<&bool> for WasmParam {
    fn from(v: &bool) -> Self {
        Self::Bool(*v)
    }
}

impl From<&i64> for WasmParam {
    fn from(v: &i64) -> Self {
        Self::Integer(*v)
    }
}

impl From<&i32> for WasmParam {
    fn from(v: &i32) -> Self {
        Self::Integer(*v as i64)
    }
}

impl From<&f64> for WasmParam {
    fn from(v: &f64) -> Self {
        Self::Real(*v)
    }
}

impl From<&Decimal> for WasmParam {
    fn from(v: &Decimal) -> Self {
        Self::Text(v.to_string())
    }
}

impl From<&Option<i32>> for WasmParam {
    fn from(v: &Option<i32>) -> Self {
        match v {
            Some(i) => Self::Integer(*i as i64),
            None => Self::Null,
        }
    }
}

impl From<&Option<i64>> for WasmParam {
    fn from(v: &Option<i64>) -> Self {
        match v {
            Some(i) => Self::Integer(*i),
            None => Self::Null,
        }
    }
}

impl From<&Option<f64>> for WasmParam {
    fn from(v: &Option<f64>) -> Self {
        match v {
            Some(f) => Self::Real(*f),
            None => Self::Null,
        }
    }
}

impl From<&Option<bool>> for WasmParam {
    fn from(v: &Option<bool>) -> Self {
        match v {
            Some(b) => Self::Bool(*b),
            None => Self::Null,
        }
    }
}

impl From<&Option<Uuid>> for WasmParam {
    fn from(v: &Option<Uuid>) -> Self {
        match v {
            Some(u) => Self::Text(u.to_string()),
            None => Self::Null,
        }
    }
}

impl From<&Option<DateTime<Utc>>> for WasmParam {
    fn from(v: &Option<DateTime<Utc>>) -> Self {
        match v {
            Some(dt) => Self::Text(dt.to_rfc3339()),
            None => Self::Null,
        }
    }
}

impl From<&Option<Decimal>> for WasmParam {
    fn from(v: &Option<Decimal>) -> Self {
        match v {
            Some(d) => Self::Text(d.to_string()),
            None => Self::Null,
        }
    }
}

impl From<&Option<serde_json::Value>> for WasmParam {
    fn from(v: &Option<serde_json::Value>) -> Self {
        match v {
            Some(j) => Self::Text(j.to_string()),
            None => Self::Null,
        }
    }
}

impl From<&serde_json::Value> for WasmParam {
    fn from(v: &serde_json::Value) -> Self {
        Self::Text(v.to_string())
    }
}

impl From<Vec<u8>> for WasmParam {
    fn from(v: Vec<u8>) -> Self {
        use base64::Engine;
        Self::Text(base64::engine::general_purpose::STANDARD.encode(&v))
    }
}

impl From<&Option<Vec<u8>>> for WasmParam {
    fn from(v: &Option<Vec<u8>>) -> Self {
        use base64::Engine;
        match v {
            Some(bytes) => Self::Text(base64::engine::general_purpose::STANDARD.encode(bytes)),
            None => Self::Null,
        }
    }
}

impl From<&DateTime<Utc>> for WasmParam {
    fn from(v: &DateTime<Utc>) -> Self {
        Self::Text(v.to_rfc3339())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Query builder
// ─────────────────────────────────────────────────────────────────────────────

/// A query builder that accumulates bind parameters.
///
/// Mimics `sqlx::Query` — use `.bind()` to add parameters, then
/// `.fetch_all()`, `.fetch_optional()`, or `.execute()` to run.
pub struct WasmQuery {
    sql: String,
    params: Vec<WasmParam>,
}

impl WasmQuery {
    pub fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
            params: Vec::new(),
        }
    }

    /// Bind a parameter value.
    pub fn bind<T: Into<WasmParam>>(mut self, value: T) -> Self {
        self.params.push(value.into());
        self
    }

    /// Execute and return all rows.
    pub async fn fetch_all(self, pool: &WasmSqlitePool) -> Result<Vec<WasmRow>, WasmDbError> {
        pool.execute_query(&self.sql, &self.params).await
    }

    /// Execute and return the first row, or None.
    pub async fn fetch_optional(
        self,
        pool: &WasmSqlitePool,
    ) -> Result<Option<WasmRow>, WasmDbError> {
        let rows = pool.execute_query(&self.sql, &self.params).await?;
        Ok(rows.into_iter().next())
    }

    /// Execute and return the first row, or error if not found.
    pub async fn fetch_one(self, pool: &WasmSqlitePool) -> Result<WasmRow, WasmDbError> {
        self.fetch_optional(pool)
            .await?
            .ok_or(WasmDbError::RowNotFound)
    }

    /// Execute a statement (INSERT/UPDATE/DELETE) and return affected row count.
    pub async fn execute(self, pool: &WasmSqlitePool) -> Result<WasmQueryResult, WasmDbError> {
        pool.execute_statement(&self.sql, &self.params).await
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Query result (for INSERT/UPDATE/DELETE)
// ─────────────────────────────────────────────────────────────────────────────

/// Result of an execute() call.
#[derive(Debug, Default)]
pub struct WasmQueryResult {
    pub rows_affected: u64,
    pub last_insert_rowid: i64,
}

impl WasmQueryResult {
    pub fn rows_affected(&self) -> u64 {
        self.rows_affected
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Row and value types
// ─────────────────────────────────────────────────────────────────────────────

/// A database value from a SQLite column.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub enum WasmValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
}

/// A single row returned from a query.
#[derive(Debug, Clone)]
pub struct WasmRow {
    pub(crate) columns: Vec<(String, WasmValue)>,
}

impl WasmRow {
    /// Get a typed value by column name.
    ///
    /// Mirrors `sqlx::Row::get()`. Panics if column not found or type mismatch
    /// (matching sqlx behavior).
    pub fn get<T: WasmDecode>(&self, col: &str) -> T {
        self.try_get(col)
            .unwrap_or_else(|e| panic!("Row::get({col}): {e}"))
    }

    /// Try to get a typed value by column name.
    pub fn try_get<T: WasmDecode>(&self, col: &str) -> Result<T, WasmDbError> {
        let value = self
            .columns
            .iter()
            .find(|(name, _)| name == col)
            .map(|(_, v)| v)
            .ok_or_else(|| WasmDbError::ColumnNotFound(col.to_string()))?;
        T::decode(value, col)
    }
}

/// Trait for decoding a `WasmValue` into a Rust type.
pub trait WasmDecode: Sized {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError>;
}

impl WasmDecode for String {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Text(s) => Ok(s.clone()),
            WasmValue::Integer(i) => Ok(i.to_string()),
            WasmValue::Real(f) => Ok(f.to_string()),
            WasmValue::Null => Err(WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "String",
                actual: "NULL".to_string(),
            }),
        }
    }
}

impl WasmDecode for Option<String> {
    fn decode(value: &WasmValue, _col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Text(s) => Ok(Some(s.clone())),
            WasmValue::Null => Ok(None),
            WasmValue::Integer(i) => Ok(Some(i.to_string())),
            WasmValue::Real(f) => Ok(Some(f.to_string())),
        }
    }
}

impl WasmDecode for i64 {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Integer(i) => Ok(*i),
            WasmValue::Real(f) => Ok(*f as i64),
            _ => Err(WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "i64",
                actual: format!("{value:?}"),
            }),
        }
    }
}

impl WasmDecode for Option<i64> {
    fn decode(value: &WasmValue, _col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Integer(i) => Ok(Some(*i)),
            WasmValue::Real(f) => Ok(Some(*f as i64)),
            WasmValue::Null => Ok(None),
            _ => Ok(None),
        }
    }
}

impl WasmDecode for i32 {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        i64::decode(value, col).map(|v| v as i32)
    }
}

impl WasmDecode for Option<i32> {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Null => Ok(None),
            _ => i32::decode(value, col).map(Some),
        }
    }
}

impl WasmDecode for bool {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Integer(i) => Ok(*i != 0),
            WasmValue::Null => Ok(false),
            _ => Err(WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "bool",
                actual: format!("{value:?}"),
            }),
        }
    }
}

impl WasmDecode for Option<bool> {
    fn decode(value: &WasmValue, _col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Integer(i) => Ok(Some(*i != 0)),
            WasmValue::Null => Ok(None),
            _ => Ok(None),
        }
    }
}

impl WasmDecode for f64 {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Real(f) => Ok(*f),
            WasmValue::Integer(i) => Ok(*i as f64),
            _ => Err(WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "f64",
                actual: format!("{value:?}"),
            }),
        }
    }
}

impl WasmDecode for NaiveDate {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Text(s) => {
                NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| WasmDbError::TypeMismatch {
                    column: col.to_string(),
                    expected: "NaiveDate",
                    actual: s.clone(),
                })
            }
            _ => Err(WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "NaiveDate",
                actual: format!("{value:?}"),
            }),
        }
    }
}

impl WasmDecode for DateTime<Utc> {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Text(s) => {
                // Try RFC 3339 first, then common SQLite formats
                DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .or_else(|_| {
                        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                            .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
                            .map(|ndt| ndt.and_utc())
                    })
                    .map_err(|_| WasmDbError::TypeMismatch {
                        column: col.to_string(),
                        expected: "DateTime",
                        actual: s.clone(),
                    })
            }
            _ => Err(WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "DateTime",
                actual: format!("{value:?}"),
            }),
        }
    }
}

impl WasmDecode for Option<DateTime<Utc>> {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Null => Ok(None),
            _ => DateTime::<Utc>::decode(value, col).map(Some),
        }
    }
}

impl WasmDecode for Uuid {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Text(s) => Uuid::parse_str(s).map_err(|_| WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "UUID",
                actual: s.clone(),
            }),
            _ => Err(WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "UUID",
                actual: format!("{value:?}"),
            }),
        }
    }
}

impl WasmDecode for Option<Uuid> {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Null => Ok(None),
            _ => Uuid::decode(value, col).map(Some),
        }
    }
}

impl WasmDecode for Decimal {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Text(s) => s.parse::<Decimal>().map_err(|_| WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "Decimal",
                actual: s.clone(),
            }),
            WasmValue::Integer(i) => Ok(Decimal::from(*i)),
            WasmValue::Real(f) => Decimal::try_from(*f).map_err(|_| WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "Decimal",
                actual: f.to_string(),
            }),
            WasmValue::Null => Err(WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "Decimal",
                actual: "NULL".to_string(),
            }),
        }
    }
}

impl WasmDecode for Option<Decimal> {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Null => Ok(None),
            _ => Decimal::decode(value, col).map(Some),
        }
    }
}

impl WasmDecode for Vec<u8> {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        use base64::Engine;
        match value {
            WasmValue::Text(s) => base64::engine::general_purpose::STANDARD
                .decode(s)
                .map_err(|_| WasmDbError::TypeMismatch {
                    column: col.to_string(),
                    expected: "Vec<u8> (base64)",
                    actual: s.clone(),
                }),
            _ => Err(WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "Vec<u8> (base64)",
                actual: format!("{value:?}"),
            }),
        }
    }
}

impl WasmDecode for Option<Vec<u8>> {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Null => Ok(None),
            _ => Vec::<u8>::decode(value, col).map(Some),
        }
    }
}

impl WasmDecode for serde_json::Value {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Text(s) => serde_json::from_str(s).map_err(|_| WasmDbError::TypeMismatch {
                column: col.to_string(),
                expected: "JSON",
                actual: s.clone(),
            }),
            WasmValue::Null => Ok(serde_json::Value::Null),
            WasmValue::Integer(i) => Ok(serde_json::json!(*i)),
            WasmValue::Real(f) => Ok(serde_json::json!(*f)),
        }
    }
}

impl WasmDecode for Option<serde_json::Value> {
    fn decode(value: &WasmValue, col: &str) -> Result<Self, WasmDbError> {
        match value {
            WasmValue::Null => Ok(None),
            _ => serde_json::Value::decode(value, col).map(Some),
        }
    }
}
