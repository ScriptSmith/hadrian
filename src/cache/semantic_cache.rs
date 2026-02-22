//! Semantic response caching for chat completions.
//!
//! Extends the simple exact-match response cache with semantic similarity matching
//! using vector embeddings. This allows cache hits for requests that are semantically
//! similar but not identical (e.g., "What is 2+2?" vs "Calculate 2 plus 2").
//!
//! # Caching Strategy
//!
//! 1. **Exact Match First**: Always attempt exact SHA-256 hash match first (fastest)
//! 2. **Semantic Match Second**: On exact miss, search for semantically similar requests
//! 3. **Background Embedding**: Embeddings are generated in background tasks to avoid
//!    blocking response delivery
//!
//! # Configuration
//!
//! ```toml
//! [features.response_caching]
//! enabled = true
//! ttl_secs = 3600
//!
//! [features.response_caching.semantic]
//! enabled = true
//! similarity_threshold = 0.95  # Minimum cosine similarity for cache hit
//! top_k = 1                    # Number of similar results to consider
//!
//! [features.response_caching.semantic.embedding]
//! provider = "openai"
//! model = "text-embedding-3-small"
//! dimensions = 1536
//!
//! [features.response_caching.semantic.vector_backend]
//! type = "pgvector"  # or "qdrant"
//! ```

use std::{sync::Arc, time::Duration};

use tokio::sync::mpsc;

use super::{
    embedding_service::{EmbeddingError, EmbeddingService},
    keys::CacheKeys,
    response_cache::CachedResponse,
    traits::{Cache, CacheExt},
    vector_store::{VectorBackend, VectorMetadata, VectorStoreError},
};
use crate::{
    api_types::CreateChatCompletionPayload, config::SemanticCachingConfig, observability::metrics,
};

/// Result of a semantic cache lookup.
#[derive(Debug)]
pub enum SemanticLookupResult {
    /// Exact cache hit - request hash matched exactly
    ExactHit(CachedResponse),
    /// Semantic cache hit - found a semantically similar cached response
    SemanticHit {
        response: CachedResponse,
        similarity: f64,
    },
    /// Cache miss - no exact or semantic match found
    Miss,
    /// Caching is disabled or request is not cacheable
    Bypass,
}

/// Background task message for embedding generation.
#[derive(Debug)]
struct EmbeddingTask {
    /// The cache key (SHA-256 hash) to associate with this embedding
    cache_key: String,
    /// The model used for the original request
    model: String,
    /// Text representation of the request for embedding
    text: String,
    /// TTL for the embedding entry
    ttl: Duration,
    /// Optional organization ID for multi-tenant isolation
    organization_id: Option<String>,
    /// Optional project ID for finer-grained isolation
    project_id: Option<String>,
}

/// Parameters for storing a response in the semantic cache.
#[derive(Debug)]
pub struct StoreParams<'a> {
    /// The original request payload
    pub payload: &'a CreateChatCompletionPayload,
    /// The model that generated the response
    pub model: &'a str,
    /// The provider that generated the response
    pub provider: &'a str,
    /// The response body bytes
    pub body: Vec<u8>,
    /// The response content type
    pub content_type: &'a str,
    /// Cache key configuration
    pub key_components: &'a crate::config::CacheKeyComponents,
    /// Time-to-live for the cached response
    pub ttl: Duration,
    /// Optional organization ID for multi-tenant isolation
    pub organization_id: Option<String>,
    /// Optional project ID for finer-grained isolation
    pub project_id: Option<String>,
}

/// Semantic cache service combining exact and semantic matching.
pub struct SemanticCache {
    /// Primary cache backend for storing responses
    cache: Arc<dyn Cache>,
    /// Vector store for semantic similarity search
    vector_store: Arc<dyn VectorBackend>,
    /// Embedding service for generating request embeddings
    embedding_service: Arc<EmbeddingService>,
    /// Configuration for semantic caching
    config: SemanticCachingConfig,
    /// Channel for background embedding tasks
    embedding_tx: mpsc::Sender<EmbeddingTask>,
}

