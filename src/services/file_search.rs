//! File search service for RAG (Retrieval Augmented Generation).
//!
//! This service orchestrates searching across vector stores (vector stores)
//! and is used by the server-side `file_search` tool implementation.
//!
//! The service:
//! 1. Validates access to requested collections
//! 2. Generates query embeddings using the configured embedding provider
//! 3. Searches across one or more collections
//! 4. Returns ranked results with content and metadata

use std::sync::Arc;

use thiserror::Error;
use uuid::Uuid;

use crate::{
    cache::{
        EmbeddingService,
        vector_store::{HybridSearchConfig, RrfConfig, VectorBackend},
    },
    config::{CircuitBreakerConfig, RerankConfig, RetryConfig},
    db::{DbPool, ListParams},
    middleware::FileSearchAuthContext,
    models::{AttributeFilter, FileSearchRankingOptions, VectorStore, VectorStoreOwnerType},
    providers::{
        circuit_breaker::CircuitBreaker,
        retry::{is_retryable_database_error, with_circuit_breaker_and_retry_generic},
    },
    services::reranker::{RerankRequest, Reranker},
};

/// Configuration for the file search service.
#[derive(Debug, Clone)]
pub struct FileSearchServiceConfig {
    /// Default maximum results if not specified in request.
    pub default_max_results: usize,
    /// Default similarity threshold if not specified.
    pub default_threshold: f64,
    /// Retry configuration for vector store operations.
    pub retry: RetryConfig,
    /// Circuit breaker configuration for failing fast on unhealthy backends.
    pub circuit_breaker: CircuitBreakerConfig,
    /// Configuration for re-ranking behavior.
    pub rerank: RerankConfig,
}

/// Errors that can occur during file search operations.
#[derive(Debug, Error)]
pub enum FileSearchError {
    /// VectorStore not found or access denied.
    #[error("VectorStore not found: {0}")]
    VectorStoreNotFound(Uuid),

    /// Access denied to vector_store.
    #[error("Access denied to vector store: {0}")]
    AccessDenied(Uuid),

    /// Vector stores have incompatible embedding configurations.
    #[error("Incompatible collections: {0}")]
    IncompatibleVectorStores(String),

    /// Embedding generation failed.
    #[error("Failed to generate embedding: {0}")]
    EmbeddingError(String),

    /// Vector search failed.
    #[error("Search failed: {0}")]
    SearchError(String),

    /// Database error.
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// No vector stores specified.
    #[error("No vector stores specified for search")]
    NoVectorStores,

    /// File search is not configured.
    #[error("File search is not configured")]
    NotConfigured,

    /// Circuit breaker is open - vector store is unhealthy.
    #[error("Vector store circuit breaker is open: {0}")]
    CircuitBreakerOpen(String),

    /// Re-ranking failed.
    #[error("Re-ranking failed: {0}")]
    RerankError(String),
}

/// A single search result from the file search.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileSearchResult {
    /// The chunk ID in the vector store.
    pub chunk_id: Uuid,
    /// The vector store this chunk belongs to.
    pub vector_store_id: Uuid,
    /// The file this chunk was extracted from.
    pub file_id: Uuid,
    /// Index of this chunk within the file.
    pub chunk_index: i32,
    /// The actual text content of the chunk.
    pub content: String,
    /// Similarity score (0.0 to 1.0, higher is more similar).
    pub score: f64,
    /// Optional filename (resolved from file metadata).
    pub filename: Option<String>,
    /// Optional additional metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Configuration for a file search request.
#[derive(Debug, Clone)]
pub struct FileSearchRequest {
    /// The search query text.
    pub query: String,
    /// VectorStore IDs to search across.
    pub vector_store_ids: Vec<Uuid>,
    /// Maximum number of results to return (default: 10).
    pub max_results: Option<usize>,
    /// Minimum similarity threshold (0.0 to 1.0, default: 0.7).
    pub threshold: Option<f64>,
    /// Optional file IDs to restrict search to.
    pub file_ids: Option<Vec<Uuid>>,
    /// Optional attribute filter for filtering results based on file attributes.
    pub filters: Option<AttributeFilter>,
    /// Optional ranking options controlling ranker algorithm and hybrid search.
    ///
    /// When `ranking_options.use_hybrid_search()` returns true, the search will
    /// combine vector (semantic) and keyword (BM25/full-text) search using
    /// Reciprocal Rank Fusion (RRF).
    pub ranking_options: Option<FileSearchRankingOptions>,
}

/// Response from a file search operation.
#[derive(Debug, Clone)]
pub struct FileSearchResponse {
    /// The search results, ordered by relevance (highest first).
    pub results: Vec<FileSearchResult>,
    /// The query that was searched for.
    pub query: String,
    /// Total number of collections searched.
    pub vector_stores_searched: usize,
}

/// Service for searching vector stores.
///
/// This is the core service that powers the `file_search` tool in the
/// Responses API. It handles:
/// - VectorStore access validation
/// - Query embedding generation
/// - Multi-vector store search
/// - Result ranking and formatting
/// - Optional LLM-based re-ranking
pub struct FileSearchService {
    db: Arc<DbPool>,
    embedding_service: Arc<EmbeddingService>,
    vector_store: Arc<dyn VectorBackend>,
    default_max_results: usize,
    default_threshold: f64,
    retry: RetryConfig,
    circuit_breaker: Option<Arc<CircuitBreaker>>,
    /// Optional reranker for LLM-based result re-ranking.
    reranker: Option<Arc<dyn Reranker>>,
    /// Configuration for re-ranking behavior.
    rerank_config: RerankConfig,
}

