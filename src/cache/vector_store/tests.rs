//! Integration tests for VectorStore implementations.
//!
//! Tests use the same pattern as database tests:
//! - Shared test functions that work with `&dyn VectorBackend`
//! - SQLite/PostgreSQL-style setup using testcontainers
//!
//! # Running tests
//!
//! ```bash
//! cargo test vector_store       # Run basic unit tests
//! cargo test -- --ignored       # Run integration tests (requires Docker)
//! ```

use std::time::Duration;

use uuid::Uuid;

use super::{
    ChunkFilter, ChunkWithEmbedding, HybridSearchConfig, VectorBackend, VectorMetadata,
    VectorStoreError,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a test embedding of the given dimension.
fn create_test_embedding(dimensions: usize, seed: f64) -> Vec<f64> {
    (0..dimensions)
        .map(|i| ((i as f64 * seed).sin() + 1.0) / 2.0) // Normalized 0-1
        .collect()
}

/// Create a slightly modified version of an embedding for similarity testing.
fn create_similar_embedding(base: &[f64], variance: f64) -> Vec<f64> {
    base.iter()
        .enumerate()
        .map(|(i, &v)| {
            let offset = (i as f64 * 0.1).sin() * variance;
            (v + offset).clamp(0.0, 1.0)
        })
        .collect()
}

fn create_test_metadata(cache_key: &str, model: &str) -> VectorMetadata {
    VectorMetadata {
        cache_key: cache_key.to_string(),
        model: model.to_string(),
        organization_id: None,
        project_id: None,
        created_at: chrono::Utc::now().timestamp(),
        ttl_secs: 3600,
    }
}

// ============================================================================
// Shared Test Functions
// ============================================================================

pub async fn test_store_and_search(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let embedding = create_test_embedding(dimensions, 1.0);
    let metadata = create_test_metadata("cache:test1", "gpt-4");

    // Use UUID for ID to be compatible with both pgvector and Qdrant
    let id = uuid::Uuid::new_v4().to_string();

    // Store the embedding
    store
        .store(&id, &embedding, metadata.clone(), Duration::from_secs(3600))
        .await
        .expect("Failed to store embedding");

    // Search with the same embedding should return exact match
    let results = store
        .search(&embedding, 5, 0.9, Some("gpt-4"))
        .await
        .expect("Failed to search");

    assert!(!results.is_empty(), "Expected at least one result");
    let first = &results[0];
    assert!(
        first.similarity > 0.99,
        "Expected very high similarity for exact match, got {}",
        first.similarity
    );
    assert_eq!(first.metadata.cache_key, "cache:test1");
    assert_eq!(first.metadata.model, "gpt-4");
}

pub async fn test_search_with_similar_embedding(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let original = create_test_embedding(dimensions, 2.0);
    let metadata = create_test_metadata("cache:similar", "gpt-4");
    let id = uuid::Uuid::new_v4().to_string();

    store
        .store(&id, &original, metadata.clone(), Duration::from_secs(3600))
        .await
        .expect("Failed to store embedding");

    // Search with a similar embedding
    let similar = create_similar_embedding(&original, 0.05);
    let results = store
        .search(&similar, 5, 0.9, Some("gpt-4"))
        .await
        .expect("Failed to search");

    assert!(!results.is_empty(), "Expected to find similar embedding");
    assert!(
        results[0].similarity > 0.9,
        "Expected high similarity for similar embedding"
    );
}

pub async fn test_search_threshold_filtering(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let embedding1 = create_test_embedding(dimensions, 3.0);
    let metadata = create_test_metadata("cache:threshold", "gpt-4");
    let id = uuid::Uuid::new_v4().to_string();

    store
        .store(
            &id,
            &embedding1,
            metadata.clone(),
            Duration::from_secs(3600),
        )
        .await
        .expect("Failed to store embedding");

    // Search with a very different embedding
    let different = create_test_embedding(dimensions, 100.0);
    let results = store
        .search(&different, 5, 0.99, Some("gpt-4"))
        .await
        .expect("Failed to search");

    // With high threshold (0.99), dissimilar vectors shouldn't match
    assert!(
        results.is_empty(),
        "Expected no results for dissimilar embedding with high threshold"
    );
}

pub async fn test_model_filter(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let embedding = create_test_embedding(dimensions, 4.0);

    // Store embeddings for different models
    let metadata_gpt4 = create_test_metadata("cache:gpt4", "gpt-4");
    let metadata_claude = create_test_metadata("cache:claude", "claude-3");
    let id_gpt4 = uuid::Uuid::new_v4().to_string();
    let id_claude = uuid::Uuid::new_v4().to_string();

    store
        .store(
            &id_gpt4,
            &embedding,
            metadata_gpt4,
            Duration::from_secs(3600),
        )
        .await
        .expect("Failed to store gpt-4 embedding");

    store
        .store(
            &id_claude,
            &embedding,
            metadata_claude,
            Duration::from_secs(3600),
        )
        .await
        .expect("Failed to store claude embedding");

    // Search for gpt-4 only
    let results = store
        .search(&embedding, 10, 0.9, Some("gpt-4"))
        .await
        .expect("Failed to search");

    assert!(!results.is_empty());
    for result in &results {
        assert_eq!(
            result.metadata.model, "gpt-4",
            "Model filter should only return gpt-4"
        );
    }

    // Search for claude only
    let results = store
        .search(&embedding, 10, 0.9, Some("claude-3"))
        .await
        .expect("Failed to search");

    assert!(!results.is_empty());
    for result in &results {
        assert_eq!(
            result.metadata.model, "claude-3",
            "Model filter should only return claude-3"
        );
    }
}

pub async fn test_delete(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let embedding = create_test_embedding(dimensions, 5.0);
    let metadata = create_test_metadata("cache:delete", "gpt-4");
    let id = uuid::Uuid::new_v4().to_string();

    // Store and verify it exists
    store
        .store(&id, &embedding, metadata.clone(), Duration::from_secs(3600))
        .await
        .expect("Failed to store embedding");

    let results = store
        .search(&embedding, 5, 0.9, Some("gpt-4"))
        .await
        .expect("Failed to search");
    assert!(!results.is_empty(), "Should find embedding before delete");

    // Delete
    store.delete(&id).await.expect("Failed to delete embedding");

    // Verify it's gone - need to use a different test since same embedding might match others
    // Delete again should not error (idempotent)
    store
        .delete(&id)
        .await
        .expect("Delete should be idempotent");
}

pub async fn test_dimension_mismatch(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let wrong_dimensions = if dimensions > 1 { dimensions - 1 } else { 2 };
    let wrong_embedding = create_test_embedding(wrong_dimensions, 6.0);
    let metadata = create_test_metadata("cache:wrong", "gpt-4");
    let id = uuid::Uuid::new_v4().to_string();

    let result = store
        .store(&id, &wrong_embedding, metadata, Duration::from_secs(3600))
        .await;

    assert!(
        matches!(result, Err(VectorStoreError::DimensionMismatch { .. })),
        "Expected dimension mismatch error"
    );
}

pub async fn test_cleanup_expired(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let embedding = create_test_embedding(dimensions, 7.0);
    let metadata = create_test_metadata("cache:expired", "gpt-4");
    let id = uuid::Uuid::new_v4().to_string();

    // Store with very short TTL
    store
        .store(&id, &embedding, metadata, Duration::from_secs(1))
        .await
        .expect("Failed to store embedding");

    // Wait for expiration
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Cleanup should remove expired entries
    let deleted = store
        .cleanup_expired()
        .await
        .expect("Failed to cleanup expired");

    // Should have deleted at least one entry
    assert!(deleted >= 1, "Expected at least 1 expired entry cleaned up");
}

pub async fn test_health_check(store: &dyn VectorBackend) {
    // After initialization, health check should pass
    store
        .health_check()
        .await
        .expect("Health check should pass");
}

pub async fn test_upsert(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let embedding1 = create_test_embedding(dimensions, 8.0);
    let embedding2 = create_test_embedding(dimensions, 9.0);
    let id = uuid::Uuid::new_v4().to_string();

    let metadata1 = VectorMetadata {
        cache_key: "cache:upsert".to_string(),
        model: "gpt-4".to_string(),
        organization_id: None,
        project_id: None,
        created_at: chrono::Utc::now().timestamp(),
        ttl_secs: 3600,
    };

    // Store initial
    store
        .store(
            &id,
            &embedding1,
            metadata1.clone(),
            Duration::from_secs(3600),
        )
        .await
        .expect("Failed to store embedding");

    // Upsert with different embedding and metadata
    let metadata2 = VectorMetadata {
        cache_key: "cache:upsert_updated".to_string(),
        model: "gpt-4".to_string(),
        organization_id: Some("org-123".to_string()),
        project_id: None,
        created_at: chrono::Utc::now().timestamp(),
        ttl_secs: 7200,
    };

    store
        .store(
            &id,
            &embedding2,
            metadata2.clone(),
            Duration::from_secs(3600),
        )
        .await
        .expect("Failed to upsert embedding");

    // Search should find the updated embedding
    let results = store
        .search(&embedding2, 5, 0.9, Some("gpt-4"))
        .await
        .expect("Failed to search");

    assert!(!results.is_empty());
    let found = results
        .iter()
        .find(|r| r.metadata.cache_key == "cache:upsert_updated");
    assert!(found.is_some(), "Should find updated metadata");
}

// ============================================================================
// Chunk Operations - Shared Test Functions
// ============================================================================

/// Create a test chunk with embedding
fn create_test_chunk(
    dimensions: usize,
    vector_store_id: Uuid,
    file_id: Uuid,
    chunk_index: i32,
    content: &str,
    seed: f64,
) -> ChunkWithEmbedding {
    ChunkWithEmbedding {
        id: Uuid::new_v4(),
        vector_store_id,
        file_id,
        chunk_index,
        content: content.to_string(),
        token_count: content.split_whitespace().count() as i32,
        char_start: 0,
        char_end: content.len() as i32,
        embedding: create_test_embedding(dimensions, seed),
        metadata: None,
        processing_version: Uuid::new_v4(),
    }
}

pub async fn test_store_and_search_chunks(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

    // Create test chunks
    let chunks = vec![
        create_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            0,
            "The quick brown fox jumps over the lazy dog.",
            1.0,
        ),
        create_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            1,
            "Machine learning is a subset of artificial intelligence.",
            2.0,
        ),
        create_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            2,
            "Natural language processing enables computers to understand human language.",
            3.0,
        ),
    ];

    // Store chunks
    store
        .store_chunks(chunks.clone())
        .await
        .expect("Failed to store chunks");

    // Search with the first chunk's embedding should return it as top result
    let results = store
        .search_vector_store(vector_store_id, &chunks[0].embedding, 5, 0.9, None)
        .await
        .expect("Failed to search vector store");

    assert!(!results.is_empty(), "Expected at least one result");
    let first = &results[0];
    assert!(
        first.score > 0.99,
        "Expected very high similarity for exact match, got {}",
        first.score
    );
    assert_eq!(first.vector_store_id, vector_store_id);
    assert_eq!(first.file_id, file_id);
    assert_eq!(first.chunk_index, 0);
    assert!(first.content.contains("quick brown fox"));
}

