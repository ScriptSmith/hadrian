//! Ranking options for vector store search.
//!
//! These types control how search results are scored and filtered in vector store searches.
//!
//! # Example
//!
//! ```json
//! {
//!   "ranking_options": {
//!     "ranker": "auto",
//!     "score_threshold": 0.5
//!   }
//! }
//! ```
//!
//! # Hybrid Search
//!
//! Hybrid search combines dense vector (semantic) search with sparse keyword (BM25/full-text)
//! search using Reciprocal Rank Fusion (RRF). This improves retrieval quality for queries
//! that benefit from both semantic understanding and exact keyword matching.
//!
//! ```json
//! {
//!   "ranking_options": {
//!     "ranker": "hybrid",
//!     "score_threshold": 0.5,
//!     "hybrid_search": {
//!       "embedding_weight": 0.7,
//!       "text_weight": 0.3
//!     }
//!   }
//! }
//! ```
//!
//! # LLM Re-ranking
//!
//! LLM-based re-ranking uses a language model to re-score search results based on
//! semantic relevance to the query. This can improve result quality at the cost of
//! additional latency and API calls.
//!
//! ```json
//! {
//!   "ranking_options": {
//!     "ranker": "llm",
//!     "score_threshold": 0.5
//!   }
//! }
//! ```

use serde::{Deserialize, Serialize};

/// The ranker algorithm to use for file search.
///
/// Controls which ranking algorithm is used to score and order search results.
/// If not specified, defaults to `auto`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum FileSearchRanker {
    /// Automatically select the best ranker based on the query and configuration.
    ///
    /// When `hybrid_search` options are provided, uses hybrid search combining
    /// vector and keyword search with RRF fusion. Otherwise uses vector-only search.
    #[default]
    Auto,
    /// Vector-only search using cosine similarity.
    ///
    /// Fast and efficient for semantic similarity matching. Best for queries
    /// where exact keyword matching is not critical.
    Vector,
    /// Hybrid search combining vector and keyword search with RRF fusion.
    ///
    /// Combines dense vector (semantic) search with sparse keyword (BM25/full-text)
    /// search using Reciprocal Rank Fusion. Best for queries that benefit from
    /// both semantic understanding and exact keyword matching.
    Hybrid,
    /// LLM-based re-ranking.
    ///
    /// Uses a language model to re-score initial search results based on
    /// semantic relevance to the query. Provides highest quality results
    /// at the cost of additional latency and API calls.
    Llm,
    /// Disable re-ranking. Returns results in raw similarity order.
    ///
    /// Can help reduce latency when re-ranking overhead is not needed.
    None,
}

impl FileSearchRanker {
    /// Returns true if this ranker supports hybrid search.
    pub fn supports_hybrid(&self) -> bool {
        matches!(self, Self::Auto | Self::Hybrid)
    }

    /// Returns true if this ranker uses vector-only search.
    pub fn is_vector_only(&self) -> bool {
        matches!(self, Self::Vector | Self::None)
    }

    /// Returns true if this ranker uses LLM-based re-ranking.
    pub fn is_llm_rerank(&self) -> bool {
        matches!(self, Self::Llm)
    }
}

impl std::fmt::Display for FileSearchRanker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Vector => write!(f, "vector"),
            Self::Hybrid => write!(f, "hybrid"),
            Self::Llm => write!(f, "llm"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Options for hybrid search combining vector and keyword search.
///
/// Hybrid search uses Reciprocal Rank Fusion (RRF) to combine results from
/// dense vector (semantic) search and sparse keyword (BM25/full-text) search.
/// The weights control how much each search method contributes to the final ranking.
///
/// # Example
///
/// ```json
/// {
///   "embedding_weight": 0.7,
///   "text_weight": 0.3
/// }
/// ```
///
/// # Weight Interpretation
///
/// - Equal weights (1.0, 1.0): Balanced fusion, good default
/// - Higher embedding_weight: Favor semantic similarity
/// - Higher text_weight: Favor exact keyword matches
///
/// Weights are relative; they don't need to sum to 1.0.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct HybridSearchOptions {
    /// The weight of the embedding (vector) search in reciprocal rank fusion.
    ///
    /// Higher values give more influence to semantic similarity matches.
    /// Default: 1.0
    pub embedding_weight: f64,

    /// The weight of the text (keyword) search in reciprocal rank fusion.
    ///
    /// Higher values give more influence to exact keyword matches.
    /// Default: 1.0
    pub text_weight: f64,
}