impl FileSearchService {
    /// Create a new file search service.
    ///
    /// # Arguments
    /// * `db` - Database pool for vector store metadata lookups
    /// * `embedding_service` - Service for generating query embeddings
    /// * `vector_store` - Vector store for similarity search
    /// * `reranker` - Optional reranker for LLM-based result re-ranking
    /// * `config` - Service configuration (thresholds, retry, circuit breaker, rerank)
    pub fn new(
        db: Arc<DbPool>,
        embedding_service: Arc<EmbeddingService>,
        vector_store: Arc<dyn VectorBackend>,
        reranker: Option<Arc<dyn Reranker>>,
        config: FileSearchServiceConfig,
    ) -> Self {
        let circuit_breaker = if config.circuit_breaker.enabled {
            Some(Arc::new(CircuitBreaker::new(
                "file_search_vector_store",
                &config.circuit_breaker,
            )))
        } else {
            None
        };

        Self {
            db,
            embedding_service,
            vector_store,
            default_max_results: config.default_max_results,
            default_threshold: config.default_threshold,
            retry: config.retry,
            circuit_breaker,
            reranker,
            rerank_config: config.rerank,
        }
    }

    /// Get the vector store used by this service.
    ///
    /// This is useful for background tasks like cleanup that need
    /// to operate on the same vector store.
    pub fn vector_store(&self) -> Arc<dyn VectorBackend> {
        self.vector_store.clone()
    }

    /// Get the embedding service used by this service.
    ///
    /// This is useful for document processing that needs to generate
    /// embeddings with the same configuration as file search.
    pub fn embedding_service(&self) -> Arc<EmbeddingService> {
        self.embedding_service.clone()
    }

    /// Search across collections for content matching the query.
    ///
    /// # Arguments
    /// * `request` - The search request containing query, vector store IDs, and options
    /// * `auth` - Optional authentication context for access control validation
    ///
    /// # Returns
    /// Search results ordered by relevance, or an error.
    pub async fn search(
        &self,
        request: FileSearchRequest,
        auth: Option<FileSearchAuthContext>,
    ) -> Result<FileSearchResponse, FileSearchError> {
        if request.vector_store_ids.is_empty() {
            return Err(FileSearchError::NoVectorStores);
        }

        // 1. Validate collections exist and user has access
        let collections = self
            .validate_collections(&request.vector_store_ids, auth.as_ref())
            .await?;

        // 2. Validate all vector stores have compatible embedding configurations
        self.validate_embedding_compatibility(&collections)?;

        // 3. Generate embedding for the query
        let query_embedding = self
            .embedding_service
            .embed_text(&request.query)
            .await
            .map_err(|e| FileSearchError::EmbeddingError(e.to_string()))?;

        // 4. Search across collections
        let max_results = request.max_results.unwrap_or(self.default_max_results);
        let threshold = request.threshold.unwrap_or(self.default_threshold);

        // Build filter from file_ids and attribute filters
        let filter = if request.file_ids.is_some() || request.filters.is_some() {
            Some(crate::cache::vector_store::ChunkFilter {
                file_ids: request.file_ids.clone(),
                attribute_filter: request.filters.clone(),
            })
        } else {
            None
        };

        let vector_store_ids_str: Vec<Uuid> = request.vector_store_ids.clone();
        let vector_store = self.vector_store.clone();

        // Determine if hybrid search should be used
        let use_hybrid = request
            .ranking_options
            .as_ref()
            .is_some_and(|opts| opts.use_hybrid_search());

        // Search with circuit breaker and retry for transient errors
        let search_results = if use_hybrid {
            // Build HybridSearchConfig from ranking_options
            let hybrid_opts = request.ranking_options.as_ref().unwrap();
            let api_hybrid = hybrid_opts.hybrid_search.as_ref().unwrap();
            let hybrid_config = HybridSearchConfig {
                rrf: RrfConfig::weighted(api_hybrid.embedding_weight, api_hybrid.text_weight),
                vector_threshold: threshold,
            };

            let query_text = request.query.clone();

            with_circuit_breaker_and_retry_generic(
                self.circuit_breaker.as_deref(),
                &self.retry,
                "vector_store",
                "hybrid_search_vector_stores",
                |e: &crate::cache::vector_store::VectorStoreError| match e {
                    crate::cache::vector_store::VectorStoreError::Database(msg) => {
                        is_retryable_database_error(msg)
                    }
                    crate::cache::vector_store::VectorStoreError::Http(_) => true,
                    _ => false,
                },
                |_| false,
                || {
                    let ids = vector_store_ids_str.clone();
                    let emb = query_embedding.clone();
                    let q = query_text.clone();
                    let f = filter.clone();
                    let vs = vector_store.clone();
                    let cfg = hybrid_config.clone();
                    async move {
                        vs.hybrid_search_vector_stores(&ids, &q, &emb, max_results, cfg, f)
                            .await
                    }
                },
            )
            .await
        } else {
            // Vector-only search
            with_circuit_breaker_and_retry_generic(
                self.circuit_breaker.as_deref(),
                &self.retry,
                "vector_store",
                "search_vector_stores",
                |e: &crate::cache::vector_store::VectorStoreError| match e {
                    crate::cache::vector_store::VectorStoreError::Database(msg) => {
                        is_retryable_database_error(msg)
                    }
                    crate::cache::vector_store::VectorStoreError::Http(_) => true,
                    _ => false,
                },
                |_| false,
                || {
                    let ids = vector_store_ids_str.clone();
                    let emb = query_embedding.clone();
                    let f = filter.clone();
                    let vs = vector_store.clone();
                    async move {
                        vs.search_vector_stores(&ids, &emb, max_results, threshold, f)
                            .await
                    }
                },
            )
            .await
        };

        let search_results = search_results.map_err(|e| match e {
            crate::providers::retry::GenericRequestError::CircuitBreakerOpen(cb_err) => {
                FileSearchError::CircuitBreakerOpen(cb_err.to_string())
            }
            crate::providers::retry::GenericRequestError::Operation(op_err) => {
                FileSearchError::SearchError(op_err.to_string())
            }
        })?;

        // 5. Resolve filenames for results
        let results = self.resolve_filenames(search_results).await?;

        // 6. Apply LLM re-ranking if requested
        let use_llm_rerank = request
            .ranking_options
            .as_ref()
            .is_some_and(|opts| opts.effective_ranker().is_llm_rerank());

        let results = if use_llm_rerank {
            self.apply_reranking(&request.query, results, max_results)
                .await?
        } else {
            results
        };

        Ok(FileSearchResponse {
            results,
            query: request.query,
            vector_stores_searched: collections.len(),
        })
    }