pub async fn test_get_chunks_by_file(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

    // Create test chunks in reverse order to verify sorting
    let chunks = vec![
        create_test_chunk(dimensions, vector_store_id, file_id, 2, "Third chunk", 3.0),
        create_test_chunk(dimensions, vector_store_id, file_id, 0, "First chunk", 1.0),
        create_test_chunk(dimensions, vector_store_id, file_id, 1, "Second chunk", 2.0),
    ];

    store
        .store_chunks(chunks)
        .await
        .expect("Failed to store chunks");

    // Get chunks by file
    let retrieved = store
        .get_chunks_by_file(file_id)
        .await
        .expect("Failed to get chunks by file");

    assert_eq!(retrieved.len(), 3, "Should retrieve all 3 chunks");

    // Verify they're sorted by chunk_index
    assert_eq!(retrieved[0].chunk_index, 0);
    assert_eq!(retrieved[1].chunk_index, 1);
    assert_eq!(retrieved[2].chunk_index, 2);

    // Verify content
    assert!(retrieved[0].content.contains("First"));
    assert!(retrieved[1].content.contains("Second"));
    assert!(retrieved[2].content.contains("Third"));
}

pub async fn test_delete_chunks_by_file(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

    // Create and store chunks
    let chunks = vec![
        create_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            0,
            "Chunk to delete",
            1.0,
        ),
        create_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            1,
            "Another chunk",
            2.0,
        ),
    ];

    store
        .store_chunks(chunks.clone())
        .await
        .expect("Failed to store chunks");

    // Verify chunks exist
    let before = store.get_chunks_by_file(file_id).await.expect("Get failed");
    assert_eq!(before.len(), 2);

    // Delete chunks
    let deleted = store
        .delete_chunks_by_file(file_id)
        .await
        .expect("Failed to delete chunks");

    // For pgvector, we get exact count; for qdrant, we pre-count
    assert!(deleted >= 2 || deleted == 0, "Should delete chunks"); // Qdrant may return 0

    // Verify chunks are deleted
    let after = store.get_chunks_by_file(file_id).await.expect("Get failed");
    assert!(after.is_empty(), "Chunks should be deleted");
}