impl Default for HybridSearchOptions {
    fn default() -> Self {
        Self {
            embedding_weight: 1.0,
            text_weight: 1.0,
        }
    }
}

impl HybridSearchOptions {
    /// Create hybrid search options with custom weights.
    pub fn new(embedding_weight: f64, text_weight: f64) -> Self {
        Self {
            embedding_weight,
            text_weight,
        }
    }

    /// Create options favoring semantic (embedding) search.
    pub fn semantic_focused() -> Self {
        Self {
            embedding_weight: 0.7,
            text_weight: 0.3,
        }
    }

    /// Create options favoring keyword (text) search.
    pub fn keyword_focused() -> Self {
        Self {
            embedding_weight: 0.3,
            text_weight: 0.7,
        }
    }
}

/// Ranking options for file search.
///
/// Controls how search results are scored, ranked, and filtered.
///
/// # Example (Vector-only search)
///
/// ```json
/// {
///   "ranker": "vector",
///   "score_threshold": 0.5
/// }
/// ```
///
/// # Example (Hybrid search)
///
/// ```json
/// {
///   "ranker": "hybrid",
///   "score_threshold": 0.5,
///   "hybrid_search": {
///     "embedding_weight": 0.7,
///     "text_weight": 0.3
///   }
/// }
/// ```
///
/// # Example (LLM re-ranking)
///
/// ```json
/// {
///   "ranker": "llm",
///   "score_threshold": 0.5
/// }
/// ```
///
/// # Fields
///
/// - `ranker`: The ranking algorithm to use (optional, defaults to `auto`)
/// - `score_threshold`: Minimum similarity score (required, 0.0-1.0)
/// - `hybrid_search`: Options for hybrid vector+keyword search (optional)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct FileSearchRankingOptions {
    /// The ranker to use for scoring results.
    ///
    /// If not specified, defaults to `auto` which automatically selects
    /// the best ranker based on the query characteristics.
    #[serde(default)]
    pub ranker: Option<FileSearchRanker>,

    /// The minimum similarity score threshold for results.
    ///
    /// Results with scores below this threshold will be filtered out.
    /// Must be a floating point number between 0.0 and 1.0.
    ///
    /// - `0.0`: Return all results regardless of score
    /// - `1.0`: Only return exact matches
    /// - Recommended: `0.5` for balanced precision/recall
    pub score_threshold: f64,

    /// **Hadrian Extension:** Options for hybrid search combining vector and keyword search.
    ///
    /// When provided with a ranker that supports hybrid search (`auto` or
    /// `hybrid`), enables hybrid search using Reciprocal Rank Fusion (RRF)
    /// to combine vector and keyword results.
    ///
    /// If not provided, uses vector-only search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hybrid_search: Option<HybridSearchOptions>,
}

impl FileSearchRankingOptions {
    /// Create new ranking options with the given score threshold.
    ///
    /// Uses the default `auto` ranker with vector-only search.
    #[allow(dead_code)] // Used in tests; public API builder
    pub fn new(score_threshold: f64) -> Self {
        Self {
            ranker: None,
            score_threshold,
            hybrid_search: None,
        }
    }

    /// Create ranking options with a specific ranker.
    #[allow(dead_code)] // Used in tests; useful for future API builders
    pub fn with_ranker(score_threshold: f64, ranker: FileSearchRanker) -> Self {
        Self {
            ranker: Some(ranker),
            score_threshold,
            hybrid_search: None,
        }
    }