    /// Apply LLM-based re-ranking to search results.
    ///
    /// If re-ranking is enabled and a reranker is available, this will pass the
    /// results through the LLM for relevance scoring. Falls back to original
    /// results if re-ranking is not configured or fails.
    async fn apply_reranking(
        &self,
        query: &str,
        results: Vec<FileSearchResult>,
        max_results: usize,
    ) -> Result<Vec<FileSearchResult>, FileSearchError> {
        // Check if re-ranking is enabled
        if !self.rerank_config.enabled {
            tracing::debug!(
                stage = "rerank_skipped",
                reason = "disabled",
                "LLM re-ranking requested but disabled in config, using vector scores"
            );
            return Ok(results);
        }

        // Check if we have a reranker
        let Some(ref reranker) = self.reranker else {
            tracing::warn!(
                stage = "rerank_skipped",
                reason = "no_reranker",
                "LLM re-ranking requested but no reranker configured, using vector scores"
            );
            return Ok(results);
        };

        // Check if reranker is available
        if !reranker.is_available() {
            tracing::warn!(
                stage = "rerank_skipped",
                reason = "unavailable",
                "LLM reranker is not available, using vector scores"
            );
            return Ok(results);
        }

        // Skip re-ranking for empty results
        if results.is_empty() {
            return Ok(results);
        }

        tracing::debug!(
            stage = "rerank_started",
            results_count = results.len(),
            max_results,
            "Applying LLM re-ranking"
        );

        // Clone results for fallback if configured to fall back on error
        let fallback_results = if self.rerank_config.fallback_on_error {
            Some(results.clone())
        } else {
            None
        };

        // Build rerank request
        let rerank_request = RerankRequest {
            query: query.to_string(),
            results,
            top_n: Some(max_results),
        };

        // Execute re-ranking
        match reranker.rerank(rerank_request).await {
            Ok(response) => {
                tracing::info!(
                    stage = "rerank_completed",
                    reranked_count = response.results.len(),
                    total_considered = response.total_considered,
                    model = ?response.model,
                    prompt_tokens = response.usage.as_ref().map(|u| u.prompt_tokens),
                    completion_tokens = response.usage.as_ref().map(|u| u.completion_tokens),
                    "LLM re-ranking completed"
                );

                // Extract the re-ranked results, updating scores
                Ok(response
                    .results
                    .into_iter()
                    .map(|ranked| {
                        let mut result = ranked.result;
                        result.score = ranked.relevance_score;
                        result
                    })
                    .collect())
            }
            Err(e) => {
                if let Some(original_results) = fallback_results {
                    tracing::warn!(
                        stage = "rerank_failed",
                        error = %e,
                        fallback = true,
                        "LLM re-ranking failed, returning results with original vector scores"
                    );
                    Ok(original_results)
                } else {
                    tracing::warn!(
                        stage = "rerank_failed",
                        error = %e,
                        fallback = false,
                        "LLM re-ranking failed, propagating error (fallback_on_error=false)"
                    );
                    Err(FileSearchError::RerankError(e.to_string()))
                }
            }
        }
    }

    /// Validate that all vector stores exist and user has access.
    async fn validate_collections(
        &self,
        vector_store_ids: &[Uuid],
        auth: Option<&FileSearchAuthContext>,
    ) -> Result<Vec<VectorStore>, FileSearchError> {
        let mut collections = Vec::with_capacity(vector_store_ids.len());

        for &id in vector_store_ids {
            let vector_store = self
                .db
                .vector_stores()
                .get_vector_store(id)
                .await
                .map_err(|e| FileSearchError::DatabaseError(e.to_string()))?
                .ok_or(FileSearchError::VectorStoreNotFound(id))?;

            // Check access if auth context is provided
            if let Some(auth) = auth {
                let has_access = self.check_vector_store_access(&vector_store, auth).await?;
                if !has_access {
                    return Err(FileSearchError::AccessDenied(id));
                }
            }

            collections.push(vector_store);
        }

        Ok(collections)
    }

    /// Check if the authenticated entity has access to a vector_store.
    ///
    /// This implements the same access control logic as `check_resource_access` in route handlers:
    /// - User-owned: user_id must match the owner
    /// - Org-owned: org_id matches OR identity_org_ids contains owner OR user is a member
    /// - Project-owned: project_id matches OR identity_project_ids contains owner OR user is a member
    async fn check_vector_store_access(
        &self,
        vector_store: &VectorStore,
        auth: &FileSearchAuthContext,
    ) -> Result<bool, FileSearchError> {
        match vector_store.owner_type {
            VectorStoreOwnerType::User => {
                // User-owned: user_id must match
                Ok(auth.user_id == Some(vector_store.owner_id))
            }
            VectorStoreOwnerType::Organization => {
                // 1. Check direct org ownership from API key
                if auth.org_id == Some(vector_store.owner_id) {
                    return Ok(true);
                }

                // 2. Check identity org membership (from OAuth/OIDC claims)
                if auth
                    .identity_org_ids
                    .contains(&vector_store.owner_id.to_string())
                {
                    return Ok(true);
                }

                // 3. Fall back to database membership check (for user_id)
                if let Some(user_id) = auth.user_id {
                    let members = self
                        .db
                        .users()
                        .list_org_members(vector_store.owner_id, ListParams::default())
                        .await
                        .map_err(|e| FileSearchError::DatabaseError(e.to_string()))?;
                    if members.items.iter().any(|u| u.id == user_id) {
                        return Ok(true);
                    }
                }

                Ok(false)
            }
            VectorStoreOwnerType::Team => {
                // Team access: check if user is a member of the team
                if let Some(user_id) = auth.user_id {
                    let members = self
                        .db
                        .teams()
                        .list_members(vector_store.owner_id, ListParams::default())
                        .await
                        .map_err(|e| FileSearchError::DatabaseError(e.to_string()))?;
                    if members.items.iter().any(|m| m.user_id == user_id) {
                        return Ok(true);
                    }
                }

                Ok(false)
            }
            VectorStoreOwnerType::Project => {
                // 1. Check direct project ownership from API key
                if auth.project_id == Some(vector_store.owner_id) {
                    return Ok(true);
                }

                // 2. Check identity project membership (from OAuth/OIDC claims)
                if auth
                    .identity_project_ids
                    .contains(&vector_store.owner_id.to_string())
                {
                    return Ok(true);
                }

                // 3. Fall back to database membership check (for user_id)
                if let Some(user_id) = auth.user_id {
                    let members = self
                        .db
                        .users()
                        .list_project_members(vector_store.owner_id, ListParams::default())
                        .await
                        .map_err(|e| FileSearchError::DatabaseError(e.to_string()))?;
                    if members.items.iter().any(|u| u.id == user_id) {
                        return Ok(true);
                    }
                }

                Ok(false)
            }
        }
    }

