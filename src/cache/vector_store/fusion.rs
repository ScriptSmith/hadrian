//! Score fusion algorithms for hybrid search.
//!
//! This module provides algorithms for combining results from multiple
//! retrieval systems (e.g., vector search and keyword search) into a
//! single ranked list.
//!
//! # Reciprocal Rank Fusion (RRF)
//!
//! The primary algorithm is Reciprocal Rank Fusion (RRF), which combines
//! ranked lists by summing reciprocal ranks:
//!
//! ```text
//! RRF_score(doc) = Σ 1 / (k + rank_i(doc))
//! ```
//!
//! Where:
//! - `k` is a smoothing constant (default 60) that prevents high-ranked
//!   items from dominating
//! - `rank_i(doc)` is the 1-indexed rank of the document in result list i
//!
//! RRF is preferred over score-based fusion because:
//! - It's robust to different score distributions across retrieval systems
//! - It doesn't require score normalization or calibration
//! - It performs well empirically across many retrieval tasks
//!
//! # References
//!
//! - Cormack, Clarke, Buettcher (2009): "Reciprocal Rank Fusion outperforms
//!   Condorcet and individual Rank Learning Methods"
//!
//! # Example
//!
//! ```ignore
//! use crate::cache::vector_store::fusion::{RrfConfig, fuse_results};
//!
//! let vector_results = vec![/* from vector search */];
//! let keyword_results = vec![/* from keyword search */];
//!
//! let fused = fuse_results(
//!     &vector_results,
//!     &keyword_results,
//!     &RrfConfig::default(),
//! );
//! ```

use std::collections::HashMap;

use uuid::Uuid;

use super::ChunkSearchResult;

/// Configuration for Reciprocal Rank Fusion.
#[derive(Debug, Clone)]
pub struct RrfConfig {
    /// The smoothing constant `k` in the RRF formula.
    ///
    /// Higher values reduce the influence of rank position, making the
    /// fusion more "democratic". Lower values give more weight to top ranks.
    ///
    /// - `k = 60` (default): Standard RRF, good general-purpose value
    /// - `k = 1`: Very aggressive, top ranks dominate
    /// - `k = 100+`: More egalitarian weighting
    ///
    /// The original RRF paper recommends k=60.
    pub k: u32,

    /// Optional weight for vector search results (0.0 to 1.0).
    ///
    /// When set, the RRF contribution from vector search is multiplied
    /// by this weight. Combined with `keyword_weight`, this allows
    /// tuning the balance between semantic and lexical matching.
    ///
    /// If `None`, both sources are weighted equally (1.0).
    pub vector_weight: Option<f64>,

    /// Optional weight for keyword search results (0.0 to 1.0).
    ///
    /// When set, the RRF contribution from keyword search is multiplied
    /// by this weight.
    ///
    /// If `None`, both sources are weighted equally (1.0).
    pub keyword_weight: Option<f64>,
}

impl Default for RrfConfig {
    fn default() -> Self {
        Self {
            k: 60,
            vector_weight: None,
            keyword_weight: None,
        }
    }
}

impl RrfConfig {
    /// Create a new RRF config with the given k constant.
    pub fn with_k(k: u32) -> Self {
        Self {
            k,
            ..Default::default()
        }
    }

    /// Create a weighted RRF config.
    ///
    /// # Arguments
    /// * `vector_weight` - Weight for vector search results (0.0 to 1.0)
    /// * `keyword_weight` - Weight for keyword search results (0.0 to 1.0)
    ///
    /// Weights don't need to sum to 1.0; they're applied as multipliers.
    pub fn weighted(vector_weight: f64, keyword_weight: f64) -> Self {
        Self {
            k: 60,
            vector_weight: Some(vector_weight),
            keyword_weight: Some(keyword_weight),
        }
    }

    /// Get the effective vector weight (1.0 if not set).
    fn effective_vector_weight(&self) -> f64 {
        self.vector_weight.unwrap_or(1.0)
    }

    /// Get the effective keyword weight (1.0 if not set).
    fn effective_keyword_weight(&self) -> f64 {
        self.keyword_weight.unwrap_or(1.0)
    }
}

