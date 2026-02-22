//! PostgreSQL with pgvector extension implementation of the VectorStore trait.
//!
//! This implementation uses the pgvector extension to store and query vector embeddings
//! for semantic caching and vector stores. It supports both IVFFlat and HNSW indexing strategies.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use super::{
    ChunkFilter, ChunkSearchResult, ChunkWithEmbedding, HybridSearchConfig, StoredChunk,
    VectorBackend, VectorMetadata, VectorSearchResult, VectorStoreError, VectorStoreResult,
    fusion::fuse_results_limited,
};
use crate::{
    config::{DistanceMetric, PgvectorIndexType},
    models::{
        AttributeFilter, ComparisonFilter, ComparisonOperator, CompoundFilter, FilterValue,
        LogicalOperator,
    },
    observability::{metrics::record_vector_store_operation, otel_span_error, otel_span_ok},
};

/// PostgreSQL pgvector implementation of VectorStore.
pub struct PgvectorStore {
    pool: PgPool,
    /// Table name for semantic cache embeddings
    table_name: String,
    /// Table name for RAG vector store chunks
    chunks_table_name: String,
    dimensions: usize,
    index_type: PgvectorIndexType,
    /// Distance metric for similarity search
    distance_metric: DistanceMetric,
}

impl PgvectorStore {
    /// Create a new pgvector store with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `pool` - PostgreSQL connection pool
    /// * `table_name` - Table name for storing semantic cache embeddings
    /// * `dimensions` - Embedding vector dimensions
    /// * `index_type` - Type of vector index to use
    /// * `distance_metric` - Distance metric for similarity search
    pub fn new(
        pool: PgPool,
        table_name: String,
        dimensions: usize,
        index_type: PgvectorIndexType,
        distance_metric: DistanceMetric,
    ) -> Self {
        let chunks_table_name = format!("{}_chunks", table_name);
        Self {
            pool,
            table_name,
            chunks_table_name,
            dimensions,
            index_type,
            distance_metric,
        }
    }

    /// Convert a distance/result value to a similarity score in the 0.0-1.0 range.
    ///
    /// Different metrics return different value ranges:
    /// - Cosine: pgvector returns distance (0-2), convert via `1 - distance`
    /// - DotProduct: pgvector `<#>` returns negative inner product, convert via `(1 + value) / 2` for normalized vectors
    /// - Euclidean: pgvector returns L2 distance (0-∞), convert via `1 / (1 + distance)`
    fn distance_to_similarity(&self, distance: f64) -> f64 {
        match self.distance_metric {
            DistanceMetric::Cosine => {
                // Cosine distance is (1 - cosine_similarity), so similarity = 1 - distance
                // Result is in 0.0-1.0 range for normalized vectors
                1.0 - distance
            }
            DistanceMetric::DotProduct => {
                // pgvector's <#> operator returns negative inner product
                // For normalized vectors, inner product ranges from -1 to 1
                // We convert to 0-1 range: (1 + inner_product) / 2
                // Since pgvector returns negative, we negate first: -(-inner_product) = inner_product
                (1.0 - distance) / 2.0
            }
            DistanceMetric::Euclidean => {
                // Euclidean distance ranges from 0 to infinity
                // We convert to 0-1 range: 1 / (1 + distance)
                // Distance 0 -> similarity 1, distance ∞ -> similarity 0
                1.0 / (1.0 + distance)
            }
        }
    }

    /// Convert a similarity threshold (0.0-1.0) to the appropriate distance threshold.
    ///
    /// This is the inverse of `distance_to_similarity`.
    fn similarity_to_distance_threshold(&self, threshold: f64) -> f64 {
        match self.distance_metric {
            DistanceMetric::Cosine => {
                // similarity = 1 - distance, so distance = 1 - similarity
                1.0 - threshold
            }
            DistanceMetric::DotProduct => {
                // similarity = (1 - distance) / 2, so distance = 1 - 2*similarity
                1.0 - 2.0 * threshold
            }
            DistanceMetric::Euclidean => {
                // similarity = 1 / (1 + distance), so distance = (1 - similarity) / similarity
                // Handle edge case where threshold is 0 (would give infinity)
                if threshold <= 0.0 {
                    f64::MAX
                } else {
                    (1.0 - threshold) / threshold
                }
            }
        }
    }

    /// Initialize the pgvector extension and create the embeddings and chunks tables.
    ///
    /// This should be called once during application startup.
    #[instrument(skip(self), fields(backend = "pgvector", operation = "initialize"))]
    pub async fn initialize(&self) -> VectorStoreResult<()> {
        let start = Instant::now();
        info!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "initialize",
            table_name = %self.table_name,
            chunks_table_name = %self.chunks_table_name,
            dimensions = self.dimensions,
            "Starting pgvector initialization"
        );

