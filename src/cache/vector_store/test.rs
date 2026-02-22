//! Test vector store implementation for unit testing.
//!
//! This implementation provides a no-op vector store that can be used in tests
//! without requiring external services like PostgreSQL or Qdrant. It returns
//! empty results for all operations, which is sufficient for testing code paths
//! that just need a configured `FileSearchService`.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use uuid::Uuid;

use super::{
    ChunkFilter, ChunkSearchResult, ChunkWithEmbedding, HybridSearchConfig, StoredChunk,
    VectorBackend, VectorMetadata, VectorSearchResult, VectorStoreResult,
};

/// Test vector store that returns no-op/empty results for all operations.
///
/// This is useful for testing code paths that require a `FileSearchService`
/// to be configured, but don't actually need vector operations to work.
/// For example, testing the `POST /v1/vector_stores/{id}/files` endpoint
/// where we just need the embedding model compatibility check to pass.
pub struct TestVectorStore {
    dimensions: usize,
}

impl TestVectorStore {
    /// Create a new test vector store with the given dimensions.
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }
}

#[async_trait]
impl VectorBackend for TestVectorStore {
    async fn store(
        &self,
        _id: &str,
        _embedding: &[f64],
        _metadata: VectorMetadata,
        _ttl: Duration,
    ) -> VectorStoreResult<()> {
        Ok(())
    }

    async fn search(
        &self,
        _embedding: &[f64],
        _limit: usize,
        _threshold: f64,
        _model_filter: Option<&str>,
    ) -> VectorStoreResult<Vec<VectorSearchResult>> {
        Ok(vec![])
    }

    async fn delete(&self, _id: &str) -> VectorStoreResult<()> {
        Ok(())
    }

    async fn cleanup_expired(&self) -> VectorStoreResult<usize> {
        Ok(0)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    async fn health_check(&self) -> VectorStoreResult<()> {
        Ok(())
    }

    // RAG VectorStore Chunk Operations

    async fn store_chunks(&self, _chunks: Vec<ChunkWithEmbedding>) -> VectorStoreResult<()> {
        Ok(())
    }

    async fn get_chunks_by_file(&self, _file_id: Uuid) -> VectorStoreResult<Vec<StoredChunk>> {
        Ok(vec![])
    }

    async fn delete_chunks_by_file(&self, _file_id: Uuid) -> VectorStoreResult<u64> {
        Ok(0)
    }

    async fn delete_chunks_by_file_and_vector_store(
        &self,
        _file_id: Uuid,
        _vector_store_id: Uuid,
    ) -> VectorStoreResult<u64> {
        Ok(0)
    }

    async fn delete_chunks_by_file_and_vector_store_except_version(
        &self,
        _file_id: Uuid,
        _vector_store_id: Uuid,
        _keep_version: Uuid,
    ) -> VectorStoreResult<u64> {
        Ok(0)
    }

    async fn delete_chunks_by_vector_store(
        &self,
        _vector_store_id: Uuid,
    ) -> VectorStoreResult<u64> {
        Ok(0)
    }

    async fn search_vector_store(
        &self,
        _vector_store_id: Uuid,
        _embedding: &[f64],
        _limit: usize,
        _threshold: f64,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        Ok(vec![])
    }

    async fn search_vector_stores(
        &self,
        _vector_store_ids: &[Uuid],
        _embedding: &[f64],
        _limit: usize,
        _threshold: f64,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        Ok(vec![])
    }

    async fn keyword_search_vector_store(
        &self,
        _vector_store_id: Uuid,
        _query: &str,
        _limit: usize,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        Ok(vec![])
    }

    async fn keyword_search_vector_stores(
        &self,
        _vector_store_ids: &[Uuid],
        _query: &str,
        _limit: usize,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        Ok(vec![])
    }

    async fn hybrid_search_vector_store(
        &self,
        _vector_store_id: Uuid,
        _query: &str,
        _embedding: &[f64],
        _limit: usize,
        _config: HybridSearchConfig,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        Ok(vec![])
    }

    async fn hybrid_search_vector_stores(
        &self,
        _vector_store_ids: &[Uuid],
        _query: &str,
        _embedding: &[f64],
        _limit: usize,
        _config: HybridSearchConfig,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        Ok(vec![])
    }
}

/// A test vector store that can return configurable mock search results.
///
/// Unlike `TestVectorStore` which always returns empty results, this
/// implementation allows tests to inject mock search results that will
/// be returned by search operations. This is useful for testing the
/// full search API flow including result transformation.
///
/// # Example
///
/// ```ignore
/// let mock_results = vec![
///     ChunkSearchResult {
///         chunk_id: Uuid::new_v4(),
///         vector_store_id: vs_uuid,
///         file_id: file_uuid,
///         chunk_index: 0,
///         content: "Test content".to_string(),
///         score: 0.95,
///         metadata: None,
///     },
/// ];
/// let store = MockableTestVectorStore::new(1536).with_search_results(mock_results);
/// ```
pub struct MockableTestVectorStore {
    dimensions: usize,
    /// Mock results to return from search operations
    mock_search_results: Arc<Mutex<Vec<ChunkSearchResult>>>,
}

impl MockableTestVectorStore {
    /// Create a new mockable test vector store with the given dimensions.
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            mock_search_results: Arc::new(Mutex::new(vec![])),
        }
    }

    /// Set the mock search results that will be returned by search operations.
    pub fn with_search_results(self, results: Vec<ChunkSearchResult>) -> Self {
        *self.mock_search_results.lock().unwrap() = results;
        self
    }

    /// Get a clone of the mock results handle for setting results after construction.
    pub fn mock_results_handle(&self) -> Arc<Mutex<Vec<ChunkSearchResult>>> {
        self.mock_search_results.clone()
    }
}