    /// Create ranking options with hybrid search enabled.
    ///
    /// Uses the `hybrid` ranker which combines vector and keyword search.
    #[allow(dead_code)] // Used in tests; useful for future API builders
    pub fn with_hybrid(score_threshold: f64, hybrid_options: HybridSearchOptions) -> Self {
        Self {
            ranker: Some(FileSearchRanker::Hybrid),
            score_threshold,
            hybrid_search: Some(hybrid_options),
        }
    }

    /// Get the effective ranker, defaulting to `Auto` if not specified.
    #[allow(dead_code)] // Used in tests; useful for future API builders
    pub fn effective_ranker(&self) -> FileSearchRanker {
        self.ranker.unwrap_or_default()
    }

    /// Returns true if hybrid search should be used.
    ///
    /// Hybrid search is enabled when:
    /// - `hybrid_search` options are provided, AND
    /// - The ranker supports hybrid search (`auto` or `hybrid`)
    pub fn use_hybrid_search(&self) -> bool {
        self.hybrid_search.is_some() && self.effective_ranker().supports_hybrid()
    }
}

impl Default for FileSearchRankingOptions {
    /// Default ranking options with 0.0 score threshold (return all results).
    ///
    /// This matches OpenAI's default behavior.
    fn default() -> Self {
        Self {
            ranker: None,
            score_threshold: 0.0,
            hybrid_search: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // FileSearchRanker tests
    // =========================================================================

    #[test]
    fn test_ranker_serialization() {
        assert_eq!(
            serde_json::to_string(&FileSearchRanker::Auto).unwrap(),
            "\"auto\""
        );
        assert_eq!(
            serde_json::to_string(&FileSearchRanker::Vector).unwrap(),
            "\"vector\""
        );
        assert_eq!(
            serde_json::to_string(&FileSearchRanker::Hybrid).unwrap(),
            "\"hybrid\""
        );
        assert_eq!(
            serde_json::to_string(&FileSearchRanker::Llm).unwrap(),
            "\"llm\""
        );
        assert_eq!(
            serde_json::to_string(&FileSearchRanker::None).unwrap(),
            "\"none\""
        );
    }

    #[test]
    fn test_ranker_deserialization() {
        assert_eq!(
            serde_json::from_str::<FileSearchRanker>("\"auto\"").unwrap(),
            FileSearchRanker::Auto
        );
        assert_eq!(
            serde_json::from_str::<FileSearchRanker>("\"vector\"").unwrap(),
            FileSearchRanker::Vector
        );
        assert_eq!(
            serde_json::from_str::<FileSearchRanker>("\"hybrid\"").unwrap(),
            FileSearchRanker::Hybrid
        );
        assert_eq!(
            serde_json::from_str::<FileSearchRanker>("\"llm\"").unwrap(),
            FileSearchRanker::Llm
        );
        assert_eq!(
            serde_json::from_str::<FileSearchRanker>("\"none\"").unwrap(),
            FileSearchRanker::None
        );
    }

    #[test]
    fn test_ranker_supports_hybrid() {
        assert!(FileSearchRanker::Auto.supports_hybrid());
        assert!(FileSearchRanker::Hybrid.supports_hybrid());
        assert!(!FileSearchRanker::Vector.supports_hybrid());
        assert!(!FileSearchRanker::Llm.supports_hybrid());
        assert!(!FileSearchRanker::None.supports_hybrid());
    }

    #[test]
    fn test_ranker_is_vector_only() {
        assert!(!FileSearchRanker::Auto.is_vector_only());
        assert!(!FileSearchRanker::Hybrid.is_vector_only());
        assert!(!FileSearchRanker::Llm.is_vector_only());
        assert!(FileSearchRanker::Vector.is_vector_only());
        assert!(FileSearchRanker::None.is_vector_only());
    }

    #[test]
    fn test_ranker_is_llm_rerank() {
        assert!(!FileSearchRanker::Auto.is_llm_rerank());
        assert!(!FileSearchRanker::Vector.is_llm_rerank());
        assert!(!FileSearchRanker::Hybrid.is_llm_rerank());
        assert!(FileSearchRanker::Llm.is_llm_rerank());
        assert!(!FileSearchRanker::None.is_llm_rerank());
    }

    #[test]
    fn test_ranker_display() {
        assert_eq!(FileSearchRanker::Auto.to_string(), "auto");
        assert_eq!(FileSearchRanker::Vector.to_string(), "vector");
        assert_eq!(FileSearchRanker::Hybrid.to_string(), "hybrid");
        assert_eq!(FileSearchRanker::Llm.to_string(), "llm");
        assert_eq!(FileSearchRanker::None.to_string(), "none");
    }

    // =========================================================================
    // HybridSearchOptions tests
    // =========================================================================

    #[test]
    fn test_hybrid_options_default() {
        let options = HybridSearchOptions::default();
        assert_eq!(options.embedding_weight, 1.0);
        assert_eq!(options.text_weight, 1.0);
    }

    #[test]
    fn test_hybrid_options_new() {
        let options = HybridSearchOptions::new(0.8, 0.2);
        assert_eq!(options.embedding_weight, 0.8);
        assert_eq!(options.text_weight, 0.2);
    }

    #[test]
    fn test_hybrid_options_presets() {
        let semantic = HybridSearchOptions::semantic_focused();
        assert_eq!(semantic.embedding_weight, 0.7);
        assert_eq!(semantic.text_weight, 0.3);

        let keyword = HybridSearchOptions::keyword_focused();
        assert_eq!(keyword.embedding_weight, 0.3);
        assert_eq!(keyword.text_weight, 0.7);
    }

    #[test]
    fn test_hybrid_options_serialization() {
        let options = HybridSearchOptions::new(0.7, 0.3);
        let json = serde_json::to_string(&options).unwrap();
        assert!(json.contains("\"embedding_weight\":0.7"));
        assert!(json.contains("\"text_weight\":0.3"));
    }

    #[test]
    fn test_hybrid_options_deserialization() {
        let json = r#"{"embedding_weight": 0.6, "text_weight": 0.4}"#;
        let options: HybridSearchOptions = serde_json::from_str(json).unwrap();
        assert_eq!(options.embedding_weight, 0.6);
        assert_eq!(options.text_weight, 0.4);
    }

    // =========================================================================
    // FileSearchRankingOptions tests
    // =========================================================================

    #[test]
    fn test_ranking_options_serialization() {
        let options = FileSearchRankingOptions::new(0.5);
        let json = serde_json::to_string(&options).unwrap();
        assert!(json.contains("\"score_threshold\":0.5"));
        assert!(json.contains("\"ranker\":null"));
        // hybrid_search should not be serialized when None
        assert!(!json.contains("hybrid_search"));

        let options_with_ranker =
            FileSearchRankingOptions::with_ranker(0.7, FileSearchRanker::Vector);
        let json = serde_json::to_string(&options_with_ranker).unwrap();
        assert!(json.contains("\"score_threshold\":0.7"));
        assert!(json.contains("\"ranker\":\"vector\""));
    }

    #[test]
    fn test_ranking_options_with_hybrid_serialization() {
        let options =
            FileSearchRankingOptions::with_hybrid(0.5, HybridSearchOptions::new(0.7, 0.3));
        let json = serde_json::to_string(&options).unwrap();
        assert!(json.contains("\"score_threshold\":0.5"));
        assert!(json.contains("\"ranker\":\"hybrid\""));
        assert!(json.contains("\"hybrid_search\""));
        assert!(json.contains("\"embedding_weight\":0.7"));
        assert!(json.contains("\"text_weight\":0.3"));
    }

    #[test]
    fn test_ranking_options_deserialization() {
        let json = r#"{"score_threshold": 0.5}"#;
        let options: FileSearchRankingOptions = serde_json::from_str(json).unwrap();
        assert_eq!(options.score_threshold, 0.5);
        assert!(options.ranker.is_none());
        assert!(options.hybrid_search.is_none());

        let json = r#"{"ranker": "auto", "score_threshold": 0.8}"#;
        let options: FileSearchRankingOptions = serde_json::from_str(json).unwrap();
        assert_eq!(options.score_threshold, 0.8);
        assert_eq!(options.ranker, Some(FileSearchRanker::Auto));
        assert!(options.hybrid_search.is_none());
    }

    #[test]
    fn test_ranking_options_with_hybrid_deserialization() {
        let json = r#"{
            "ranker": "hybrid",
            "score_threshold": 0.6,
            "hybrid_search": {
                "embedding_weight": 0.8,
                "text_weight": 0.2
            }
        }"#;
        let options: FileSearchRankingOptions = serde_json::from_str(json).unwrap();
        assert_eq!(options.score_threshold, 0.6);
        assert_eq!(options.ranker, Some(FileSearchRanker::Hybrid));
        assert!(options.hybrid_search.is_some());
        let hybrid = options.hybrid_search.unwrap();
        assert_eq!(hybrid.embedding_weight, 0.8);
        assert_eq!(hybrid.text_weight, 0.2);
    }

    #[test]
    fn test_ranking_options_with_llm_deserialization() {
        let json = r#"{"ranker": "llm", "score_threshold": 0.7}"#;
        let options: FileSearchRankingOptions = serde_json::from_str(json).unwrap();
        assert_eq!(options.score_threshold, 0.7);
        assert_eq!(options.ranker, Some(FileSearchRanker::Llm));
        assert!(options.hybrid_search.is_none());
    }

    #[test]
    fn test_effective_ranker() {
        let options = FileSearchRankingOptions::new(0.5);
        assert_eq!(options.effective_ranker(), FileSearchRanker::Auto);

        let options = FileSearchRankingOptions::with_ranker(0.5, FileSearchRanker::Vector);
        assert_eq!(options.effective_ranker(), FileSearchRanker::Vector);

        let options = FileSearchRankingOptions::with_ranker(0.5, FileSearchRanker::Hybrid);
        assert_eq!(options.effective_ranker(), FileSearchRanker::Hybrid);

        let options = FileSearchRankingOptions::with_ranker(0.5, FileSearchRanker::Llm);
        assert_eq!(options.effective_ranker(), FileSearchRanker::Llm);
    }

    #[test]
    fn test_use_hybrid_search() {
        // No hybrid options -> no hybrid search
        let options = FileSearchRankingOptions::new(0.5);
        assert!(!options.use_hybrid_search());

        // Hybrid options with auto ranker -> hybrid search
        let mut options = FileSearchRankingOptions::new(0.5);
        options.hybrid_search = Some(HybridSearchOptions::default());
        assert!(options.use_hybrid_search());

        // Hybrid options with hybrid ranker -> hybrid search
        let options = FileSearchRankingOptions::with_hybrid(0.5, HybridSearchOptions::default());
        assert!(options.use_hybrid_search());

        // Hybrid options with vector-only ranker -> no hybrid search
        let mut options = FileSearchRankingOptions::with_ranker(0.5, FileSearchRanker::Vector);
        options.hybrid_search = Some(HybridSearchOptions::default());
        assert!(!options.use_hybrid_search());

        // Hybrid options with llm ranker -> no hybrid search (llm does its own ranking)
        let mut options = FileSearchRankingOptions::with_ranker(0.5, FileSearchRanker::Llm);
        options.hybrid_search = Some(HybridSearchOptions::default());
        assert!(!options.use_hybrid_search());

        // Hybrid options with none ranker -> no hybrid search
        let mut options = FileSearchRankingOptions::with_ranker(0.5, FileSearchRanker::None);
        options.hybrid_search = Some(HybridSearchOptions::default());
        assert!(!options.use_hybrid_search());
    }

    #[test]
    fn test_default() {
        let options = FileSearchRankingOptions::default();
        assert_eq!(options.score_threshold, 0.0);
        assert!(options.ranker.is_none());
        assert!(options.hybrid_search.is_none());
    }
}