impl SemanticCache {
    /// Create a new semantic cache service.
    ///
    /// # Arguments
    /// * `cache` - Primary cache backend for storing responses
    /// * `vector_store` - Vector store for semantic similarity search
    /// * `embedding_service` - Service for generating embeddings
    /// * `config` - Semantic caching configuration
    ///
    /// # Returns
    /// A tuple of (SemanticCache, background task handle)
    pub fn new(
        cache: Arc<dyn Cache>,
        vector_store: Arc<dyn VectorBackend>,
        embedding_service: Arc<EmbeddingService>,
        config: SemanticCachingConfig,
    ) -> (Self, impl std::future::Future<Output = ()> + Send) {
        // Create channel for background embedding tasks
        let (embedding_tx, embedding_rx) = mpsc::channel::<EmbeddingTask>(1000);

        let semantic_cache = Self {
            cache,
            vector_store: vector_store.clone(),
            embedding_service: embedding_service.clone(),
            config,
            embedding_tx,
        };

        // Background task for processing embeddings
        let background_task =
            Self::run_embedding_worker(embedding_rx, vector_store, embedding_service);

        (semantic_cache, background_task)
    }

    /// Run the background worker for processing embedding tasks.
    async fn run_embedding_worker(
        mut rx: mpsc::Receiver<EmbeddingTask>,
        vector_store: Arc<dyn VectorBackend>,
        embedding_service: Arc<EmbeddingService>,
    ) {
        while let Some(task) = rx.recv().await {
            // Generate embedding for the request
            let embedding = match embedding_service.embed_text(&task.text).await {
                Ok(emb) => emb,
                Err(e) => {
                    tracing::warn!(
                        cache_key = %task.cache_key,
                        error = %e,
                        "Failed to generate embedding for semantic cache"
                    );
                    metrics::record_cache_operation("semantic", "embed", "error");
                    continue;
                }
            };

            // Store the embedding in the vector store
            let metadata = VectorMetadata {
                cache_key: task.cache_key.clone(),
                model: task.model,
                organization_id: task.organization_id,
                project_id: task.project_id,
                created_at: chrono::Utc::now().timestamp(),
                ttl_secs: task.ttl.as_secs(),
            };

            if let Err(e) = vector_store
                .store(&task.cache_key, &embedding, metadata, task.ttl)
                .await
            {
                tracing::warn!(
                    cache_key = %task.cache_key,
                    error = %e,
                    "Failed to store embedding in vector store"
                );
                metrics::record_cache_operation("semantic", "store_embedding", "error");
            } else {
                tracing::debug!(
                    cache_key = %task.cache_key,
                    "Stored embedding in semantic cache"
                );
                metrics::record_cache_operation("semantic", "store_embedding", "success");
            }
        }
    }

