//! Re-ranking service for improving search result quality.
//!
//! This module provides a trait and types for re-ranking search results using
//! various algorithms, including LLM-based re-ranking that uses a language model
//! to score relevance.
//!
//! # Overview
//!
//! Re-ranking is a second-stage retrieval technique that takes initial search results
//! (from vector or hybrid search) and re-scores them based on deeper semantic analysis.
//! This typically improves precision at the cost of additional latency.
//!
//! # Usage
//!
//! ```ignore
//! use crate::services::reranker::{Reranker, RerankRequest};
//!
//! let request = RerankRequest {
//!     query: "How do I configure authentication?".to_string(),
//!     results: initial_search_results,
//!     top_n: Some(5),
//! };
//!
//! let response = reranker.rerank(request).await?;
//! for ranked in response.results {
//!     println!("Score: {:.3}, Content: {}", ranked.relevance_score, ranked.result.content);
//! }
//! ```

use std::{fmt, sync::Arc, time::Duration};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{Level, event, instrument};

use super::FileSearchResult;
use crate::{
    api_types::{
        CreateChatCompletionPayload,
        chat_completion::{JsonSchemaConfig, Message, MessageContent, ResponseFormat},
    },
    config::RerankConfig,
    providers::Provider,
};

/// Errors that can occur during re-ranking operations.
#[derive(Debug, Error)]
pub enum RerankError {
    /// The underlying provider (e.g., LLM API) returned an error.
    #[error("Provider error: {0}")]
    Provider(String),

    /// Failed to parse the reranker's response.
    #[error("Failed to parse reranker response: {0}")]
    ParseError(String),

    /// No results were provided to rerank.
    #[error("No results provided for reranking")]
    EmptyResults,

    /// The reranker is not properly configured.
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// The operation timed out.
    #[error("Reranking operation timed out")]
    Timeout,

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {0}")]
    RateLimited(String),
}

/// A request to re-rank search results.
#[derive(Debug, Clone)]
pub struct RerankRequest {
    /// The original search query.
    pub query: String,

    /// The search results to re-rank.
    ///
    /// These are typically the top-N results from an initial vector or hybrid search.
    pub results: Vec<FileSearchResult>,

    /// Maximum number of results to return after re-ranking.
    ///
    /// If `None`, returns all results in re-ranked order.
    /// If specified, returns only the top `top_n` results.
    pub top_n: Option<usize>,
}

impl RerankRequest {
    /// Create a new rerank request.
    pub fn new(query: impl Into<String>, results: Vec<FileSearchResult>) -> Self {
        Self {
            query: query.into(),
            results,
            top_n: None,
        }
    }

    /// Set the maximum number of results to return.
    pub fn with_top_n(mut self, top_n: usize) -> Self {
        self.top_n = Some(top_n);
        self
    }
}

/// A single result after re-ranking, with updated relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedResult {
    /// The original search result.
    #[serde(flatten)]
    pub result: FileSearchResult,

    /// The relevance score assigned by the reranker (0.0 to 1.0).
    ///
    /// Higher scores indicate higher relevance to the query.
    /// This score replaces the original similarity score for ranking purposes.
    pub relevance_score: f64,

    /// The result's position before re-ranking (0-indexed).
    ///
    /// Useful for analyzing how much the reranker changed the ordering.
    pub original_rank: usize,
}

impl RankedResult {
    /// Create a new ranked result.
    pub fn new(result: FileSearchResult, relevance_score: f64, original_rank: usize) -> Self {
        Self {
            result,
            relevance_score,
            original_rank,
        }
    }

    /// Returns how many positions the result moved (positive = moved up, negative = moved down).
    pub fn rank_change(&self, new_rank: usize) -> i32 {
        self.original_rank as i32 - new_rank as i32
    }
}

/// Token usage from re-ranking operations.
///
/// Tracks the total tokens consumed across all LLM calls during re-ranking.
/// This is important for cost tracking and billing.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RerankUsage {
    /// Total input/prompt tokens across all batches.
    pub prompt_tokens: i64,

    /// Total output/completion tokens across all batches.
    pub completion_tokens: i64,

    /// Total tokens (prompt + completion).
    pub total_tokens: i64,
}

