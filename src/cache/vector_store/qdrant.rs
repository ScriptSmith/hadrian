//! Qdrant vector database implementation of the VectorStore trait.
//!
//! This implementation uses Qdrant's HTTP API to store and query vector embeddings
//! for semantic caching and vector stores.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use super::{
    ChunkFilter, ChunkSearchResult, ChunkWithEmbedding, HybridSearchConfig, StoredChunk,
    VectorBackend, VectorMetadata, VectorSearchResult, VectorStoreError, VectorStoreResult,
    fusion::fuse_results_limited,
};
use crate::{
    config::DistanceMetric,
    models::{
        AttributeFilter, ComparisonFilter, ComparisonOperator, CompoundFilter, FilterValue,
        LogicalOperator,
    },
    observability::{metrics::record_vector_store_operation, otel_span_error, otel_span_ok},
};

/// Qdrant HTTP API implementation of VectorStore.
pub struct QdrantStore {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    /// VectorStore name for semantic cache embeddings
    qdrant_collection_name: String,
    /// VectorStore name for RAG vector store chunks
    qdrant_chunks_collection_name: String,
    dimensions: usize,
    /// Distance metric for similarity search
    distance_metric: DistanceMetric,
}

impl QdrantStore {
    /// Create a new Qdrant store with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Qdrant server URL (e.g., "http://localhost:6333")
    /// * `api_key` - Optional API key for authentication
    /// * `collection_name` - VectorStore name for storing semantic cache embeddings
    /// * `dimensions` - Embedding vector dimensions
    /// * `distance_metric` - Distance metric for similarity search
    pub fn new(
        base_url: String,
        api_key: Option<String>,
        qdrant_collection_name: String,
        dimensions: usize,
        distance_metric: DistanceMetric,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        // Remove trailing slash from base_url
        let base_url = base_url.trim_end_matches('/').to_string();
        let qdrant_chunks_collection_name = format!("{}_chunks", qdrant_collection_name);

        Self {
            client,
            base_url,
            api_key,
            qdrant_collection_name,
            qdrant_chunks_collection_name,
            dimensions,
            distance_metric,
        }
    }

    /// Convert a Qdrant score to a normalized similarity score (0.0-1.0).
    ///
    /// Qdrant's score interpretation depends on the distance metric:
    /// - Cosine: Returns similarity directly, typically in [-1, 1] for raw or [0, 1] for normalized vectors
    /// - Dot: Returns raw dot product, unbounded
    /// - Euclidean: Returns `1 / (1 + sqrt(distance))`, already in (0, 1] range
    ///
    /// We normalize all metrics to 0.0-1.0 where higher = more similar.
    fn score_to_similarity(&self, score: f64) -> f64 {
        match self.distance_metric {
            DistanceMetric::Cosine => {
                // Qdrant cosine score is already similarity
                // Clamp to [0, 1] for safety (negative values can occur with non-normalized vectors)
                score.clamp(0.0, 1.0)
            }
            DistanceMetric::DotProduct => {
                // For normalized vectors, dot product ranges from -1 to 1
                // Convert to 0-1 range: (1 + score) / 2
                ((1.0 + score) / 2.0).clamp(0.0, 1.0)
            }
            DistanceMetric::Euclidean => {
                // Qdrant already converts Euclidean to similarity via 1/(1+distance)
                // Result is in (0, 1] range, just clamp for safety
                score.clamp(0.0, 1.0)
            }
        }
    }

    /// Convert a similarity threshold (0.0-1.0) to the appropriate Qdrant score threshold.
    ///
    /// This is the inverse of `score_to_similarity`.
    fn similarity_to_score_threshold(&self, threshold: f64) -> f64 {
        match self.distance_metric {
            DistanceMetric::Cosine => {
                // Direct mapping
                threshold
            }
            DistanceMetric::DotProduct => {
                // similarity = (1 + score) / 2, so score = 2 * similarity - 1
                2.0 * threshold - 1.0
            }
            DistanceMetric::Euclidean => {
                // Qdrant handles Euclidean scores internally, direct mapping works
                threshold
            }
        }
    }

    /// Build a request with optional API key header.
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method, &url);
        if let Some(key) = &self.api_key {
            req = req.header("api-key", key);
        }
        req.header("Content-Type", "application/json")
    }

    /// Initialize the semantic cache and chunks collections if they don't exist.
    ///
    /// This should be called once during application startup.
    #[instrument(skip(self), fields(backend = "qdrant", operation = "initialize"))]
    pub async fn initialize(&self) -> VectorStoreResult<()> {
        let start = Instant::now();
        info!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "initialize",
            collection_name = %self.qdrant_collection_name,
            qdrant_chunks_collection_name = %self.qdrant_chunks_collection_name,
            dimensions = self.dimensions,
            "Starting Qdrant initialization"
        );

        // Initialize semantic cache index
        self.initialize_qdrant_collection(&self.qdrant_collection_name)
            .await?;

        // Create payload indexes for semantic cache
        self.create_payload_index(&self.qdrant_collection_name, "model", "keyword")
            .await?;
        self.create_payload_index(&self.qdrant_collection_name, "expires_at", "integer")
            .await?;

        // Initialize chunks index
        self.initialize_qdrant_collection(&self.qdrant_chunks_collection_name)
            .await?;

        // Create payload indexes for chunks index
        self.create_payload_index(
            &self.qdrant_chunks_collection_name,
            "vector_store_id",
            "keyword",
        )
        .await?;
        self.create_payload_index(&self.qdrant_chunks_collection_name, "file_id", "keyword")
            .await?;
        self.create_payload_index(
            &self.qdrant_chunks_collection_name,
            "chunk_index",
            "integer",
        )
        .await?;
        // processing_version enables atomic shadow-copy updates:
        // new chunks are stored with a new version, then old version chunks are deleted
        self.create_payload_index(
            &self.qdrant_chunks_collection_name,
            "processing_version",
            "keyword",
        )
        .await?;

        // Text index on content for full-text search (hybrid search)
        // Qdrant's text index enables keyword/phrase matching alongside vector search
        self.create_payload_index(&self.qdrant_chunks_collection_name, "content", "text")
            .await?;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        record_vector_store_operation("qdrant", "initialize", "success", duration, 1);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "initialize",
            status = "success",
            duration_ms = duration_ms,
            "Qdrant initialization completed"
        );

        otel_span_ok!();
        Ok(())
    }

    /// Initialize a single Qdrant index if it doesn't exist.
    async fn initialize_qdrant_collection(&self, collection_name: &str) -> VectorStoreResult<()> {
        // Check if Qdrant index exists
        let resp = self
            .request(
                reqwest::Method::GET,
                &format!("/collections/{}", collection_name),
            )
            .send()
            .await
            .map_err(|e| VectorStoreError::Http(e.to_string()))?;

        if resp.status().is_success() {
            // VectorStore exists
            return Ok(());
        }

        // Create Qdrant index with configured distance metric
        let create_body = CreateVectorStoreRequest {
            vectors: VectorConfig {
                size: self.dimensions,
                distance: self.distance_metric.qdrant_distance().to_string(),
            },
        };

        let resp = self
            .request(
                reqwest::Method::PUT,
                &format!("/collections/{}", collection_name),
            )
            .json(&create_body)
            .send()
            .await
            .map_err(|e| VectorStoreError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            return Err(VectorStoreError::Database(format!(
                "Failed to create Qdrant index {}: {}",
                collection_name, error_text
            )));
        }

        Ok(())
    }

    /// Create a payload index for efficient filtering.
    async fn create_payload_index(
        &self,
        collection_name: &str,
        field_name: &str,
        field_type: &str,
    ) -> VectorStoreResult<()> {
        let body = CreateIndexRequest {
            field_name: field_name.to_string(),
            field_schema: field_type.to_string(),
        };

        let resp = self
            .request(
                reqwest::Method::PUT,
                &format!("/collections/{}/index", collection_name),
            )
            .json(&body)
            .send()
            .await
            .map_err(|e| VectorStoreError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            // Index might already exist, which is fine
            if !error_text.contains("already exists") {
                tracing::warn!("Failed to create index for {}: {}", field_name, error_text);
            }
        }

        Ok(())
    }

    /// Convert metadata to Qdrant payload format.
    fn metadata_to_payload(
        metadata: &VectorMetadata,
        expires_at: i64,
    ) -> HashMap<String, serde_json::Value> {
        let mut payload = HashMap::new();
        payload.insert(
            "cache_key".to_string(),
            serde_json::json!(metadata.cache_key),
        );
        payload.insert("model".to_string(), serde_json::json!(metadata.model));
        payload.insert(
            "created_at".to_string(),
            serde_json::json!(metadata.created_at),
        );
        payload.insert("ttl_secs".to_string(), serde_json::json!(metadata.ttl_secs));
        payload.insert("expires_at".to_string(), serde_json::json!(expires_at));

        if let Some(org_id) = &metadata.organization_id {
            payload.insert("organization_id".to_string(), serde_json::json!(org_id));
        }
        if let Some(proj_id) = &metadata.project_id {
            payload.insert("project_id".to_string(), serde_json::json!(proj_id));
        }

        payload
    }

    /// Convert Qdrant payload to metadata.
    fn payload_to_metadata(payload: &HashMap<String, serde_json::Value>) -> Option<VectorMetadata> {
        let cache_key = payload.get("cache_key")?.as_str()?.to_string();
        let model = payload.get("model")?.as_str()?.to_string();
        let created_at = payload.get("created_at")?.as_i64()?;
        let ttl_secs = payload.get("ttl_secs")?.as_u64()?;
        let organization_id = payload
            .get("organization_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let project_id = payload
            .get("project_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Some(VectorMetadata {
            cache_key,
            model,
            organization_id,
            project_id,
            created_at,
            ttl_secs,
        })
    }
}

