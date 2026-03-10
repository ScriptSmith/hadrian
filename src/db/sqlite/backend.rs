//! Backend abstraction layer that allows the same repo code to work with both
//! native SQLite (via sqlx) and WASM SQLite (via wa-sqlite JS bridge).
//!
//! Provides cfg-switched type aliases, a unified row access trait, query
//! constructor, and error helpers.

use crate::db::error::DbError;

// ─────────────────────────────────────────────────────────────────────────────
// Pool type alias
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database-sqlite")]
pub(crate) type Pool = sqlx::SqlitePool;

#[cfg(feature = "database-wasm-sqlite")]
pub(crate) type Pool = crate::db::wasm_sqlite::WasmSqlitePool;

// ─────────────────────────────────────────────────────────────────────────────
// Row type alias
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database-sqlite")]
pub(crate) type Row = sqlx::sqlite::SqliteRow;

#[cfg(feature = "database-wasm-sqlite")]
pub(crate) type Row = crate::db::wasm_sqlite::WasmRow;

// ─────────────────────────────────────────────────────────────────────────────
// Error type alias
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database-sqlite")]
pub(crate) type BackendError = sqlx::Error;

#[cfg(feature = "database-wasm-sqlite")]
pub(crate) type BackendError = crate::db::wasm_sqlite::WasmDbError;

// ─────────────────────────────────────────────────────────────────────────────
// ColDecode — bridging sqlx and WASM decode traits
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database-sqlite")]
pub(crate) trait ColDecode:
    for<'r> sqlx::Decode<'r, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>
{
}

#[cfg(feature = "database-sqlite")]
impl<T: for<'r> sqlx::Decode<'r, sqlx::Sqlite> + sqlx::Type<sqlx::Sqlite>> ColDecode for T {}

#[cfg(feature = "database-wasm-sqlite")]
pub(crate) trait ColDecode: crate::db::wasm_sqlite::WasmDecode {}

#[cfg(feature = "database-wasm-sqlite")]
impl<T: crate::db::wasm_sqlite::WasmDecode> ColDecode for T {}

// ─────────────────────────────────────────────────────────────────────────────
// RowExt — unified row access with `.col::<T>("name")`
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) trait RowExt {
    fn col<T: ColDecode>(&self, name: &str) -> T;
}

#[cfg(feature = "database-sqlite")]
impl RowExt for sqlx::sqlite::SqliteRow {
    fn col<T: ColDecode>(&self, name: &str) -> T {
        use sqlx::Row;
        self.get(name)
    }
}

#[cfg(feature = "database-wasm-sqlite")]
impl RowExt for crate::db::wasm_sqlite::WasmRow {
    fn col<T: ColDecode>(&self, name: &str) -> T {
        self.get(name)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Query constructor
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database-sqlite")]
pub(crate) fn query(
    sql: &str,
) -> sqlx::query::Query<'_, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'_>> {
    sqlx::query(sql)
}

#[cfg(feature = "database-wasm-sqlite")]
pub(crate) fn query(sql: &str) -> crate::db::wasm_sqlite::WasmQuery {
    crate::db::wasm_sqlite::query(sql)
}

// ─────────────────────────────────────────────────────────────────────────────
// Scalar query constructor
// ─────────────────────────────────────────────────────────────────────────────

/// Create a query that returns a single column, decoded as `T`.
/// Mirrors `sqlx::query_scalar()`.
#[cfg(feature = "database-sqlite")]
pub(crate) fn query_scalar<T>(
    sql: &str,
) -> sqlx::query::QueryScalar<'_, sqlx::Sqlite, T, sqlx::sqlite::SqliteArguments<'_>>
where
    T: sqlx::Type<sqlx::Sqlite> + for<'r> sqlx::Decode<'r, sqlx::Sqlite>,
    (T,): for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow>,
{
    sqlx::query_scalar(sql)
}

#[cfg(feature = "database-wasm-sqlite")]
pub(crate) fn query_scalar<T: crate::db::wasm_sqlite::WasmDecode>(
    sql: &str,
) -> crate::db::wasm_sqlite::WasmQueryScalar<T> {
    crate::db::wasm_sqlite::WasmQueryScalar::new(sql)
}

// ─────────────────────────────────────────────────────────────────────────────
// Transaction
// ─────────────────────────────────────────────────────────────────────────────

/// A database transaction that works for both native and WASM SQLite.
///
/// Native: wraps `sqlx::Transaction<'a, Sqlite>` with auto-rollback on drop.
/// WASM: sends BEGIN/COMMIT/ROLLBACK through the JS bridge.
///
/// Use `begin(&pool)` to start, then `query(sql).execute(&mut *tx)` for
/// queries, and `tx.commit()` to finalize.
#[cfg(feature = "database-sqlite")]
pub(crate) struct Transaction<'a>(sqlx::Transaction<'a, sqlx::Sqlite>);

#[cfg(feature = "database-wasm-sqlite")]
pub(crate) struct Transaction<'a>(Pool, std::marker::PhantomData<&'a ()>);

