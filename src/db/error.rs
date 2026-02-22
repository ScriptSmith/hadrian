use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database not configured")]
    NotConfigured,

    #[error("Not found")]
    NotFound,

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[cfg(any(feature = "database-sqlite", feature = "database-postgres"))]
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[cfg(any(feature = "database-sqlite", feature = "database-postgres"))]
    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type DbResult<T> = Result<T, DbError>;