// ============================================================================
// Attribute Filter to Qdrant Filter Conversion
// ============================================================================

/// Result of converting an AttributeFilter to Qdrant filter format.
///
/// Qdrant uses a complex nested JSON structure for filters with:
/// - `must`: array of conditions that ALL must match (AND)
/// - `should`: array of conditions where ANY must match (OR)
/// - `must_not`: array of conditions that must NOT match
#[derive(Debug, Clone)]
pub struct QdrantAttributeFilter {
    /// The Qdrant filter as a JSON value
    pub filter: serde_json::Value,
}

/// Build a Qdrant filter from an AttributeFilter.
///
/// The resulting filter can be merged into the main search filter's `must` array.
///
/// # Arguments
/// * `filter` - The attribute filter to convert
///
/// # Returns
/// A Qdrant filter JSON structure ready to be included in a search request.
///
/// # Example
/// ```ignore
/// let filter = AttributeFilter::eq("author", "John");
/// let qdrant_filter = build_qdrant_attribute_filter(&filter);
/// // Returns: {"key": "metadata.author", "match": {"value": "John"}}
/// ```
pub fn build_qdrant_attribute_filter(filter: &AttributeFilter) -> QdrantAttributeFilter {
    let filter_json = build_filter_condition(filter);
    QdrantAttributeFilter {
        filter: filter_json,
    }
}

/// Recursively build Qdrant filter conditions from an AttributeFilter.
fn build_filter_condition(filter: &AttributeFilter) -> serde_json::Value {
    match filter {
        AttributeFilter::Comparison(comp) => build_comparison_condition(comp),
        AttributeFilter::Compound(compound) => build_compound_condition(compound),
    }
}

/// Build a Qdrant filter condition from a comparison filter.
///
/// Maps comparison operators to Qdrant's filter syntax:
/// - `eq`: `{"key": "metadata.field", "match": {"value": val}}`
/// - `ne`: Wrapped in `must_not`
/// - `gt`, `gte`, `lt`, `lte`: `{"key": "metadata.field", "range": {...}}`
fn build_comparison_condition(comp: &ComparisonFilter) -> serde_json::Value {
    // Qdrant stores chunk metadata in "metadata" payload field
    let key = format!("metadata.{}", comp.key);

    match comp.operator {
        ComparisonOperator::Eq => {
            // Exact match
            let value = filter_value_to_json(&comp.value);
            serde_json::json!({
                "key": key,
                "match": {
                    "value": value
                }
            })
        }
        ComparisonOperator::Ne => {
            // Not equal - wrap in must_not
            let value = filter_value_to_json(&comp.value);
            serde_json::json!({
                "must_not": [{
                    "key": key,
                    "match": {
                        "value": value
                    }
                }]
            })
        }
        ComparisonOperator::Gt => {
            let value = filter_value_to_number(&comp.value);
            serde_json::json!({
                "key": key,
                "range": {
                    "gt": value
                }
            })
        }
        ComparisonOperator::Gte => {
            let value = filter_value_to_number(&comp.value);
            serde_json::json!({
                "key": key,
                "range": {
                    "gte": value
                }
            })
        }
        ComparisonOperator::Lt => {
            let value = filter_value_to_number(&comp.value);
            serde_json::json!({
                "key": key,
                "range": {
                    "lt": value
                }
            })
        }
        ComparisonOperator::Lte => {
            let value = filter_value_to_number(&comp.value);
            serde_json::json!({
                "key": key,
                "range": {
                    "lte": value
                }
            })
        }
    }
}

/// Build a Qdrant compound filter (AND/OR).
fn build_compound_condition(compound: &CompoundFilter) -> serde_json::Value {
    if compound.filters.is_empty() {
        // Empty filter matches everything - return a no-op condition
        // Qdrant doesn't have a "match all" so we return an empty must which is implicitly true
        return serde_json::json!({
            "must": []
        });
    }

    let conditions: Vec<serde_json::Value> = compound
        .filters
        .iter()
        .map(build_filter_condition)
        .collect();

    match compound.operator {
        LogicalOperator::And => {
            serde_json::json!({
                "must": conditions
            })
        }
        LogicalOperator::Or => {
            serde_json::json!({
                "should": conditions
            })
        }
    }
}

/// Convert a FilterValue to JSON for Qdrant match conditions.
fn filter_value_to_json(value: &FilterValue) -> serde_json::Value {
    match value {
        FilterValue::String(s) => serde_json::json!(s),
        FilterValue::Number(n) => serde_json::json!(n),
        FilterValue::Boolean(b) => serde_json::json!(b),
        FilterValue::Array(arr) => {
            let items: Vec<serde_json::Value> = arr
                .iter()
                .map(|item| match item {
                    crate::models::FilterValueItem::String(s) => serde_json::json!(s),
                    crate::models::FilterValueItem::Number(n) => serde_json::json!(n),
                })
                .collect();
            serde_json::json!(items)
        }
    }
}