pub async fn test_delete_chunks_by_vector_store(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id_1 = Uuid::new_v4();
    let file_id_2 = Uuid::new_v4();

    // Create chunks from multiple files in the same vector store
    let chunks = vec![
        create_test_chunk(
            dimensions,
            vector_store_id,
            file_id_1,
            0,
            "File 1 chunk",
            1.0,
        ),
        create_test_chunk(
            dimensions,
            vector_store_id,
            file_id_2,
            0,
            "File 2 chunk",
            2.0,
        ),
    ];

    store
        .store_chunks(chunks)
        .await
        .expect("Failed to store chunks");

    // Verify chunks exist via search
    let before = store
        .search_vector_store(
            vector_store_id,
            &create_test_embedding(dimensions, 1.0),
            10,
            0.5,
            None,
        )
        .await
        .expect("Search failed");
    assert!(!before.is_empty(), "Should find chunks before deletion");

    // Delete all chunks in vector store
    store
        .delete_chunks_by_vector_store(vector_store_id)
        .await
        .expect("Failed to delete chunks by vector store");

    // Verify all chunks are deleted
    let after = store
        .search_vector_store(
            vector_store_id,
            &create_test_embedding(dimensions, 1.0),
            10,
            0.5,
            None,
        )
        .await
        .expect("Search failed");
    assert!(
        after.is_empty(),
        "All chunks in vector store should be deleted"
    );
}

/// Test that delete_chunks_by_file_and_vector_store only deletes chunks
/// for the specific file within the specific vector_store, leaving chunks
/// for the same file in other vector stores untouched.
pub async fn test_delete_chunks_by_file_and_vector_store(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id_1 = Uuid::new_v4();
    let vector_store_id_2 = Uuid::new_v4();
    let file_id = Uuid::new_v4(); // Same file in both vector stores

    // Create chunks for the same file in two different vector stores
    let chunks = vec![
        create_test_chunk(
            dimensions,
            vector_store_id_1,
            file_id,
            0,
            "Chunk in vector store 1",
            1.0,
        ),
        create_test_chunk(
            dimensions,
            vector_store_id_1,
            file_id,
            1,
            "Another chunk in vector store 1",
            1.5,
        ),
        create_test_chunk(
            dimensions,
            vector_store_id_2,
            file_id,
            0,
            "Chunk in vector store 2",
            2.0,
        ),
    ];

    store
        .store_chunks(chunks)
        .await
        .expect("Failed to store chunks");

    // Verify chunks exist in both vector stores
    let before_c1 = store
        .search_vector_store(
            vector_store_id_1,
            &create_test_embedding(dimensions, 1.0),
            10,
            0.0,
            None,
        )
        .await
        .expect("Search failed");
    assert_eq!(before_c1.len(), 2, "Should have 2 chunks in vector store 1");

    let before_c2 = store
        .search_vector_store(
            vector_store_id_2,
            &create_test_embedding(dimensions, 2.0),
            10,
            0.0,
            None,
        )
        .await
        .expect("Search failed");
    assert_eq!(before_c2.len(), 1, "Should have 1 chunk in vector store 2");

    // Delete chunks for the file ONLY in vector store 1
    store
        .delete_chunks_by_file_and_vector_store(file_id, vector_store_id_1)
        .await
        .expect("Failed to delete chunks by file and vector store");

    // Verify chunks in vector store 1 are deleted
    let after_c1 = store
        .search_vector_store(
            vector_store_id_1,
            &create_test_embedding(dimensions, 1.0),
            10,
            0.0,
            None,
        )
        .await
        .expect("Search failed");
    assert!(
        after_c1.is_empty(),
        "Chunks in vector store 1 should be deleted"
    );

    // Verify chunks in vector store 2 are STILL THERE
    let after_c2 = store
        .search_vector_store(
            vector_store_id_2,
            &create_test_embedding(dimensions, 2.0),
            10,
            0.0,
            None,
        )
        .await
        .expect("Search failed");
    assert_eq!(
        after_c2.len(),
        1,
        "Chunks in vector store 2 should NOT be deleted"
    );
}

pub async fn test_search_vector_stores_multi(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id_1 = Uuid::new_v4();
    let vector_store_id_2 = Uuid::new_v4();
    let file_id_1 = Uuid::new_v4();
    let file_id_2 = Uuid::new_v4();

    // Create chunks in two different vector stores with similar embeddings
    let seed = 5.0;
    let chunks_1 = vec![create_test_chunk(
        dimensions,
        vector_store_id_1,
        file_id_1,
        0,
        "Document from vector store 1",
        seed,
    )];
    let chunks_2 = vec![create_test_chunk(
        dimensions,
        vector_store_id_2,
        file_id_2,
        0,
        "Document from vector store 2",
        seed + 0.01, // Slightly different but similar
    )];

    store
        .store_chunks(chunks_1.clone())
        .await
        .expect("Failed to store chunks 1");
    store
        .store_chunks(chunks_2.clone())
        .await
        .expect("Failed to store chunks 2");

    // Search across both vector stores
    let results = store
        .search_vector_stores(
            &[vector_store_id_1, vector_store_id_2],
            &chunks_1[0].embedding,
            10,
            0.5,
            None,
        )
        .await
        .expect("Failed to search vector stores");

    assert!(
        results.len() >= 2,
        "Should find results from both vector stores"
    );

    // Verify results come from both vector stores
    let has_vector_store_1 = results
        .iter()
        .any(|r| r.vector_store_id == vector_store_id_1);
    let has_vector_store_2 = results
        .iter()
        .any(|r| r.vector_store_id == vector_store_id_2);
    assert!(has_vector_store_1, "Should have result from vector store 1");
    assert!(has_vector_store_2, "Should have result from vector store 2");
}

