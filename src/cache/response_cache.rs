//! Response caching for chat completions.
//!
//! Provides caching for non-streaming chat completion responses to reduce
//! latency and costs for repeated identical requests.
//!
//! # Caching Strategy
//!
//! - **Simple Exact Match**: Cache key is derived from a hash of the request
//!   components (model, messages, temperature, tools, etc.)
//! - **Deterministic Only**: By default, only responses with temperature=0 are cached
//!   to ensure reproducibility
//! - **Non-streaming Only**: Streaming responses are not cached (would require
//!   buffering the entire stream)
//! - **Size Limited**: Responses larger than `max_size_bytes` are not cached
//!
//! # Configuration
//!
//! ```toml
//! [features.response_caching]
//! enabled = true
//! ttl_secs = 3600              # Cache TTL (default: 1 hour)
//! only_deterministic = true    # Only cache temperature=0 responses
//! max_size_bytes = 1048576     # Max response size to cache (default: 1MB)
//!
//! [features.response_caching.key_components]
//! model = true                 # Include model in cache key
//! temperature = true           # Include temperature in cache key
//! system_prompt = true         # Include system prompt in cache key
//! tools = true                 # Include tools in cache key
//! ```

use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};

use super::{
    keys::CacheKeys,
    traits::{Cache, CacheExt},
};
use crate::{
    api_types::{
        CreateChatCompletionPayload, CreateCompletionPayload, CreateEmbeddingPayload,
        CreateResponsesPayload,
    },
    config::ResponseCachingConfig,
    observability::metrics,
};

/// Cached response data for chat completions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    /// The serialized response body (JSON bytes)
    pub body: Vec<u8>,
    /// Content-Type header
    pub content_type: String,
    /// Provider that generated the response
    pub provider: String,
    /// Model that generated the response
    pub model: String,
    /// Timestamp when the response was cached
    pub cached_at: i64,
}

/// Result of a cache lookup.
#[derive(Debug)]
pub enum CacheLookupResult {
    /// Cache hit - return the cached response
    Hit(CachedResponse),
    /// Cache miss - request should proceed to provider
    Miss,
    /// Caching is disabled or request is not cacheable
    Bypass,
}

/// Response cache service.
pub struct ResponseCache {
    cache: Arc<dyn Cache>,
    config: ResponseCachingConfig,
}

impl ResponseCache {
    /// Create a new response cache service.
    pub fn new(cache: Arc<dyn Cache>, config: ResponseCachingConfig) -> Self {
        Self { cache, config }
    }

    /// Check if a request should use the cache and look up any cached response.
    ///
    /// Returns `CacheLookupResult::Hit` if a cached response exists,
    /// `CacheLookupResult::Miss` if the request is cacheable but not cached,
    /// or `CacheLookupResult::Bypass` if the request should not use caching.
    ///
    /// The `force_refresh` parameter can be used to bypass the cache (e.g., from
    /// request headers like `Cache-Control: no-cache` or `X-Cache-Force-Refresh`).
    pub async fn lookup(
        &self,
        payload: &CreateChatCompletionPayload,
        model: &str,
        force_refresh: bool,
    ) -> CacheLookupResult {
        // Force refresh bypasses cache lookup but still allows caching the response
        if force_refresh {
            tracing::debug!("Cache force refresh requested");
            return CacheLookupResult::Miss;
        }
        // Check if caching is enabled
        if !self.config.enabled {
            return CacheLookupResult::Bypass;
        }

        // Don't cache streaming requests
        if payload.stream {
            return CacheLookupResult::Bypass;
        }

        // Check determinism requirement
        if self.config.only_deterministic {
            let temperature = payload.temperature.unwrap_or(1.0);
            if temperature != 0.0 {
                return CacheLookupResult::Bypass;
            }
        }

        // Generate cache key
        let cache_key = CacheKeys::response_cache(payload, model, &self.config.key_components);

        // Look up in cache
        match self.cache.get_json::<CachedResponse>(&cache_key).await {
            Ok(Some(cached)) => {
                metrics::record_cache_operation("response", "get", "hit");
                tracing::debug!(
                    cache_key = %cache_key,
                    provider = %cached.provider,
                    model = %cached.model,
                    "Response cache hit"
                );
                CacheLookupResult::Hit(cached)
            }
            Ok(None) => {
                metrics::record_cache_operation("response", "get", "miss");
                tracing::debug!(cache_key = %cache_key, "Response cache miss");
                CacheLookupResult::Miss
            }
            Err(e) => {
                metrics::record_cache_operation("response", "get", "error");
                tracing::warn!(
                    cache_key = %cache_key,
                    error = %e,
                    "Response cache lookup error, treating as miss"
                );
                CacheLookupResult::Miss
            }
        }
    }