/// Convert a FilterValue to a number for Qdrant range conditions.
fn filter_value_to_number(value: &FilterValue) -> f64 {
    match value {
        FilterValue::Number(n) => *n,
        FilterValue::String(s) => s.parse().unwrap_or(0.0),
        FilterValue::Boolean(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        FilterValue::Array(_) => 0.0,
    }
}

// Qdrant API request/response types

#[derive(Serialize)]
struct CreateVectorStoreRequest {
    vectors: VectorConfig,
}

#[derive(Serialize)]
struct VectorConfig {
    size: usize,
    distance: String,
}

#[derive(Serialize)]
struct CreateIndexRequest {
    field_name: String,
    field_schema: String,
}

#[derive(Serialize)]
struct UpsertPointsRequest {
    points: Vec<Point>,
}

#[derive(Serialize)]
struct Point {
    id: String,
    vector: Vec<f64>,
    payload: HashMap<String, serde_json::Value>,
}

#[derive(Serialize)]
struct SearchRequest {
    vector: Vec<f64>,
    limit: usize,
    score_threshold: f64,
    with_payload: bool,
    filter: Option<SearchFilter>,
}

#[derive(Serialize)]
struct SearchFilter {
    must: Vec<FilterCondition>,
}

#[derive(Serialize)]
struct FilterCondition {
    key: String,
    #[serde(flatten)]
    condition: FilterMatch,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum FilterMatch {
    Match { value: serde_json::Value },
    Range { gt: i64 },
}

#[derive(Deserialize)]
struct SearchResponse {
    result: Vec<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    score: f64,
    payload: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Serialize)]
struct DeletePointsRequest {
    points: Vec<String>,
}

#[derive(Serialize)]
struct ScrollRequest {
    filter: SearchFilter,
    limit: usize,
    with_payload: bool,
}

#[derive(Deserialize)]
struct ScrollResponse {
    result: ScrollResult,
}

#[derive(Deserialize)]
struct ScrollResult {
    points: Vec<ScrollPoint>,
}

#[derive(Deserialize)]
struct ScrollPoint {
    id: serde_json::Value,
}

// Chunk-specific request/response types

#[derive(Serialize)]
struct UpsertChunkPointsRequest {
    points: Vec<ChunkPoint>,
}

#[derive(Serialize)]
struct ChunkPoint {
    id: String,
    vector: Vec<f64>,
    payload: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct ChunkScrollResponse {
    result: ChunkScrollResult,
}

#[derive(Deserialize)]
struct ChunkScrollResult {
    points: Vec<ChunkScrollPoint>,
}

#[derive(Deserialize)]
struct ChunkScrollPoint {
    id: String,
    payload: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize)]
struct ChunkSearchResponse {
    result: Vec<ChunkSearchResultData>,
}

#[derive(Deserialize)]
struct ChunkSearchResultData {
    id: String,
    score: f64,
    payload: Option<HashMap<String, serde_json::Value>>,
}

#[async_trait]
impl VectorBackend for QdrantStore {
    #[instrument(
        skip(self, embedding, metadata),
        fields(backend = "qdrant", operation = "store")
    )]
    async fn store(
        &self,
        id: &str,
        embedding: &[f64],
        metadata: VectorMetadata,
        ttl: Duration,
    ) -> VectorStoreResult<()> {
        if embedding.len() != self.dimensions {
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "store",
                status = "error",
                error = "dimension_mismatch",
                expected = self.dimensions,
                actual = embedding.len(),
                "Vector dimension mismatch"
            );
            otel_span_error!("Dimension mismatch");
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.dimensions,
                actual: embedding.len(),
            });
        }

        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "store",
            id = %id,
            model = %metadata.model,
            "Starting vector store operation"
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let expires_at = now + ttl.as_secs() as i64;

        let body = UpsertPointsRequest {
            points: vec![Point {
                id: id.to_string(),
                vector: embedding.to_vec(),
                payload: Self::metadata_to_payload(&metadata, expires_at),
            }],
        };

        let resp = self
            .request(
                reqwest::Method::PUT,
                &format!("/collections/{}/points", self.qdrant_collection_name),
            )
            .query(&[("wait", "true")])
            .json(&body)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "upsert", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "store",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Vector store operation failed (HTTP error)"
                );
                otel_span_error!("HTTP error: {}", e);
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "upsert", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "store",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                "Vector store operation failed"
            );
            otel_span_error!("Upsert failed: {}", error_text);
            return Err(VectorStoreError::Database(format!(
                "Failed to upsert point: {}",
                error_text
            )));
        }

        record_vector_store_operation("qdrant", "upsert", "success", duration, 1);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "store",
            status = "success",
            duration_ms = duration_ms,
            item_count = 1,
            "Vector store operation completed"
        );
        otel_span_ok!();
        Ok(())
    }

    #[instrument(skip(self, embedding), fields(backend = "qdrant", operation = "search", limit = limit))]
    async fn search(
        &self,
        embedding: &[f64],
        limit: usize,
        threshold: f64,
        model_filter: Option<&str>,
    ) -> VectorStoreResult<Vec<VectorSearchResult>> {
        if embedding.len() != self.dimensions {
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "search",
                status = "error",
                error = "dimension_mismatch",
                expected = self.dimensions,
                actual = embedding.len(),
                "Vector dimension mismatch"
            );
            otel_span_error!("Dimension mismatch");
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.dimensions,
                actual: embedding.len(),
            });
        }

        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "search",
            limit = limit,
            threshold = threshold,
            model_filter = ?model_filter,
            "Starting vector search operation"
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Build filter conditions
        let mut must = vec![FilterCondition {
            key: "expires_at".to_string(),
            condition: FilterMatch::Range { gt: now },
        }];

        if let Some(model) = model_filter {
            must.push(FilterCondition {
                key: "model".to_string(),
                condition: FilterMatch::Match {
                    value: serde_json::json!(model),
                },
            });
        }

        // Convert similarity threshold to Qdrant score threshold
        let score_threshold = self.similarity_to_score_threshold(threshold);

        let body = SearchRequest {
            vector: embedding.to_vec(),
            limit,
            score_threshold,
            with_payload: true,
            filter: Some(SearchFilter { must }),
        };

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{}/points/search", self.qdrant_collection_name),
            )
            .json(&body)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "search",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Vector search operation failed (HTTP error)"
                );
                otel_span_error!("HTTP error: {}", e);
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "search", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "search",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                "Vector search operation failed"
            );
            otel_span_error!("Search failed: {}", error_text);
            return Err(VectorStoreError::Database(format!(
                "Failed to search: {}",
                error_text
            )));
        }

        let search_resp: SearchResponse = match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "search",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Vector search operation failed (deserialization)"
                );
                otel_span_error!("Deserialization failed: {}", e);
                return Err(VectorStoreError::Serialization(e.to_string()));
            }
        };

        let results: Vec<VectorSearchResult> = search_resp
            .result
            .into_iter()
            .filter_map(|r| {
                let payload = r.payload?;
                let metadata = Self::payload_to_metadata(&payload)?;
                Some(VectorSearchResult {
                    metadata,
                    // Convert Qdrant score to normalized similarity (0.0-1.0)
                    similarity: self.score_to_similarity(r.score),
                })
            })
            .collect();

        let result_count = results.len();
        record_vector_store_operation("qdrant", "search", "success", duration, result_count as u32);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "search",
            status = "success",
            duration_ms = duration_ms,
            item_count = result_count,
            "Vector search operation completed"
        );
        otel_span_ok!();
        Ok(results)
    }

    #[instrument(skip(self), fields(backend = "qdrant", operation = "delete"))]
    async fn delete(&self, id: &str) -> VectorStoreResult<()> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "delete",
            id = %id,
            "Starting vector delete operation"
        );

        let body = DeletePointsRequest {
            points: vec![id.to_string()],
        };

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{}/points/delete", self.qdrant_collection_name),
            )
            .query(&[("wait", "true")])
            .json(&body)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "delete", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "delete",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Vector delete operation failed (HTTP error)"
                );
                otel_span_error!("HTTP error: {}", e);
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "delete", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "delete",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                "Vector delete operation failed"
            );
            otel_span_error!("Delete failed: {}", error_text);
            return Err(VectorStoreError::Database(format!(
                "Failed to delete point: {}",
                error_text
            )));
        }

        record_vector_store_operation("qdrant", "delete", "success", duration, 1);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "delete",
            status = "success",
            duration_ms = duration_ms,
            item_count = 1,
            "Vector delete operation completed"
        );
        otel_span_ok!();
        Ok(())
    }

    #[instrument(skip(self), fields(backend = "qdrant", operation = "cleanup_expired"))]
    async fn cleanup_expired(&self) -> VectorStoreResult<usize> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "cleanup_expired",
            "Starting expired vectors cleanup"
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Find expired points using scroll
        let scroll_body = ScrollRequest {
            filter: SearchFilter {
                must: vec![FilterCondition {
                    key: "expires_at".to_string(),
                    condition: FilterMatch::Range { gt: 0 }, // We'll filter client-side for <= now
                }],
            },
            limit: 1000,
            with_payload: true,
        };

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{}/points/scroll", self.qdrant_collection_name),
            )
            .json(&scroll_body)
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                let duration = start.elapsed().as_secs_f64();
                let duration_ms = (duration * 1000.0) as u64;
                record_vector_store_operation("qdrant", "cleanup", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "cleanup_expired",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Expired vectors cleanup failed (scroll HTTP error)"
                );
                otel_span_error!("Scroll HTTP error: {}", e);
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            let duration = start.elapsed().as_secs_f64();
            let duration_ms = (duration * 1000.0) as u64;
            record_vector_store_operation("qdrant", "cleanup", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "cleanup_expired",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                "Expired vectors cleanup failed (scroll error)"
            );
            otel_span_error!("Scroll failed: {}", error_text);
            return Err(VectorStoreError::Database(format!(
                "Failed to scroll: {}",
                error_text
            )));
        }

        let scroll_resp: ScrollResponse = match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                let duration = start.elapsed().as_secs_f64();
                let duration_ms = (duration * 1000.0) as u64;
                record_vector_store_operation("qdrant", "cleanup", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "cleanup_expired",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Expired vectors cleanup failed (deserialization)"
                );
                otel_span_error!("Deserialization failed: {}", e);
                return Err(VectorStoreError::Serialization(e.to_string()));
            }
        };

        // Collect IDs of expired points
        let expired_ids: Vec<String> = scroll_resp
            .result
            .points
            .iter()
            .filter_map(|p| match &p.id {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .collect();

        if expired_ids.is_empty() {
            let duration = start.elapsed().as_secs_f64();
            let duration_ms = (duration * 1000.0) as u64;
            record_vector_store_operation("qdrant", "cleanup", "success", duration, 0);
            info!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "cleanup_expired",
                status = "success",
                duration_ms = duration_ms,
                item_count = 0,
                "Expired vectors cleanup completed (no expired entries)"
            );
            otel_span_ok!();
            return Ok(0);
        }

        // Delete expired points - use filter-based deletion for expired entries
        let delete_filter = serde_json::json!({
            "filter": {
                "must": [{
                    "key": "expires_at",
                    "range": {
                        "lte": now
                    }
                }]
            }
        });

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{}/points/delete", self.qdrant_collection_name),
            )
            .query(&[("wait", "true")])
            .json(&delete_filter)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "cleanup", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "cleanup_expired",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Expired vectors cleanup failed (delete HTTP error)"
                );
                otel_span_error!("Delete HTTP error: {}", e);
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "cleanup", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "cleanup_expired",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                "Expired vectors cleanup failed (delete error)"
            );
            otel_span_error!("Delete failed: {}", error_text);
            return Err(VectorStoreError::Database(format!(
                "Failed to delete expired points: {}",
                error_text
            )));
        }

        // Return approximate count (actual count requires separate call)
        let count = expired_ids.len();
        record_vector_store_operation("qdrant", "cleanup", "success", duration, count as u32);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "cleanup_expired",
            status = "success",
            duration_ms = duration_ms,
            item_count = count,
            "Expired vectors cleanup completed"
        );
        otel_span_ok!();
        Ok(count)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    #[instrument(skip(self), fields(backend = "qdrant", operation = "health_check"))]
    async fn health_check(&self) -> VectorStoreResult<()> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "health_check",
            "Starting health check"
        );

        let resp = self
            .request(
                reqwest::Method::GET,
                &format!("/collections/{}", self.qdrant_collection_name),
            )
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "health_check", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "health_check",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Health check failed (HTTP error)"
                );
                otel_span_error!("HTTP error: {}", e);
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            record_vector_store_operation("qdrant", "health_check", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "health_check",
                status = "error",
                duration_ms = duration_ms,
                collection = %self.qdrant_collection_name,
                "Health check failed - Qdrant index not available"
            );
            otel_span_error!("VectorStore not available");
            return Err(VectorStoreError::Unavailable(format!(
                "VectorStore {} not available",
                self.qdrant_collection_name
            )));
        }

        record_vector_store_operation("qdrant", "health_check", "success", duration, 1);
        debug!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "health_check",
            status = "success",
            duration_ms = duration_ms,
            "Health check completed"
        );

        otel_span_ok!();
        Ok(())
    }

    // ========================================================================
    // RAG VectorStore Chunk Operations
    // ========================================================================

    #[instrument(
        skip(self, chunks),
        fields(backend = "qdrant", operation = "store_chunks")
    )]
    async fn store_chunks(&self, chunks: Vec<ChunkWithEmbedding>) -> VectorStoreResult<()> {
        if chunks.is_empty() {
            return Ok(());
        }

        let chunk_count = chunks.len();
        let vector_store_id = chunks.first().map(|c| c.vector_store_id);

        // Validate dimensions
        for chunk in &chunks {
            if chunk.embedding.len() != self.dimensions {
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "store_chunks",
                    status = "error",
                    error = "dimension_mismatch",
                    expected = self.dimensions,
                    actual = chunk.embedding.len(),
                    "Chunk embedding dimension mismatch"
                );
                return Err(VectorStoreError::DimensionMismatch {
                    expected: self.dimensions,
                    actual: chunk.embedding.len(),
                });
            }
        }

        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "store_chunks",
            vector_store_id = ?vector_store_id,
            chunk_count = chunk_count,
            "Starting chunk store operation"
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let points: Vec<ChunkPoint> = chunks
            .into_iter()
            .map(|chunk| {
                let mut payload = HashMap::new();
                payload.insert(
                    "vector_store_id".to_string(),
                    serde_json::json!(chunk.vector_store_id.to_string()),
                );
                payload.insert(
                    "file_id".to_string(),
                    serde_json::json!(chunk.file_id.to_string()),
                );
                payload.insert(
                    "chunk_index".to_string(),
                    serde_json::json!(chunk.chunk_index),
                );
                payload.insert("content".to_string(), serde_json::json!(chunk.content));
                payload.insert(
                    "token_count".to_string(),
                    serde_json::json!(chunk.token_count),
                );
                payload.insert(
                    "char_start".to_string(),
                    serde_json::json!(chunk.char_start),
                );
                payload.insert("char_end".to_string(), serde_json::json!(chunk.char_end));
                payload.insert("created_at".to_string(), serde_json::json!(now));
                payload.insert(
                    "processing_version".to_string(),
                    serde_json::json!(chunk.processing_version.to_string()),
                );
                if let Some(metadata) = chunk.metadata {
                    payload.insert("metadata".to_string(), metadata);
                }

                ChunkPoint {
                    id: chunk.id.to_string(),
                    vector: chunk.embedding,
                    payload,
                }
            })
            .collect();

        let body = UpsertChunkPointsRequest { points };

        let resp = self
            .request(
                reqwest::Method::PUT,
                &format!("/collections/{}/points", self.qdrant_chunks_collection_name),
            )
            .query(&[("wait", "true")])
            .json(&body)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "insert", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "store_chunks",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Chunk store operation failed (HTTP error)"
                );
                otel_span_error!("HTTP error: {}", e);
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "insert", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "store_chunks",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                "Chunk store operation failed"
            );
            otel_span_error!("Upsert failed: {}", error_text);
            return Err(VectorStoreError::Database(format!(
                "Failed to upsert chunks: {}",
                error_text
            )));
        }

        record_vector_store_operation("qdrant", "insert", "success", duration, chunk_count as u32);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "store_chunks",
            status = "success",
            duration_ms = duration_ms,
            item_count = chunk_count,
            vector_store_id = ?vector_store_id,
            "Chunk store operation completed"
        );
        otel_span_ok!();
        Ok(())
    }

    #[instrument(skip(self), fields(backend = "qdrant", operation = "get_chunks_by_file", file_id = %file_id))]
    async fn get_chunks_by_file(&self, file_id: Uuid) -> VectorStoreResult<Vec<StoredChunk>> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "get_chunks_by_file",
            file_id = %file_id,
            "Starting get chunks by file operation"
        );

        // Scroll through all chunks for this file
        let filter = serde_json::json!({
            "filter": {
                "must": [{
                    "key": "file_id",
                    "match": {
                        "value": file_id.to_string()
                    }
                }]
            },
            "limit": 10000,
            "with_payload": true
        });

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!(
                    "/collections/{}/points/scroll",
                    self.qdrant_chunks_collection_name
                ),
            )
            .json(&filter)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "get_chunks", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "get_chunks_by_file",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    file_id = %file_id,
                    "Get chunks by file failed (HTTP error)"
                );
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            let duration = start.elapsed().as_secs_f64();
            let duration_ms = (duration * 1000.0) as u64;
            record_vector_store_operation("qdrant", "get_chunks", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "get_chunks_by_file",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                file_id = %file_id,
                "Get chunks by file failed"
            );
            return Err(VectorStoreError::Database(format!(
                "Failed to scroll chunks: {}",
                error_text
            )));
        }

        let scroll_resp: ChunkScrollResponse = match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                let duration = start.elapsed().as_secs_f64();
                let duration_ms = (duration * 1000.0) as u64;
                record_vector_store_operation("qdrant", "get_chunks", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "get_chunks_by_file",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    file_id = %file_id,
                    "Get chunks by file failed (deserialization)"
                );
                return Err(VectorStoreError::Serialization(e.to_string()));
            }
        };

        let mut chunks: Vec<StoredChunk> = scroll_resp
            .result
            .points
            .into_iter()
            .filter_map(|p| {
                let payload = p.payload?;
                Some(StoredChunk {
                    id: p.id.parse().ok()?,
                    vector_store_id: payload.get("vector_store_id")?.as_str()?.parse().ok()?,
                    file_id: payload.get("file_id")?.as_str()?.parse().ok()?,
                    chunk_index: payload.get("chunk_index")?.as_i64()? as i32,
                    content: payload.get("content")?.as_str()?.to_string(),
                    token_count: payload.get("token_count")?.as_i64()? as i32,
                    char_start: payload.get("char_start")?.as_i64()? as i32,
                    char_end: payload.get("char_end")?.as_i64()? as i32,
                    metadata: payload.get("metadata").cloned(),
                    created_at: payload.get("created_at")?.as_i64()?,
                    processing_version: payload
                        .get("processing_version")?
                        .as_str()?
                        .parse()
                        .ok()?,
                })
            })
            .collect();

        // Sort by chunk_index
        chunks.sort_by_key(|c| c.chunk_index);

        let chunk_count = chunks.len();
        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        record_vector_store_operation(
            "qdrant",
            "get_chunks",
            "success",
            duration,
            chunk_count as u32,
        );
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "get_chunks_by_file",
            status = "success",
            duration_ms = duration_ms,
            item_count = chunk_count,
            file_id = %file_id,
            "Get chunks by file completed"
        );

        Ok(chunks)
    }

    #[instrument(skip(self), fields(backend = "qdrant", operation = "delete_chunks_by_file", file_id = %file_id))]
    async fn delete_chunks_by_file(&self, file_id: Uuid) -> VectorStoreResult<u64> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "delete_chunks_by_file",
            file_id = %file_id,
            "Starting delete chunks by file operation"
        );

        // First, get count of chunks to delete
        let chunks = self.get_chunks_by_file(file_id).await?;
        let count = chunks.len() as u64;

        if count == 0 {
            let duration = start.elapsed().as_secs_f64();
            let duration_ms = (duration * 1000.0) as u64;
            record_vector_store_operation("qdrant", "delete", "success", duration, 0);
            info!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "delete_chunks_by_file",
                status = "success",
                duration_ms = duration_ms,
                item_count = 0,
                file_id = %file_id,
                "Delete chunks by file completed (no chunks)"
            );
            return Ok(0);
        }

        // Delete by filter
        let delete_filter = serde_json::json!({
            "filter": {
                "must": [{
                    "key": "file_id",
                    "match": {
                        "value": file_id.to_string()
                    }
                }]
            }
        });

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!(
                    "/collections/{}/points/delete",
                    self.qdrant_chunks_collection_name
                ),
            )
            .query(&[("wait", "true")])
            .json(&delete_filter)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "delete", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "delete_chunks_by_file",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    file_id = %file_id,
                    "Delete chunks by file failed (HTTP error)"
                );
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "delete", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "delete_chunks_by_file",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                file_id = %file_id,
                "Delete chunks by file failed"
            );
            return Err(VectorStoreError::Database(format!(
                "Failed to delete chunks by file: {}",
                error_text
            )));
        }

        record_vector_store_operation("qdrant", "delete", "success", duration, count as u32);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "delete_chunks_by_file",
            status = "success",
            duration_ms = duration_ms,
            item_count = count,
            file_id = %file_id,
            "Delete chunks by file completed"
        );
        Ok(count)
    }

    #[instrument(skip(self), fields(backend = "qdrant", operation = "delete_chunks_by_file_and_vector_store", file_id = %file_id, vector_store_id = %vector_store_id))]
    async fn delete_chunks_by_file_and_vector_store(
        &self,
        file_id: Uuid,
        vector_store_id: Uuid,
    ) -> VectorStoreResult<u64> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "delete_chunks_by_file_and_vector_store",
            file_id = %file_id,
            vector_store_id = %vector_store_id,
            "Starting delete chunks by file and vector store operation"
        );

        // Delete by filter matching both file_id AND vector_store_id
        let delete_filter = serde_json::json!({
            "filter": {
                "must": [
                    {
                        "key": "file_id",
                        "match": {
                            "value": file_id.to_string()
                        }
                    },
                    {
                        "key": "vector_store_id",
                        "match": {
                            "value": vector_store_id.to_string()
                        }
                    }
                ]
            }
        });

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!(
                    "/collections/{}/points/delete",
                    self.qdrant_chunks_collection_name
                ),
            )
            .query(&[("wait", "true")])
            .json(&delete_filter)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "delete", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "delete_chunks_by_file_and_vector_store",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    file_id = %file_id,
                    vector_store_id = %vector_store_id,
                    "Delete chunks by file and vector store failed (HTTP error)"
                );
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "delete", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "delete_chunks_by_file_and_vector_store",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                file_id = %file_id,
                vector_store_id = %vector_store_id,
                "Delete chunks by file and vector store failed"
            );
            return Err(VectorStoreError::Database(format!(
                "Failed to delete chunks by file and vector store: {}",
                error_text
            )));
        }

        // Qdrant doesn't return count of deleted items in filter-based delete
        // Return 0 as we can't know the exact count without pre-counting
        record_vector_store_operation("qdrant", "delete", "success", duration, 0);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "delete_chunks_by_file_and_vector_store",
            status = "success",
            duration_ms = duration_ms,
            item_count = 0,
            file_id = %file_id,
            vector_store_id = %vector_store_id,
            "Delete chunks by file and vector store completed"
        );
        Ok(0)
    }

    #[instrument(skip(self), fields(backend = "qdrant", operation = "delete_chunks_by_file_and_vector_store_except_version", file_id = %file_id, vector_store_id = %vector_store_id, keep_version = %keep_version))]
    async fn delete_chunks_by_file_and_vector_store_except_version(
        &self,
        file_id: Uuid,
        vector_store_id: Uuid,
        keep_version: Uuid,
    ) -> VectorStoreResult<u64> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "delete_chunks_by_file_and_vector_store_except_version",
            file_id = %file_id,
            vector_store_id = %vector_store_id,
            keep_version = %keep_version,
            "Starting delete chunks by file and vector store except version operation"
        );

        // Delete by filter matching file_id AND vector_store_id, but NOT keep_version
        let delete_filter = serde_json::json!({
            "filter": {
                "must": [
                    {
                        "key": "file_id",
                        "match": {
                            "value": file_id.to_string()
                        }
                    },
                    {
                        "key": "vector_store_id",
                        "match": {
                            "value": vector_store_id.to_string()
                        }
                    }
                ],
                "must_not": [
                    {
                        "key": "processing_version",
                        "match": {
                            "value": keep_version.to_string()
                        }
                    }
                ]
            }
        });

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!(
                    "/collections/{}/points/delete",
                    self.qdrant_chunks_collection_name
                ),
            )
            .query(&[("wait", "true")])
            .json(&delete_filter)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "delete", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "delete_chunks_by_file_and_vector_store_except_version",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    file_id = %file_id,
                    vector_store_id = %vector_store_id,
                    keep_version = %keep_version,
                    "Delete chunks by file and vector store except version failed (HTTP error)"
                );
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "delete", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "delete_chunks_by_file_and_vector_store_except_version",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                file_id = %file_id,
                vector_store_id = %vector_store_id,
                keep_version = %keep_version,
                "Delete chunks by file and vector store except version failed"
            );
            return Err(VectorStoreError::Database(format!(
                "Failed to delete chunks: {}",
                error_text
            )));
        }

        // Qdrant doesn't return count for filter-based deletes, so we return 0
        // The operation succeeded, but we don't know exact count
        record_vector_store_operation("qdrant", "delete", "success", duration, 0);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "delete_chunks_by_file_and_vector_store_except_version",
            status = "success",
            duration_ms = duration_ms,
            item_count = 0,
            file_id = %file_id,
            vector_store_id = %vector_store_id,
            keep_version = %keep_version,
            "Delete chunks by file and vector store except version completed"
        );
        Ok(0)
    }

    #[instrument(skip(self), fields(backend = "qdrant", operation = "delete_chunks_by_vector_store", vector_store_id = %vector_store_id))]
    async fn delete_chunks_by_vector_store(&self, vector_store_id: Uuid) -> VectorStoreResult<u64> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "delete_chunks_by_vector_store",
            vector_store_id = %vector_store_id,
            "Starting delete chunks by vector store operation"
        );

        // We can't easily count deleted items in Qdrant, so we delete by filter
        // and return an estimate
        let delete_filter = serde_json::json!({
            "filter": {
                "must": [{
                    "key": "vector_store_id",
                    "match": {
                        "value": vector_store_id.to_string()
                    }
                }]
            }
        });

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!(
                    "/collections/{}/points/delete",
                    self.qdrant_chunks_collection_name
                ),
            )
            .query(&[("wait", "true")])
            .json(&delete_filter)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "delete", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "delete_chunks_by_vector_store",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    vector_store_id = %vector_store_id,
                    "Delete chunks by vector store failed (HTTP error)"
                );
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "delete", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "delete_chunks_by_vector_store",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                vector_store_id = %vector_store_id,
                "Delete chunks by vector store failed"
            );
            return Err(VectorStoreError::Database(format!(
                "Failed to delete chunks by vector store: {}",
                error_text
            )));
        }

        // Qdrant doesn't return count of deleted items in filter-based delete
        // Return 0 to indicate success without count
        record_vector_store_operation("qdrant", "delete", "success", duration, 0);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "delete_chunks_by_vector_store",
            status = "success",
            duration_ms = duration_ms,
            item_count = 0,
            vector_store_id = %vector_store_id,
            "Delete chunks by vector store completed"
        );
        Ok(0)
    }

    async fn search_vector_store(
        &self,
        vector_store_id: Uuid,
        embedding: &[f64],
        limit: usize,
        threshold: f64,
        filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        self.search_vector_stores(&[vector_store_id], embedding, limit, threshold, filter)
            .await
    }

    #[instrument(skip(self, embedding, filter), fields(backend = "qdrant", operation = "search_vector_stores", vector_store_count = vector_store_ids.len(), limit = limit))]
    async fn search_vector_stores(
        &self,
        vector_store_ids: &[Uuid],
        embedding: &[f64],
        limit: usize,
        threshold: f64,
        filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        if embedding.len() != self.dimensions {
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "search_vector_stores",
                status = "error",
                error = "dimension_mismatch",
                expected = self.dimensions,
                actual = embedding.len(),
                "Vector dimension mismatch"
            );
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.dimensions,
                actual: embedding.len(),
            });
        }

        if vector_store_ids.is_empty() {
            return Ok(vec![]);
        }

        let start = Instant::now();
        let vector_store_count = vector_store_ids.len();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "search_vector_stores",
            vector_store_count = vector_store_count,
            limit = limit,
            threshold = threshold,
            has_filter = filter.is_some(),
            "Starting vector store search operation"
        );

        // Build filter conditions
        let mut must_conditions = Vec::new();

        // VectorStore filter
        if vector_store_ids.len() == 1 {
            must_conditions.push(serde_json::json!({
                "key": "vector_store_id",
                "match": {
                    "value": vector_store_ids[0].to_string()
                }
            }));
        } else {
            // Multiple vector stores - use should (OR) within a must
            let should_conditions: Vec<_> = vector_store_ids
                .iter()
                .map(|id| {
                    serde_json::json!({
                        "key": "vector_store_id",
                        "match": {
                            "value": id.to_string()
                        }
                    })
                })
                .collect();
            must_conditions.push(serde_json::json!({
                "should": should_conditions
            }));
        }

        // File filter
        if let Some(ref f) = filter
            && let Some(ref file_ids) = f.file_ids
            && !file_ids.is_empty()
        {
            if file_ids.len() == 1 {
                must_conditions.push(serde_json::json!({
                    "key": "file_id",
                    "match": {
                        "value": file_ids[0].to_string()
                    }
                }));
            } else {
                let should_conditions: Vec<_> = file_ids
                    .iter()
                    .map(|id| {
                        serde_json::json!({
                            "key": "file_id",
                            "match": {
                                "value": id.to_string()
                            }
                        })
                    })
                    .collect();
                must_conditions.push(serde_json::json!({
                    "should": should_conditions
                }));
            }
        }

        // Attribute filter
        if let Some(ref f) = filter
            && let Some(ref attr_filter) = f.attribute_filter
        {
            let qdrant_filter = build_qdrant_attribute_filter(attr_filter);
            must_conditions.push(qdrant_filter.filter);
        }

        // Convert similarity threshold to Qdrant score threshold
        let score_threshold = self.similarity_to_score_threshold(threshold);

        let search_body = serde_json::json!({
            "vector": embedding,
            "limit": limit,
            "score_threshold": score_threshold,
            "with_payload": true,
            "filter": {
                "must": must_conditions
            }
        });

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!(
                    "/collections/{}/points/search",
                    self.qdrant_chunks_collection_name
                ),
            )
            .json(&search_body)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "search_vector_stores",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    vector_store_count = vector_store_count,
                    "VectorStore search operation failed (HTTP error)"
                );
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "search", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "search_vector_stores",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                vector_store_count = vector_store_count,
                "VectorStore search operation failed"
            );
            return Err(VectorStoreError::Database(format!(
                "Failed to search chunks: {}",
                error_text
            )));
        }

        let search_resp: ChunkSearchResponse = match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "search_vector_stores",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    vector_store_count = vector_store_count,
                    "VectorStore search operation failed (deserialization)"
                );
                return Err(VectorStoreError::Serialization(e.to_string()));
            }
        };

        let results: Vec<ChunkSearchResult> = search_resp
            .result
            .into_iter()
            .filter_map(|r| {
                let payload = r.payload?;
                Some(ChunkSearchResult {
                    chunk_id: r.id.parse().ok()?,
                    vector_store_id: payload.get("vector_store_id")?.as_str()?.parse().ok()?,
                    file_id: payload.get("file_id")?.as_str()?.parse().ok()?,
                    chunk_index: payload.get("chunk_index")?.as_i64()? as i32,
                    content: payload.get("content")?.as_str()?.to_string(),
                    // Convert Qdrant score to normalized similarity (0.0-1.0)
                    score: self.score_to_similarity(r.score),
                    metadata: payload.get("metadata").cloned(),
                })
            })
            .collect();

        let result_count = results.len();
        record_vector_store_operation("qdrant", "search", "success", duration, result_count as u32);
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "search_vector_stores",
            status = "success",
            duration_ms = duration_ms,
            item_count = result_count,
            vector_store_count = vector_store_count,
            "VectorStore search operation completed"
        );
        Ok(results)
    }

    // ========================================================================
    // Keyword Search Operations (for Hybrid Search)
    // ========================================================================

    async fn keyword_search_vector_store(
        &self,
        vector_store_id: Uuid,
        query: &str,
        limit: usize,
        filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        self.keyword_search_vector_stores(&[vector_store_id], query, limit, filter)
            .await
    }

    #[instrument(skip(self, filter), fields(backend = "qdrant", operation = "keyword_search_vector_stores", vector_store_count = vector_store_ids.len(), limit = limit))]
    async fn keyword_search_vector_stores(
        &self,
        vector_store_ids: &[Uuid],
        query: &str,
        limit: usize,
        filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        if vector_store_ids.is_empty() {
            return Ok(vec![]);
        }

        // Empty query returns no results
        let query = query.trim();
        if query.is_empty() {
            return Ok(vec![]);
        }

        let start = Instant::now();
        let vector_store_count = vector_store_ids.len();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "keyword_search_vector_stores",
            vector_store_count = vector_store_count,
            limit = limit,
            query = %query,
            has_filter = filter.is_some(),
            "Starting keyword search operation"
        );

        // Build filter conditions
        let mut must_conditions = Vec::new();

        // VectorStore filter
        if vector_store_ids.len() == 1 {
            must_conditions.push(serde_json::json!({
                "key": "vector_store_id",
                "match": {
                    "value": vector_store_ids[0].to_string()
                }
            }));
        } else {
            // Multiple vector stores - use should (OR) within a must
            let should_conditions: Vec<_> = vector_store_ids
                .iter()
                .map(|id| {
                    serde_json::json!({
                        "key": "vector_store_id",
                        "match": {
                            "value": id.to_string()
                        }
                    })
                })
                .collect();
            must_conditions.push(serde_json::json!({
                "should": should_conditions
            }));
        }

        // File filter
        if let Some(ref f) = filter
            && let Some(ref file_ids) = f.file_ids
            && !file_ids.is_empty()
        {
            if file_ids.len() == 1 {
                must_conditions.push(serde_json::json!({
                    "key": "file_id",
                    "match": {
                        "value": file_ids[0].to_string()
                    }
                }));
            } else {
                let should_conditions: Vec<_> = file_ids
                    .iter()
                    .map(|id| {
                        serde_json::json!({
                            "key": "file_id",
                            "match": {
                                "value": id.to_string()
                            }
                        })
                    })
                    .collect();
                must_conditions.push(serde_json::json!({
                    "should": should_conditions
                }));
            }
        }

        // Attribute filter
        if let Some(ref f) = filter
            && let Some(ref attr_filter) = f.attribute_filter
        {
            let qdrant_filter = build_qdrant_attribute_filter(attr_filter);
            must_conditions.push(qdrant_filter.filter);
        }

        // Add text search filter on content field
        // Qdrant text search uses the "text" match type for indexed text fields
        must_conditions.push(serde_json::json!({
            "key": "content",
            "match": {
                "text": query
            }
        }));

        // Use scroll with filter since Qdrant's text search doesn't have
        // built-in ranking like PostgreSQL's ts_rank. We retrieve matches
        // and assign uniform scores.
        //
        // Note: Qdrant's text index is less sophisticated than PostgreSQL's
        // full-text search - it performs keyword matching without BM25/TF-IDF
        // ranking. For better hybrid search results, consider using PostgreSQL
        // as the primary backend.
        let scroll_body = serde_json::json!({
            "filter": {
                "must": must_conditions
            },
            "limit": limit,
            "with_payload": true
        });

        let resp = self
            .request(
                reqwest::Method::POST,
                &format!(
                    "/collections/{}/points/scroll",
                    self.qdrant_chunks_collection_name
                ),
            )
            .json(&scroll_body)
            .send()
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "keyword_search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "keyword_search_vector_stores",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    vector_store_count = vector_store_count,
                    "Keyword search operation failed (HTTP error)"
                );
                return Err(VectorStoreError::Http(e.to_string()));
            }
        };

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            record_vector_store_operation("qdrant", "keyword_search", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "qdrant",
                operation = "keyword_search_vector_stores",
                status = "error",
                duration_ms = duration_ms,
                error = %error_text,
                vector_store_count = vector_store_count,
                "Keyword search operation failed"
            );
            return Err(VectorStoreError::Database(format!(
                "Failed to search chunks: {}",
                error_text
            )));
        }

        let scroll_resp: ChunkScrollResponse = match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                record_vector_store_operation("qdrant", "keyword_search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "keyword_search_vector_stores",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    vector_store_count = vector_store_count,
                    "Keyword search operation failed (deserialization)"
                );
                return Err(VectorStoreError::Serialization(e.to_string()));
            }
        };

        // Qdrant scroll doesn't return scores for text matching,
        // so we assign a uniform score of 0.5 to all matches.
        // This is a known limitation compared to PostgreSQL's ts_rank.
        let results: Vec<ChunkSearchResult> = scroll_resp
            .result
            .points
            .into_iter()
            .filter_map(|p| {
                let payload = p.payload?;
                Some(ChunkSearchResult {
                    chunk_id: p.id.parse().ok()?,
                    vector_store_id: payload.get("vector_store_id")?.as_str()?.parse().ok()?,
                    file_id: payload.get("file_id")?.as_str()?.parse().ok()?,
                    chunk_index: payload.get("chunk_index")?.as_i64()? as i32,
                    content: payload.get("content")?.as_str()?.to_string(),
                    // Uniform score since Qdrant text search doesn't rank results
                    score: 0.5,
                    metadata: payload.get("metadata").cloned(),
                })
            })
            .collect();

        let result_count = results.len();
        record_vector_store_operation(
            "qdrant",
            "keyword_search",
            "success",
            duration,
            result_count as u32,
        );
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "keyword_search_vector_stores",
            status = "success",
            duration_ms = duration_ms,
            item_count = result_count,
            vector_store_count = vector_store_count,
            "Keyword search operation completed"
        );
        Ok(results)
    }

    // ========================================================================
    // Hybrid Search Operations
    // ========================================================================

    async fn hybrid_search_vector_store(
        &self,
        vector_store_id: Uuid,
        query: &str,
        embedding: &[f64],
        limit: usize,
        config: HybridSearchConfig,
        filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        self.hybrid_search_vector_stores(
            &[vector_store_id],
            query,
            embedding,
            limit,
            config,
            filter,
        )
        .await
    }

    #[instrument(skip(self, embedding, filter, config), fields(backend = "qdrant", operation = "hybrid_search_vector_stores", vector_store_count = vector_store_ids.len(), limit = limit))]
    async fn hybrid_search_vector_stores(
        &self,
        vector_store_ids: &[Uuid],
        query: &str,
        embedding: &[f64],
        limit: usize,
        config: HybridSearchConfig,
        filter: Option<ChunkFilter>,
    ) -> VectorStoreResult<Vec<ChunkSearchResult>> {
        if vector_store_ids.is_empty() {
            return Ok(vec![]);
        }

        let start = Instant::now();
        let vector_store_count = vector_store_ids.len();
        debug!(
            stage = "vector_operation_started",
            backend = "qdrant",
            operation = "hybrid_search_vector_stores",
            vector_store_count = vector_store_count,
            limit = limit,
            vector_threshold = config.vector_threshold,
            has_filter = filter.is_some(),
            "Starting hybrid search operation"
        );

        // Request more results from each search to have good candidates for fusion
        // RRF benefits from having overlapping results from both sources
        let search_limit = limit * 3;

        // Run vector and keyword searches in parallel
        let vector_future = self.search_vector_stores(
            vector_store_ids,
            embedding,
            search_limit,
            config.vector_threshold,
            filter.clone(),
        );
        let keyword_future =
            self.keyword_search_vector_stores(vector_store_ids, query, search_limit, filter);

        let (vector_result, keyword_result) = tokio::join!(vector_future, keyword_future);

        // Handle errors from either search
        let vector_results = match vector_result {
            Ok(results) => results,
            Err(e) => {
                let duration = start.elapsed().as_secs_f64();
                let duration_ms = (duration * 1000.0) as u64;
                record_vector_store_operation("qdrant", "hybrid_search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "hybrid_search_vector_stores",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    error_source = "vector_search",
                    vector_store_count = vector_store_count,
                    "Hybrid search failed (vector search error)"
                );
                otel_span_error!("Vector search failed: {}", e);
                return Err(e);
            }
        };

        let keyword_results = match keyword_result {
            Ok(results) => results,
            Err(e) => {
                let duration = start.elapsed().as_secs_f64();
                let duration_ms = (duration * 1000.0) as u64;
                record_vector_store_operation("qdrant", "hybrid_search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "qdrant",
                    operation = "hybrid_search_vector_stores",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    error_source = "keyword_search",
                    vector_store_count = vector_store_count,
                    "Hybrid search failed (keyword search error)"
                );
                otel_span_error!("Keyword search failed: {}", e);
                return Err(e);
            }
        };

        // Fuse results using RRF
        let fused_results =
            fuse_results_limited(&vector_results, &keyword_results, &config.rrf, limit);

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        let result_count = fused_results.len();
        record_vector_store_operation(
            "qdrant",
            "hybrid_search",
            "success",
            duration,
            result_count as u32,
        );
        info!(
            stage = "vector_operation_completed",
            backend = "qdrant",
            operation = "hybrid_search_vector_stores",
            status = "success",
            duration_ms = duration_ms,
            item_count = result_count,
            vector_count = vector_results.len(),
            keyword_count = keyword_results.len(),
            vector_store_count = vector_store_count,
            "Hybrid search operation completed"
        );
        otel_span_ok!();
        Ok(fused_results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_to_payload() {
        let metadata = VectorMetadata {
            cache_key: "sha256:abc123".to_string(),
            model: "gpt-4".to_string(),
            organization_id: Some("org-123".to_string()),
            project_id: None,
            created_at: 1699999999,
            ttl_secs: 3600,
        };
        let expires_at = 1700003599;

        let payload = QdrantStore::metadata_to_payload(&metadata, expires_at);

        assert_eq!(payload.get("cache_key").unwrap(), "sha256:abc123");
        assert_eq!(payload.get("model").unwrap(), "gpt-4");
        assert_eq!(payload.get("organization_id").unwrap(), "org-123");
        assert!(!payload.contains_key("project_id"));
        assert_eq!(payload.get("created_at").unwrap(), 1699999999);
        assert_eq!(payload.get("ttl_secs").unwrap(), 3600);
        assert_eq!(payload.get("expires_at").unwrap(), 1700003599);
    }

    #[test]
    fn test_payload_to_metadata() {
        let mut payload = HashMap::new();
        payload.insert("cache_key".to_string(), serde_json::json!("key123"));
        payload.insert("model".to_string(), serde_json::json!("claude-3"));
        payload.insert("created_at".to_string(), serde_json::json!(1700000000_i64));
        payload.insert("ttl_secs".to_string(), serde_json::json!(7200_u64));
        payload.insert("organization_id".to_string(), serde_json::json!("org-456"));

        let metadata = QdrantStore::payload_to_metadata(&payload).unwrap();

        assert_eq!(metadata.cache_key, "key123");
        assert_eq!(metadata.model, "claude-3");
        assert_eq!(metadata.organization_id, Some("org-456".to_string()));
        assert_eq!(metadata.project_id, None);
        assert_eq!(metadata.created_at, 1700000000);
        assert_eq!(metadata.ttl_secs, 7200);
    }

    #[test]
    fn test_payload_to_metadata_missing_field() {
        let mut payload = HashMap::new();
        payload.insert("cache_key".to_string(), serde_json::json!("key123"));
        // Missing model field

        let metadata = QdrantStore::payload_to_metadata(&payload);
        assert!(metadata.is_none());
    }

    // ========================================================================
    // Attribute Filter to Qdrant Filter Tests
    // ========================================================================

    #[test]
    fn test_qdrant_filter_string_eq() {
        let filter = AttributeFilter::eq("author", "John Doe");
        let result = build_qdrant_attribute_filter(&filter);

        assert_eq!(
            result.filter,
            serde_json::json!({
                "key": "metadata.author",
                "match": {
                    "value": "John Doe"
                }
            })
        );
    }

    #[test]
    fn test_qdrant_filter_number_gt() {
        let filter = AttributeFilter::gt("score", 0.5);
        let result = build_qdrant_attribute_filter(&filter);

        assert_eq!(
            result.filter,
            serde_json::json!({
                "key": "metadata.score",
                "range": {
                    "gt": 0.5
                }
            })
        );
    }

    #[test]
    fn test_qdrant_filter_number_gte() {
        let filter = AttributeFilter::gte("date", 1704067200);
        let result = build_qdrant_attribute_filter(&filter);

        assert_eq!(
            result.filter,
            serde_json::json!({
                "key": "metadata.date",
                "range": {
                    "gte": 1704067200.0
                }
            })
        );
    }

    #[test]
    fn test_qdrant_filter_number_lt() {
        let filter = AttributeFilter::lt("priority", 5);
        let result = build_qdrant_attribute_filter(&filter);

        assert_eq!(
            result.filter,
            serde_json::json!({
                "key": "metadata.priority",
                "range": {
                    "lt": 5.0
                }
            })
        );
    }

    #[test]
    fn test_qdrant_filter_number_lte() {
        let filter = AttributeFilter::lte("count", 100);
        let result = build_qdrant_attribute_filter(&filter);

        assert_eq!(
            result.filter,
            serde_json::json!({
                "key": "metadata.count",
                "range": {
                    "lte": 100.0
                }
            })
        );
    }

    #[test]
    fn test_qdrant_filter_ne() {
        let filter = AttributeFilter::ne("status", "draft");
        let result = build_qdrant_attribute_filter(&filter);

        assert_eq!(
            result.filter,
            serde_json::json!({
                "must_not": [{
                    "key": "metadata.status",
                    "match": {
                        "value": "draft"
                    }
                }]
            })
        );
    }

    #[test]
    fn test_qdrant_filter_boolean_eq() {
        let filter = AttributeFilter::eq("is_active", true);
        let result = build_qdrant_attribute_filter(&filter);

        assert_eq!(
            result.filter,
            serde_json::json!({
                "key": "metadata.is_active",
                "match": {
                    "value": true
                }
            })
        );
    }

    #[test]
    fn test_qdrant_filter_compound_and() {
        let filter = AttributeFilter::and(vec![
            AttributeFilter::eq("category", "docs"),
            AttributeFilter::gte("date", 1704067200),
        ]);
        let result = build_qdrant_attribute_filter(&filter);

        assert_eq!(
            result.filter,
            serde_json::json!({
                "must": [
                    {
                        "key": "metadata.category",
                        "match": {
                            "value": "docs"
                        }
                    },
                    {
                        "key": "metadata.date",
                        "range": {
                            "gte": 1704067200.0
                        }
                    }
                ]
            })
        );
    }

    #[test]
    fn test_qdrant_filter_compound_or() {
        let filter = AttributeFilter::or(vec![
            AttributeFilter::eq("status", "active"),
            AttributeFilter::eq("status", "pending"),
        ]);
        let result = build_qdrant_attribute_filter(&filter);

        assert_eq!(
            result.filter,
            serde_json::json!({
                "should": [
                    {
                        "key": "metadata.status",
                        "match": {
                            "value": "active"
                        }
                    },
                    {
                        "key": "metadata.status",
                        "match": {
                            "value": "pending"
                        }
                    }
                ]
            })
        );
    }

    #[test]
    fn test_qdrant_filter_nested_compound() {
        let filter = AttributeFilter::and(vec![
            AttributeFilter::eq("category", "documentation"),
            AttributeFilter::or(vec![
                AttributeFilter::eq("author", "Alice"),
                AttributeFilter::eq("author", "Bob"),
            ]),
        ]);
        let result = build_qdrant_attribute_filter(&filter);

        assert_eq!(
            result.filter,
            serde_json::json!({
                "must": [
                    {
                        "key": "metadata.category",
                        "match": {
                            "value": "documentation"
                        }
                    },
                    {
                        "should": [
                            {
                                "key": "metadata.author",
                                "match": {
                                    "value": "Alice"
                                }
                            },
                            {
                                "key": "metadata.author",
                                "match": {
                                    "value": "Bob"
                                }
                            }
                        ]
                    }
                ]
            })
        );
    }

    #[test]
    fn test_qdrant_filter_empty_compound() {
        let filter = AttributeFilter::and(vec![]);
        let result = build_qdrant_attribute_filter(&filter);

        // Empty compound returns empty must array (matches all)
        assert_eq!(
            result.filter,
            serde_json::json!({
                "must": []
            })
        );
    }
}