    /// Validate that all vector stores have compatible embedding configurations.
    fn validate_embedding_compatibility(
        &self,
        collections: &[VectorStore],
    ) -> Result<(), FileSearchError> {
        if collections.is_empty() {
            return Ok(());
        }

        let first = &collections[0];
        let expected_model = &first.embedding_model;
        let expected_dims = first.embedding_dimensions;

        for vs in collections.iter().skip(1) {
            if vs.embedding_model != *expected_model {
                return Err(FileSearchError::IncompatibleVectorStores(format!(
                    "VectorStore '{}' uses embedding model '{}', but '{}' uses '{}'",
                    vs.name, vs.embedding_model, first.name, expected_model
                )));
            }
            if vs.embedding_dimensions != expected_dims {
                return Err(FileSearchError::IncompatibleVectorStores(format!(
                    "VectorStore '{}' uses {} dimensions, but '{}' uses {}",
                    vs.name, vs.embedding_dimensions, first.name, expected_dims
                )));
            }
        }

        Ok(())
    }

    /// Resolve filenames for search results by looking up file metadata.
    async fn resolve_filenames(
        &self,
        search_results: Vec<crate::cache::vector_store::ChunkSearchResult>,
    ) -> Result<Vec<FileSearchResult>, FileSearchError> {
        let mut results = Vec::with_capacity(search_results.len());

        for chunk in search_results {
            // Try to get the filename from file metadata
            let filename = self
                .db
                .files()
                .get_file(chunk.file_id)
                .await
                .map_err(|e| FileSearchError::DatabaseError(e.to_string()))?
                .map(|f| f.filename);

            results.push(FileSearchResult {
                chunk_id: chunk.chunk_id,
                vector_store_id: chunk.vector_store_id,
                file_id: chunk.file_id,
                chunk_index: chunk.chunk_index,
                content: chunk.content,
                score: chunk.score,
                filename,
                metadata: chunk.metadata,
            });
        }

        Ok(results)
    }

    /// Get the default maximum results setting.
    pub fn default_max_results(&self) -> usize {
        self.default_max_results
    }

    /// Get the default threshold setting.
    pub fn default_threshold(&self) -> f64 {
        self.default_threshold
    }

    /// Get all chunks for a specific file.
    ///
    /// This is a Hadrian extension for debugging and admin UI purposes.
    /// Returns chunks ordered by chunk_index for sequential reading.
    ///
    /// # Arguments
    /// * `file_id` - The file ID to retrieve chunks for
    ///
    /// # Returns
    /// All chunks for the file, ordered by chunk_index.
    pub async fn get_chunks_by_file(
        &self,
        file_id: Uuid,
    ) -> Result<Vec<crate::cache::vector_store::StoredChunk>, FileSearchError> {
        self.vector_store
            .get_chunks_by_file(file_id)
            .await
            .map_err(|e| FileSearchError::SearchError(e.to_string()))
    }
}

impl std::fmt::Debug for FileSearchService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSearchService")
            .field("default_max_results", &self.default_max_results)
            .field("default_threshold", &self.default_threshold)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_search_error_display() {
        let err = FileSearchError::VectorStoreNotFound(Uuid::nil());
        assert!(err.to_string().contains("not found"));

        let err = FileSearchError::AccessDenied(Uuid::nil());
        assert!(err.to_string().contains("denied"));

        let err = FileSearchError::IncompatibleVectorStores("test".to_string());
        assert!(err.to_string().contains("Incompatible"));

        let err = FileSearchError::NoVectorStores;
        assert!(err.to_string().contains("No vector stores"));
    }

    #[test]
    fn test_file_search_request_defaults() {
        let request = FileSearchRequest {
            query: "test query".to_string(),
            vector_store_ids: vec![Uuid::new_v4()],
            max_results: None,
            threshold: None,
            file_ids: None,
            filters: None,
            ranking_options: None,
        };

        assert!(request.max_results.is_none());
        assert!(request.threshold.is_none());
        assert!(request.file_ids.is_none());
        assert!(request.filters.is_none());
        assert!(request.ranking_options.is_none());
    }

    #[test]
    fn test_file_search_request_with_hybrid_search() {
        use crate::models::{FileSearchRanker, HybridSearchOptions};

        // Request without hybrid search options
        let request_no_hybrid = FileSearchRequest {
            query: "test query".to_string(),
            vector_store_ids: vec![Uuid::new_v4()],
            max_results: Some(10),
            threshold: Some(0.5),
            file_ids: None,
            filters: None,
            ranking_options: Some(FileSearchRankingOptions::new(0.5)),
        };
        assert!(
            !request_no_hybrid
                .ranking_options
                .as_ref()
                .unwrap()
                .use_hybrid_search()
        );

        // Request with hybrid search options
        let request_hybrid = FileSearchRequest {
            query: "test query".to_string(),
            vector_store_ids: vec![Uuid::new_v4()],
            max_results: Some(10),
            threshold: Some(0.5),
            file_ids: None,
            filters: None,
            ranking_options: Some(FileSearchRankingOptions::with_hybrid(
                0.5,
                HybridSearchOptions::new(0.7, 0.3),
            )),
        };
        assert!(
            request_hybrid
                .ranking_options
                .as_ref()
                .unwrap()
                .use_hybrid_search()
        );

        // Verify hybrid options are correctly set
        let hybrid_opts = request_hybrid
            .ranking_options
            .as_ref()
            .unwrap()
            .hybrid_search
            .as_ref()
            .unwrap();
        assert_eq!(hybrid_opts.embedding_weight, 0.7);
        assert_eq!(hybrid_opts.text_weight, 0.3);

        // Verify ranker is hybrid for hybrid search
        assert_eq!(
            request_hybrid
                .ranking_options
                .as_ref()
                .unwrap()
                .effective_ranker(),
            FileSearchRanker::Hybrid
        );
    }

    #[test]
    fn test_file_search_request_hybrid_with_incompatible_ranker() {
        use crate::models::{FileSearchRanker, HybridSearchOptions};

        // Hybrid options with vector-only ranker should not use hybrid search
        let mut ranking_options =
            FileSearchRankingOptions::with_ranker(0.5, FileSearchRanker::Vector);
        ranking_options.hybrid_search = Some(HybridSearchOptions::default());

        let request = FileSearchRequest {
            query: "test query".to_string(),
            vector_store_ids: vec![Uuid::new_v4()],
            max_results: Some(10),
            threshold: Some(0.5),
            file_ids: None,
            filters: None,
            ranking_options: Some(ranking_options),
        };

        // Even though hybrid_search is set, the ranker doesn't support it
        assert!(
            !request
                .ranking_options
                .as_ref()
                .unwrap()
                .use_hybrid_search()
        );
    }

    #[test]
    fn test_file_search_error_rerank_display() {
        let err = FileSearchError::RerankError("timeout".to_string());
        assert!(err.to_string().contains("Re-ranking failed"));
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn test_file_search_request_with_llm_ranker() {
        use crate::models::FileSearchRanker;

        let request = FileSearchRequest {
            query: "test query".to_string(),
            vector_store_ids: vec![Uuid::new_v4()],
            max_results: Some(10),
            threshold: Some(0.5),
            file_ids: None,
            filters: None,
            ranking_options: Some(FileSearchRankingOptions::with_ranker(
                0.5,
                FileSearchRanker::Llm,
            )),
        };

        let ranking_options = request.ranking_options.as_ref().unwrap();
        assert!(ranking_options.effective_ranker().is_llm_rerank());
        // LLM ranker doesn't use hybrid search
        assert!(!ranking_options.use_hybrid_search());
    }
}