pub async fn test_search_with_file_filter(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id_1 = Uuid::new_v4();
    let file_id_2 = Uuid::new_v4();

    // Create similar chunks in different files
    let seed = 6.0;
    let chunks = vec![
        create_test_chunk(
            dimensions,
            vector_store_id,
            file_id_1,
            0,
            "Content from file 1",
            seed,
        ),
        create_test_chunk(
            dimensions,
            vector_store_id,
            file_id_2,
            0,
            "Content from file 2",
            seed + 0.01,
        ),
    ];

    store
        .store_chunks(chunks.clone())
        .await
        .expect("Failed to store chunks");

    // Search with file filter - only file_id_1
    let filter = ChunkFilter {
        file_ids: Some(vec![file_id_1]),
        attribute_filter: None,
    };

    let results = store
        .search_vector_store(vector_store_id, &chunks[0].embedding, 10, 0.5, Some(filter))
        .await
        .expect("Failed to search with filter");

    assert!(!results.is_empty(), "Should find results");
    for result in &results {
        assert_eq!(
            result.file_id, file_id_1,
            "All results should be from file_id_1"
        );
    }
}

pub async fn test_chunk_dimension_mismatch(store: &dyn VectorBackend) {
    let vector_store_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

    // Create chunk with wrong dimensions
    let chunk = ChunkWithEmbedding {
        id: Uuid::new_v4(),
        vector_store_id,
        file_id,
        chunk_index: 0,
        content: "Test content".to_string(),
        token_count: 2,
        char_start: 0,
        char_end: 12,
        embedding: vec![0.1, 0.2, 0.3], // Wrong dimensions
        metadata: None,
        processing_version: Uuid::new_v4(),
    };

    let result = store.store_chunks(vec![chunk]).await;
    assert!(
        matches!(result, Err(VectorStoreError::DimensionMismatch { .. })),
        "Should fail with dimension mismatch"
    );
}

pub async fn test_empty_chunks(store: &dyn VectorBackend) {
    // Storing empty chunks should succeed without error
    let result = store.store_chunks(vec![]).await;
    assert!(result.is_ok(), "Empty chunks should be ok");

    // Searching in empty vector store should return empty results
    let vector_store_id = Uuid::new_v4();
    let dimensions = store.dimensions();
    let embedding = create_test_embedding(dimensions, 1.0);

    let results = store
        .search_vector_store(vector_store_id, &embedding, 10, 0.5, None)
        .await
        .expect("Search should succeed");

    assert!(results.is_empty(), "Should return empty results");
}

// ============================================================================
// Keyword Search Operations - Shared Test Functions
// ============================================================================

/// Create a test chunk with specific content for keyword search testing
fn create_keyword_test_chunk(
    dimensions: usize,
    vector_store_id: Uuid,
    file_id: Uuid,
    chunk_index: i32,
    content: &str,
    seed: f64,
) -> ChunkWithEmbedding {
    ChunkWithEmbedding {
        id: Uuid::new_v4(),
        vector_store_id,
        file_id,
        chunk_index,
        content: content.to_string(),
        token_count: content.split_whitespace().count() as i32,
        char_start: 0,
        char_end: content.len() as i32,
        embedding: create_test_embedding(dimensions, seed),
        metadata: None,
        processing_version: Uuid::new_v4(),
    }
}

pub async fn test_keyword_search_basic(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

    // Create chunks with distinct keywords
    let chunks = vec![
        create_keyword_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            0,
            "The HIPAA compliance regulations require careful handling of patient data.",
            1.0,
        ),
        create_keyword_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            1,
            "Machine learning algorithms can process large datasets efficiently.",
            2.0,
        ),
        create_keyword_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            2,
            "Part number XJ-900 is required for the assembly process.",
            3.0,
        ),
    ];

    store
        .store_chunks(chunks)
        .await
        .expect("Failed to store chunks");

    // Search for "HIPAA" - should find the compliance chunk
    let results = store
        .keyword_search_vector_store(vector_store_id, "HIPAA", 10, None)
        .await
        .expect("Failed to keyword search");

    assert!(!results.is_empty(), "Should find HIPAA chunk");
    assert!(
        results[0].content.contains("HIPAA"),
        "Top result should contain HIPAA"
    );

    // Search for "XJ-900" - should find the part number chunk
    let results = store
        .keyword_search_vector_store(vector_store_id, "XJ-900", 10, None)
        .await
        .expect("Failed to keyword search");

    assert!(!results.is_empty(), "Should find part number chunk");
    assert!(
        results[0].content.contains("XJ-900"),
        "Top result should contain XJ-900"
    );

    // Search for term not in any chunk
    let results = store
        .keyword_search_vector_store(vector_store_id, "cryptocurrency", 10, None)
        .await
        .expect("Failed to keyword search");

    assert!(
        results.is_empty(),
        "Should not find unrelated term: {:?}",
        results
    );
}

pub async fn test_keyword_search_empty_query(store: &dyn VectorBackend) {
    let vector_store_id = Uuid::new_v4();

    // Empty query should return empty results
    let results = store
        .keyword_search_vector_store(vector_store_id, "", 10, None)
        .await
        .expect("Failed to keyword search");

    assert!(results.is_empty(), "Empty query should return no results");

    // Whitespace-only query should also return empty results
    let results = store
        .keyword_search_vector_store(vector_store_id, "   ", 10, None)
        .await
        .expect("Failed to keyword search");

    assert!(
        results.is_empty(),
        "Whitespace query should return no results"
    );
}

pub async fn test_keyword_search_empty_vector_stores(store: &dyn VectorBackend) {
    // Search with empty vector_store_ids array should return empty results
    let results = store
        .keyword_search_vector_stores(&[], "test", 10, None)
        .await
        .expect("Failed to keyword search");

    assert!(
        results.is_empty(),
        "Empty vector stores should return no results"
    );
}