        // Enable pgvector extension
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Create the semantic cache embeddings table
        let create_table = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {} (
                id TEXT PRIMARY KEY,
                embedding vector({}) NOT NULL,
                cache_key TEXT NOT NULL,
                model TEXT NOT NULL,
                organization_id TEXT,
                project_id TEXT,
                created_at BIGINT NOT NULL,
                ttl_secs BIGINT NOT NULL,
                expires_at BIGINT NOT NULL
            )
            "#,
            self.table_name, self.dimensions
        );
        sqlx::query(&create_table)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Create indexes for semantic cache table
        let index_name = format!("{}_embedding_idx", self.table_name);
        let ops_class = self.distance_metric.pgvector_ops_class();
        let index_sql = match self.index_type {
            PgvectorIndexType::IvfFlat => {
                // IVFFlat: faster to build, good for moderate dataset sizes
                // lists parameter: typically sqrt(n) for n rows, we use 100 as a reasonable default
                format!(
                    r#"
                    CREATE INDEX IF NOT EXISTS {} ON {}
                    USING ivfflat (embedding {})
                    WITH (lists = 100)
                    "#,
                    index_name, self.table_name, ops_class
                )
            }
            PgvectorIndexType::Hnsw => {
                // HNSW: better query performance, slower to build
                // m=16, ef_construction=64 are reasonable defaults
                format!(
                    r#"
                    CREATE INDEX IF NOT EXISTS {} ON {}
                    USING hnsw (embedding {})
                    WITH (m = 16, ef_construction = 64)
                    "#,
                    index_name, self.table_name, ops_class
                )
            }
        };
        sqlx::query(&index_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Create index on expires_at for efficient cleanup
        let expires_idx = format!(
            "CREATE INDEX IF NOT EXISTS {}_expires_idx ON {} (expires_at)",
            self.table_name, self.table_name
        );
        sqlx::query(&expires_idx)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Create index on model for efficient filtering
        let model_idx = format!(
            "CREATE INDEX IF NOT EXISTS {}_model_idx ON {} (model)",
            self.table_name, self.table_name
        );
        sqlx::query(&model_idx)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Create the RAG vector store chunks table
        // Includes content_tsvector column for full-text search (hybrid search)
        // The processing_version column enables atomic shadow-copy updates:
        // new chunks are stored with a new version, then old version chunks are deleted
        let create_chunks_table = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {} (
                id UUID PRIMARY KEY,
                vector_store_id UUID NOT NULL,
                file_id UUID NOT NULL,
                chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                token_count INTEGER NOT NULL,
                char_start INTEGER NOT NULL,
                char_end INTEGER NOT NULL,
                embedding vector({}) NOT NULL,
                metadata JSONB,
                created_at BIGINT NOT NULL,
                content_tsvector TSVECTOR,
                processing_version UUID NOT NULL,
                UNIQUE(vector_store_id, file_id, chunk_index, processing_version)
            )
            "#,
            self.chunks_table_name, self.dimensions
        );
        sqlx::query(&create_chunks_table)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Add content_tsvector column if it doesn't exist (for existing tables)
        let add_tsvector_column = format!(
            r#"
            DO $$
            BEGIN
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name = '{}' AND column_name = 'content_tsvector'
                ) THEN
                    ALTER TABLE {} ADD COLUMN content_tsvector TSVECTOR;
                END IF;
            END $$;
            "#,
            self.chunks_table_name, self.chunks_table_name
        );
        sqlx::query(&add_tsvector_column)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Add processing_version column if it doesn't exist (for existing tables)
        // This enables atomic shadow-copy updates for re-processing files
        let add_processing_version_column = format!(
            r#"
            DO $$
            BEGIN
                IF NOT EXISTS (
                    SELECT 1 FROM information_schema.columns
                    WHERE table_name = '{}' AND column_name = 'processing_version'
                ) THEN
                    -- Add column with a default UUID for existing rows
                    ALTER TABLE {} ADD COLUMN processing_version UUID NOT NULL DEFAULT gen_random_uuid();
                    -- Remove the default for new rows (they must provide a version)
                    ALTER TABLE {} ALTER COLUMN processing_version DROP DEFAULT;
                END IF;
            END $$;
            "#,
            self.chunks_table_name, self.chunks_table_name, self.chunks_table_name
        );
        sqlx::query(&add_processing_version_column)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Update the unique constraint to include processing_version (for existing tables)
        // This allows shadow-copy: new chunks with new version can coexist with old chunks
        let update_unique_constraint = format!(
            r#"
            DO $$
            DECLARE
                constraint_name TEXT;
            BEGIN
                -- Find the old unique constraint on (vector_store_id, file_id, chunk_index)
                SELECT c.conname INTO constraint_name
                FROM pg_constraint c
                JOIN pg_class t ON c.conrelid = t.oid
                WHERE t.relname = '{}'
                  AND c.contype = 'u'
                  AND array_length(c.conkey, 1) = 3;

                -- Drop the old constraint if found and create new one
                IF constraint_name IS NOT NULL THEN
                    EXECUTE 'ALTER TABLE {} DROP CONSTRAINT ' || constraint_name;
                    ALTER TABLE {} ADD CONSTRAINT {}_vector_store_file_chunk_version_unique
                        UNIQUE (vector_store_id, file_id, chunk_index, processing_version);
                END IF;
            END $$;
            "#,
            self.chunks_table_name,
            self.chunks_table_name,
            self.chunks_table_name,
            self.chunks_table_name
        );
        sqlx::query(&update_unique_constraint)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Create indexes for chunks table
        let chunks_embedding_idx = format!("{}_embedding_idx", self.chunks_table_name);
        let chunks_index_sql = match self.index_type {
            PgvectorIndexType::IvfFlat => format!(
                r#"
                CREATE INDEX IF NOT EXISTS {} ON {}
                USING ivfflat (embedding {})
                WITH (lists = 100)
                "#,
                chunks_embedding_idx, self.chunks_table_name, ops_class
            ),
            PgvectorIndexType::Hnsw => format!(
                r#"
                CREATE INDEX IF NOT EXISTS {} ON {}
                USING hnsw (embedding {})
                WITH (m = 16, ef_construction = 64)
                "#,
                chunks_embedding_idx, self.chunks_table_name, ops_class
            ),
        };
        sqlx::query(&chunks_index_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Index on vector_store_id for efficient vector store-scoped searches
        let vector_store_idx = format!(
            "CREATE INDEX IF NOT EXISTS {}_vector_store_idx ON {} (vector_store_id)",
            self.chunks_table_name, self.chunks_table_name
        );
        sqlx::query(&vector_store_idx)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Index on file_id for efficient file-scoped operations
        let file_idx = format!(
            "CREATE INDEX IF NOT EXISTS {}_file_idx ON {} (file_id)",
            self.chunks_table_name, self.chunks_table_name
        );
        sqlx::query(&file_idx)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // Composite index for efficient version-based cleanup during shadow-copy updates
        // Supports queries like: DELETE FROM chunks WHERE file_id = ? AND vector_store_id = ? AND processing_version != ?
        let version_idx = format!(
            "CREATE INDEX IF NOT EXISTS {}_file_collection_version_idx ON {} (file_id, vector_store_id, processing_version)",
            self.chunks_table_name, self.chunks_table_name
        );
        sqlx::query(&version_idx)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        // GIN index on content_tsvector for full-text search (hybrid search)
        // Uses GIN (Generalized Inverted Index) which is optimal for tsvector queries
        let tsvector_idx = format!(
            "CREATE INDEX IF NOT EXISTS {}_content_tsvector_idx ON {} USING GIN (content_tsvector)",
            self.chunks_table_name, self.chunks_table_name
        );
        sqlx::query(&tsvector_idx)
            .execute(&self.pool)
            .await
            .map_err(|e| VectorStoreError::Database(e.to_string()))?;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        record_vector_store_operation("pgvector", "initialize", "success", duration, 1);
        info!(
            stage = "vector_operation_completed",
            backend = "pgvector",
            operation = "initialize",
            status = "success",
            duration_ms = duration_ms,
            "Pgvector initialization completed"
        );

        otel_span_ok!();
        Ok(())
    }

    /// Convert f64 slice to the format expected by pgvector.
    fn vec_to_pgvector(vec: &[f64]) -> String {
        let values: Vec<String> = vec.iter().map(|v| v.to_string()).collect();
        format!("[{}]", values.join(","))
    }
}

// ============================================================================
// Attribute Filter SQL Generation
// ============================================================================

/// A bind value for parameterized SQL queries.
#[derive(Debug, Clone)]
pub enum SqlBindValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

/// Result of building an attribute filter SQL clause.
#[derive(Debug)]
pub struct AttributeFilterSql {
    /// The SQL WHERE clause fragment (e.g., "(metadata->>'key' = $5)")
    pub clause: String,
    /// The bind values to use with the clause
    pub bind_values: Vec<SqlBindValue>,
}