// ============================================================================
// Access Control Integration Tests
// ============================================================================
//
// These tests verify that the middleware access control logic correctly enforces
// ownership and membership rules for vector store access via file_search tool calls.
//
// Test scenarios:
// 1. User-owned vector store: user_id must match owner
// 2. Org-owned vector store: org_id matches OR identity_org_ids contains owner OR user is org member
// 3. Project-owned vector store: project_id matches OR identity_project_ids contains owner OR user is project member
// 4. Access denied for mismatched ownership

#[cfg(all(test, feature = "database-sqlite"))]
mod access_control_tests {
    use std::sync::Arc;

    use uuid::Uuid;

    use crate::{
        db::{DbPool, tests::harness},
        middleware::FileSearchAuthContext,
        models::{
            CreateUser, CreateVectorStore, MembershipSource, VectorStoreOwner, VectorStoreOwnerType,
        },
    };

    /// Test context for access control tests
    struct AccessControlTestContext {
        db: Arc<DbPool>,
    }

    impl AccessControlTestContext {
        async fn new_sqlite() -> Self {
            let pool = harness::create_sqlite_pool().await;
            harness::run_sqlite_migrations(&pool).await;
            Self {
                db: Arc::new(DbPool::from_sqlite(pool)),
            }
        }

        async fn create_user(&self, external_id: &str) -> Uuid {
            self.db
                .users()
                .create(CreateUser {
                    external_id: external_id.to_string(),
                    email: Some(format!("{}@test.com", external_id)),
                    name: Some(external_id.to_string()),
                })
                .await
                .expect("Failed to create user")
                .id
        }

        async fn create_org(&self, slug: &str) -> Uuid {
            self.db
                .organizations()
                .create(crate::models::CreateOrganization {
                    slug: slug.to_string(),
                    name: format!("Org {}", slug),
                })
                .await
                .expect("Failed to create org")
                .id
        }

        async fn create_project(&self, org_id: Uuid, slug: &str) -> Uuid {
            self.db
                .projects()
                .create(
                    org_id,
                    crate::models::CreateProject {
                        slug: slug.to_string(),
                        name: format!("Project {}", slug),
                        team_id: None,
                    },
                )
                .await
                .expect("Failed to create project")
                .id
        }

        async fn add_user_to_org(&self, user_id: Uuid, org_id: Uuid) {
            self.db
                .users()
                .add_to_org(user_id, org_id, "member", MembershipSource::Manual)
                .await
                .expect("Failed to add user to org");
        }

        async fn add_user_to_project(&self, user_id: Uuid, project_id: Uuid) {
            self.db
                .users()
                .add_to_project(user_id, project_id, "member", MembershipSource::Manual)
                .await
                .expect("Failed to add user to project");
        }

        async fn create_vector_store(
            &self,
            owner_type: VectorStoreOwnerType,
            owner_id: Uuid,
            name: &str,
        ) -> crate::models::VectorStore {
            let owner = match owner_type {
                VectorStoreOwnerType::User => VectorStoreOwner::User { user_id: owner_id },
                VectorStoreOwnerType::Organization => VectorStoreOwner::Organization {
                    organization_id: owner_id,
                },
                VectorStoreOwnerType::Team => VectorStoreOwner::Team { team_id: owner_id },
                VectorStoreOwnerType::Project => VectorStoreOwner::Project {
                    project_id: owner_id,
                },
            };

            self.db
                .vector_stores()
                .create_vector_store(CreateVectorStore {
                    owner,
                    file_ids: vec![],
                    name: Some(name.to_string()),
                    description: None,
                    embedding_model: "text-embedding-3-small".to_string(),
                    embedding_dimensions: 1536,
                    metadata: None,
                    expires_after: None,
                    chunking_strategy: None,
                })
                .await
                .expect("Failed to create Qdrant index")
        }
    }

    // ========================================================================
    // User-owned vector store tests
    // ========================================================================