    /// Store a response in the cache.
    ///
    /// The response body bytes and metadata will be cached for the configured TTL.
    /// Returns `true` if the response was cached, `false` if it was too large or caching failed.
    pub async fn store(
        &self,
        payload: &CreateChatCompletionPayload,
        model: &str,
        provider: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> bool {
        // Check if caching is enabled
        if !self.config.enabled {
            return false;
        }

        // Don't cache streaming requests
        if payload.stream {
            return false;
        }

        // Check determinism requirement
        if self.config.only_deterministic {
            let temperature = payload.temperature.unwrap_or(1.0);
            if temperature != 0.0 {
                return false;
            }
        }

        // Check response size
        if body.len() > self.config.max_size_bytes {
            tracing::debug!(
                size = body.len(),
                max_size = self.config.max_size_bytes,
                "Response too large to cache"
            );
            return false;
        }

        // Generate cache key
        let cache_key = CacheKeys::response_cache(payload, model, &self.config.key_components);

        // Create cached response
        let cached = CachedResponse {
            body,
            content_type: content_type.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            cached_at: chrono::Utc::now().timestamp(),
        };

        // Store in cache
        let ttl = Duration::from_secs(self.config.ttl_secs);
        match self.cache.set_json(&cache_key, &cached, ttl).await {
            Ok(()) => {
                metrics::record_cache_operation("response", "set", "success");
                tracing::debug!(
                    cache_key = %cache_key,
                    provider = %provider,
                    model = %model,
                    size = cached.body.len(),
                    ttl_secs = self.config.ttl_secs,
                    "Response cached"
                );
                true
            }
            Err(e) => {
                metrics::record_cache_operation("response", "set", "error");
                tracing::warn!(
                    cache_key = %cache_key,
                    error = %e,
                    "Failed to cache response"
                );
                false
            }
        }
    }

    /// Check if a responses API request should use the cache and look up any cached response.
    ///
    /// Similar to `lookup` but for the Responses API payload structure.
    pub async fn lookup_responses(
        &self,
        payload: &CreateResponsesPayload,
        model: &str,
        force_refresh: bool,
    ) -> CacheLookupResult {
        // Force refresh bypasses cache lookup but still allows caching the response
        if force_refresh {
            tracing::debug!("Cache force refresh requested");
            return CacheLookupResult::Miss;
        }
        // Check if caching is enabled
        if !self.config.enabled {
            return CacheLookupResult::Bypass;
        }

        // Don't cache streaming requests
        if payload.stream {
            return CacheLookupResult::Bypass;
        }

        // Check determinism requirement
        if self.config.only_deterministic {
            let temperature = payload.temperature.unwrap_or(1.0);
            if temperature != 0.0 {
                return CacheLookupResult::Bypass;
            }
        }

        // Generate cache key
        let cache_key = CacheKeys::responses_cache(payload, model, &self.config.key_components);

        // Look up in cache
        match self.cache.get_json::<CachedResponse>(&cache_key).await {
            Ok(Some(cached)) => {
                metrics::record_cache_operation("response", "get", "hit");
                tracing::debug!(
                    cache_key = %cache_key,
                    provider = %cached.provider,
                    model = %cached.model,
                    "Responses cache hit"
                );
                CacheLookupResult::Hit(cached)
            }
            Ok(None) => {
                metrics::record_cache_operation("response", "get", "miss");
                tracing::debug!(cache_key = %cache_key, "Responses cache miss");
                CacheLookupResult::Miss
            }
            Err(e) => {
                metrics::record_cache_operation("response", "get", "error");
                tracing::warn!(
                    cache_key = %cache_key,
                    error = %e,
                    "Responses cache lookup error, treating as miss"
                );
                CacheLookupResult::Miss
            }
        }
    }

    /// Store a responses API response in the cache.
    pub async fn store_responses(
        &self,
        payload: &CreateResponsesPayload,
        model: &str,
        provider: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> bool {
        // Check if caching is enabled
        if !self.config.enabled {
            return false;
        }

        // Don't cache streaming requests
        if payload.stream {
            return false;
        }

        // Check determinism requirement
        if self.config.only_deterministic {
            let temperature = payload.temperature.unwrap_or(1.0);
            if temperature != 0.0 {
                return false;
            }
        }

        // Check response size
        if body.len() > self.config.max_size_bytes {
            tracing::debug!(
                size = body.len(),
                max_size = self.config.max_size_bytes,
                "Responses response too large to cache"
            );
            return false;
        }

        // Generate cache key
        let cache_key = CacheKeys::responses_cache(payload, model, &self.config.key_components);

        // Create cached response
        let cached = CachedResponse {
            body,
            content_type: content_type.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            cached_at: chrono::Utc::now().timestamp(),
        };

        // Store in cache
        let ttl = Duration::from_secs(self.config.ttl_secs);
        match self.cache.set_json(&cache_key, &cached, ttl).await {
            Ok(()) => {
                metrics::record_cache_operation("response", "set", "success");
                tracing::debug!(
                    cache_key = %cache_key,
                    provider = %provider,
                    model = %model,
                    size = cached.body.len(),
                    ttl_secs = self.config.ttl_secs,
                    "Responses response cached"
                );
                true
            }
            Err(e) => {
                metrics::record_cache_operation("response", "set", "error");
                tracing::warn!(
                    cache_key = %cache_key,
                    error = %e,
                    "Failed to cache responses response"
                );
                false
            }
        }
    }

    /// Check if a responses API request is cacheable (without doing a cache lookup).
    pub fn is_responses_cacheable(&self, payload: &CreateResponsesPayload) -> bool {
        if !self.config.enabled {
            return false;
        }

        if payload.stream {
            return false;
        }

        if self.config.only_deterministic {
            let temperature = payload.temperature.unwrap_or(1.0);
            if temperature != 0.0 {
                return false;
            }
        }

        true
    }

    /// Check if a request is cacheable (without doing a cache lookup).
    pub fn is_cacheable(&self, payload: &CreateChatCompletionPayload) -> bool {
        if !self.config.enabled {
            return false;
        }

        if payload.stream {
            return false;
        }

        if self.config.only_deterministic {
            let temperature = payload.temperature.unwrap_or(1.0);
            if temperature != 0.0 {
                return false;
            }
        }

        true
    }

    /// Check if a completions API request should use the cache and look up any cached response.
    ///
    /// Similar to `lookup` but for the Completions API payload structure.
    pub async fn lookup_completions(
        &self,
        payload: &CreateCompletionPayload,
        model: &str,
        force_refresh: bool,
    ) -> CacheLookupResult {
        // Force refresh bypasses cache lookup but still allows caching the response
        if force_refresh {
            tracing::debug!("Cache force refresh requested");
            return CacheLookupResult::Miss;
        }
        // Check if caching is enabled
        if !self.config.enabled {
            return CacheLookupResult::Bypass;
        }

        // Don't cache streaming requests
        if payload.stream {
            return CacheLookupResult::Bypass;
        }

        // Check determinism requirement
        if self.config.only_deterministic {
            let temperature = payload.temperature.unwrap_or(1.0);
            if temperature != 0.0 {
                return CacheLookupResult::Bypass;
            }
        }

        // Generate cache key
        let cache_key = CacheKeys::completions_cache(payload, model, &self.config.key_components);

        // Look up in cache
        match self.cache.get_json::<CachedResponse>(&cache_key).await {
            Ok(Some(cached)) => {
                metrics::record_cache_operation("response", "get", "hit");
                tracing::debug!(
                    cache_key = %cache_key,
                    provider = %cached.provider,
                    model = %cached.model,
                    "Completions cache hit"
                );
                CacheLookupResult::Hit(cached)
            }
            Ok(None) => {
                metrics::record_cache_operation("response", "get", "miss");
                tracing::debug!(cache_key = %cache_key, "Completions cache miss");
                CacheLookupResult::Miss
            }
            Err(e) => {
                metrics::record_cache_operation("response", "get", "error");
                tracing::warn!(
                    cache_key = %cache_key,
                    error = %e,
                    "Completions cache lookup error, treating as miss"
                );
                CacheLookupResult::Miss
            }
        }
    }

    /// Store a completions API response in the cache.
    pub async fn store_completions(
        &self,
        payload: &CreateCompletionPayload,
        model: &str,
        provider: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> bool {
        // Check if caching is enabled
        if !self.config.enabled {
            return false;
        }

        // Don't cache streaming requests
        if payload.stream {
            return false;
        }

        // Check determinism requirement
        if self.config.only_deterministic {
            let temperature = payload.temperature.unwrap_or(1.0);
            if temperature != 0.0 {
                return false;
            }
        }

        // Check response size
        if body.len() > self.config.max_size_bytes {
            tracing::debug!(
                size = body.len(),
                max_size = self.config.max_size_bytes,
                "Completions response too large to cache"
            );
            return false;
        }

        // Generate cache key
        let cache_key = CacheKeys::completions_cache(payload, model, &self.config.key_components);

        // Create cached response
        let cached = CachedResponse {
            body,
            content_type: content_type.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            cached_at: chrono::Utc::now().timestamp(),
        };

        // Store in cache
        let ttl = Duration::from_secs(self.config.ttl_secs);
        match self.cache.set_json(&cache_key, &cached, ttl).await {
            Ok(()) => {
                metrics::record_cache_operation("response", "set", "success");
                tracing::debug!(
                    cache_key = %cache_key,
                    provider = %provider,
                    model = %model,
                    size = cached.body.len(),
                    ttl_secs = self.config.ttl_secs,
                    "Completions response cached"
                );
                true
            }
            Err(e) => {
                metrics::record_cache_operation("response", "set", "error");
                tracing::warn!(
                    cache_key = %cache_key,
                    error = %e,
                    "Failed to cache completions response"
                );
                false
            }
        }
    }

    /// Check if a completions API request is cacheable (without doing a cache lookup).
    pub fn is_completions_cacheable(&self, payload: &CreateCompletionPayload) -> bool {
        if !self.config.enabled {
            return false;
        }

        if payload.stream {
            return false;
        }

        if self.config.only_deterministic {
            let temperature = payload.temperature.unwrap_or(1.0);
            if temperature != 0.0 {
                return false;
            }
        }

        true
    }

    /// Check if an embeddings API request should use the cache and look up any cached response.
    ///
    /// Note: Embeddings are fully deterministic (no temperature/seed/streaming),
    /// making them excellent candidates for caching.
    pub async fn lookup_embeddings(
        &self,
        payload: &CreateEmbeddingPayload,
        model: &str,
        force_refresh: bool,
    ) -> CacheLookupResult {
        // Force refresh bypasses cache lookup but still allows caching the response
        if force_refresh {
            tracing::debug!("Cache force refresh requested");
            return CacheLookupResult::Miss;
        }
        // Check if caching is enabled
        if !self.config.enabled {
            return CacheLookupResult::Bypass;
        }

        // Embeddings don't have streaming or temperature, so no bypass checks needed

        // Generate cache key
        let cache_key = CacheKeys::embeddings_cache(payload, model);

        // Look up in cache
        match self.cache.get_json::<CachedResponse>(&cache_key).await {
            Ok(Some(cached)) => {
                metrics::record_cache_operation("response", "get", "hit");
                tracing::debug!(
                    cache_key = %cache_key,
                    provider = %cached.provider,
                    model = %cached.model,
                    "Embeddings cache hit"
                );
                CacheLookupResult::Hit(cached)
            }
            Ok(None) => {
                metrics::record_cache_operation("response", "get", "miss");
                tracing::debug!(cache_key = %cache_key, "Embeddings cache miss");
                CacheLookupResult::Miss
            }
            Err(e) => {
                metrics::record_cache_operation("response", "get", "error");
                tracing::warn!(
                    cache_key = %cache_key,
                    error = %e,
                    "Embeddings cache lookup error, treating as miss"
                );
                CacheLookupResult::Miss
            }
        }
    }

    /// Store an embeddings API response in the cache.
    pub async fn store_embeddings(
        &self,
        payload: &CreateEmbeddingPayload,
        model: &str,
        provider: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> bool {
        // Check if caching is enabled
        if !self.config.enabled {
            return false;
        }

        // Embeddings don't have streaming or temperature, so no bypass checks needed

        // Check response size
        if body.len() > self.config.max_size_bytes {
            tracing::debug!(
                size = body.len(),
                max_size = self.config.max_size_bytes,
                "Embeddings response too large to cache"
            );
            return false;
        }

        // Generate cache key
        let cache_key = CacheKeys::embeddings_cache(payload, model);

        // Create cached response
        let cached = CachedResponse {
            body,
            content_type: content_type.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            cached_at: chrono::Utc::now().timestamp(),
        };

        // Store in cache
        let ttl = Duration::from_secs(self.config.ttl_secs);
        match self.cache.set_json(&cache_key, &cached, ttl).await {
            Ok(()) => {
                metrics::record_cache_operation("response", "set", "success");
                tracing::debug!(
                    cache_key = %cache_key,
                    provider = %provider,
                    model = %model,
                    size = cached.body.len(),
                    ttl_secs = self.config.ttl_secs,
                    "Embeddings response cached"
                );
                true
            }
            Err(e) => {
                metrics::record_cache_operation("response", "set", "error");
                tracing::warn!(
                    cache_key = %cache_key,
                    error = %e,
                    "Failed to cache embeddings response"
                );
                false
            }
        }
    }

    /// Check if an embeddings API request is cacheable (without doing a cache lookup).
    ///
    /// Embeddings are always cacheable when caching is enabled, as they are
    /// fully deterministic (no temperature, seed, or streaming).
    pub fn is_embeddings_cacheable(&self) -> bool {
        self.config.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        api_types::{Message, MessageContent},
        cache::MemoryCache,
        config::{CacheKeyComponents, MemoryCacheConfig},
    };

    fn create_test_cache() -> Arc<dyn Cache> {
        Arc::new(MemoryCache::new(&MemoryCacheConfig::default()))
    }

    fn create_test_config() -> ResponseCachingConfig {
        ResponseCachingConfig {
            enabled: true,
            ttl_secs: 3600,
            only_deterministic: true,
            max_size_bytes: 1024 * 1024,
            key_components: CacheKeyComponents::default(),
            semantic: None,
        }
    }

    fn create_test_payload(stream: bool, temperature: Option<f64>) -> CreateChatCompletionPayload {
        CreateChatCompletionPayload {
            messages: vec![Message::User {
                content: MessageContent::Text("Hello".to_string()),
                name: None,
            }],
            model: Some("gpt-4".to_string()),
            models: None,
            temperature,
            seed: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            frequency_penalty: None,
            logit_bias: None,
            logprobs: None,
            top_logprobs: None,
            max_completion_tokens: None,
            max_tokens: None,
            metadata: None,
            presence_penalty: None,
            reasoning: None,
            stop: None,
            stream,
            stream_options: None,
            top_p: None,
            user: None,
        }
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let cache = create_test_cache();
        let mut config = create_test_config();
        config.enabled = false;

        let response_cache = ResponseCache::new(cache, config);
        let payload = create_test_payload(false, Some(0.0));

        let result = response_cache.lookup(&payload, "gpt-4", false).await;
        assert!(matches!(result, CacheLookupResult::Bypass));
    }

    #[tokio::test]
    async fn test_streaming_bypasses_cache() {
        let cache = create_test_cache();
        let config = create_test_config();

        let response_cache = ResponseCache::new(cache, config);
        let payload = create_test_payload(true, Some(0.0));

        let result = response_cache.lookup(&payload, "gpt-4", false).await;
        assert!(matches!(result, CacheLookupResult::Bypass));
    }

    #[tokio::test]
    async fn test_non_deterministic_bypasses_cache() {
        let cache = create_test_cache();
        let config = create_test_config();

        let response_cache = ResponseCache::new(cache, config);
        let payload = create_test_payload(false, Some(0.7));

        let result = response_cache.lookup(&payload, "gpt-4", false).await;
        assert!(matches!(result, CacheLookupResult::Bypass));
    }

    #[tokio::test]
    async fn test_cache_miss_then_hit() {
        let cache = create_test_cache();
        let config = create_test_config();

        let response_cache = ResponseCache::new(cache, config);
        let payload = create_test_payload(false, Some(0.0));

        // First lookup should be a miss
        let result = response_cache.lookup(&payload, "gpt-4", false).await;
        assert!(matches!(result, CacheLookupResult::Miss));

        // Store a response
        let body = br#"{"id":"test","object":"chat.completion"}"#.to_vec();
        let stored = response_cache
            .store(
                &payload,
                "gpt-4",
                "openai",
                body.clone(),
                "application/json",
            )
            .await;
        assert!(stored);

        // Second lookup should be a hit
        let result = response_cache.lookup(&payload, "gpt-4", false).await;
        match result {
            CacheLookupResult::Hit(cached) => {
                assert_eq!(cached.body, body);
                assert_eq!(cached.provider, "openai");
                assert_eq!(cached.model, "gpt-4");
                assert_eq!(cached.content_type, "application/json");
            }
            _ => panic!("Expected cache hit"),
        }
    }

    #[tokio::test]
    async fn test_force_refresh_bypasses_cache() {
        let cache = create_test_cache();
        let config = create_test_config();

        let response_cache = ResponseCache::new(cache, config);
        let payload = create_test_payload(false, Some(0.0));

        // Store a response
        let body = br#"{"id":"test","object":"chat.completion"}"#.to_vec();
        response_cache
            .store(&payload, "gpt-4", "openai", body, "application/json")
            .await;

        // With force_refresh=true, should return Miss even though cached
        let result = response_cache.lookup(&payload, "gpt-4", true).await;
        assert!(matches!(result, CacheLookupResult::Miss));

        // With force_refresh=false, should return Hit
        let result = response_cache.lookup(&payload, "gpt-4", false).await;
        assert!(matches!(result, CacheLookupResult::Hit(_)));
    }

    #[tokio::test]
    async fn test_response_too_large() {
        let cache = create_test_cache();
        let mut config = create_test_config();
        config.max_size_bytes = 10; // Very small limit

        let response_cache = ResponseCache::new(cache, config);
        let payload = create_test_payload(false, Some(0.0));

        // Try to store a response larger than the limit
        let body = br#"{"id":"test","object":"chat.completion"}"#.to_vec();
        let stored = response_cache
            .store(&payload, "gpt-4", "openai", body, "application/json")
            .await;
        assert!(!stored);
    }

    #[tokio::test]
    async fn test_is_cacheable() {
        let cache = create_test_cache();
        let config = create_test_config();

        let response_cache = ResponseCache::new(cache, config);

        // Deterministic, non-streaming request is cacheable
        let payload = create_test_payload(false, Some(0.0));
        assert!(response_cache.is_cacheable(&payload));

        // Streaming request is not cacheable
        let payload = create_test_payload(true, Some(0.0));
        assert!(!response_cache.is_cacheable(&payload));

        // Non-deterministic request is not cacheable (when only_deterministic is true)
        let payload = create_test_payload(false, Some(0.7));
        assert!(!response_cache.is_cacheable(&payload));
    }
}