    /// Look up a cached response using hybrid exact + semantic matching.
    ///
    /// # Strategy
    /// 1. First, attempt exact match using SHA-256 hash (fast)
    /// 2. If exact miss, generate embedding and search for similar requests
    /// 3. Return the best semantic match above the similarity threshold
    ///
    /// # Arguments
    /// * `payload` - The chat completion request
    /// * `model` - The resolved model name
    /// * `key_components` - Cache key configuration
    /// * `force_refresh` - If true, bypass cache lookup but still allow caching
    ///
    /// # Returns
    /// A `SemanticLookupResult` indicating exact hit, semantic hit, miss, or bypass
    pub async fn lookup(
        &self,
        payload: &CreateChatCompletionPayload,
        model: &str,
        key_components: &crate::config::CacheKeyComponents,
        force_refresh: bool,
    ) -> SemanticLookupResult {
        // Force refresh bypasses cache lookup
        if force_refresh {
            tracing::debug!("Cache force refresh requested");
            return SemanticLookupResult::Miss;
        }

        // Check if semantic caching is enabled
        if !self.config.enabled {
            return SemanticLookupResult::Bypass;
        }

        // Don't cache streaming requests
        if payload.stream {
            return SemanticLookupResult::Bypass;
        }

        // Generate exact cache key
        let cache_key = CacheKeys::response_cache(payload, model, key_components);

        // Step 1: Try exact match first (fastest)
        match self.cache.get_json::<CachedResponse>(&cache_key).await {
            Ok(Some(cached)) => {
                metrics::record_cache_operation("semantic", "get", "exact_hit");
                tracing::debug!(
                    cache_key = %cache_key,
                    provider = %cached.provider,
                    model = %cached.model,
                    "Semantic cache exact hit"
                );
                return SemanticLookupResult::ExactHit(cached);
            }
            Ok(None) => {
                // Continue to semantic search
            }
            Err(e) => {
                tracing::warn!(
                    cache_key = %cache_key,
                    error = %e,
                    "Cache lookup error, treating as miss"
                );
                metrics::record_cache_operation("semantic", "get", "error");
            }
        }

        // Step 2: Generate embedding for semantic search
        let embedding = match self.embedding_service.embed_request(payload).await {
            Ok(emb) => emb,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to generate embedding for semantic lookup, treating as miss"
                );
                metrics::record_cache_operation("semantic", "embed", "error");
                return SemanticLookupResult::Miss;
            }
        };

        // Step 3: Search for similar embeddings
        let search_results = match self
            .vector_store
            .search(
                &embedding,
                self.config.top_k,
                self.config.similarity_threshold,
                Some(model),
            )
            .await
        {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Vector search failed, treating as miss"
                );
                metrics::record_cache_operation("semantic", "search", "error");
                return SemanticLookupResult::Miss;
            }
        };

        // Step 4: Find best semantic match
        if let Some(best_match) = search_results.into_iter().next() {
            // Look up the cached response using the matched cache key
            match self
                .cache
                .get_json::<CachedResponse>(&best_match.metadata.cache_key)
                .await
            {
                Ok(Some(cached)) => {
                    metrics::record_cache_operation("semantic", "get", "semantic_hit");
                    tracing::debug!(
                        original_key = %cache_key,
                        matched_key = %best_match.metadata.cache_key,
                        similarity = %best_match.similarity,
                        provider = %cached.provider,
                        model = %cached.model,
                        "Semantic cache similarity hit"
                    );
                    return SemanticLookupResult::SemanticHit {
                        response: cached,
                        similarity: best_match.similarity,
                    };
                }
                Ok(None) => {
                    // The embedding exists but the cached response has expired
                    // This is a race condition between vector store TTL and cache TTL
                    tracing::debug!(
                        matched_key = %best_match.metadata.cache_key,
                        "Semantic match found but cached response expired"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        matched_key = %best_match.metadata.cache_key,
                        error = %e,
                        "Failed to retrieve cached response for semantic match"
                    );
                }
            }
        }

        metrics::record_cache_operation("semantic", "get", "miss");
        SemanticLookupResult::Miss
    }

    /// Store a response in both the exact cache and queue embedding for semantic search.
    ///
    /// # Arguments
    /// * `params` - Store parameters including payload, response data, and configuration
    ///
    /// # Returns
    /// `true` if the response was cached successfully
    pub async fn store(&self, params: StoreParams<'_>) -> bool {
        // Check if semantic caching is enabled
        if !self.config.enabled {
            return false;
        }

        // Don't cache streaming requests
        if params.payload.stream {
            return false;
        }

        // Generate exact cache key
        let cache_key =
            CacheKeys::response_cache(params.payload, params.model, params.key_components);

        // Create cached response
        let cached = CachedResponse {
            body: params.body,
            content_type: params.content_type.to_string(),
            provider: params.provider.to_string(),
            model: params.model.to_string(),
            cached_at: chrono::Utc::now().timestamp(),
        };

        // Store in primary cache
        if let Err(e) = self.cache.set_json(&cache_key, &cached, params.ttl).await {
            tracing::warn!(
                cache_key = %cache_key,
                error = %e,
                "Failed to cache response"
            );
            metrics::record_cache_operation("semantic", "set", "error");
            return false;
        }

        metrics::record_cache_operation("semantic", "set", "success");
        tracing::debug!(
            cache_key = %cache_key,
            provider = %params.provider,
            model = %params.model,
            size = cached.body.len(),
            ttl_secs = params.ttl.as_secs(),
            "Response cached"
        );

        // Queue background embedding task (don't block response)
        let text = self.embedding_service_text_for_payload(params.payload);
        let task = EmbeddingTask {
            cache_key,
            model: params.model.to_string(),
            text,
            ttl: params.ttl,
            organization_id: params.organization_id,
            project_id: params.project_id,
        };

        if let Err(e) = self.embedding_tx.try_send(task) {
            tracing::warn!(
                error = %e,
                "Failed to queue embedding task (channel full or closed)"
            );
        }

        true
    }

    /// Generate text representation of a payload for embedding.
    fn embedding_service_text_for_payload(&self, payload: &CreateChatCompletionPayload) -> String {
        // Re-use the embedding service's normalization logic
        // Since we can't access the private method directly, we'll duplicate the logic
        use crate::api_types::Message;

        let mut parts = Vec::new();

        for message in &payload.messages {
            let (role, content_str) = match message {
                Message::System { content, .. } => ("system", message_content_to_string(content)),
                Message::Developer { content, .. } => {
                    ("developer", message_content_to_string(content))
                }
                Message::User { content, .. } => ("user", message_content_to_string(content)),
                Message::Assistant {
                    content,
                    tool_calls,
                    ..
                } => {
                    let mut text = content
                        .as_ref()
                        .map(message_content_to_string)
                        .unwrap_or_default();
                    if let Some(calls) = tool_calls {
                        for call in calls {
                            text.push_str(&format!(" [tool:{}]", call.function.name));
                        }
                    }
                    ("assistant", text)
                }
                Message::Tool { content, .. } => ("tool", message_content_to_string(content)),
            };

            if !content_str.is_empty() {
                parts.push(format!("{}: {}", role, content_str));
            }
        }

        parts.join("\n")
    }

    /// Check if the semantic cache is healthy.
    pub async fn health_check(&self) -> Result<(), SemanticCacheError> {
        self.vector_store
            .health_check()
            .await
            .map_err(SemanticCacheError::VectorStore)
    }

    /// Get the similarity threshold.
    pub fn similarity_threshold(&self) -> f64 {
        self.config.similarity_threshold
    }

    /// Check if semantic caching is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