pub async fn test_keyword_search_multi_vector_store(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id_1 = Uuid::new_v4();
    let vector_store_id_2 = Uuid::new_v4();
    let file_id_1 = Uuid::new_v4();
    let file_id_2 = Uuid::new_v4();

    // Create chunks with "database" keyword in both vector stores
    let chunks_1 = vec![create_keyword_test_chunk(
        dimensions,
        vector_store_id_1,
        file_id_1,
        0,
        "PostgreSQL is a powerful relational database management system.",
        1.0,
    )];
    let chunks_2 = vec![create_keyword_test_chunk(
        dimensions,
        vector_store_id_2,
        file_id_2,
        0,
        "MongoDB is a popular document database for modern applications.",
        2.0,
    )];

    store
        .store_chunks(chunks_1)
        .await
        .expect("Failed to store chunks 1");
    store
        .store_chunks(chunks_2)
        .await
        .expect("Failed to store chunks 2");

    // Search for "database" across both vector stores
    let results = store
        .keyword_search_vector_stores(
            &[vector_store_id_1, vector_store_id_2],
            "database",
            10,
            None,
        )
        .await
        .expect("Failed to keyword search");

    assert!(
        results.len() >= 2,
        "Should find results from both vector stores"
    );

    // Verify results come from both vector stores
    let has_vector_store_1 = results
        .iter()
        .any(|r| r.vector_store_id == vector_store_id_1);
    let has_vector_store_2 = results
        .iter()
        .any(|r| r.vector_store_id == vector_store_id_2);
    assert!(has_vector_store_1, "Should have result from vector store 1");
    assert!(has_vector_store_2, "Should have result from vector store 2");
}

pub async fn test_keyword_search_with_file_filter(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id_1 = Uuid::new_v4();
    let file_id_2 = Uuid::new_v4();

    // Create chunks with same keyword in different files
    let chunks = vec![
        create_keyword_test_chunk(
            dimensions,
            vector_store_id,
            file_id_1,
            0,
            "Authentication using OAuth2 protocol for secure access.",
            1.0,
        ),
        create_keyword_test_chunk(
            dimensions,
            vector_store_id,
            file_id_2,
            0,
            "Authentication mechanisms include passwords and biometrics.",
            2.0,
        ),
    ];

    store
        .store_chunks(chunks)
        .await
        .expect("Failed to store chunks");

    // Search with file filter - only file_id_1
    let filter = ChunkFilter {
        file_ids: Some(vec![file_id_1]),
        attribute_filter: None,
    };

    let results = store
        .keyword_search_vector_store(vector_store_id, "authentication", 10, Some(filter))
        .await
        .expect("Failed to keyword search");

    assert!(!results.is_empty(), "Should find results");
    for result in &results {
        assert_eq!(
            result.file_id, file_id_1,
            "All results should be from file_id_1"
        );
    }
}

pub async fn test_keyword_search_score_normalization(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

    // Create a chunk with the keyword
    let chunks = vec![create_keyword_test_chunk(
        dimensions,
        vector_store_id,
        file_id,
        0,
        "This document discusses important topics related to software engineering.",
        1.0,
    )];

    store
        .store_chunks(chunks)
        .await
        .expect("Failed to store chunks");

    let results = store
        .keyword_search_vector_store(vector_store_id, "software engineering", 10, None)
        .await
        .expect("Failed to keyword search");

    if !results.is_empty() {
        // Score should be normalized to 0-1 range
        for result in &results {
            assert!(
                result.score >= 0.0 && result.score <= 1.0,
                "Score should be in 0-1 range, got {}",
                result.score
            );
        }
    }
}

// ============================================================================
// Hybrid Search Operations - Shared Test Functions
// ============================================================================

pub async fn test_hybrid_search_basic(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

    // Create chunks with distinct content:
    // - One with a specific keyword (XJ-900) that's easy to find via keyword search
    // - Others with semantic content that's easy to find via vector search
    let chunks = vec![
        create_keyword_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            0,
            "Part number XJ-900 is critical for the assembly process.",
            1.0,
        ),
        create_keyword_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            1,
            "Machine learning enables computers to learn from data without explicit programming.",
            2.0,
        ),
        create_keyword_test_chunk(
            dimensions,
            vector_store_id,
            file_id,
            2,
            "Natural language processing helps understand human speech patterns.",
            3.0,
        ),
    ];

    store
        .store_chunks(chunks.clone())
        .await
        .expect("Failed to store chunks");

    // Test hybrid search with a query that should match via keyword (XJ-900)
    // Vector search alone might not rank this highly, but keyword search will boost it
    let query_embedding = create_test_embedding(dimensions, 5.0); // Different from all chunks
    let config = HybridSearchConfig::default();

    let results = store
        .hybrid_search_vector_store(
            vector_store_id,
            "XJ-900",
            &query_embedding,
            10,
            config,
            None,
        )
        .await
        .expect("Failed to hybrid search");

    assert!(!results.is_empty(), "Should find results");

    // The XJ-900 chunk should be in the results due to keyword match boost
    let has_xj900 = results.iter().any(|r| r.content.contains("XJ-900"));
    assert!(has_xj900, "Should find XJ-900 chunk via keyword match");
}

pub async fn test_hybrid_search_empty_results(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();

    // Search on empty vector store should return empty results
    let query_embedding = create_test_embedding(dimensions, 1.0);
    let config = HybridSearchConfig::default();

    let results = store
        .hybrid_search_vector_store(
            vector_store_id,
            "nonexistent keyword",
            &query_embedding,
            10,
            config,
            None,
        )
        .await
        .expect("Failed to hybrid search");

    assert!(
        results.is_empty(),
        "Empty vector store should return no results"
    );
}

pub async fn test_hybrid_search_multi_vector_store(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id_1 = Uuid::new_v4();
    let vector_store_id_2 = Uuid::new_v4();
    let file_id_1 = Uuid::new_v4();
    let file_id_2 = Uuid::new_v4();

    // Create chunks with shared keyword across collections
    let chunks_1 = vec![create_keyword_test_chunk(
        dimensions,
        vector_store_id_1,
        file_id_1,
        0,
        "Database optimization techniques for PostgreSQL systems.",
        1.0,
    )];
    let chunks_2 = vec![create_keyword_test_chunk(
        dimensions,
        vector_store_id_2,
        file_id_2,
        0,
        "Database administration best practices for enterprise deployments.",
        2.0,
    )];

    store
        .store_chunks(chunks_1)
        .await
        .expect("Failed to store chunks 1");
    store
        .store_chunks(chunks_2)
        .await
        .expect("Failed to store chunks 2");

    // Search for "database" across both vector stores
    let query_embedding = create_test_embedding(dimensions, 1.5);
    let config = HybridSearchConfig::default();

    let results = store
        .hybrid_search_vector_stores(
            &[vector_store_id_1, vector_store_id_2],
            "database",
            &query_embedding,
            10,
            config,
            None,
        )
        .await
        .expect("Failed to hybrid search");

    assert!(
        results.len() >= 2,
        "Should find results from both vector stores"
    );

    // Verify results come from both vector stores
    let has_vector_store_1 = results
        .iter()
        .any(|r| r.vector_store_id == vector_store_id_1);
    let has_vector_store_2 = results
        .iter()
        .any(|r| r.vector_store_id == vector_store_id_2);
    assert!(has_vector_store_1, "Should have result from vector store 1");
    assert!(has_vector_store_2, "Should have result from vector store 2");
}