/// Build a SQL WHERE clause from an AttributeFilter for PostgreSQL JSONB metadata.
///
/// # Arguments
/// * `filter` - The attribute filter to convert
/// * `start_param_idx` - The starting parameter index (e.g., if you already have $1-$4, pass 5)
///
/// # Returns
/// A SQL clause and the bind values to use with it.
///
/// # Example
/// ```ignore
/// let filter = AttributeFilter::eq("author", "John");
/// let result = build_attribute_filter_sql(&filter, 5);
/// // result.clause = "(metadata->>'author' = $5)"
/// // result.bind_values = [SqlBindValue::String("John")]
/// ```
pub fn build_attribute_filter_sql(
    filter: &AttributeFilter,
    start_param_idx: usize,
) -> AttributeFilterSql {
    let mut bind_values = Vec::new();
    let clause = build_filter_clause(filter, start_param_idx, &mut bind_values);
    AttributeFilterSql {
        clause,
        bind_values,
    }
}

/// Recursively build the SQL clause for a filter.
fn build_filter_clause(
    filter: &AttributeFilter,
    param_idx: usize,
    bind_values: &mut Vec<SqlBindValue>,
) -> String {
    match filter {
        AttributeFilter::Comparison(comp) => build_comparison_clause(comp, param_idx, bind_values),
        AttributeFilter::Compound(compound) => {
            build_compound_clause(compound, param_idx, bind_values)
        }
    }
}

/// Build SQL for a comparison filter.
fn build_comparison_clause(
    comp: &ComparisonFilter,
    param_idx: usize,
    bind_values: &mut Vec<SqlBindValue>,
) -> String {
    let key = &comp.key;
    let op = match comp.operator {
        ComparisonOperator::Eq => "=",
        ComparisonOperator::Ne => "!=",
        ComparisonOperator::Gt => ">",
        ComparisonOperator::Gte => ">=",
        ComparisonOperator::Lt => "<",
        ComparisonOperator::Lte => "<=",
    };

    // Generate SQL based on value type
    // PostgreSQL JSONB: metadata->>'key' extracts as text
    // For numeric comparisons, we cast to double precision
    // For boolean comparisons, we cast to boolean
    match &comp.value {
        FilterValue::String(s) => {
            bind_values.push(SqlBindValue::String(s.clone()));
            format!("(metadata->>'{}' {} ${})", key, op, param_idx)
        }
        FilterValue::Number(n) => {
            bind_values.push(SqlBindValue::Number(*n));
            // Cast to double precision for numeric comparison
            format!(
                "((metadata->>'{}')::double precision {} ${})",
                key, op, param_idx
            )
        }
        FilterValue::Boolean(b) => {
            bind_values.push(SqlBindValue::Boolean(*b));
            // Cast to boolean for boolean comparison
            format!("((metadata->>'{}')::boolean {} ${})", key, op, param_idx)
        }
        FilterValue::Array(_) => {
            // Array values are for future `in`/`nin` support
            // For now, treat as unsupported and return a no-match clause
            "FALSE".to_string()
        }
    }
}

/// Build SQL for a compound filter (AND/OR).
fn build_compound_clause(
    compound: &CompoundFilter,
    mut param_idx: usize,
    bind_values: &mut Vec<SqlBindValue>,
) -> String {
    if compound.filters.is_empty() {
        // Empty compound filter matches everything
        return "TRUE".to_string();
    }

    let logical_op = match compound.operator {
        LogicalOperator::And => " AND ",
        LogicalOperator::Or => " OR ",
    };

    let mut clauses = Vec::new();
    for sub_filter in &compound.filters {
        let clause = build_filter_clause(sub_filter, param_idx, bind_values);
        // Count how many bind values were added for this sub-filter
        let values_added = count_filter_binds(sub_filter);
        param_idx += values_added;
        clauses.push(clause);
    }

    format!("({})", clauses.join(logical_op))
}

/// Count how many bind values a filter will produce.
fn count_filter_binds(filter: &AttributeFilter) -> usize {
    match filter {
        AttributeFilter::Comparison(comp) => {
            // Array values don't add binds (they produce FALSE)
            match &comp.value {
                FilterValue::Array(_) => 0,
                _ => 1,
            }
        }
        AttributeFilter::Compound(compound) => {
            compound.filters.iter().map(count_filter_binds).sum()
        }
    }
}

#[async_trait]
impl VectorBackend for PgvectorStore {
    #[instrument(
        skip(self, embedding, metadata),
        fields(backend = "pgvector", operation = "store")
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
                backend = "pgvector",
                operation = "store",
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

        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
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
        let embedding_str = Self::vec_to_pgvector(embedding);

        let query = format!(
            r#"
            INSERT INTO {} (id, embedding, cache_key, model, organization_id, project_id, created_at, ttl_secs, expires_at)
            VALUES ($1, $2::vector, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO UPDATE SET
                embedding = EXCLUDED.embedding,
                cache_key = EXCLUDED.cache_key,
                model = EXCLUDED.model,
                organization_id = EXCLUDED.organization_id,
                project_id = EXCLUDED.project_id,
                created_at = EXCLUDED.created_at,
                ttl_secs = EXCLUDED.ttl_secs,
                expires_at = EXCLUDED.expires_at
            "#,
            self.table_name
        );

        let result = sqlx::query(&query)
            .bind(id)
            .bind(&embedding_str)
            .bind(&metadata.cache_key)
            .bind(&metadata.model)
            .bind(&metadata.organization_id)
            .bind(&metadata.project_id)
            .bind(metadata.created_at)
            .bind(metadata.ttl_secs as i64)
            .bind(expires_at)
            .execute(&self.pool)
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        match result {
            Ok(_) => {
                record_vector_store_operation("pgvector", "upsert", "success", duration, 1);
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "store",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = 1,
                    "Vector store operation completed"
                );
                Ok(())
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "upsert", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "store",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Vector store operation failed"
                );
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
    }

    #[instrument(skip(self, embedding), fields(backend = "pgvector", operation = "search", limit = limit))]
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
                backend = "pgvector",
                operation = "search",
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

        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
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
        let embedding_str = Self::vec_to_pgvector(embedding);

        // Convert similarity threshold to distance threshold for the configured metric
        let distance_threshold = self.similarity_to_distance_threshold(threshold);
        let op = self.distance_metric.pgvector_operator();

        // Build query with optional model filter
        // We select the raw distance and convert to similarity in Rust
        let query = if model_filter.is_some() {
            format!(
                r#"
                SELECT
                    id,
                    cache_key,
                    model,
                    organization_id,
                    project_id,
                    created_at,
                    ttl_secs,
                    (embedding {op} $1::vector) as distance
                FROM {}
                WHERE expires_at > $2
                  AND model = $3
                  AND (embedding {op} $1::vector) < $4
                ORDER BY embedding {op} $1::vector
                LIMIT $5
                "#,
                self.table_name
            )
        } else {
            format!(
                r#"
                SELECT
                    id,
                    cache_key,
                    model,
                    organization_id,
                    project_id,
                    created_at,
                    ttl_secs,
                    (embedding {op} $1::vector) as distance
                FROM {}
                WHERE expires_at > $2
                  AND (embedding {op} $1::vector) < $3
                ORDER BY embedding {op} $1::vector
                LIMIT $4
                "#,
                self.table_name
            )
        };

