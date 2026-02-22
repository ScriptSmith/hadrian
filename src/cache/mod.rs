mod embedding_service;
mod error;
mod keys;
mod memory;
#[cfg(feature = "redis")]
mod redis;
mod response_cache;
mod semantic_cache;
mod traits;
pub mod vector_store;

// Public API exports
#[cfg(any(
    feature = "document-extraction-basic",
    feature = "document-extraction-full"
))]
pub use embedding_service::EmbeddingError;
pub use embedding_service::EmbeddingService;
pub use keys::CacheKeys;
pub use memory::MemoryCache;
#[cfg(feature = "redis")]
pub use redis::RedisCache;
pub use response_cache::{CacheLookupResult, ResponseCache};
pub use semantic_cache::{SemanticCache, SemanticLookupResult, StoreParams};
#[cfg(feature = "sso")]
pub use traits::CacheExt;
pub use traits::{BudgetCheckParams, Cache, RateLimitCheckParams, RateLimitResult};