impl RerankUsage {
    /// Create a new usage tracker.
    pub fn new(prompt_tokens: i64, completion_tokens: i64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }

    /// Add usage from another operation.
    pub fn add(&mut self, other: &RerankUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
    }

    /// Returns true if no tokens were used.
    pub fn is_empty(&self) -> bool {
        self.total_tokens == 0
    }
}

/// The response from a re-ranking operation.
#[derive(Debug, Clone)]
pub struct RerankResponse {
    /// The re-ranked results, ordered by relevance (highest first).
    pub results: Vec<RankedResult>,

    /// The model used for re-ranking, if applicable.
    ///
    /// For LLM-based rerankers, this is the model identifier.
    /// For algorithmic rerankers, this may be `None`.
    pub model: Option<String>,

    /// Total number of results that were considered for re-ranking.
    ///
    /// May differ from `results.len()` if `top_n` was specified.
    pub total_considered: usize,

    /// Token usage from LLM-based re-ranking.
    ///
    /// Only populated for rerankers that use LLMs (e.g., `LlmReranker`).
    /// Algorithmic rerankers like `NoOpReranker` will have `None`.
    pub usage: Option<RerankUsage>,
}

impl RerankResponse {
    /// Create a new rerank response.
    pub fn new(results: Vec<RankedResult>, total_considered: usize) -> Self {
        Self {
            results,
            model: None,
            total_considered,
            usage: None,
        }
    }

    /// Set the model used for re-ranking.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the token usage from re-ranking.
    pub fn with_usage(mut self, usage: RerankUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    /// Returns true if no results were returned.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Returns the number of results.
    pub fn len(&self) -> usize {
        self.results.len()
    }
}

/// Trait for re-ranking search results.
///
/// Implementations of this trait can use different strategies to re-rank results:
/// - LLM-based: Use a language model to score relevance
/// - Cross-encoder: Use a specialized cross-encoder model
/// - Algorithmic: Apply heuristics or rules-based scoring
///
/// # Example Implementation
///
/// ```ignore
/// struct MyReranker { /* ... */ }
///
/// #[async_trait]
/// impl Reranker for MyReranker {
///     async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, RerankError> {
///         // Score each result against the query
///         let mut ranked: Vec<RankedResult> = request.results
///             .into_iter()
///             .enumerate()
///             .map(|(i, result)| {
///                 let score = self.compute_relevance(&request.query, &result);
///                 RankedResult::new(result, score, i)
///             })
///             .collect();
///
///         // Sort by relevance score (highest first)
///         ranked.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());
///
///         // Apply top_n limit if specified
///         if let Some(top_n) = request.top_n {
///             ranked.truncate(top_n);
///         }
///
///         Ok(RerankResponse::new(ranked, request.results.len()))
///     }
///
///     fn name(&self) -> &str {
///         "my-reranker"
///     }
/// }
/// ```
#[async_trait]
pub trait Reranker: Send + Sync {
    /// Re-rank the given search results based on relevance to the query.
    ///
    /// # Arguments
    /// * `request` - The rerank request containing query and results
    ///
    /// # Returns
    /// A response containing re-ranked results ordered by relevance, or an error.
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, RerankError>;

    /// Returns the name/identifier of this reranker.
    ///
    /// Used for logging, metrics, and debugging.
    fn name(&self) -> &str;

    /// Returns true if this reranker is available and properly configured.
    ///
    /// Default implementation returns `true`. Override to add health checks.
    fn is_available(&self) -> bool {
        true
    }
}

impl fmt::Debug for dyn Reranker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reranker")
            .field("name", &self.name())
            .field("available", &self.is_available())
            .finish()
    }
}

/// A no-op reranker that returns results in their original order.
///
/// Useful as a fallback or for testing.
pub struct NoOpReranker;

#[async_trait]
impl Reranker for NoOpReranker {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, RerankError> {
        if request.results.is_empty() {
            return Err(RerankError::EmptyResults);
        }

        let total = request.results.len();
        let mut ranked: Vec<RankedResult> = request
            .results
            .into_iter()
            .enumerate()
            .map(|(i, result)| {
                // Preserve original score as relevance score
                let score = result.score;
                RankedResult::new(result, score, i)
            })
            .collect();

