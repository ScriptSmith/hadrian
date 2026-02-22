use thiserror::Error;

#[derive(Debug, Error)]
pub enum DlqError {
    #[error("DLQ not configured")]
    NotConfigured,

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "redis")]
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[cfg(any(feature = "database-sqlite", feature = "database-postgres"))]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type DlqResult<T> = Result<T, DlqError>;