/// Begin a new transaction.
#[cfg(feature = "database-sqlite")]
pub(crate) async fn begin(pool: &Pool) -> Result<Transaction<'_>, BackendError> {
    Ok(Transaction(pool.begin().await?))
}

#[cfg(feature = "database-wasm-sqlite")]
pub(crate) async fn begin(pool: &Pool) -> Result<Transaction<'_>, BackendError> {
    query("BEGIN").execute(pool).await?;
    Ok(Transaction(pool.clone(), std::marker::PhantomData))
}

#[cfg(feature = "database-sqlite")]
impl Transaction<'_> {
    pub async fn commit(self) -> Result<(), BackendError> {
        self.0.commit().await
    }
}

#[cfg(feature = "database-wasm-sqlite")]
impl Transaction<'_> {
    pub async fn commit(self) -> Result<(), BackendError> {
        query("COMMIT").execute(&self.0).await?;
        Ok(())
    }
}

#[cfg(feature = "database-sqlite")]
impl std::ops::Deref for Transaction<'_> {
    type Target = sqlx::SqliteConnection;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "database-sqlite")]
impl std::ops::DerefMut for Transaction<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(feature = "database-wasm-sqlite")]
impl std::ops::Deref for Transaction<'_> {
    type Target = crate::db::wasm_sqlite::WasmSqlitePool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "database-wasm-sqlite")]
impl std::ops::DerefMut for Transaction<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Error helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Check if a backend error is a unique constraint violation.
#[cfg(feature = "database-sqlite")]
pub(crate) fn is_unique_violation(e: &BackendError) -> bool {
    matches!(e, sqlx::Error::Database(db_err) if db_err.is_unique_violation())
}

#[cfg(feature = "database-wasm-sqlite")]
pub(crate) fn is_unique_violation(e: &BackendError) -> bool {
    e.is_unique_violation()
}

/// Map a backend error to `DbError`, converting unique violations to `DbError::Conflict`.
pub(crate) fn map_unique_violation(msg: impl Into<String>) -> impl FnOnce(BackendError) -> DbError {
    let msg = msg.into();
    move |e: BackendError| {
        if is_unique_violation(&e) {
            DbError::Conflict(msg)
        } else {
            DbError::from(e)
        }
    }
}

/// Extract the error message from a unique violation, or `None` if not a unique violation.
#[cfg(feature = "database-sqlite")]
pub(crate) fn unique_violation_message(e: &BackendError) -> Option<String> {
    match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            Some(db_err.message().to_string())
        }
        _ => None,
    }
}

#[cfg(feature = "database-wasm-sqlite")]
pub(crate) fn unique_violation_message(e: &BackendError) -> Option<String> {
    match e {
        crate::db::wasm_sqlite::WasmDbError::UniqueViolation(msg) => Some(msg.clone()),
        _ => None,
    }
}