/// Configuration for hybrid search combining vector and keyword search.
///
/// Hybrid search runs both semantic (vector) and lexical (keyword) searches,
/// then combines results using Reciprocal Rank Fusion (RRF).
#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    /// RRF configuration for score fusion.
    pub rrf: RrfConfig,

    /// Similarity threshold for vector search (0.0 to 1.0).
    ///
    /// Only vector results above this threshold are included before fusion.
    /// Keyword search has no threshold (all matches are included).
    ///
    /// Default: 0.0 (include all vector results)
    pub vector_threshold: f64,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            rrf: RrfConfig::default(),
            vector_threshold: 0.0,
        }
    }
}

impl HybridSearchConfig {
    /// Create a hybrid search config with custom RRF settings.
    pub fn with_rrf(rrf: RrfConfig) -> Self {
        Self {
            rrf,
            ..Default::default()
        }
    }

    /// Create a hybrid search config with custom weights.
    ///
    /// # Arguments
    /// * `vector_weight` - Weight for vector search results (0.0 to 1.0)
    /// * `keyword_weight` - Weight for keyword search results (0.0 to 1.0)
    pub fn weighted(vector_weight: f64, keyword_weight: f64) -> Self {
        Self {
            rrf: RrfConfig::weighted(vector_weight, keyword_weight),
            ..Default::default()
        }
    }

    /// Set the vector similarity threshold.
    pub fn with_vector_threshold(mut self, threshold: f64) -> Self {
        self.vector_threshold = threshold;
        self
    }
}

/// Internal struct to accumulate RRF scores during fusion.
#[derive(Debug)]
struct FusionCandidate {
    /// The chunk search result (we keep the one with highest original score)
    result: ChunkSearchResult,
    /// Accumulated RRF score
    rrf_score: f64,
    /// Whether this appeared in vector results
    in_vector: bool,
    /// Whether this appeared in keyword results
    in_keyword: bool,
}

/// Fuse results from vector search and keyword search using RRF.
///
/// This function combines two ranked result lists into a single list
/// using Reciprocal Rank Fusion. Documents appearing in both lists
/// receive contributions from both rankings.
///
/// # Arguments
///
/// * `vector_results` - Results from vector (semantic) search, ordered by score
/// * `keyword_results` - Results from keyword (lexical) search, ordered by score
/// * `config` - RRF configuration (k constant, optional weights)
///
/// # Returns
///
/// A merged list of results sorted by RRF score (highest first).
/// Each result's `score` field is replaced with its RRF score.
///
/// # Score Interpretation
///
/// RRF scores are not directly comparable to similarity scores:
/// - Maximum possible score: `2 / (k + 1)` when doc is ranked #1 in both lists
/// - Typical range with k=60: approximately 0.0 to 0.033
///
/// The scores are relative rankings, not absolute similarity measures.
pub fn fuse_results(
    vector_results: &[ChunkSearchResult],
    keyword_results: &[ChunkSearchResult],
    config: &RrfConfig,
) -> Vec<ChunkSearchResult> {
    let mut candidates: HashMap<Uuid, FusionCandidate> = HashMap::new();
    let k = config.k as f64;
    let vector_weight = config.effective_vector_weight();
    let keyword_weight = config.effective_keyword_weight();

    // Process vector results
    for (rank_0, result) in vector_results.iter().enumerate() {
        let rank = (rank_0 + 1) as f64; // 1-indexed rank
        let rrf_contribution = vector_weight / (k + rank);

        candidates
            .entry(result.chunk_id)
            .and_modify(|c| {
                c.rrf_score += rrf_contribution;
                c.in_vector = true;
                // Keep the result with the higher original score
                if result.score > c.result.score {
                    c.result = result.clone();
                }
            })
            .or_insert(FusionCandidate {
                result: result.clone(),
                rrf_score: rrf_contribution,
                in_vector: true,
                in_keyword: false,
            });
    }

    // Process keyword results
    for (rank_0, result) in keyword_results.iter().enumerate() {
        let rank = (rank_0 + 1) as f64; // 1-indexed rank
        let rrf_contribution = keyword_weight / (k + rank);

        candidates
            .entry(result.chunk_id)
            .and_modify(|c| {
                c.rrf_score += rrf_contribution;
                c.in_keyword = true;
                // Keep the result with the higher original score
                if result.score > c.result.score {
                    c.result = result.clone();
                }
            })
            .or_insert(FusionCandidate {
                result: result.clone(),
                rrf_score: rrf_contribution,
                in_vector: false,
                in_keyword: true,
            });
    }

    // Convert to results with RRF scores and sort
    let mut fused: Vec<ChunkSearchResult> = candidates
        .into_values()
        .map(|c| ChunkSearchResult {
            score: c.rrf_score,
            ..c.result
        })
        .collect();

    // Sort by RRF score descending
    fused.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    fused
}

