use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Cache not configured")]
    NotConfigured,

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[cfg(feature = "redis")]
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type CacheResult<T> = Result<T, CacheError>;