        // Apply top_n limit if specified
        if let Some(top_n) = request.top_n {
            ranked.truncate(top_n);
        }

        Ok(RerankResponse::new(ranked, total))
    }

    fn name(&self) -> &str {
        "noop"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LLM-based Reranker
// ─────────────────────────────────────────────────────────────────────────────

/// LLM-based reranker that uses a language model to score result relevance.
///
/// This reranker sends search results to an LLM with a prompt asking it to
/// score each result's relevance to the query. Results are then sorted by
/// the LLM-assigned scores.
///
/// # Configuration
///
/// The reranker is configured via `RerankConfig`:
/// - `model`: The LLM model to use (optional, uses provider default)
/// - `batch_size`: Number of results per LLM call (default: 10)
/// - `max_results_to_rerank`: Maximum results to consider (default: 20)
/// - `timeout_secs`: Timeout for the entire operation (default: 30)
///
/// # Example
///
/// ```ignore
/// let reranker = LlmReranker::new(
///     provider,
///     http_client,
///     config,
///     "openai".to_string(),
/// );
///
/// let request = RerankRequest::new("authentication setup", results);
/// let response = reranker.rerank(request).await?;
/// ```
pub struct LlmReranker {
    provider: Arc<dyn Provider>,
    http_client: Client,
    config: RerankConfig,
    provider_name: String,
}

impl LlmReranker {
    /// Create a new LLM-based reranker.
    ///
    /// # Arguments
    /// * `provider` - The LLM provider to use for scoring
    /// * `http_client` - HTTP client for API calls
    /// * `config` - Reranker configuration
    /// * `provider_name` - Name of the provider (for logging/metrics)
    pub fn new(
        provider: Arc<dyn Provider>,
        http_client: Client,
        config: RerankConfig,
        provider_name: String,
    ) -> Self {
        Self {
            provider,
            http_client,
            config,
            provider_name,
        }
    }

    /// Build the system prompt for the reranking task.
    fn build_system_prompt() -> String {
        r#"You are a relevance scoring assistant. Your task is to evaluate how relevant each document passage is to the user's search query.

For each passage, assign a relevance score from 0.0 to 1.0:
- 1.0: Directly answers the query or contains exactly what was asked for
- 0.8-0.9: Highly relevant, contains most of the needed information
- 0.6-0.7: Moderately relevant, contains related information
- 0.4-0.5: Somewhat relevant, tangentially related
- 0.2-0.3: Minimally relevant, only loosely connected
- 0.0-0.1: Not relevant to the query

Respond with a JSON object containing a "scores" array with objects having "index" (0-based) and "score" (0.0-1.0) for each passage."#
            .to_string()
    }

    /// Build the user prompt with the query and passages to score.
    fn build_user_prompt(query: &str, results: &[(usize, &FileSearchResult)]) -> String {
        let mut prompt = format!("Query: {}\n\nPassages to score:\n", query);

        for (batch_idx, (_, result)) in results.iter().enumerate() {
            // Truncate content to avoid context overflow
            let content = if result.content.len() > 1000 {
                format!("{}...", &result.content[..1000])
            } else {
                result.content.clone()
            };

            prompt.push_str(&format!("\n[Passage {}]\n{}\n", batch_idx, content.trim()));
        }

        prompt.push_str("\nProvide relevance scores for each passage as JSON.");
        prompt
    }

    /// Build the JSON schema for structured output.
    fn build_response_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "scores": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "index": {
                                "type": "integer",
                                "description": "0-based index of the passage"
                            },
                            "score": {
                                "type": "number",
                                "minimum": 0.0,
                                "maximum": 1.0,
                                "description": "Relevance score from 0.0 to 1.0"
                            }
                        },
                        "required": ["index", "score"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["scores"],
            "additionalProperties": false
        })
    }

    /// Parse scores from the LLM response.
    fn parse_scores(response_body: &[u8]) -> Result<Vec<LlmScore>, RerankError> {
        // Parse the HTTP response body as JSON
        let response: serde_json::Value = serde_json::from_slice(response_body)
            .map_err(|e| RerankError::ParseError(format!("Invalid JSON response: {}", e)))?;

        // Extract content from choices[0].message.content
        let content = response
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| RerankError::ParseError("Missing content in response".to_string()))?;

        // Parse the JSON content from the model's response
        let scores_response: LlmScoresResponse = serde_json::from_str(content)
            .map_err(|e| RerankError::ParseError(format!("Invalid scores JSON: {}", e)))?;

        Ok(scores_response.scores)
    }

    /// Parse token usage from the LLM response.
    ///
    /// Extracts prompt_tokens and completion_tokens from the response's usage field.
    /// Returns default (zero) usage if not present.
    fn parse_usage(response_body: &[u8]) -> RerankUsage {
        let Ok(response) = serde_json::from_slice::<serde_json::Value>(response_body) else {
            return RerankUsage::default();
        };

        let Some(usage) = response.get("usage") else {
            return RerankUsage::default();
        };

        // Support both OpenAI format (prompt_tokens) and alternative format (input_tokens)
        let prompt_tokens = usage
            .get("prompt_tokens")
            .or_else(|| usage.get("input_tokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let completion_tokens = usage
            .get("completion_tokens")
            .or_else(|| usage.get("output_tokens"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        RerankUsage::new(prompt_tokens, completion_tokens)
    }

    /// Score a batch of results using the LLM.
    ///
    /// Returns a tuple of (scores, usage) where scores maps original indices to relevance scores.
    #[instrument(skip(self, query, batch), fields(batch_size = batch.len()))]
    async fn score_batch(
        &self,
        query: &str,
        batch: &[(usize, &FileSearchResult)],
    ) -> Result<(Vec<(usize, f64)>, RerankUsage), RerankError> {
        let system_prompt = Self::build_system_prompt();
        let user_prompt = Self::build_user_prompt(query, batch);

        let payload = CreateChatCompletionPayload {
            messages: vec![
                Message::System {
                    content: MessageContent::Text(system_prompt),
                    name: None,
                },
                Message::User {
                    content: MessageContent::Text(user_prompt),
                    name: None,
                },
            ],
            model: self.config.model.clone(),
            stream: false,
            temperature: Some(0.0), // Deterministic scoring
            response_format: Some(ResponseFormat::JsonSchema {
                json_schema: JsonSchemaConfig {
                    name: "rerank_scores".to_string(),
                    description: Some("Relevance scores for search results".to_string()),
                    schema: Some(Self::build_response_schema()),
                    strict: Some(true),
                },
            }),
            // Set reasonable defaults for other fields
            models: None,
            frequency_penalty: None,
            logit_bias: None,
            logprobs: None,
            top_logprobs: None,
            max_completion_tokens: Some(1000),
            max_tokens: None,
            metadata: None,
            presence_penalty: None,
            reasoning: None,
            seed: None,
            stop: None,
            stream_options: None,
            tool_choice: None,
            tools: None,
            top_p: None,
            user: None,
        };

        event!(
            Level::DEBUG,
            stage = "llm_rerank_request",
            provider = %self.provider_name,
            model = ?self.config.model,
            batch_size = batch.len(),
            "Sending batch to LLM for scoring"
        );

        let response = self
            .provider
            .create_chat_completion(&self.http_client, payload)
            .await
            .map_err(|e| RerankError::Provider(e.to_string()))?;

        // Read the response body
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .map_err(|e| RerankError::Provider(format!("Failed to read response: {}", e)))?;

        // Check for error response
        if let Ok(error) = serde_json::from_slice::<serde_json::Value>(&body)
            && let Some(err_msg) = error.get("error").and_then(|e| e.get("message"))
        {
            let msg = err_msg.as_str().unwrap_or("Unknown error");
            if msg.contains("rate limit") || msg.contains("429") {
                return Err(RerankError::RateLimited(msg.to_string()));
            }
            return Err(RerankError::Provider(msg.to_string()));
        }

        let scores = Self::parse_scores(&body)?;
        let usage = Self::parse_usage(&body);

        // Map batch indices back to original indices
        let result: Vec<(usize, f64)> = scores
            .into_iter()
            .filter_map(|s| {
                let batch_idx = s.index as usize;
                if batch_idx < batch.len() {
                    let (original_idx, _) = batch[batch_idx];
                    // Clamp score to valid range
                    let score = s.score.clamp(0.0, 1.0);
                    Some((original_idx, score))
                } else {
                    event!(
                        Level::WARN,
                        batch_idx,
                        batch_len = batch.len(),
                        "LLM returned out-of-bounds index"
                    );
                    None
                }
            })
            .collect();

        event!(
            Level::DEBUG,
            stage = "llm_rerank_response",
            scores_count = result.len(),
            prompt_tokens = usage.prompt_tokens,
            completion_tokens = usage.completion_tokens,
            "Received scores from LLM"
        );

        Ok((result, usage))
    }
}

/// Internal struct for parsing LLM score responses.
#[derive(Debug, Deserialize)]
struct LlmScoresResponse {
    scores: Vec<LlmScore>,
}

/// A single score from the LLM response.
#[derive(Debug, Deserialize)]
struct LlmScore {
    index: i32,
    score: f64,
}

#[async_trait]
impl Reranker for LlmReranker {
    #[instrument(skip(self, request), fields(
        query_len = request.query.len(),
        results_count = request.results.len(),
        top_n = ?request.top_n
    ))]
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, RerankError> {
        if request.results.is_empty() {
            return Err(RerankError::EmptyResults);
        }

        let total_results = request.results.len();

        // Limit results to max_results_to_rerank
        let results_to_process: Vec<_> = request
            .results
            .iter()
            .take(self.config.max_results_to_rerank)
            .collect();

        event!(
            Level::INFO,
            stage = "rerank_started",
            total_results,
            processing = results_to_process.len(),
            batch_size = self.config.batch_size,
            timeout_secs = self.config.timeout_secs,
            "Starting LLM reranking"
        );

        // Create batches with original indices
        let indexed_results: Vec<(usize, &FileSearchResult)> =
            results_to_process.iter().copied().enumerate().collect();

        let batches: Vec<Vec<(usize, &FileSearchResult)>> = indexed_results
            .chunks(self.config.batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        // Process batches with timeout
        let timeout_duration = Duration::from_secs(self.config.timeout_secs);

        let scores_result = tokio::time::timeout(timeout_duration, async {
            let mut all_scores: Vec<(usize, f64)> = Vec::with_capacity(results_to_process.len());
            let mut total_usage = RerankUsage::default();

            for (batch_idx, batch) in batches.iter().enumerate() {
                event!(
                    Level::DEBUG,
                    batch_idx,
                    batch_size = batch.len(),
                    "Processing batch"
                );

                match self.score_batch(&request.query, batch).await {
                    Ok((scores, usage)) => {
                        all_scores.extend(scores);
                        total_usage.add(&usage);
                    }
                    Err(e) => {
                        event!(
                            Level::WARN,
                            batch_idx,
                            error = %e,
                            "Batch scoring failed, using original scores for batch"
                        );
                        // Fall back to original scores for this batch
                        for (original_idx, result) in batch {
                            all_scores.push((*original_idx, result.score));
                        }
                    }
                }
            }

            Ok::<_, RerankError>((all_scores, total_usage))
        })
        .await;

        let (all_scores, total_usage) = match scores_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                event!(
                    Level::WARN,
                    timeout_secs = self.config.timeout_secs,
                    "Reranking timed out"
                );
                return Err(RerankError::Timeout);
            }
        };

        // Build a map from original index to new score
        let score_map: std::collections::HashMap<usize, f64> = all_scores.into_iter().collect();

        // Create ranked results
        let mut ranked: Vec<RankedResult> = request
            .results
            .into_iter()
            .enumerate()
            .map(|(original_idx, result)| {
                // Use LLM score if available, otherwise use original score
                let relevance_score = score_map
                    .get(&original_idx)
                    .copied()
                    .unwrap_or(result.score);
                RankedResult::new(result, relevance_score, original_idx)
            })
            .collect();

        // Sort by relevance score (highest first)
        ranked.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply top_n limit if specified
        if let Some(top_n) = request.top_n {
            ranked.truncate(top_n);
        }

        event!(
            Level::INFO,
            stage = "rerank_completed",
            total_considered = total_results,
            returned = ranked.len(),
            prompt_tokens = total_usage.prompt_tokens,
            completion_tokens = total_usage.completion_tokens,
            total_tokens = total_usage.total_tokens,
            "LLM reranking completed"
        );

        let mut response = RerankResponse::new(ranked, total_results);
        if let Some(ref model) = self.config.model {
            response = response.with_model(model);
        }
        if !total_usage.is_empty() {
            response = response.with_usage(total_usage);
        }

        Ok(response)
    }

    fn name(&self) -> &str {
        "llm"
    }

    fn is_available(&self) -> bool {
        self.config.enabled
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn make_test_result(content: &str, score: f64) -> FileSearchResult {
        FileSearchResult {
            chunk_id: Uuid::new_v4(),
            vector_store_id: Uuid::new_v4(),
            file_id: Uuid::new_v4(),
            chunk_index: 0,
            content: content.to_string(),
            score,
            filename: Some("test.txt".to_string()),
            metadata: None,
        }
    }

    #[test]
    fn test_rerank_request_builder() {
        let results = vec![
            make_test_result("content 1", 0.9),
            make_test_result("content 2", 0.8),
        ];

        let request = RerankRequest::new("test query", results.clone()).with_top_n(1);

        assert_eq!(request.query, "test query");
        assert_eq!(request.results.len(), 2);
        assert_eq!(request.top_n, Some(1));
    }

    #[test]
    fn test_ranked_result_rank_change() {
        let result = make_test_result("test", 0.9);
        let ranked = RankedResult::new(result, 0.95, 5);

        // Moved from position 5 to position 0 = moved up 5 positions
        assert_eq!(ranked.rank_change(0), 5);

        // Moved from position 5 to position 7 = moved down 2 positions
        assert_eq!(ranked.rank_change(7), -2);

        // Same position
        assert_eq!(ranked.rank_change(5), 0);
    }

    #[test]
    fn test_rerank_response_builder() {
        let results = vec![
            RankedResult::new(make_test_result("a", 0.9), 0.95, 0),
            RankedResult::new(make_test_result("b", 0.8), 0.85, 1),
        ];

        let response = RerankResponse::new(results, 10).with_model("gpt-4");

        assert_eq!(response.len(), 2);
        assert_eq!(response.total_considered, 10);
        assert_eq!(response.model, Some("gpt-4".to_string()));
        assert!(!response.is_empty());
    }

    #[test]
    fn test_rerank_response_empty() {
        let response = RerankResponse::new(vec![], 0);
        assert!(response.is_empty());
        assert_eq!(response.len(), 0);
    }

    #[tokio::test]
    async fn test_noop_reranker() {
        let reranker = NoOpReranker;

        let results = vec![
            make_test_result("first", 0.9),
            make_test_result("second", 0.8),
            make_test_result("third", 0.7),
        ];

        let request = RerankRequest::new("query", results);
        let response = reranker.rerank(request).await.unwrap();

        assert_eq!(response.len(), 3);
        assert_eq!(response.total_considered, 3);
        assert_eq!(reranker.name(), "noop");

        // Verify order is preserved and scores match original
        assert_eq!(response.results[0].relevance_score, 0.9);
        assert_eq!(response.results[0].original_rank, 0);
        assert_eq!(response.results[1].relevance_score, 0.8);
        assert_eq!(response.results[1].original_rank, 1);
        assert_eq!(response.results[2].relevance_score, 0.7);
        assert_eq!(response.results[2].original_rank, 2);
    }

    #[tokio::test]
    async fn test_noop_reranker_with_top_n() {
        let reranker = NoOpReranker;

        let results = vec![
            make_test_result("first", 0.9),
            make_test_result("second", 0.8),
            make_test_result("third", 0.7),
        ];

        let request = RerankRequest::new("query", results).with_top_n(2);
        let response = reranker.rerank(request).await.unwrap();

        assert_eq!(response.len(), 2);
        assert_eq!(response.total_considered, 3);
    }

    #[tokio::test]
    async fn test_noop_reranker_empty_results() {
        let reranker = NoOpReranker;
        let request = RerankRequest::new("query", vec![]);
        let result = reranker.rerank(request).await;

        assert!(matches!(result, Err(RerankError::EmptyResults)));
    }

    #[test]
    fn test_reranker_is_available() {
        let reranker = NoOpReranker;
        assert!(reranker.is_available());
    }

    #[test]
    fn test_rerank_error_display() {
        let err = RerankError::Provider("connection failed".to_string());
        assert!(err.to_string().contains("Provider error"));
        assert!(err.to_string().contains("connection failed"));

        let err = RerankError::ParseError("invalid JSON".to_string());
        assert!(err.to_string().contains("parse"));

        let err = RerankError::EmptyResults;
        assert!(err.to_string().contains("No results"));

        let err = RerankError::ConfigurationError("missing API key".to_string());
        assert!(err.to_string().contains("Configuration"));

        let err = RerankError::Timeout;
        assert!(err.to_string().contains("timed out"));

        let err = RerankError::RateLimited("try again in 60s".to_string());
        assert!(err.to_string().contains("Rate limit"));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // LlmReranker Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_llm_build_system_prompt() {
        let prompt = LlmReranker::build_system_prompt();
        assert!(prompt.contains("relevance scoring"));
        assert!(prompt.contains("0.0 to 1.0"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn test_llm_build_user_prompt() {
        let results = [
            make_test_result("Content about authentication", 0.9),
            make_test_result("Content about databases", 0.8),
        ];
        let indexed: Vec<(usize, &FileSearchResult)> = results.iter().enumerate().collect();

        let prompt = LlmReranker::build_user_prompt("how to authenticate", &indexed);

        assert!(prompt.contains("Query: how to authenticate"));
        assert!(prompt.contains("[Passage 0]"));
        assert!(prompt.contains("[Passage 1]"));
        assert!(prompt.contains("authentication"));
        assert!(prompt.contains("databases"));
    }

    #[test]
    fn test_llm_build_user_prompt_truncates_long_content() {
        let long_content = "x".repeat(2000);
        let results = [make_test_result(&long_content, 0.9)];
        let indexed: Vec<(usize, &FileSearchResult)> = results.iter().enumerate().collect();

        let prompt = LlmReranker::build_user_prompt("query", &indexed);

        // Should be truncated to 1000 chars + "..."
        assert!(prompt.contains("..."));
        assert!(prompt.len() < 2000);
    }

    #[test]
    fn test_llm_build_response_schema() {
        let schema = LlmReranker::build_response_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["scores"].is_object());
        assert_eq!(schema["properties"]["scores"]["type"], "array");
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("scores"))
        );
    }

    #[test]
    fn test_llm_parse_scores_valid() {
        let response = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "{\"scores\": [{\"index\": 0, \"score\": 0.95}, {\"index\": 1, \"score\": 0.6}]}"
                }
            }]
        });
        let body = serde_json::to_vec(&response).unwrap();

        let scores = LlmReranker::parse_scores(&body).unwrap();

        assert_eq!(scores.len(), 2);
        assert_eq!(scores[0].index, 0);
        assert!((scores[0].score - 0.95).abs() < 0.001);
        assert_eq!(scores[1].index, 1);
        assert!((scores[1].score - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_llm_parse_scores_missing_content() {
        let response = serde_json::json!({
            "choices": [{
                "message": {}
            }]
        });
        let body = serde_json::to_vec(&response).unwrap();

        let result = LlmReranker::parse_scores(&body);
        assert!(matches!(result, Err(RerankError::ParseError(_))));
    }

    #[test]
    fn test_llm_parse_scores_invalid_json_content() {
        let response = serde_json::json!({
            "choices": [{
                "message": {
                    "content": "not valid json"
                }
            }]
        });
        let body = serde_json::to_vec(&response).unwrap();

        let result = LlmReranker::parse_scores(&body);
        assert!(matches!(result, Err(RerankError::ParseError(_))));
    }

    #[test]
    fn test_llm_parse_scores_empty_choices() {
        let response = serde_json::json!({
            "choices": []
        });
        let body = serde_json::to_vec(&response).unwrap();

        let result = LlmReranker::parse_scores(&body);
        assert!(matches!(result, Err(RerankError::ParseError(_))));
    }

    #[test]
    fn test_llm_score_struct_deserialization() {
        let json = r#"{"index": 2, "score": 0.75}"#;
        let score: LlmScore = serde_json::from_str(json).unwrap();

        assert_eq!(score.index, 2);
        assert!((score.score - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_llm_scores_response_deserialization() {
        let json = r#"{"scores": [{"index": 0, "score": 0.9}, {"index": 1, "score": 0.5}]}"#;
        let response: LlmScoresResponse = serde_json::from_str(json).unwrap();

        assert_eq!(response.scores.len(), 2);
        assert_eq!(response.scores[0].index, 0);
        assert!((response.scores[0].score - 0.9).abs() < 0.001);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // RerankUsage Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_rerank_usage_new() {
        let usage = RerankUsage::new(100, 50);

        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_rerank_usage_default() {
        let usage = RerankUsage::default();

        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
        assert!(usage.is_empty());
    }

    #[test]
    fn test_rerank_usage_add() {
        let mut usage = RerankUsage::new(100, 50);
        let other = RerankUsage::new(200, 100);

        usage.add(&other);

        assert_eq!(usage.prompt_tokens, 300);
        assert_eq!(usage.completion_tokens, 150);
        assert_eq!(usage.total_tokens, 450);
    }

    #[test]
    fn test_rerank_usage_is_empty() {
        let empty = RerankUsage::default();
        let non_empty = RerankUsage::new(1, 0);

        assert!(empty.is_empty());
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_rerank_response_with_usage() {
        let results = vec![RankedResult::new(make_test_result("a", 0.9), 0.95, 0)];
        let usage = RerankUsage::new(100, 50);

        let response = RerankResponse::new(results, 1).with_usage(usage);

        assert!(response.usage.is_some());
        let u = response.usage.unwrap();
        assert_eq!(u.prompt_tokens, 100);
        assert_eq!(u.completion_tokens, 50);
        assert_eq!(u.total_tokens, 150);
    }

    #[test]
    fn test_llm_parse_usage_openai_format() {
        let response = serde_json::json!({
            "choices": [{"message": {"content": "{\"scores\": []}"}}],
            "usage": {
                "prompt_tokens": 150,
                "completion_tokens": 75,
                "total_tokens": 225
            }
        });
        let body = serde_json::to_vec(&response).unwrap();

        let usage = LlmReranker::parse_usage(&body);

        assert_eq!(usage.prompt_tokens, 150);
        assert_eq!(usage.completion_tokens, 75);
        assert_eq!(usage.total_tokens, 225);
    }

    #[test]
    fn test_llm_parse_usage_alternative_format() {
        let response = serde_json::json!({
            "choices": [{"message": {"content": "{\"scores\": []}"}}],
            "usage": {
                "input_tokens": 200,
                "output_tokens": 100
            }
        });
        let body = serde_json::to_vec(&response).unwrap();

        let usage = LlmReranker::parse_usage(&body);

        assert_eq!(usage.prompt_tokens, 200);
        assert_eq!(usage.completion_tokens, 100);
        assert_eq!(usage.total_tokens, 300);
    }

    #[test]
    fn test_llm_parse_usage_missing() {
        let response = serde_json::json!({
            "choices": [{"message": {"content": "{\"scores\": []}"}}]
        });
        let body = serde_json::to_vec(&response).unwrap();

        let usage = LlmReranker::parse_usage(&body);

        assert!(usage.is_empty());
    }

    #[test]
    fn test_llm_parse_usage_invalid_json() {
        let body = b"not valid json";

        let usage = LlmReranker::parse_usage(body);

        assert!(usage.is_empty());
    }

    #[test]
    fn test_rerank_usage_serialization() {
        let usage = RerankUsage::new(100, 50);
        let json = serde_json::to_string(&usage).unwrap();

        assert!(json.contains("\"prompt_tokens\":100"));
        assert!(json.contains("\"completion_tokens\":50"));
        assert!(json.contains("\"total_tokens\":150"));
    }
}