/// Fuse results and limit to top N.
///
/// Convenience function that fuses results and returns only the top `limit` results.
pub fn fuse_results_limited(
    vector_results: &[ChunkSearchResult],
    keyword_results: &[ChunkSearchResult],
    config: &RrfConfig,
    limit: usize,
) -> Vec<ChunkSearchResult> {
    let mut fused = fuse_results(vector_results, keyword_results, config);
    fused.truncate(limit);
    fused
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(chunk_id: Uuid, score: f64, content: &str) -> ChunkSearchResult {
        ChunkSearchResult {
            chunk_id,
            vector_store_id: Uuid::nil(),
            file_id: Uuid::nil(),
            chunk_index: 0,
            content: content.to_string(),
            score,
            metadata: None,
        }
    }

    #[test]
    fn test_rrf_config_default() {
        let config = RrfConfig::default();
        assert_eq!(config.k, 60);
        assert!(config.vector_weight.is_none());
        assert!(config.keyword_weight.is_none());
        assert_eq!(config.effective_vector_weight(), 1.0);
        assert_eq!(config.effective_keyword_weight(), 1.0);
    }

    #[test]
    fn test_rrf_config_with_k() {
        let config = RrfConfig::with_k(100);
        assert_eq!(config.k, 100);
    }

    #[test]
    fn test_rrf_config_weighted() {
        let config = RrfConfig::weighted(0.7, 0.3);
        assert_eq!(config.effective_vector_weight(), 0.7);
        assert_eq!(config.effective_keyword_weight(), 0.3);
    }

    #[test]
    fn test_fuse_empty_results() {
        let config = RrfConfig::default();
        let fused = fuse_results(&[], &[], &config);
        assert!(fused.is_empty());
    }

    #[test]
    fn test_fuse_vector_only() {
        let config = RrfConfig::default();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let vector_results = vec![
            make_result(id1, 0.95, "first"),
            make_result(id2, 0.85, "second"),
        ];

        let fused = fuse_results(&vector_results, &[], &config);

        assert_eq!(fused.len(), 2);
        // First result should have higher RRF score
        assert_eq!(fused[0].chunk_id, id1);
        assert_eq!(fused[1].chunk_id, id2);
        // RRF scores: 1/(60+1) ≈ 0.0164, 1/(60+2) ≈ 0.0161
        assert!(fused[0].score > fused[1].score);
    }

    #[test]
    fn test_fuse_keyword_only() {
        let config = RrfConfig::default();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let keyword_results = vec![
            make_result(id1, 0.8, "first"),
            make_result(id2, 0.6, "second"),
        ];

        let fused = fuse_results(&[], &keyword_results, &config);

        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].chunk_id, id1);
        assert_eq!(fused[1].chunk_id, id2);
    }

    #[test]
    fn test_fuse_overlapping_results() {
        let config = RrfConfig::default();
        let shared_id = Uuid::new_v4();
        let vector_only_id = Uuid::new_v4();
        let keyword_only_id = Uuid::new_v4();

        // Shared doc is #1 in vector, #2 in keyword
        let vector_results = vec![
            make_result(shared_id, 0.95, "shared doc"),
            make_result(vector_only_id, 0.85, "vector only"),
        ];

        // Shared doc is #2 in keyword
        let keyword_results = vec![
            make_result(keyword_only_id, 0.9, "keyword only"),
            make_result(shared_id, 0.7, "shared doc"),
        ];

        let fused = fuse_results(&vector_results, &keyword_results, &config);

        assert_eq!(fused.len(), 3);

        // Shared doc should be first (appears in both lists)
        // RRF: 1/(60+1) + 1/(60+2) ≈ 0.0164 + 0.0161 ≈ 0.0325
        assert_eq!(fused[0].chunk_id, shared_id);
        assert_eq!(fused[0].content, "shared doc");

        // The shared doc should have the higher original score (0.95 from vector)
        // but now has RRF score

        // Other two docs only appear in one list
        // vector_only: 1/(60+2) ≈ 0.0161
        // keyword_only: 1/(60+1) ≈ 0.0164
        // So keyword_only should be second
        assert_eq!(fused[1].chunk_id, keyword_only_id);
        assert_eq!(fused[2].chunk_id, vector_only_id);
    }

    #[test]
    fn test_fuse_preserves_metadata() {
        let config = RrfConfig::default();
        let id = Uuid::new_v4();
        let vector_store_id = Uuid::new_v4();
        let file_id = Uuid::new_v4();

        let vector_results = vec![ChunkSearchResult {
            chunk_id: id,
            vector_store_id,
            file_id,
            chunk_index: 5,
            content: "test content".to_string(),
            score: 0.9,
            metadata: Some(serde_json::json!({"key": "value"})),
        }];

        let fused = fuse_results(&vector_results, &[], &config);

        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].chunk_id, id);
        assert_eq!(fused[0].vector_store_id, vector_store_id);
        assert_eq!(fused[0].file_id, file_id);
        assert_eq!(fused[0].chunk_index, 5);
        assert_eq!(fused[0].content, "test content");
        assert_eq!(fused[0].metadata, Some(serde_json::json!({"key": "value"})));
    }

    #[test]
    fn test_fuse_rrf_score_calculation() {
        let config = RrfConfig::with_k(60);
        let id = Uuid::new_v4();

        // Doc appears at rank 1 in both lists
        let vector_results = vec![make_result(id, 0.9, "doc")];
        let keyword_results = vec![make_result(id, 0.8, "doc")];

        let fused = fuse_results(&vector_results, &keyword_results, &config);

        assert_eq!(fused.len(), 1);
        // Expected RRF: 1/(60+1) + 1/(60+1) = 2/61 ≈ 0.0328
        let expected = 2.0 / 61.0;
        assert!((fused[0].score - expected).abs() < 1e-10);
    }

    #[test]
    fn test_fuse_weighted() {
        let config = RrfConfig::weighted(1.0, 0.5);
        let vector_id = Uuid::new_v4();
        let keyword_id = Uuid::new_v4();

        let vector_results = vec![make_result(vector_id, 0.9, "vector doc")];
        let keyword_results = vec![make_result(keyword_id, 0.9, "keyword doc")];

        let fused = fuse_results(&vector_results, &keyword_results, &config);

        assert_eq!(fused.len(), 2);

        // Vector doc: 1.0 * 1/(60+1) ≈ 0.0164
        // Keyword doc: 0.5 * 1/(60+1) ≈ 0.0082
        // Vector should rank higher due to higher weight
        assert_eq!(fused[0].chunk_id, vector_id);
        assert_eq!(fused[1].chunk_id, keyword_id);

        // Verify the ratio
        let ratio = fused[0].score / fused[1].score;
        assert!((ratio - 2.0).abs() < 1e-10); // Should be exactly 2x
    }

    #[test]
    fn test_fuse_weighted_overlapping() {
        let config = RrfConfig::weighted(0.7, 0.3);
        let shared_id = Uuid::new_v4();

        // Doc at rank 1 in both
        let vector_results = vec![make_result(shared_id, 0.9, "shared")];
        let keyword_results = vec![make_result(shared_id, 0.8, "shared")];

        let fused = fuse_results(&vector_results, &keyword_results, &config);

        assert_eq!(fused.len(), 1);
        // Expected: 0.7/(60+1) + 0.3/(60+1) = 1.0/61 ≈ 0.0164
        let expected = 1.0 / 61.0;
        assert!((fused[0].score - expected).abs() < 1e-10);
    }

    #[test]
    fn test_fuse_results_limited() {
        let config = RrfConfig::default();

        let mut vector_results = Vec::new();
        for i in 0..10 {
            vector_results.push(make_result(
                Uuid::new_v4(),
                0.9 - (i as f64 * 0.05),
                &format!("doc {}", i),
            ));
        }

        let fused = fuse_results_limited(&vector_results, &[], &config, 5);

        assert_eq!(fused.len(), 5);
        // Should be the top 5 by RRF score (same as original order for single source)
    }

    #[test]
    fn test_fuse_different_k_values() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // id1 is #1 in vector, #3 in keyword
        // id2 is #2 in vector, #1 in keyword
        let vector_results = vec![make_result(id1, 0.9, "doc1"), make_result(id2, 0.8, "doc2")];
        let keyword_results = vec![
            make_result(id2, 0.9, "doc2"),
            make_result(Uuid::new_v4(), 0.8, "other"),
            make_result(id1, 0.7, "doc1"),
        ];

        // With k=60 (standard)
        let config_60 = RrfConfig::with_k(60);
        let fused_60 = fuse_results(&vector_results, &keyword_results, &config_60);

        // id1: 1/(60+1) + 1/(60+3) = 0.0164 + 0.0159 = 0.0323
        // id2: 1/(60+2) + 1/(60+1) = 0.0161 + 0.0164 = 0.0325
        // id2 should win slightly
        assert_eq!(fused_60[0].chunk_id, id2);

        // With k=1 (aggressive, top ranks matter more)
        let config_1 = RrfConfig::with_k(1);
        let fused_1 = fuse_results(&vector_results, &keyword_results, &config_1);

        // id1: 1/(1+1) + 1/(1+3) = 0.5 + 0.25 = 0.75
        // id2: 1/(1+2) + 1/(1+1) = 0.333 + 0.5 = 0.833
        // id2 wins more decisively
        assert_eq!(fused_1[0].chunk_id, id2);
    }

    #[test]
    fn test_fuse_keeps_higher_score_result() {
        let config = RrfConfig::default();
        let id = Uuid::new_v4();

        // Same doc with different content/metadata in each list
        let vector_results = vec![ChunkSearchResult {
            chunk_id: id,
            vector_store_id: Uuid::nil(),
            file_id: Uuid::nil(),
            chunk_index: 0,
            content: "vector version".to_string(),
            score: 0.95, // Higher score
            metadata: Some(serde_json::json!({"source": "vector"})),
        }];

        let keyword_results = vec![ChunkSearchResult {
            chunk_id: id,
            vector_store_id: Uuid::nil(),
            file_id: Uuid::nil(),
            chunk_index: 0,
            content: "keyword version".to_string(),
            score: 0.7, // Lower score
            metadata: Some(serde_json::json!({"source": "keyword"})),
        }];

        let fused = fuse_results(&vector_results, &keyword_results, &config);

        assert_eq!(fused.len(), 1);
        // Should keep the vector version (higher original score)
        assert_eq!(fused[0].content, "vector version");
        assert_eq!(
            fused[0].metadata,
            Some(serde_json::json!({"source": "vector"}))
        );
    }

    #[test]
    fn test_fuse_large_result_sets() {
        let config = RrfConfig::default();

        // Create 100 results for each source with 50% overlap
        let mut vector_results = Vec::new();
        let mut keyword_results = Vec::new();
        let mut shared_ids = Vec::new();

        for i in 0..50 {
            let shared = Uuid::new_v4();
            shared_ids.push(shared);
            vector_results.push(make_result(
                shared,
                0.9 - (i as f64 * 0.01),
                &format!("shared {}", i),
            ));
            keyword_results.push(make_result(
                shared,
                0.9 - (i as f64 * 0.01),
                &format!("shared {}", i),
            ));
        }

        for i in 0..50 {
            vector_results.push(make_result(
                Uuid::new_v4(),
                0.4 - (i as f64 * 0.005),
                &format!("vector only {}", i),
            ));
            keyword_results.push(make_result(
                Uuid::new_v4(),
                0.4 - (i as f64 * 0.005),
                &format!("keyword only {}", i),
            ));
        }

        let fused = fuse_results(&vector_results, &keyword_results, &config);

        // Should have 150 unique results (50 shared + 50 vector-only + 50 keyword-only)
        assert_eq!(fused.len(), 150);

        // Shared results should be at the top (they appear in both lists)
        for (i, result) in fused.iter().enumerate().take(50) {
            assert!(
                shared_ids.contains(&result.chunk_id),
                "Position {} should be a shared result",
                i
            );
        }
    }

    #[test]
    fn test_fuse_score_range() {
        let config = RrfConfig::with_k(60);
        let id = Uuid::new_v4();

        // Best case: rank 1 in both lists
        let best = fuse_results(
            &[make_result(id, 1.0, "best")],
            &[make_result(id, 1.0, "best")],
            &config,
        );

        // Worst case: single result from one list
        let worst = fuse_results(&[make_result(Uuid::new_v4(), 0.1, "worst")], &[], &config);

        // Best score should be 2/(k+1)
        let expected_max = 2.0 / 61.0;
        assert!((best[0].score - expected_max).abs() < 1e-10);

        // Worst score should be 1/(k+1)
        let expected_min = 1.0 / 61.0;
        assert!((worst[0].score - expected_min).abs() < 1e-10);

        // All scores should be positive
        assert!(best[0].score > 0.0);
        assert!(worst[0].score > 0.0);
    }
}