#[async_trait]
impl VectorBackend for MockableTestVectorStore {
    async fn store(
        &self,
        _id: &str,
        _embedding: &[f64],
        _metadata: VectorMetadata,
        _ttl: Duration,
    ) -> VectorStoreResult<()> {
        Ok(())
    }

    async fn search(
        &self,
        _embedding: &[f64],
        _limit: usize,
        _threshold: f64,
        _model_filter: Option<&str>,
    ) -> VectorStoreResult<Vec<VectorSearchResult>> {
        Ok(vec![])
    }

    async fn delete(&self, _id: &str) -> VectorStoreResult<()> {
        Ok(())
    }

    async fn cleanup_expired(&self) -> VectorStoreResult<usize> {
        Ok(0)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    async fn health_check(&self) -> VectorStoreResult<()> {
        Ok(())
    }

    async fn store_chunks(&self, _chunks: Vec<ChunkWithEmbedding>) -> VectorStoreResult<()> {
        Ok(())
    }

    async fn get_chunks_by_file(&self, _file_id: Uuid) -> VectorStoreResult<Vec<StoredChunk>> {
        Ok(vec![])
    }

    async fn delete_chunks_by_file(&self, _file_id: Uuid) -> VectorStoreResult<u64> {
        Ok(0)
    }

    async fn delete_chunks_by_file_and_vector_store(
        &self,
        _file_id: Uuid,
        _vector_store_id: Uuid,
    ) -> VectorStoreResult<u64> {
        Ok(0)
    }

    async fn delete_chunks_by_file_and_vector_store_except_version(
        &self,
        _file_id: Uuid,
        _vector_store_id: Uuid,
        _keep_version: Uuid,
    ) -> VectorStoreResult<u64> {
        Ok(0)
    }

    async fn delete_chunks_by_vector_store(
        &self,
        _vector_store_id: Uuid,
    ) -> VectorStoreResult<u64> {
        Ok(0)
    }

    async fn search_vector_store(
        &self,
        _vector_store_id: Uuid,
        _embedding: &[f64],
        limit: usize,
        _threshold: f64,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        let results = self.mock_search_results.lock().unwrap();
        Ok(results.iter().take(limit).cloned().collect())
    }

    async fn search_vector_stores(
        &self,
        _vector_store_ids: &[Uuid],
        _embedding: &[f64],
        limit: usize,
        _threshold: f64,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        let results = self.mock_search_results.lock().unwrap();
        Ok(results.iter().take(limit).cloned().collect())
    }

    async fn keyword_search_vector_store(
        &self,
        _vector_store_id: Uuid,
        _query: &str,
        limit: usize,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        let results = self.mock_search_results.lock().unwrap();
        Ok(results.iter().take(limit).cloned().collect())
    }

    async fn keyword_search_vector_stores(
        &self,
        _vector_store_ids: &[Uuid],
        _query: &str,
        limit: usize,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        let results = self.mock_search_results.lock().unwrap();
        Ok(results.iter().take(limit).cloned().collect())
    }

    async fn hybrid_search_vector_store(
        &self,
        _vector_store_id: Uuid,
        _query: &str,
        _embedding: &[f64],
        limit: usize,
        _config: HybridSearchConfig,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        let results = self.mock_search_results.lock().unwrap();
        Ok(results.iter().take(limit).cloned().collect())
    }

    async fn hybrid_search_vector_stores(
        &self,
        _vector_store_ids: &[Uuid],
        _query: &str,
        _embedding: &[f64],
        limit: usize,
        _config: HybridSearchConfig,
        _filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        let results = self.mock_search_results.lock().unwrap();
        Ok(results.iter().take(limit).cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vector_store_dimensions() {
        let store = TestVectorStore::new(1536);
        assert_eq!(store.dimensions(), 1536);
    }

    #[tokio::test]
    async fn test_vector_store_health_check() {
        let store = TestVectorStore::new(1536);
        assert!(store.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_vector_store_search_returns_empty() {
        let store = TestVectorStore::new(1536);
        let results = store.search(&[0.0; 1536], 10, 0.8, None).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_vector_store_store_succeeds() {
        let store = TestVectorStore::new(1536);
        let metadata = VectorMetadata {
            cache_key: "test".to_string(),
            model: "test-model".to_string(),
            organization_id: None,
            project_id: None,
            created_at: 0,
            ttl_secs: 3600,
        };
        assert!(
            store
                .store("id", &[0.0; 1536], metadata, Duration::from_secs(3600))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn test_mockable_vector_store_returns_empty_by_default() {
        let store = MockableTestVectorStore::new(1536);
        let results = store
            .search_vector_stores(&[Uuid::new_v4()], &[0.0; 1536], 10, 0.8, None)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_mockable_vector_store_returns_configured_results() {
        let vector_store_id = Uuid::new_v4();
        let file_id = Uuid::new_v4();
        let chunk_id = Uuid::new_v4();

        let mock_results = vec![ChunkSearchResult {
            chunk_id,
            vector_store_id,
            file_id,
            chunk_index: 0,
            content: "Test content".to_string(),
            score: 0.95,
            metadata: None,
        }];

        let store = MockableTestVectorStore::new(1536).with_search_results(mock_results);
        let results = store
            .search_vector_stores(&[vector_store_id], &[0.0; 1536], 10, 0.8, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk_id, chunk_id);
        assert_eq!(results[0].content, "Test content");
        assert_eq!(results[0].score, 0.95);
    }

    #[tokio::test]
    async fn test_mockable_vector_store_respects_limit() {
        let vector_store_id = Uuid::new_v4();
        let file_id = Uuid::new_v4();

        let mock_results: Vec<ChunkSearchResult> = (0..5)
            .map(|i| ChunkSearchResult {
                chunk_id: Uuid::new_v4(),
                vector_store_id,
                file_id,
                chunk_index: i,
                content: format!("Content {}", i),
                score: 0.9 - (i as f64 * 0.1),
                metadata: None,
            })
            .collect();

        let store = MockableTestVectorStore::new(1536).with_search_results(mock_results);

        // Request only 2 results
        let results = store
            .search_vector_stores(&[vector_store_id], &[0.0; 1536], 2, 0.8, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_mockable_vector_store_handle_allows_late_configuration() {
        let store = MockableTestVectorStore::new(1536);
        let handle = store.mock_results_handle();

        // Initially empty
        let results = store
            .search_vector_stores(&[Uuid::new_v4()], &[0.0; 1536], 10, 0.8, None)
            .await
            .unwrap();
        assert!(results.is_empty());

        // Configure mock results via handle
        let vector_store_id = Uuid::new_v4();
        *handle.lock().unwrap() = vec![ChunkSearchResult {
            chunk_id: Uuid::new_v4(),
            vector_store_id,
            file_id: Uuid::new_v4(),
            chunk_index: 0,
            content: "Late configured".to_string(),
            score: 0.8,
            metadata: None,
        }];

        // Now returns the configured results
        let results = store
            .search_vector_stores(&[vector_store_id], &[0.0; 1536], 10, 0.8, None)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Late configured");
    }
}