/// Convert MessageContent to a plain string.
fn message_content_to_string(content: &crate::api_types::MessageContent) -> String {
    use crate::api_types::{MessageContent, chat_completion::ContentPart};

    match content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| {
                if let ContentPart::Text { text, .. } = part {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Errors that can occur during semantic caching operations.
#[derive(Debug, thiserror::Error)]
pub enum SemanticCacheError {
    #[error("Vector store error: {0}")]
    VectorStore(#[from] VectorStoreError),

    #[error("Embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    #[error("Cache error: {0}")]
    Cache(String),
}

impl std::fmt::Debug for SemanticCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SemanticCache")
            .field("enabled", &self.config.enabled)
            .field("similarity_threshold", &self.config.similarity_threshold)
            .field("top_k", &self.config.top_k)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_content_to_string_text() {
        use crate::api_types::MessageContent;

        let content = MessageContent::Text("Hello, world!".to_string());
        assert_eq!(message_content_to_string(&content), "Hello, world!");
    }

    #[test]
    fn test_message_content_to_string_parts() {
        use crate::api_types::{MessageContent, chat_completion::ContentPart};

        let content = MessageContent::Parts(vec![
            ContentPart::Text {
                text: "First".to_string(),
                cache_control: None,
            },
            ContentPart::Text {
                text: "Second".to_string(),
                cache_control: None,
            },
        ]);
        assert_eq!(message_content_to_string(&content), "First Second");
    }
}