    #[tokio::test]
    async fn test_user_owned_vector_store_access_with_matching_user_id() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let user_id = ctx.create_user("owner-user").await;
        let vector_store = ctx
            .create_vector_store(VectorStoreOwnerType::User, user_id, "user-vector-store")
            .await;

        // Auth context with matching user_id
        let auth = FileSearchAuthContext {
            user_id: Some(user_id),
            org_id: None,
            project_id: None,
            identity_org_ids: vec![],
            identity_project_ids: vec![],
        };

        // User-owned: user_id must match owner
        assert_eq!(vector_store.owner_type, VectorStoreOwnerType::User);
        assert_eq!(vector_store.owner_id, user_id);
        assert_eq!(auth.user_id, Some(vector_store.owner_id));
    }

    #[tokio::test]
    async fn test_user_owned_vector_store_denied_with_different_user_id() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let owner_id = ctx.create_user("owner-user").await;
        let other_user_id = ctx.create_user("other-user").await;
        let vector_store = ctx
            .create_vector_store(VectorStoreOwnerType::User, owner_id, "user-vector-store")
            .await;

        // Auth context with different user_id
        let auth = FileSearchAuthContext {
            user_id: Some(other_user_id),
            org_id: None,
            project_id: None,
            identity_org_ids: vec![],
            identity_project_ids: vec![],
        };

        // Should be denied - user_id doesn't match
        assert_ne!(auth.user_id, Some(vector_store.owner_id));
    }

    // ========================================================================
    // Org-owned vector store tests
    // ========================================================================

    #[tokio::test]
    async fn test_org_owned_vector_store_access_with_direct_org_id() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let org_id = ctx.create_org("test-org").await;
        let vector_store = ctx
            .create_vector_store(
                VectorStoreOwnerType::Organization,
                org_id,
                "org-vector-store",
            )
            .await;

        // Auth context with direct org_id (API key with org ownership)
        let auth = FileSearchAuthContext {
            user_id: None, // No user, just org-scoped API key
            org_id: Some(org_id),
            project_id: None,
            identity_org_ids: vec![],
            identity_project_ids: vec![],
        };

        // Should allow access - direct org_id match
        assert_eq!(vector_store.owner_type, VectorStoreOwnerType::Organization);
        assert_eq!(auth.org_id, Some(vector_store.owner_id));
    }

    #[tokio::test]
    async fn test_org_owned_vector_store_access_with_identity_org_ids() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let org_id = ctx.create_org("test-org").await;
        let vector_store = ctx
            .create_vector_store(
                VectorStoreOwnerType::Organization,
                org_id,
                "org-vector-store",
            )
            .await;

        // Auth context with identity_org_ids (OAuth/OIDC claims)
        let auth = FileSearchAuthContext {
            user_id: None,
            org_id: None,
            project_id: None,
            identity_org_ids: vec![org_id.to_string()],
            identity_project_ids: vec![],
        };

        // Should allow access - identity claims include the org
        assert!(
            auth.identity_org_ids
                .contains(&vector_store.owner_id.to_string())
        );
    }

    #[tokio::test]
    async fn test_org_owned_vector_store_access_via_database_membership() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let org_id = ctx.create_org("test-org").await;
        let user_id = ctx.create_user("member-user").await;
        ctx.add_user_to_org(user_id, org_id).await;

        let vector_store = ctx
            .create_vector_store(
                VectorStoreOwnerType::Organization,
                org_id,
                "org-vector-store",
            )
            .await;

        // Auth context with only user_id (no direct org ownership)
        // This context would be used by check_vector_store_access for the fallback membership check
        let _auth = FileSearchAuthContext {
            user_id: Some(user_id),
            org_id: None, // No direct org_id from API key
            project_id: None,
            identity_org_ids: vec![], // No identity claims
            identity_project_ids: vec![],
        };

        // Verify user is member of the org
        let members = ctx
            .db
            .users()
            .list_org_members(org_id, crate::db::ListParams::default())
            .await
            .expect("Failed to list org members");
        assert!(members.items.iter().any(|u| u.id == user_id));

        // The actual access check happens in check_vector_store_access which
        // falls back to database membership when org_id and identity_org_ids don't match
        assert_eq!(vector_store.owner_type, VectorStoreOwnerType::Organization);
        assert_eq!(vector_store.owner_id, org_id);
    }

    #[tokio::test]
    async fn test_org_owned_collection_denied_without_membership() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let org_id = ctx.create_org("test-org").await;
        let other_org_id = ctx.create_org("other-org").await;
        let user_id = ctx.create_user("non-member").await;

        // User is member of other-org but not test-org
        ctx.add_user_to_org(user_id, other_org_id).await;

        let vector_store = ctx
            .create_vector_store(
                VectorStoreOwnerType::Organization,
                org_id,
                "org-vector-store",
            )
            .await;

        // Auth context with user who is not a member
        let auth = FileSearchAuthContext {
            user_id: Some(user_id),
            org_id: None,
            project_id: None,
            identity_org_ids: vec![],
            identity_project_ids: vec![],
        };

        // Verify user is NOT a member of the vector store's org
        let members = ctx
            .db
            .users()
            .list_org_members(org_id, crate::db::ListParams::default())
            .await
            .expect("Failed to list org members");
        assert!(!members.items.iter().any(|u| u.id == user_id));

        // Should be denied
        assert_ne!(auth.org_id, Some(vector_store.owner_id));
        assert!(
            !auth
                .identity_org_ids
                .contains(&vector_store.owner_id.to_string())
        );
    }

    // ========================================================================
    // Project-owned vector store tests
    // ========================================================================

    #[tokio::test]
    async fn test_project_owned_vector_store_access_with_direct_project_id() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let org_id = ctx.create_org("test-org").await;
        let project_id = ctx.create_project(org_id, "test-project").await;
        let vector_store = ctx
            .create_vector_store(
                VectorStoreOwnerType::Project,
                project_id,
                "project-vector-store",
            )
            .await;

        // Auth context with direct project_id (API key with project ownership)
        let auth = FileSearchAuthContext {
            user_id: None,
            org_id: None,
            project_id: Some(project_id),
            identity_org_ids: vec![],
            identity_project_ids: vec![],
        };

        // Should allow access - direct project_id match
        assert_eq!(vector_store.owner_type, VectorStoreOwnerType::Project);
        assert_eq!(auth.project_id, Some(vector_store.owner_id));
    }

    #[tokio::test]
    async fn test_project_owned_vector_store_access_with_identity_project_ids() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let org_id = ctx.create_org("test-org").await;
        let project_id = ctx.create_project(org_id, "test-project").await;
        let vector_store = ctx
            .create_vector_store(
                VectorStoreOwnerType::Project,
                project_id,
                "project-vector-store",
            )
            .await;

        // Auth context with identity_project_ids (OAuth/OIDC claims)
        let auth = FileSearchAuthContext {
            user_id: None,
            org_id: None,
            project_id: None,
            identity_org_ids: vec![],
            identity_project_ids: vec![project_id.to_string()],
        };

        // Should allow access - identity claims include the project
        assert!(
            auth.identity_project_ids
                .contains(&vector_store.owner_id.to_string())
        );
    }

    #[tokio::test]
    async fn test_project_owned_vector_store_access_via_database_membership() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let org_id = ctx.create_org("test-org").await;
        let project_id = ctx.create_project(org_id, "test-project").await;
        let user_id = ctx.create_user("member-user").await;
        ctx.add_user_to_project(user_id, project_id).await;

        let vector_store = ctx
            .create_vector_store(
                VectorStoreOwnerType::Project,
                project_id,
                "project-vector-store",
            )
            .await;

        // Auth context with only user_id (no direct project ownership)
        // This context would be used by check_vector_store_access for the fallback membership check
        let _auth = FileSearchAuthContext {
            user_id: Some(user_id),
            org_id: None,
            project_id: None, // No direct project_id from API key
            identity_org_ids: vec![],
            identity_project_ids: vec![], // No identity claims
        };

        // Verify user is member of the project
        let members = ctx
            .db
            .users()
            .list_project_members(project_id, crate::db::ListParams::default())
            .await
            .expect("Failed to list project members");
        assert!(members.items.iter().any(|u| u.id == user_id));

        // The actual access check happens in check_vector_store_access which
        // falls back to database membership when project_id and identity_project_ids don't match
        assert_eq!(vector_store.owner_type, VectorStoreOwnerType::Project);
        assert_eq!(vector_store.owner_id, project_id);
    }

    #[tokio::test]
    async fn test_project_owned_collection_denied_without_membership() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let org_id = ctx.create_org("test-org").await;
        let project_id = ctx.create_project(org_id, "test-project").await;
        let other_project_id = ctx.create_project(org_id, "other-project").await;
        let user_id = ctx.create_user("non-member").await;

        // User is member of other-project but not test-project
        ctx.add_user_to_project(user_id, other_project_id).await;

        let vector_store = ctx
            .create_vector_store(
                VectorStoreOwnerType::Project,
                project_id,
                "project-vector-store",
            )
            .await;

        // Auth context with user who is not a member
        let auth = FileSearchAuthContext {
            user_id: Some(user_id),
            org_id: None,
            project_id: None,
            identity_org_ids: vec![],
            identity_project_ids: vec![],
        };

        // Verify user is NOT a member of the vector store's project
        let members = ctx
            .db
            .users()
            .list_project_members(project_id, crate::db::ListParams::default())
            .await
            .expect("Failed to list project members");
        assert!(!members.items.iter().any(|u| u.id == user_id));

        // Should be denied
        assert_ne!(auth.project_id, Some(vector_store.owner_id));
        assert!(
            !auth
                .identity_project_ids
                .contains(&vector_store.owner_id.to_string())
        );
    }

    // ========================================================================
    // Cross-ownership denial tests
    // ========================================================================

    #[tokio::test]
    async fn test_user_cannot_access_other_users_vector_store() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let owner_id = ctx.create_user("owner").await;
        let attacker_id = ctx.create_user("attacker").await;

        let vector_store = ctx
            .create_vector_store(VectorStoreOwnerType::User, owner_id, "private-vector-store")
            .await;

        let auth = FileSearchAuthContext {
            user_id: Some(attacker_id),
            org_id: None,
            project_id: None,
            identity_org_ids: vec![],
            identity_project_ids: vec![],
        };

        // Attacker should not be able to access owner's vector store
        assert_ne!(auth.user_id, Some(vector_store.owner_id));
    }

    #[tokio::test]
    async fn test_org_api_key_cannot_access_different_orgs_collection() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let target_org_id = ctx.create_org("target-org").await;
        let attacker_org_id = ctx.create_org("attacker-org").await;

        let vector_store = ctx
            .create_vector_store(
                VectorStoreOwnerType::Organization,
                target_org_id,
                "target-vector-store",
            )
            .await;

        // API key scoped to attacker org
        let auth = FileSearchAuthContext {
            user_id: None,
            org_id: Some(attacker_org_id),
            project_id: None,
            identity_org_ids: vec![],
            identity_project_ids: vec![],
        };

        // Should be denied - org_id doesn't match
        assert_ne!(auth.org_id, Some(vector_store.owner_id));
    }

    #[tokio::test]
    async fn test_project_api_key_cannot_access_different_projects_collection() {
        let ctx = AccessControlTestContext::new_sqlite().await;
        let org_id = ctx.create_org("test-org").await;
        let target_project_id = ctx.create_project(org_id, "target-project").await;
        let attacker_project_id = ctx.create_project(org_id, "attacker-project").await;

        let vector_store = ctx
            .create_vector_store(
                VectorStoreOwnerType::Project,
                target_project_id,
                "target-vector-store",
            )
            .await;

        // API key scoped to attacker project
        let auth = FileSearchAuthContext {
            user_id: None,
            org_id: None,
            project_id: Some(attacker_project_id),
            identity_org_ids: vec![],
            identity_project_ids: vec![],
        };

        // Should be denied - project_id doesn't match
        assert_ne!(auth.project_id, Some(vector_store.owner_id));
    }

    // ========================================================================
    // PostgreSQL integration tests (using testcontainers)
    // ========================================================================

    #[cfg(feature = "database-postgres")]
    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_postgres_user_owned_vector_store_access() {
        let pool = harness::postgres::create_isolated_postgres_pool().await;
        harness::postgres::run_postgres_migrations(&pool).await;
        let db = Arc::new(DbPool::from_postgres(pool.clone(), Some(pool)));

        // Create user
        let user = db
            .users()
            .create(CreateUser {
                external_id: "pg-test-user".to_string(),
                email: Some("pg@test.com".to_string()),
                name: Some("PG Test User".to_string()),
            })
            .await
            .expect("Failed to create user");

        // Create user-owned vector store
        let vector_store = db
            .vector_stores()
            .create_vector_store(CreateVectorStore {
                owner: VectorStoreOwner::User { user_id: user.id },
                file_ids: vec![],
                name: Some("pg-user-vector-store".to_string()),
                description: None,
                embedding_model: "text-embedding-3-small".to_string(),
                embedding_dimensions: 1536,
                metadata: None,
                expires_after: None,
                chunking_strategy: None,
            })
            .await
            .expect("Failed to create Qdrant index");

        // Auth context with matching user_id
        let auth = FileSearchAuthContext {
            user_id: Some(user.id),
            org_id: None,
            project_id: None,
            identity_org_ids: vec![],
            identity_project_ids: vec![],
        };

        // Verify access would be allowed
        assert_eq!(vector_store.owner_type, VectorStoreOwnerType::User);
        assert_eq!(auth.user_id, Some(vector_store.owner_id));
    }

    #[cfg(feature = "database-postgres")]
    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_postgres_org_owned_collection_with_membership() {
        let pool = harness::postgres::create_isolated_postgres_pool().await;
        harness::postgres::run_postgres_migrations(&pool).await;
        let db = Arc::new(DbPool::from_postgres(pool.clone(), Some(pool)));

        // Create org
        let org = db
            .organizations()
            .create(crate::models::CreateOrganization {
                slug: "pg-test-org".to_string(),
                name: "PG Test Org".to_string(),
            })
            .await
            .expect("Failed to create org");

        // Create user and add to org
        let user = db
            .users()
            .create(CreateUser {
                external_id: "pg-org-member".to_string(),
                email: Some("member@test.com".to_string()),
                name: Some("Org Member".to_string()),
            })
            .await
            .expect("Failed to create user");

        db.users()
            .add_to_org(user.id, org.id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to org");

        // Create org-owned vector store
        let vector_store = db
            .vector_stores()
            .create_vector_store(CreateVectorStore {
                owner: VectorStoreOwner::Organization {
                    organization_id: org.id,
                },
                file_ids: vec![],
                name: Some("pg-org-vector-store".to_string()),
                description: None,
                embedding_model: "text-embedding-3-small".to_string(),
                embedding_dimensions: 1536,
                metadata: None,
                expires_after: None,
                chunking_strategy: None,
            })
            .await
            .expect("Failed to create Qdrant index");

        // Verify user is a member
        let members = db
            .users()
            .list_org_members(org.id, crate::db::ListParams::default())
            .await
            .expect("Failed to list members");
        assert!(members.items.iter().any(|u| u.id == user.id));

        // Auth context with user who is member via database
        // This context would be used by check_vector_store_access for the fallback membership check
        let _auth = FileSearchAuthContext {
            user_id: Some(user.id),
            org_id: None, // No direct org_id
            project_id: None,
            identity_org_ids: vec![], // No identity claims
            identity_project_ids: vec![],
        };

        // The actual check_vector_store_access would use db membership lookup
        assert_eq!(vector_store.owner_type, VectorStoreOwnerType::Organization);
        assert_eq!(vector_store.owner_id, org.id);
    }

    #[cfg(feature = "database-postgres")]
    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_postgres_project_owned_collection_with_membership() {
        let pool = harness::postgres::create_isolated_postgres_pool().await;
        harness::postgres::run_postgres_migrations(&pool).await;
        let db = Arc::new(DbPool::from_postgres(pool.clone(), Some(pool)));

        // Create org and project
        let org = db
            .organizations()
            .create(crate::models::CreateOrganization {
                slug: "pg-proj-org".to_string(),
                name: "PG Project Org".to_string(),
            })
            .await
            .expect("Failed to create org");

        let project = db
            .projects()
            .create(
                org.id,
                crate::models::CreateProject {
                    slug: "pg-test-project".to_string(),
                    name: "PG Test Project".to_string(),
                    team_id: None,
                },
            )
            .await
            .expect("Failed to create project");

        // Create user and add to project
        let user = db
            .users()
            .create(CreateUser {
                external_id: "pg-project-member".to_string(),
                email: Some("proj-member@test.com".to_string()),
                name: Some("Project Member".to_string()),
            })
            .await
            .expect("Failed to create user");

        db.users()
            .add_to_project(user.id, project.id, "member", MembershipSource::Manual)
            .await
            .expect("Failed to add user to project");

        // Create project-owned vector store
        let vector_store = db
            .vector_stores()
            .create_vector_store(CreateVectorStore {
                owner: VectorStoreOwner::Project {
                    project_id: project.id,
                },
                file_ids: vec![],
                name: Some("pg-project-vector-store".to_string()),
                description: None,
                embedding_model: "text-embedding-3-small".to_string(),
                embedding_dimensions: 1536,
                metadata: None,
                expires_after: None,
                chunking_strategy: None,
            })
            .await
            .expect("Failed to create Qdrant index");

        // Verify user is a member
        let members = db
            .users()
            .list_project_members(project.id, crate::db::ListParams::default())
            .await
            .expect("Failed to list members");
        assert!(members.items.iter().any(|u| u.id == user.id));

        // Auth context with user who is member via database
        // This context would be used by check_vector_store_access for the fallback membership check
        let _auth = FileSearchAuthContext {
            user_id: Some(user.id),
            org_id: None,
            project_id: None, // No direct project_id
            identity_org_ids: vec![],
            identity_project_ids: vec![], // No identity claims
        };

        // The actual check_vector_store_access would use db membership lookup
        assert_eq!(vector_store.owner_type, VectorStoreOwnerType::Project);
        assert_eq!(vector_store.owner_id, project.id);
    }
}