        #[derive(sqlx::FromRow)]
        struct SearchRow {
            cache_key: String,
            model: String,
            organization_id: Option<String>,
            project_id: Option<String>,
            created_at: i64,
            ttl_secs: i64,
            distance: f64,
        }

        let result: Result<Vec<SearchRow>, _> = if let Some(model) = model_filter {
            sqlx::query_as(&query)
                .bind(&embedding_str)
                .bind(now)
                .bind(model)
                .bind(distance_threshold)
                .bind(limit as i32)
                .fetch_all(&self.pool)
                .await
        } else {
            sqlx::query_as(&query)
                .bind(&embedding_str)
                .bind(now)
                .bind(distance_threshold)
                .bind(limit as i32)
                .fetch_all(&self.pool)
                .await
        };

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        match result {
            Ok(rows) => {
                let results: Vec<VectorSearchResult> = rows
                    .into_iter()
                    .map(|row| VectorSearchResult {
                        metadata: VectorMetadata {
                            cache_key: row.cache_key,
                            model: row.model,
                            organization_id: row.organization_id,
                            project_id: row.project_id,
                            created_at: row.created_at,
                            ttl_secs: row.ttl_secs as u64,
                        },
                        // Convert raw distance to normalized similarity (0.0-1.0)
                        similarity: self.distance_to_similarity(row.distance),
                    })
                    .collect();
                let result_count = results.len();
                record_vector_store_operation(
                    "pgvector",
                    "search",
                    "success",
                    duration,
                    result_count as u32,
                );
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "search",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = result_count,
                    "Vector search operation completed"
                );
                otel_span_ok!();
                Ok(results)
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "search",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Vector search operation failed"
                );
                otel_span_error!("Search failed: {}", e);
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
    }

    #[instrument(skip(self), fields(backend = "pgvector", operation = "delete"))]
    async fn delete(&self, id: &str) -> VectorStoreResult<()> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "delete",
            id = %id,
            "Starting vector delete operation"
        );

        let query = format!("DELETE FROM {} WHERE id = $1", self.table_name);
        let result = sqlx::query(&query).bind(id).execute(&self.pool).await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        match result {
            Ok(r) => {
                let deleted_count = r.rows_affected() as u32;
                record_vector_store_operation(
                    "pgvector",
                    "delete",
                    "success",
                    duration,
                    deleted_count,
                );
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "delete",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = deleted_count,
                    "Vector delete operation completed"
                );
                otel_span_ok!();
                Ok(())
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "delete", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "delete",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Vector delete operation failed"
                );
                otel_span_error!("Delete failed: {}", e);
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
    }

    #[instrument(
        skip(self),
        fields(backend = "pgvector", operation = "cleanup_expired")
    )]
    async fn cleanup_expired(&self) -> VectorStoreResult<usize> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "cleanup_expired",
            "Starting expired vectors cleanup"
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let query = format!("DELETE FROM {} WHERE expires_at <= $1", self.table_name);
        let result = sqlx::query(&query).bind(now).execute(&self.pool).await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        match result {
            Ok(r) => {
                let count = r.rows_affected() as usize;
                record_vector_store_operation(
                    "pgvector",
                    "cleanup",
                    "success",
                    duration,
                    count as u32,
                );
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "cleanup_expired",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = count,
                    "Expired vectors cleanup completed"
                );
                otel_span_ok!();
                Ok(count)
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "cleanup", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "cleanup_expired",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Expired vectors cleanup failed"
                );
                otel_span_error!("Cleanup failed: {}", e);
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    #[instrument(skip(self), fields(backend = "pgvector", operation = "health_check"))]
    async fn health_check(&self) -> VectorStoreResult<()> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "health_check",
            "Starting health check"
        );

        // Check that the table exists and pgvector is available
        let query = format!("SELECT EXISTS(SELECT 1 FROM {} LIMIT 1)", self.table_name);
        let result = sqlx::query(&query).execute(&self.pool).await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;

        match result {
            Ok(_) => {
                record_vector_store_operation("pgvector", "health_check", "success", duration, 1);
                debug!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "health_check",
                    status = "success",
                    duration_ms = duration_ms,
                    "Health check completed"
                );
                otel_span_ok!();
                Ok(())
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "health_check", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "health_check",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Health check failed"
                );
                otel_span_error!("Health check failed: {}", e);
                Err(VectorStoreError::Unavailable(e.to_string()))
            }
        }
    }

    // ========================================================================
    // RAG VectorStore Chunk Operations
    // ========================================================================

    #[instrument(
        skip(self, chunks),
        fields(backend = "pgvector", operation = "store_chunks")
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
                    backend = "pgvector",
                    operation = "store_chunks",
                    status = "error",
                    error = "dimension_mismatch",
                    expected = self.dimensions,
                    actual = chunk.embedding.len(),
                    "Chunk embedding dimension mismatch"
                );
                otel_span_error!("Dimension mismatch");
                return Err(VectorStoreError::DimensionMismatch {
                    expected: self.dimensions,
                    actual: chunk.embedding.len(),
                });
            }
        }

        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "store_chunks",
            vector_store_id = ?vector_store_id,
            chunk_count = chunk_count,
            "Starting chunk store operation"
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use a transaction for atomicity
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                let duration = start.elapsed().as_secs_f64();
                let duration_ms = (duration * 1000.0) as u64;
                record_vector_store_operation("pgvector", "insert", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "store_chunks",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Failed to begin transaction"
                );
                otel_span_error!("Transaction begin failed: {}", e);
                return Err(VectorStoreError::Database(e.to_string()));
            }
        };

        for chunk in chunks {
            let embedding_str = Self::vec_to_pgvector(&chunk.embedding);
            let metadata_json = chunk
                .metadata
                .map(|m| serde_json::to_string(&m).unwrap_or_default());

            // Insert chunk with content_tsvector computed using to_tsvector()
            // Using 'english' config for stemming and stop words; this can be made
            // configurable in the future for multi-language support.
            // Note: With shadow-copy, each processing_version creates new rows.
            // Old versions are cleaned up after successful processing.
            let query = format!(
                r#"
                INSERT INTO {} (
                    id, vector_store_id, file_id, chunk_index, content,
                    token_count, char_start, char_end, embedding, metadata, created_at,
                    content_tsvector, processing_version
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::vector, $10::jsonb, $11,
                        to_tsvector('english', $5), $12)
                ON CONFLICT (vector_store_id, file_id, chunk_index, processing_version) DO UPDATE SET
                    id = EXCLUDED.id,
                    content = EXCLUDED.content,
                    token_count = EXCLUDED.token_count,
                    char_start = EXCLUDED.char_start,
                    char_end = EXCLUDED.char_end,
                    embedding = EXCLUDED.embedding,
                    metadata = EXCLUDED.metadata,
                    created_at = EXCLUDED.created_at,
                    content_tsvector = EXCLUDED.content_tsvector
                "#,
                self.chunks_table_name
            );

            if let Err(e) = sqlx::query(&query)
                .bind(chunk.id)
                .bind(chunk.vector_store_id)
                .bind(chunk.file_id)
                .bind(chunk.chunk_index)
                .bind(&chunk.content)
                .bind(chunk.token_count)
                .bind(chunk.char_start)
                .bind(chunk.char_end)
                .bind(&embedding_str)
                .bind(metadata_json)
                .bind(now)
                .bind(chunk.processing_version)
                .execute(&mut *tx)
                .await
            {
                let duration = start.elapsed().as_secs_f64();
                let duration_ms = (duration * 1000.0) as u64;
                record_vector_store_operation("pgvector", "insert", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "store_chunks",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    "Failed to insert chunk"
                );
                otel_span_error!("Insert chunk failed: {}", e);
                return Err(VectorStoreError::Database(e.to_string()));
            }
        }

        if let Err(e) = tx.commit().await {
            let duration = start.elapsed().as_secs_f64();
            let duration_ms = (duration * 1000.0) as u64;
            record_vector_store_operation("pgvector", "insert", "error", duration, 0);
            warn!(
                stage = "vector_operation_completed",
                backend = "pgvector",
                operation = "store_chunks",
                status = "error",
                duration_ms = duration_ms,
                error = %e,
                "Failed to commit transaction"
            );
            otel_span_error!("Commit failed: {}", e);
            return Err(VectorStoreError::Database(e.to_string()));
        }

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        record_vector_store_operation(
            "pgvector",
            "insert",
            "success",
            duration,
            chunk_count as u32,
        );
        info!(
            stage = "vector_operation_completed",
            backend = "pgvector",
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

    #[instrument(skip(self), fields(backend = "pgvector", operation = "get_chunks_by_file", file_id = %file_id))]
    async fn get_chunks_by_file(&self, file_id: Uuid) -> VectorStoreResult<Vec<StoredChunk>> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "get_chunks_by_file",
            file_id = %file_id,
            "Starting get chunks by file operation"
        );

        #[derive(sqlx::FromRow)]
        struct ChunkRow {
            id: Uuid,
            vector_store_id: Uuid,
            file_id: Uuid,
            chunk_index: i32,
            content: String,
            token_count: i32,
            char_start: i32,
            char_end: i32,
            metadata: Option<String>,
            created_at: i64,
            processing_version: Uuid,
        }

        let query = format!(
            r#"
            SELECT id, vector_store_id, file_id, chunk_index, content,
                   token_count, char_start, char_end, metadata::TEXT, created_at,
                   processing_version
            FROM {}
            WHERE file_id = $1
            ORDER BY chunk_index
            "#,
            self.chunks_table_name
        );

        let result = sqlx::query_as(&query)
            .bind(file_id)
            .fetch_all(&self.pool)
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;

        match result {
            Ok(rows) => {
                let chunks: Vec<StoredChunk> = rows
                    .into_iter()
                    .map(|row: ChunkRow| StoredChunk {
                        id: row.id,
                        vector_store_id: row.vector_store_id,
                        file_id: row.file_id,
                        chunk_index: row.chunk_index,
                        content: row.content,
                        token_count: row.token_count,
                        char_start: row.char_start,
                        char_end: row.char_end,
                        metadata: row.metadata.and_then(|s| serde_json::from_str(&s).ok()),
                        created_at: row.created_at,
                        processing_version: row.processing_version,
                    })
                    .collect();

                let chunk_count = chunks.len();
                record_vector_store_operation(
                    "pgvector",
                    "get_chunks",
                    "success",
                    duration,
                    chunk_count as u32,
                );
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "get_chunks_by_file",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = chunk_count,
                    file_id = %file_id,
                    "Get chunks by file completed"
                );
                otel_span_ok!();
                Ok(chunks)
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "get_chunks", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "get_chunks_by_file",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    file_id = %file_id,
                    "Get chunks by file failed"
                );
                otel_span_error!("Get chunks failed: {}", e);
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
    }

    #[instrument(skip(self), fields(backend = "pgvector", operation = "delete_chunks_by_file", file_id = %file_id))]
    async fn delete_chunks_by_file(&self, file_id: Uuid) -> VectorStoreResult<u64> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "delete_chunks_by_file",
            file_id = %file_id,
            "Starting delete chunks by file operation"
        );

        let query = format!("DELETE FROM {} WHERE file_id = $1", self.chunks_table_name);
        let result = sqlx::query(&query).bind(file_id).execute(&self.pool).await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        match result {
            Ok(r) => {
                let count = r.rows_affected();
                record_vector_store_operation(
                    "pgvector",
                    "delete",
                    "success",
                    duration,
                    count as u32,
                );
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "delete_chunks_by_file",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = count,
                    file_id = %file_id,
                    "Delete chunks by file completed"
                );
                otel_span_ok!();
                Ok(count)
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "delete", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "delete_chunks_by_file",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    file_id = %file_id,
                    "Delete chunks by file failed"
                );
                otel_span_error!("Delete chunks failed: {}", e);
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
    }

    #[instrument(skip(self), fields(backend = "pgvector", operation = "delete_chunks_by_file_and_vector_store", file_id = %file_id, vector_store_id = %vector_store_id))]
    async fn delete_chunks_by_file_and_vector_store(
        &self,
        file_id: Uuid,
        vector_store_id: Uuid,
    ) -> VectorStoreResult<u64> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "delete_chunks_by_file_and_vector_store",
            file_id = %file_id,
            vector_store_id = %vector_store_id,
            "Starting delete chunks by file and vector store operation"
        );

        let query = format!(
            "DELETE FROM {} WHERE file_id = $1 AND vector_store_id = $2",
            self.chunks_table_name
        );
        let result = sqlx::query(&query)
            .bind(file_id)
            .bind(vector_store_id)
            .execute(&self.pool)
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        match result {
            Ok(r) => {
                let count = r.rows_affected();
                record_vector_store_operation(
                    "pgvector",
                    "delete",
                    "success",
                    duration,
                    count as u32,
                );
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "delete_chunks_by_file_and_vector_store",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = count,
                    file_id = %file_id,
                    vector_store_id = %vector_store_id,
                    "Delete chunks by file and vector store completed"
                );
                otel_span_ok!();
                Ok(count)
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "delete", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "delete_chunks_by_file_and_vector_store",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    file_id = %file_id,
                    vector_store_id = %vector_store_id,
                    "Delete chunks by file and vector store failed"
                );
                otel_span_error!("Delete chunks failed: {}", e);
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
    }

    #[instrument(skip(self), fields(backend = "pgvector", operation = "delete_chunks_by_file_and_vector_store_except_version", file_id = %file_id, vector_store_id = %vector_store_id, keep_version = %keep_version))]
    async fn delete_chunks_by_file_and_vector_store_except_version(
        &self,
        file_id: Uuid,
        vector_store_id: Uuid,
        keep_version: Uuid,
    ) -> VectorStoreResult<u64> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "delete_chunks_by_file_and_vector_store_except_version",
            file_id = %file_id,
            vector_store_id = %vector_store_id,
            keep_version = %keep_version,
            "Starting delete chunks by file and vector store except version operation"
        );

        let query = format!(
            "DELETE FROM {} WHERE file_id = $1 AND vector_store_id = $2 AND processing_version != $3",
            self.chunks_table_name
        );
        let result = sqlx::query(&query)
            .bind(file_id)
            .bind(vector_store_id)
            .bind(keep_version)
            .execute(&self.pool)
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        match result {
            Ok(r) => {
                let count = r.rows_affected();
                record_vector_store_operation(
                    "pgvector",
                    "delete",
                    "success",
                    duration,
                    count as u32,
                );
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "delete_chunks_by_file_and_vector_store_except_version",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = count,
                    file_id = %file_id,
                    vector_store_id = %vector_store_id,
                    keep_version = %keep_version,
                    "Delete chunks by file and vector store except version completed"
                );
                otel_span_ok!();
                Ok(count)
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "delete", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "delete_chunks_by_file_and_vector_store_except_version",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    file_id = %file_id,
                    vector_store_id = %vector_store_id,
                    keep_version = %keep_version,
                    "Delete chunks by file and vector store except version failed"
                );
                otel_span_error!("Delete chunks failed: {}", e);
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
    }

    #[instrument(skip(self), fields(backend = "pgvector", operation = "delete_chunks_by_vector_store", vector_store_id = %vector_store_id))]
    async fn delete_chunks_by_vector_store(&self, vector_store_id: Uuid) -> VectorStoreResult<u64> {
        let start = Instant::now();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "delete_chunks_by_vector_store",
            vector_store_id = %vector_store_id,
            "Starting delete chunks by vector store operation"
        );

        let query = format!(
            "DELETE FROM {} WHERE vector_store_id = $1",
            self.chunks_table_name
        );
        let result = sqlx::query(&query)
            .bind(vector_store_id)
            .execute(&self.pool)
            .await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        match result {
            Ok(r) => {
                let count = r.rows_affected();
                record_vector_store_operation(
                    "pgvector",
                    "delete",
                    "success",
                    duration,
                    count as u32,
                );
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "delete_chunks_by_vector_store",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = count,
                    vector_store_id = %vector_store_id,
                    "Delete chunks by vector store completed"
                );
                otel_span_ok!();
                Ok(count)
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "delete", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "delete_chunks_by_vector_store",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    vector_store_id = %vector_store_id,
                    "Delete chunks by vector store failed"
                );
                otel_span_error!("Delete chunks failed: {}", e);
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
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

    #[instrument(skip(self, embedding, filter), fields(backend = "pgvector", operation = "search_vector_stores", vector_store_count = vector_store_ids.len(), limit = limit))]
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
                backend = "pgvector",
                operation = "search_vector_stores",
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

        if vector_store_ids.is_empty() {
            return Ok(vec![]);
        }

        let start = Instant::now();
        let vector_store_count = vector_store_ids.len();
        debug!(
            stage = "vector_operation_started",
            backend = "pgvector",
            operation = "search_vector_stores",
            vector_store_count = vector_store_count,
            limit = limit,
            threshold = threshold,
            has_filter = filter.is_some(),
            "Starting vector store search operation"
        );

        let embedding_str = Self::vec_to_pgvector(embedding);
        let distance_threshold = self.similarity_to_distance_threshold(threshold);
        let op = self.distance_metric.pgvector_operator();

        // Track parameter index for dynamic query building
        // $1 = embedding
        // $2..$N = vector_store_ids
        // $N+1 = distance_threshold
        // $N+2 = limit
        // Then file_ids and attribute_filter binds follow

        // Build the query with dynamic filters
        let vector_store_placeholders: Vec<String> = (2..=vector_store_ids.len() + 1)
            .map(|i| format!("${}", i))
            .collect();
        let vector_store_filter = format!(
            "vector_store_id IN ({})",
            vector_store_placeholders.join(", ")
        );

        // Track next available parameter index
        let mut next_param_idx = vector_store_ids.len() + 4; // After embedding, vector_store_ids, distance_threshold, limit

        // Add file_ids filter if provided
        let (file_filter, file_ids) = if let Some(ref f) = filter {
            if let Some(ref ids) = f.file_ids {
                if !ids.is_empty() {
                    let file_placeholders: Vec<String> = (next_param_idx
                        ..next_param_idx + ids.len())
                        .map(|i| format!("${}", i))
                        .collect();
                    next_param_idx += ids.len();
                    (
                        format!(" AND file_id IN ({})", file_placeholders.join(", ")),
                        Some(ids.clone()),
                    )
                } else {
                    (String::new(), None)
                }
            } else {
                (String::new(), None)
            }
        } else {
            (String::new(), None)
        };

        // Build attribute filter SQL if provided
        let (attr_filter_clause, attr_filter_binds) = if let Some(ref f) = filter {
            if let Some(ref attr_filter) = f.attribute_filter {
                let filter_sql = build_attribute_filter_sql(attr_filter, next_param_idx);
                (
                    format!(" AND {}", filter_sql.clause),
                    filter_sql.bind_values,
                )
            } else {
                (String::new(), Vec::new())
            }
        } else {
            (String::new(), Vec::new())
        };

        let query = format!(
            r#"
            SELECT
                id,
                vector_store_id,
                file_id,
                chunk_index,
                content,
                metadata::TEXT,
                (embedding {op} $1::vector) as distance
            FROM {}
            WHERE {}{}{}
              AND (embedding {op} $1::vector) < ${}
            ORDER BY embedding {op} $1::vector
            LIMIT ${}
            "#,
            self.chunks_table_name,
            vector_store_filter,
            file_filter,
            attr_filter_clause,
            vector_store_ids.len() + 2,
            vector_store_ids.len() + 3,
        );

        #[derive(sqlx::FromRow)]
        struct SearchRow {
            id: Uuid,
            vector_store_id: Uuid,
            file_id: Uuid,
            chunk_index: i32,
            content: String,
            metadata: Option<String>,
            distance: f64,
        }

        let mut query_builder = sqlx::query_as::<_, SearchRow>(&query).bind(&embedding_str);

        // Bind vector store IDs
        for vector_store_id in vector_store_ids {
            query_builder = query_builder.bind(*vector_store_id);
        }

        // Bind distance threshold and limit
        query_builder = query_builder.bind(distance_threshold).bind(limit as i32);

        // Bind file IDs if provided
        if let Some(ids) = file_ids {
            for file_id in ids {
                query_builder = query_builder.bind(file_id);
            }
        }

        // Bind attribute filter values
        for bind_value in attr_filter_binds {
            query_builder = match bind_value {
                SqlBindValue::String(s) => query_builder.bind(s),
                SqlBindValue::Number(n) => query_builder.bind(n),
                SqlBindValue::Boolean(b) => query_builder.bind(b),
            };
        }

        let result = query_builder.fetch_all(&self.pool).await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        match result {
            Ok(rows) => {
                let results: Vec<ChunkSearchResult> = rows
                    .into_iter()
                    .map(|row| ChunkSearchResult {
                        chunk_id: row.id,
                        vector_store_id: row.vector_store_id,
                        file_id: row.file_id,
                        chunk_index: row.chunk_index,
                        content: row.content,
                        // Convert raw distance to normalized similarity (0.0-1.0)
                        score: self.distance_to_similarity(row.distance),
                        metadata: row.metadata.and_then(|s| serde_json::from_str(&s).ok()),
                    })
                    .collect();
                let result_count = results.len();
                record_vector_store_operation(
                    "pgvector",
                    "search",
                    "success",
                    duration,
                    result_count as u32,
                );
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "search_vector_stores",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = result_count,
                    vector_store_count = vector_store_count,
                    "VectorStore search operation completed"
                );
                otel_span_ok!();
                Ok(results)
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "search_vector_stores",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    vector_store_count = vector_store_count,
                    "VectorStore search operation failed"
                );
                otel_span_error!("VectorStore search failed: {}", e);
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
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

    #[instrument(skip(self, filter), fields(backend = "pgvector", operation = "keyword_search_vector_stores", vector_store_count = vector_store_ids.len(), limit = limit))]
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
            backend = "pgvector",
            operation = "keyword_search_vector_stores",
            vector_store_count = vector_store_count,
            limit = limit,
            query = %query,
            has_filter = filter.is_some(),
            "Starting keyword search operation"
        );

        // Build the query with dynamic filters
        // $1 = tsquery (parsed from user query)
        // $2..$N = vector_store_ids
        // $N+1 = limit
        // Then file_ids and attribute_filter binds follow

        let vector_store_placeholders: Vec<String> = (2..=vector_store_ids.len() + 1)
            .map(|i| format!("${}", i))
            .collect();
        let vector_store_filter = format!(
            "vector_store_id IN ({})",
            vector_store_placeholders.join(", ")
        );

        // Track next available parameter index
        let mut next_param_idx = vector_store_ids.len() + 3; // After tsquery, vector_store_ids, limit

        // Add file_ids filter if provided
        let (file_filter, file_ids) = if let Some(ref f) = filter {
            if let Some(ref ids) = f.file_ids {
                if !ids.is_empty() {
                    let file_placeholders: Vec<String> = (next_param_idx
                        ..next_param_idx + ids.len())
                        .map(|i| format!("${}", i))
                        .collect();
                    next_param_idx += ids.len();
                    (
                        format!(" AND file_id IN ({})", file_placeholders.join(", ")),
                        Some(ids.clone()),
                    )
                } else {
                    (String::new(), None)
                }
            } else {
                (String::new(), None)
            }
        } else {
            (String::new(), None)
        };

        // Build attribute filter SQL if provided
        let (attr_filter_clause, attr_filter_binds) = if let Some(ref f) = filter {
            if let Some(ref attr_filter) = f.attribute_filter {
                let filter_sql = build_attribute_filter_sql(attr_filter, next_param_idx);
                (
                    format!(" AND {}", filter_sql.clause),
                    filter_sql.bind_values,
                )
            } else {
                (String::new(), Vec::new())
            }
        } else {
            (String::new(), Vec::new())
        };

        // Use websearch_to_tsquery for user-friendly query parsing:
        // - Supports quoted phrases: "machine learning"
        // - Supports OR: cats OR dogs
        // - Supports exclusion: -spam
        // - Handles special characters gracefully
        //
        // ts_rank_cd uses cover density ranking which works well for document search.
        // We normalize the rank to 0-1 range using: rank / (1 + rank)
        // This maps [0, ∞) to [0, 1) with diminishing returns for very high ranks.
        let sql_query = format!(
            r#"
            SELECT
                id,
                vector_store_id,
                file_id,
                chunk_index,
                content,
                metadata::TEXT,
                ts_rank_cd(content_tsvector, websearch_to_tsquery('english', $1)) as rank
            FROM {}
            WHERE {}{}{}
              AND content_tsvector @@ websearch_to_tsquery('english', $1)
            ORDER BY rank DESC
            LIMIT ${}
            "#,
            self.chunks_table_name,
            vector_store_filter,
            file_filter,
            attr_filter_clause,
            vector_store_ids.len() + 2,
        );

        #[derive(sqlx::FromRow)]
        struct KeywordSearchRow {
            id: Uuid,
            vector_store_id: Uuid,
            file_id: Uuid,
            chunk_index: i32,
            content: String,
            metadata: Option<String>,
            rank: f32,
        }

        let mut query_builder = sqlx::query_as::<_, KeywordSearchRow>(&sql_query).bind(query);

        // Bind vector store IDs
        for vector_store_id in vector_store_ids {
            query_builder = query_builder.bind(*vector_store_id);
        }

        // Bind limit
        query_builder = query_builder.bind(limit as i32);

        // Bind file IDs if provided
        if let Some(ids) = file_ids {
            for file_id in ids {
                query_builder = query_builder.bind(file_id);
            }
        }

        // Bind attribute filter values
        for bind_value in attr_filter_binds {
            query_builder = match bind_value {
                SqlBindValue::String(s) => query_builder.bind(s),
                SqlBindValue::Number(n) => query_builder.bind(n),
                SqlBindValue::Boolean(b) => query_builder.bind(b),
            };
        }

        let result = query_builder.fetch_all(&self.pool).await;

        let duration = start.elapsed().as_secs_f64();
        let duration_ms = (duration * 1000.0) as u64;
        match result {
            Ok(rows) => {
                let results: Vec<ChunkSearchResult> = rows
                    .into_iter()
                    .map(|row| ChunkSearchResult {
                        chunk_id: row.id,
                        vector_store_id: row.vector_store_id,
                        file_id: row.file_id,
                        chunk_index: row.chunk_index,
                        content: row.content,
                        // Normalize rank to 0-1 range: rank / (1 + rank)
                        // This maps [0, ∞) to [0, 1) with good spread for typical ranks
                        score: (row.rank as f64) / (1.0 + row.rank as f64),
                        metadata: row.metadata.and_then(|s| serde_json::from_str(&s).ok()),
                    })
                    .collect();
                let result_count = results.len();
                record_vector_store_operation(
                    "pgvector",
                    "keyword_search",
                    "success",
                    duration,
                    result_count as u32,
                );
                info!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "keyword_search_vector_stores",
                    status = "success",
                    duration_ms = duration_ms,
                    item_count = result_count,
                    vector_store_count = vector_store_count,
                    "Keyword search operation completed"
                );
                otel_span_ok!();
                Ok(results)
            }
            Err(e) => {
                record_vector_store_operation("pgvector", "keyword_search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
                    operation = "keyword_search_vector_stores",
                    status = "error",
                    duration_ms = duration_ms,
                    error = %e,
                    vector_store_count = vector_store_count,
                    "Keyword search operation failed"
                );
                otel_span_error!("Keyword search failed: {}", e);
                Err(VectorStoreError::Database(e.to_string()))
            }
        }
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

    #[instrument(skip(self, embedding, filter, config), fields(backend = "pgvector", operation = "hybrid_search_vector_stores", vector_store_count = vector_store_ids.len(), limit = limit))]
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
            backend = "pgvector",
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
                record_vector_store_operation("pgvector", "hybrid_search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
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
                record_vector_store_operation("pgvector", "hybrid_search", "error", duration, 0);
                warn!(
                    stage = "vector_operation_completed",
                    backend = "pgvector",
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
            "pgvector",
            "hybrid_search",
            "success",
            duration,
            result_count as u32,
        );
        info!(
            stage = "vector_operation_completed",
            backend = "pgvector",
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
    fn test_vec_to_pgvector() {
        let vec = vec![0.1, 0.2, 0.3];
        let result = PgvectorStore::vec_to_pgvector(&vec);
        assert_eq!(result, "[0.1,0.2,0.3]");
    }

    #[test]
    fn test_vec_to_pgvector_empty() {
        let vec: Vec<f64> = vec![];
        let result = PgvectorStore::vec_to_pgvector(&vec);
        assert_eq!(result, "[]");
    }

    #[test]
    fn test_vec_to_pgvector_scientific_notation() {
        let vec = vec![1e-10, 0.5, 1.0];
        let result = PgvectorStore::vec_to_pgvector(&vec);
        // Rust's default f64 to_string handles this correctly
        assert!(result.contains("0.0000000001") || result.contains("1e-10"));
    }

    // ========================================================================
    // Attribute Filter SQL Generation Tests
    // ========================================================================

    #[test]
    fn test_build_attribute_filter_sql_string_eq() {
        let filter = AttributeFilter::eq("author", "John Doe");
        let result = build_attribute_filter_sql(&filter, 5);

        assert_eq!(result.clause, "(metadata->>'author' = $5)");
        assert_eq!(result.bind_values.len(), 1);
        match &result.bind_values[0] {
            SqlBindValue::String(s) => assert_eq!(s, "John Doe"),
            _ => panic!("Expected String bind value"),
        }
    }

    #[test]
    fn test_build_attribute_filter_sql_number_gt() {
        let filter = AttributeFilter::gt("score", 0.5);
        let result = build_attribute_filter_sql(&filter, 1);

        assert_eq!(
            result.clause,
            "((metadata->>'score')::double precision > $1)"
        );
        assert_eq!(result.bind_values.len(), 1);
        match &result.bind_values[0] {
            SqlBindValue::Number(n) => assert!((n - 0.5).abs() < f64::EPSILON),
            _ => panic!("Expected Number bind value"),
        }
    }

    #[test]
    fn test_build_attribute_filter_sql_boolean_eq() {
        let filter = AttributeFilter::eq("is_active", true);
        let result = build_attribute_filter_sql(&filter, 3);

        assert_eq!(result.clause, "((metadata->>'is_active')::boolean = $3)");
        assert_eq!(result.bind_values.len(), 1);
        match &result.bind_values[0] {
            SqlBindValue::Boolean(b) => assert!(*b),
            _ => panic!("Expected Boolean bind value"),
        }
    }

    #[test]
    fn test_build_attribute_filter_sql_compound_and() {
        let filter = AttributeFilter::and(vec![
            AttributeFilter::eq("category", "docs"),
            AttributeFilter::gte("date", 1704067200),
        ]);
        let result = build_attribute_filter_sql(&filter, 5);

        assert_eq!(
            result.clause,
            "((metadata->>'category' = $5) AND ((metadata->>'date')::double precision >= $6))"
        );
        assert_eq!(result.bind_values.len(), 2);
    }

    #[test]
    fn test_build_attribute_filter_sql_compound_or() {
        let filter = AttributeFilter::or(vec![
            AttributeFilter::eq("status", "active"),
            AttributeFilter::eq("status", "pending"),
        ]);
        let result = build_attribute_filter_sql(&filter, 1);

        assert_eq!(
            result.clause,
            "((metadata->>'status' = $1) OR (metadata->>'status' = $2))"
        );
        assert_eq!(result.bind_values.len(), 2);
    }

    #[test]
    fn test_build_attribute_filter_sql_nested_compound() {
        let filter = AttributeFilter::and(vec![
            AttributeFilter::eq("category", "documentation"),
            AttributeFilter::or(vec![
                AttributeFilter::eq("author", "Alice"),
                AttributeFilter::eq("author", "Bob"),
            ]),
        ]);
        let result = build_attribute_filter_sql(&filter, 10);

        assert_eq!(
            result.clause,
            "((metadata->>'category' = $10) AND ((metadata->>'author' = $11) OR (metadata->>'author' = $12)))"
        );
        assert_eq!(result.bind_values.len(), 3);
    }

    #[test]
    fn test_build_attribute_filter_sql_all_comparison_operators() {
        // Test all comparison operators generate correct SQL
        let test_cases = [
            (AttributeFilter::eq("key", "val"), "="),
            (AttributeFilter::ne("key", "val"), "!="),
            (AttributeFilter::gt("key", 1), ">"),
            (AttributeFilter::gte("key", 1), ">="),
            (AttributeFilter::lt("key", 1), "<"),
            (AttributeFilter::lte("key", 1), "<="),
        ];

        for (filter, expected_op) in test_cases {
            let result = build_attribute_filter_sql(&filter, 1);
            assert!(
                result.clause.contains(expected_op),
                "Expected operator {} in clause {}",
                expected_op,
                result.clause
            );
        }
    }

    #[test]
    fn test_build_attribute_filter_sql_empty_compound() {
        let filter = AttributeFilter::and(vec![]);
        let result = build_attribute_filter_sql(&filter, 1);

        assert_eq!(result.clause, "TRUE");
        assert!(result.bind_values.is_empty());
    }

    #[test]
    fn test_count_filter_binds() {
        // Simple comparison
        let filter = AttributeFilter::eq("key", "value");
        assert_eq!(count_filter_binds(&filter), 1);

        // Compound with 2 filters
        let filter = AttributeFilter::and(vec![
            AttributeFilter::eq("a", "1"),
            AttributeFilter::eq("b", "2"),
        ]);
        assert_eq!(count_filter_binds(&filter), 2);

        // Nested compound
        let filter = AttributeFilter::and(vec![
            AttributeFilter::eq("a", "1"),
            AttributeFilter::or(vec![
                AttributeFilter::eq("b", "2"),
                AttributeFilter::eq("c", "3"),
            ]),
        ]);
        assert_eq!(count_filter_binds(&filter), 3);
    }
}