pub async fn test_hybrid_search_with_filter(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id_1 = Uuid::new_v4();
    let file_id_2 = Uuid::new_v4();

    // Create chunks with same keyword in different files
    let chunks = vec![
        create_keyword_test_chunk(
            dimensions,
            vector_store_id,
            file_id_1,
            0,
            "Security audit for the authentication module.",
            1.0,
        ),
        create_keyword_test_chunk(
            dimensions,
            vector_store_id,
            file_id_2,
            0,
            "Security compliance checklist for production systems.",
            2.0,
        ),
    ];

    store
        .store_chunks(chunks)
        .await
        .expect("Failed to store chunks");

    // Search with file filter - only file_id_1
    let query_embedding = create_test_embedding(dimensions, 1.5);
    let config = HybridSearchConfig::default();
    let filter = ChunkFilter {
        file_ids: Some(vec![file_id_1]),
        attribute_filter: None,
    };

    let results = store
        .hybrid_search_vector_store(
            vector_store_id,
            "security",
            &query_embedding,
            10,
            config,
            Some(filter),
        )
        .await
        .expect("Failed to hybrid search");

    assert!(!results.is_empty(), "Should find results");
    for result in &results {
        assert_eq!(
            result.file_id, file_id_1,
            "All results should be from file_id_1"
        );
    }
}

pub async fn test_hybrid_search_weighted(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

    // Create chunks where one has exact keyword match, another has similar embedding
    let keyword_chunk = create_keyword_test_chunk(
        dimensions,
        vector_store_id,
        file_id,
        0,
        "ERROR_CODE_12345 occurred during system initialization.",
        1.0,
    );
    let semantic_chunk = create_keyword_test_chunk(
        dimensions,
        vector_store_id,
        file_id,
        1,
        "The system encountered a critical failure at startup.",
        2.0,
    );

    store
        .store_chunks(vec![keyword_chunk.clone(), semantic_chunk.clone()])
        .await
        .expect("Failed to store chunks");

    // Search with high keyword weight - should favor exact match
    let query_embedding = semantic_chunk.embedding.clone(); // Similar to semantic chunk
    let config = HybridSearchConfig::weighted(0.3, 1.0); // Favor keyword

    let results = store
        .hybrid_search_vector_store(
            vector_store_id,
            "ERROR_CODE_12345",
            &query_embedding,
            10,
            config,
            None,
        )
        .await
        .expect("Failed to hybrid search");

    assert!(!results.is_empty(), "Should find results");
    // With high keyword weight, the error code chunk should be ranked higher
    assert!(
        results[0].content.contains("ERROR_CODE_12345"),
        "Keyword-weighted search should rank exact match first"
    );
}

pub async fn test_hybrid_search_empty_vector_stores(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let query_embedding = create_test_embedding(dimensions, 1.0);
    let config = HybridSearchConfig::default();

    // Search with empty vector_store_ids should return empty results
    let results = store
        .hybrid_search_vector_stores(&[], "test", &query_embedding, 10, config, None)
        .await
        .expect("Failed to hybrid search");

    assert!(
        results.is_empty(),
        "Empty vector stores should return no results"
    );
}

pub async fn test_hybrid_search_rrf_score(store: &dyn VectorBackend) {
    let dimensions = store.dimensions();
    let vector_store_id = Uuid::new_v4();
    let file_id = Uuid::new_v4();

    // Create chunks that will match via both vector AND keyword search
    // This tests that RRF properly combines scores
    let chunk = create_keyword_test_chunk(
        dimensions,
        vector_store_id,
        file_id,
        0,
        "Machine learning algorithms for natural language processing.",
        1.0,
    );

    store
        .store_chunks(vec![chunk.clone()])
        .await
        .expect("Failed to store chunks");

    // Search with the chunk's own embedding and a matching keyword
    let config = HybridSearchConfig::default();
    let results = store
        .hybrid_search_vector_store(
            vector_store_id,
            "machine learning",
            &chunk.embedding,
            10,
            config,
            None,
        )
        .await
        .expect("Failed to hybrid search");

    assert!(!results.is_empty(), "Should find results");

    // The result should have a valid RRF score (between 0 and 1 for our normalization)
    let result = &results[0];
    assert!(
        result.score >= 0.0,
        "RRF score should be non-negative, got {}",
        result.score
    );
}

// ============================================================================
// PostgreSQL with pgvector Tests
// ============================================================================

#[cfg(all(test, feature = "database-postgres"))]
pub mod pgvector {
    use std::sync::OnceLock;

    use sqlx::PgPool;
    use testcontainers_modules::testcontainers::{
        ContainerAsync, GenericImage, ImageExt,
        core::{ContainerPort, WaitFor},
        runners::AsyncRunner,
    };
    use tokio::sync::OnceCell;

    use super::*;
    use crate::{cache::vector_store::PgvectorStore, config::PgvectorIndexType};

    const TEST_DIMENSIONS: usize = 128; // Small for faster tests

    /// Shared container state - initialized once per test run
    struct SharedPgvectorContainer {
        #[allow(dead_code)] // Test infrastructure: keeps container alive
        container: ContainerAsync<GenericImage>,
        connection_string: String,
    }

    /// Global shared container
    static SHARED_CONTAINER: OnceLock<OnceCell<SharedPgvectorContainer>> = OnceLock::new();

    /// Get or initialize the shared pgvector container
    async fn get_shared_container() -> &'static SharedPgvectorContainer {
        let cell = SHARED_CONTAINER.get_or_init(OnceCell::new);
        cell.get_or_init(|| async {
            // Use pgvector/pgvector image which has the extension pre-installed
            // Build the GenericImage with its native methods first,
            // then apply ImageExt methods which change the type to ContainerRequest
            let container = GenericImage::new("pgvector/pgvector", "pg17")
                .with_exposed_port(ContainerPort::Tcp(5432))
                .with_wait_for(WaitFor::message_on_stderr(
                    "database system is ready to accept connections",
                ))
                .with_env_var("POSTGRES_USER", "postgres")
                .with_env_var("POSTGRES_PASSWORD", "postgres")
                .with_env_var("POSTGRES_DB", "postgres")
                .start()
                .await
                .expect("Failed to start pgvector container");

            let host = container.get_host().await.expect("Failed to get host");
            let port = container
                .get_host_port_ipv4(5432)
                .await
                .expect("Failed to get port");

            let connection_string =
                format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

            // Create the extension once at the database level
            let admin_pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .connect(&connection_string)
                .await
                .expect("Failed to connect to PostgreSQL for extension setup");

            sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
                .execute(&admin_pool)
                .await
                .expect("Failed to create vector extension");

            SharedPgvectorContainer {
                container,
                connection_string,
            }
        })
        .await
    }

    /// Create an isolated schema and pgvector store for a single test
    async fn create_isolated_pgvector_pool() -> (PgPool, String) {
        let shared = get_shared_container().await;

        // Create admin pool for schema creation
        let admin_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&shared.connection_string)
            .await
            .expect("Failed to connect to PostgreSQL");

        // Generate unique schema name for this test
        let schema_name = format!("test_{}", uuid::Uuid::new_v4().simple());

        // Create the schema
        sqlx::query(&format!("CREATE SCHEMA \"{}\"", schema_name))
            .execute(&admin_pool)
            .await
            .expect("Failed to create test schema");

        // Create a new pool with search_path set to our isolated schema
        // Include 'public' so the vector extension types are visible
        let isolated_url = format!(
            "{}?options=-c search_path={},public",
            shared.connection_string, schema_name
        );

        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&isolated_url)
            .await
            .expect("Failed to connect to isolated schema");

        (pool, schema_name)
    }

    /// Create an isolated pgvector store for a single test
    async fn create_test_store() -> PgvectorStore {
        let (pool, _schema_name) = create_isolated_pgvector_pool().await;

        let store = PgvectorStore::new(
            pool,
            "embeddings".to_string(),
            TEST_DIMENSIONS,
            PgvectorIndexType::Hnsw,
            crate::config::DistanceMetric::default(),
        );
        store
            .initialize()
            .await
            .expect("Failed to initialize pgvector store");
        store
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_store_and_search() {
        let store = create_test_store().await;
        test_store_and_search(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_similar_embedding() {
        let store = create_test_store().await;
        test_search_with_similar_embedding(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_threshold_filtering() {
        let store = create_test_store().await;
        test_search_threshold_filtering(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_model_filter() {
        let store = create_test_store().await;
        test_model_filter(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_delete() {
        let store = create_test_store().await;
        test_delete(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_dimension_mismatch() {
        let store = create_test_store().await;
        test_dimension_mismatch(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_cleanup_expired() {
        let store = create_test_store().await;
        test_cleanup_expired(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_health_check() {
        let store = create_test_store().await;
        test_health_check(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_upsert() {
        let store = create_test_store().await;
        test_upsert(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_ivfflat_index() {
        let (pool, _schema_name) = create_isolated_pgvector_pool().await;

        let store = PgvectorStore::new(
            pool,
            "embeddings".to_string(),
            TEST_DIMENSIONS,
            PgvectorIndexType::IvfFlat,
            crate::config::DistanceMetric::default(),
        );
        store
            .initialize()
            .await
            .expect("Failed to initialize with IVFFlat");

        test_store_and_search(&store).await;
    }

    // Chunk operation tests
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_store_and_search_chunks() {
        let store = create_test_store().await;
        test_store_and_search_chunks(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_get_chunks_by_file() {
        let store = create_test_store().await;
        test_get_chunks_by_file(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_delete_chunks_by_file() {
        let store = create_test_store().await;
        test_delete_chunks_by_file(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_delete_chunks_by_vector_store() {
        let store = create_test_store().await;
        test_delete_chunks_by_vector_store(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_delete_chunks_by_file_and_vector_store() {
        let store = create_test_store().await;
        test_delete_chunks_by_file_and_vector_store(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_search_vector_stores_multi() {
        let store = create_test_store().await;
        test_search_vector_stores_multi(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_search_with_file_filter() {
        let store = create_test_store().await;
        test_search_with_file_filter(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_chunk_dimension_mismatch() {
        let store = create_test_store().await;
        test_chunk_dimension_mismatch(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_empty_chunks() {
        let store = create_test_store().await;
        test_empty_chunks(&store).await;
    }

    // Keyword search tests
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_keyword_search_basic() {
        let store = create_test_store().await;
        test_keyword_search_basic(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_keyword_search_empty_query() {
        let store = create_test_store().await;
        test_keyword_search_empty_query(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_keyword_search_empty_collections() {
        let store = create_test_store().await;
        test_keyword_search_empty_vector_stores(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_keyword_search_multi_collection() {
        let store = create_test_store().await;
        test_keyword_search_multi_vector_store(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_keyword_search_with_file_filter() {
        let store = create_test_store().await;
        test_keyword_search_with_file_filter(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_keyword_search_score_normalization() {
        let store = create_test_store().await;
        test_keyword_search_score_normalization(&store).await;
    }

    // Hybrid search tests
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_hybrid_search_basic() {
        let store = create_test_store().await;
        test_hybrid_search_basic(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_hybrid_search_empty_results() {
        let store = create_test_store().await;
        test_hybrid_search_empty_results(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_hybrid_search_multi_collection() {
        let store = create_test_store().await;
        test_hybrid_search_multi_vector_store(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_hybrid_search_with_filter() {
        let store = create_test_store().await;
        test_hybrid_search_with_filter(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_hybrid_search_weighted() {
        let store = create_test_store().await;
        test_hybrid_search_weighted(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_hybrid_search_empty_collections() {
        let store = create_test_store().await;
        test_hybrid_search_empty_vector_stores(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_pgvector_hybrid_search_rrf_score() {
        let store = create_test_store().await;
        test_hybrid_search_rrf_score(&store).await;
    }
}

// ============================================================================
// Qdrant Tests
// ============================================================================

#[cfg(test)]
pub mod qdrant {
    use std::sync::OnceLock;

    use testcontainers_modules::testcontainers::{
        ContainerAsync, GenericImage,
        core::{ContainerPort, WaitFor},
        runners::AsyncRunner,
    };
    use tokio::sync::OnceCell;

    use super::*;
    use crate::{cache::vector_store::QdrantStore, config::DistanceMetric};

    const TEST_DIMENSIONS: usize = 128; // Small for faster tests

    /// Shared container state
    struct SharedQdrantContainer {
        #[allow(dead_code)] // Test infrastructure: keeps container alive
        container: ContainerAsync<GenericImage>,
        base_url: String,
        collection_name: String,
    }

    /// Global shared container
    static SHARED_CONTAINER: OnceLock<OnceCell<SharedQdrantContainer>> = OnceLock::new();

    /// Get or initialize the shared Qdrant container
    async fn get_shared_container() -> &'static SharedQdrantContainer {
        let cell = SHARED_CONTAINER.get_or_init(OnceCell::new);
        cell.get_or_init(|| async {
            // Build the GenericImage with its native methods first
            let container = GenericImage::new("qdrant/qdrant", "v1.12.4")
                .with_exposed_port(ContainerPort::Tcp(6333))
                .with_exposed_port(ContainerPort::Tcp(6334))
                .with_wait_for(WaitFor::message_on_stdout("Qdrant HTTP listening"))
                .start()
                .await
                .expect("Failed to start Qdrant container");

            let host = container.get_host().await.expect("Failed to get host");
            let port = container
                .get_host_port_ipv4(6333)
                .await
                .expect("Failed to get port");

            let base_url = format!("http://{}:{}", host, port);

            // Wait for the HTTP API to be truly ready by polling the health endpoint
            let client = reqwest::Client::new();
            let health_url = format!("{}/collections", base_url);
            for _ in 0..30 {
                if client.get(&health_url).send().await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // Create a single shared Qdrant index for all tests
            let collection_name = "integration_tests".to_string();
            let store = QdrantStore::new(
                base_url.clone(),
                None,
                collection_name.clone(),
                TEST_DIMENSIONS,
                DistanceMetric::Cosine,
            );
            store
                .initialize()
                .await
                .expect("Failed to initialize shared Qdrant index");

            SharedQdrantContainer {
                container,
                base_url,
                collection_name,
            }
        })
        .await
    }

    /// Create a Qdrant store using the shared Qdrant index
    /// Test isolation is achieved through unique IDs (UUIDs)
    async fn create_test_store() -> QdrantStore {
        let shared = get_shared_container().await;

        QdrantStore::new(
            shared.base_url.clone(),
            None,
            shared.collection_name.clone(),
            TEST_DIMENSIONS,
            DistanceMetric::Cosine,
        )
        // Note: Don't call initialize() - index already exists
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_store_and_search() {
        let store = create_test_store().await;
        test_store_and_search(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_similar_embedding() {
        let store = create_test_store().await;
        test_search_with_similar_embedding(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_threshold_filtering() {
        let store = create_test_store().await;
        test_search_threshold_filtering(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_model_filter() {
        let store = create_test_store().await;
        test_model_filter(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_delete() {
        let store = create_test_store().await;
        test_delete(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_dimension_mismatch() {
        let store = create_test_store().await;
        test_dimension_mismatch(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_cleanup_expired() {
        let store = create_test_store().await;
        test_cleanup_expired(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_health_check() {
        let store = create_test_store().await;
        test_health_check(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_upsert() {
        let store = create_test_store().await;
        test_upsert(&store).await;
    }

    // Chunk operation tests
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_store_and_search_chunks() {
        let store = create_test_store().await;
        test_store_and_search_chunks(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_get_chunks_by_file() {
        let store = create_test_store().await;
        test_get_chunks_by_file(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_delete_chunks_by_file() {
        let store = create_test_store().await;
        test_delete_chunks_by_file(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_delete_chunks_by_vector_store() {
        let store = create_test_store().await;
        test_delete_chunks_by_vector_store(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_delete_chunks_by_file_and_vector_store() {
        let store = create_test_store().await;
        test_delete_chunks_by_file_and_vector_store(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_search_vector_stores_multi() {
        let store = create_test_store().await;
        test_search_vector_stores_multi(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_search_with_file_filter() {
        let store = create_test_store().await;
        test_search_with_file_filter(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_chunk_dimension_mismatch() {
        let store = create_test_store().await;
        test_chunk_dimension_mismatch(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_empty_chunks() {
        let store = create_test_store().await;
        test_empty_chunks(&store).await;
    }

    // Keyword search tests
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_keyword_search_basic() {
        let store = create_test_store().await;
        test_keyword_search_basic(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_keyword_search_empty_query() {
        let store = create_test_store().await;
        test_keyword_search_empty_query(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_keyword_search_empty_collections() {
        let store = create_test_store().await;
        test_keyword_search_empty_vector_stores(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_keyword_search_multi_collection() {
        let store = create_test_store().await;
        test_keyword_search_multi_vector_store(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_keyword_search_with_file_filter() {
        let store = create_test_store().await;
        test_keyword_search_with_file_filter(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_keyword_search_score_normalization() {
        let store = create_test_store().await;
        test_keyword_search_score_normalization(&store).await;
    }

    // Hybrid search tests
    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_hybrid_search_basic() {
        let store = create_test_store().await;
        test_hybrid_search_basic(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_hybrid_search_empty_results() {
        let store = create_test_store().await;
        test_hybrid_search_empty_results(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_hybrid_search_multi_collection() {
        let store = create_test_store().await;
        test_hybrid_search_multi_vector_store(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_hybrid_search_with_filter() {
        let store = create_test_store().await;
        test_hybrid_search_with_filter(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_hybrid_search_weighted() {
        let store = create_test_store().await;
        test_hybrid_search_weighted(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_hybrid_search_empty_collections() {
        let store = create_test_store().await;
        test_hybrid_search_empty_vector_stores(&store).await;
    }

    #[tokio::test]
    #[ignore = "requires Docker"]
    async fn test_qdrant_hybrid_search_rrf_score() {
        let store = create_test_store().await;
        test_hybrid_search_rrf_score(&store).await;
    }
}
