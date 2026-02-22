//! Prometheus metrics for the gateway.
//!
//! Provides metrics for:
//! - HTTP request latency and counts
//! - LLM token usage
//! - Provider health and latency
//! - Budget and rate limiting

#[cfg(feature = "prometheus")]
use std::sync::OnceLock;

#[cfg(feature = "prometheus")]
use metrics::{counter, gauge, histogram};
#[cfg(feature = "prometheus")]
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

use crate::config::MetricsConfig;

/// Global Prometheus handle for the metrics endpoint.
#[cfg(feature = "prometheus")]
static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Initialize the metrics system with the given configuration.
#[cfg(feature = "prometheus")]
pub fn init_metrics(config: &MetricsConfig) -> Result<(), MetricsError> {
    if !config.enabled {
        return Ok(());
    }

    // Build Prometheus exporter with custom buckets
    let builder = PrometheusBuilder::new()
        .set_buckets_for_metric(
            metrics_exporter_prometheus::Matcher::Suffix("_duration_seconds".to_string()),
            &seconds_from_ms(&config.latency_buckets_ms),
        )
        .map_err(|e| MetricsError::Setup(e.to_string()))?
        .set_buckets_for_metric(
            metrics_exporter_prometheus::Matcher::Suffix("_tokens".to_string()),
            &config.token_buckets,
        )
        .map_err(|e| MetricsError::Setup(e.to_string()))?;

    let handle = builder.install_recorder().map_err(MetricsError::Install)?;

    // Store handle for the metrics endpoint
    PROMETHEUS_HANDLE
        .set(handle)
        .map_err(|_| MetricsError::Setup("Metrics already initialized".to_string()))?;

    Ok(())
}

/// Initialize the metrics system (no-op without prometheus feature).
#[cfg(not(feature = "prometheus"))]
pub fn init_metrics(_config: &MetricsConfig) -> Result<(), MetricsError> {
    Ok(())
}

/// Convert millisecond buckets to seconds.
#[cfg(feature = "prometheus")]
fn seconds_from_ms(ms_buckets: &[f64]) -> Vec<f64> {
    ms_buckets.iter().map(|ms| ms / 1000.0).collect()
}

/// Get the Prometheus handle for rendering metrics.
#[cfg(feature = "prometheus")]
pub fn get_prometheus_handle() -> Option<&'static PrometheusHandle> {
    PROMETHEUS_HANDLE.get()
}

// ─────────────────────────────────────────────────────────────────────────────
// Metric Recording Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Record an HTTP request.
pub fn record_http_request(method: &str, path: &str, status: u16, duration_secs: f64) {
    #[cfg(feature = "prometheus")]
    {
        let status_str = status.to_string();
        let status_class = format!("{}xx", status / 100);

        counter!("http_requests_total", "method" => method.to_string(), "path" => path.to_string(), "status" => status_str.clone(), "status_class" => status_class.clone())
            .increment(1);

        histogram!("http_request_duration_seconds", "method" => method.to_string(), "path" => path.to_string(), "status_class" => status_class)
            .record(duration_secs);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (method, path, status, duration_secs);
    }
}

/// Metrics for an LLM request.
#[derive(Debug, Clone)]
pub struct LlmRequestMetrics<'a> {
    /// Provider name (e.g., "openai", "anthropic")
    pub provider: &'a str,
    /// Model name (e.g., "gpt-4", "claude-3")
    pub model: &'a str,
    /// Request status ("success" or "error")
    pub status: &'a str,
    /// HTTP status code (for error tracking by code)
    pub status_code: Option<u16>,
    /// Request duration in seconds
    pub duration_secs: f64,
    /// Number of input tokens (if available)
    pub input_tokens: Option<i64>,
    /// Number of output tokens (if available)
    pub output_tokens: Option<i64>,
    /// Cost in microcents (if available)
    pub cost_microcents: Option<i64>,
}

/// Record an LLM request.
pub fn record_llm_request(metrics: LlmRequestMetrics<'_>) {
    #[cfg(feature = "prometheus")]
    {
        let LlmRequestMetrics {
            provider,
            model,
            status,
            status_code,
            duration_secs,
            input_tokens,
            output_tokens,
            cost_microcents,
        } = metrics;
        // Use "0" as sentinel value instead of empty string to avoid Prometheus aggregation issues
        let status_code_str = status_code.map_or("0".to_string(), |c| c.to_string());
        counter!(
            "llm_requests_total",
            "provider" => provider.to_string(),
            "model" => model.to_string(),
            "status" => status.to_string(),
            "status_code" => status_code_str
        )
        .increment(1);

        histogram!("llm_request_duration_seconds", "provider" => provider.to_string(), "model" => model.to_string())
            .record(duration_secs);

        if let Some(input) = input_tokens {
            histogram!("llm_input_tokens", "provider" => provider.to_string(), "model" => model.to_string())
                .record(input as f64);
            counter!("llm_input_tokens_total", "provider" => provider.to_string(), "model" => model.to_string())
                .increment(input as u64);
        }

        if let Some(output) = output_tokens {
            histogram!("llm_output_tokens", "provider" => provider.to_string(), "model" => model.to_string())
                .record(output as f64);
            counter!("llm_output_tokens_total", "provider" => provider.to_string(), "model" => model.to_string())
                .increment(output as u64);
        }

        if let Some(cost) = cost_microcents {
            counter!("llm_cost_microcents_total", "provider" => provider.to_string(), "model" => model.to_string())
                .increment(cost as u64);
        }
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = metrics;
    }
}

/// Record streaming response metrics.
///
/// Tracks comprehensive streaming metrics including:
/// - Chunk count per stream
/// - Time to first chunk (TTFC)
/// - Total stream duration
/// - Stream completion outcome (completed, error, cancelled)
pub fn record_streaming_response(
    provider: &str,
    model: &str,
    chunk_count: u64,
    time_to_first_chunk_secs: Option<f64>,
    total_duration_secs: f64,
    outcome: &str,
) {
    #[cfg(feature = "prometheus")]
    {
        // Total chunks processed
        counter!("llm_streaming_chunks_total", "provider" => provider.to_string(), "model" => model.to_string())
            .increment(chunk_count);

        // Chunks per stream (histogram for distribution analysis)
        histogram!("llm_streaming_chunk_count", "provider" => provider.to_string(), "model" => model.to_string())
            .record(chunk_count as f64);

        // Time to first chunk (TTFC) - critical latency metric
        if let Some(ttfc) = time_to_first_chunk_secs {
            histogram!("llm_streaming_time_to_first_chunk_seconds", "provider" => provider.to_string(), "model" => model.to_string())
                .record(ttfc);
        }

        // Total stream duration
        histogram!("llm_streaming_duration_seconds", "provider" => provider.to_string(), "model" => model.to_string())
            .record(total_duration_secs);

        // Stream completion outcome
        counter!("llm_streaming_completions_total", "provider" => provider.to_string(), "model" => model.to_string(), "outcome" => outcome.to_string())
            .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (
            provider,
            model,
            chunk_count,
            time_to_first_chunk_secs,
            total_duration_secs,
            outcome,
        );
    }
}

/// Record authentication result.
pub fn record_auth_attempt(method: &str, success: bool) {
    #[cfg(feature = "prometheus")]
    {
        let status = if success { "success" } else { "failure" };
        counter!("auth_attempts_total", "method" => method.to_string(), "status" => status.to_string())
            .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (method, success);
    }
}

/// Record JIT (Just-in-Time) provisioning event.
///
/// # Arguments
/// * `resource_type` - The type of resource: "user", "organization", "team", "org_membership", "team_membership"
/// * `outcome` - The outcome: "created", "removed", "email_blocked"
pub fn record_jit_provision(resource_type: &str, outcome: &str) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "jit_provisions_total",
            "resource_type" => resource_type.to_string(),
            "outcome" => outcome.to_string()
        )
        .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (resource_type, outcome);
    }
}

/// Record multiple JIT (Just-in-Time) provisioning events at once.
///
/// This is more efficient than calling `record_jit_provision` in a loop.
///
/// # Arguments
/// * `resource_type` - The type of resource: "user", "organization", "team", "org_membership", "team_membership"
/// * `outcome` - The outcome: "created", "removed", "email_blocked"
/// * `count` - The number of events to record
pub fn record_jit_provisions(resource_type: &str, outcome: &str, count: u64) {
    #[cfg(feature = "prometheus")]
    {
        if count > 0 {
            counter!(
                "jit_provisions_total",
                "resource_type" => resource_type.to_string(),
                "outcome" => outcome.to_string()
            )
            .increment(count);
        }
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (resource_type, outcome, count);
    }
}

/// Record budget check.
pub fn record_budget_check(result: &str, api_key_id: Option<uuid::Uuid>) {
    #[cfg(feature = "prometheus")]
    {
        counter!("budget_checks_total", "result" => result.to_string()).increment(1);

        if let Some(id) = api_key_id {
            counter!("budget_checks_by_key_total", "api_key_id" => id.to_string(), "result" => result.to_string())
                .increment(1);
        }
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (result, api_key_id);
    }
}

/// Record budget warning when spending approaches the limit.
///
/// # Arguments
/// * `api_key_id` - The API key that triggered the warning
/// * `spend_percentage` - Current spend as a percentage of the limit (0.0-1.0+)
/// * `period` - The budget period (daily or monthly)
pub fn record_budget_warning(api_key_id: uuid::Uuid, spend_percentage: f64, period: &str) {
    #[cfg(feature = "prometheus")]
    {
        counter!("budget_warnings_total", "period" => period.to_string()).increment(1);

        counter!(
            "budget_warnings_by_key_total",
            "api_key_id" => api_key_id.to_string(),
            "period" => period.to_string()
        )
        .increment(1);

        // Also record the current spend percentage as a gauge for dashboards
        gauge!(
            "budget_spend_percentage",
            "api_key_id" => api_key_id.to_string(),
            "period" => period.to_string()
        )
        .set(spend_percentage);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (api_key_id, spend_percentage, period);
    }
}

/// Record rate limit check.
pub fn record_rate_limit(result: &str, api_key_id: Option<uuid::Uuid>) {
    #[cfg(feature = "prometheus")]
    {
        counter!("rate_limit_checks_total", "result" => result.to_string()).increment(1);

        if result == "limited"
            && let Some(id) = api_key_id
        {
            counter!("rate_limit_hits_by_key_total", "api_key_id" => id.to_string()).increment(1);
        }
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (result, api_key_id);
    }
}

/// Record cache operation with cache type for visibility into different cache layers.
///
/// # Arguments
/// * `cache_type` - The type of cache being accessed (e.g., "api_key", "session", "provider_config")
/// * `operation` - The operation being performed (e.g., "get", "set", "delete")
/// * `result` - The result of the operation (e.g., "hit", "miss", "success", "error")
pub fn record_cache_operation(cache_type: &str, operation: &str, result: &str) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "cache_operations_total",
            "cache_type" => cache_type.to_string(),
            "operation" => operation.to_string(),
            "result" => result.to_string()
        )
        .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (cache_type, operation, result);
    }
}

/// Record dead-letter queue operation.
pub fn record_dlq_operation(operation: &str, entry_type: &str) {
    #[cfg(feature = "prometheus")]
    {
        counter!("dlq_operations_total", "operation" => operation.to_string(), "entry_type" => entry_type.to_string())
            .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (operation, entry_type);
    }
}

/// Record data retention deletion.
///
/// Tracks records deleted by the retention worker, enabling:
/// - Visibility into retention policy effectiveness
/// - Capacity planning for storage
/// - Alerting on unexpected deletion volumes
///
/// # Arguments
/// * `table` - The table from which records were deleted (e.g., "usage_records", "daily_spend", "audit_logs", "conversations")
/// * `count` - The number of records deleted
pub fn record_retention_deletion(table: &str, count: u64) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "retention_deletions_total",
            "table" => table.to_string()
        )
        .increment(count);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (table, count);
    }
}

/// Record vector store cleanup deletion.
///
/// Tracks resources deleted by the vector store cleanup worker:
/// - `vector_stores` - Number of stores hard-deleted
/// - `vector_store_files` - Number of orphaned files deleted
/// - `vector_store_chunks` - Number of chunks deleted from vector DB
///
/// # Arguments
/// * `resource` - The type of resource deleted
/// * `count` - The number of resources deleted
pub fn record_cleanup_deletion(resource: &str, count: u64) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "vector_store_cleanup_deletions_total",
            "resource" => resource.to_string()
        )
        .increment(count);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (resource, count);
    }
}

/// Record vector store cleanup error.
///
/// Tracks errors during cleanup operations for alerting and debugging.
pub fn record_cleanup_error(job: &str) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "vector_store_cleanup_errors_total",
            "job" => job.to_string()
        )
        .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = job;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RAG / Document Processing Metrics
// ─────────────────────────────────────────────────────────────────────────────

/// Record document processing completion.
///
/// Tracks file processing operations for RAG ingestion pipelines, enabling:
/// - Processing time monitoring (p50/p95/p99)
/// - Success/failure rate tracking
/// - Throughput analysis by file size
/// - Chunk generation efficiency
///
/// # Arguments
/// * `status` - Processing result ("success", "error", "cancelled")
/// * `duration_secs` - Total processing time in seconds
/// * `chunks_created` - Number of chunks generated from the file
/// * `file_size_bytes` - Original file size in bytes
/// * `file_type` - File type/extension (e.g., "txt", "md", "pdf")
pub fn record_document_processing(
    status: &str,
    duration_secs: f64,
    chunks_created: u32,
    file_size_bytes: u64,
    file_type: &str,
) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "rag_document_processing_total",
            "status" => status.to_string(),
            "file_type" => file_type.to_string()
        )
        .increment(1);

        histogram!(
            "rag_document_processing_duration_seconds",
            "status" => status.to_string(),
            "file_type" => file_type.to_string()
        )
        .record(duration_secs);

        // Track chunks created per file (histogram for distribution)
        histogram!("rag_document_chunks_created", "file_type" => file_type.to_string())
            .record(chunks_created as f64);

        // Track file sizes processed (histogram for capacity planning)
        histogram!("rag_document_file_size_bytes", "file_type" => file_type.to_string())
            .record(file_size_bytes as f64);

        // Running totals
        counter!("rag_document_chunks_total", "file_type" => file_type.to_string())
            .increment(chunks_created as u64);
        counter!("rag_document_bytes_processed_total", "file_type" => file_type.to_string())
            .increment(file_size_bytes);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (
            status,
            duration_secs,
            chunks_created,
            file_size_bytes,
            file_type,
        );
    }
}

/// Record embedding generation operation.
///
/// Tracks embedding API calls for monitoring latency and errors, enabling:
/// - Embedding API latency monitoring
/// - Error rate tracking by provider/model
/// - Token usage analysis
/// - Batch size optimization
///
/// # Arguments
/// * `provider` - Embedding provider name (e.g., "openai", "voyage", "cohere")
/// * `model` - Embedding model name
/// * `status` - Operation result ("success", "error", "timeout")
/// * `duration_secs` - API call latency in seconds
/// * `token_count` - Number of tokens embedded (if available)
/// * `batch_size` - Number of texts in the batch
pub fn record_embedding_generation(
    provider: &str,
    model: &str,
    status: &str,
    duration_secs: f64,
    token_count: Option<u32>,
    batch_size: u32,
) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "rag_embedding_requests_total",
            "provider" => provider.to_string(),
            "model" => model.to_string(),
            "status" => status.to_string()
        )
        .increment(1);

        histogram!(
            "rag_embedding_duration_seconds",
            "provider" => provider.to_string(),
            "model" => model.to_string()
        )
        .record(duration_secs);

        histogram!(
            "rag_embedding_batch_size",
            "provider" => provider.to_string(),
            "model" => model.to_string()
        )
        .record(batch_size as f64);

        if let Some(tokens) = token_count {
            histogram!(
                "rag_embedding_tokens",
                "provider" => provider.to_string(),
                "model" => model.to_string()
            )
            .record(tokens as f64);

            counter!(
                "rag_embedding_tokens_total",
                "provider" => provider.to_string(),
                "model" => model.to_string()
            )
            .increment(tokens as u64);
        }
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (
            provider,
            model,
            status,
            duration_secs,
            token_count,
            batch_size,
        );
    }
}

/// Record file search query execution.
///
/// Tracks search query performance for the file_search tool, enabling:
/// - Search latency monitoring (p50/p95/p99)
/// - Results quality analysis
/// - Cache effectiveness tracking
/// - VectorStore size impact analysis
///
/// # Arguments
/// * `status` - Search result ("success", "error", "timeout", "no_results")
/// * `duration_secs` - Total search time in seconds
/// * `results_count` - Number of results returned
/// * `vector_stores_searched` - Number of vector store collections queried
/// * `cache_hit` - Whether the query was served from cache
pub fn record_file_search(
    status: &str,
    duration_secs: f64,
    results_count: u32,
    vector_stores_searched: u32,
    cache_hit: bool,
) {
    #[cfg(feature = "prometheus")]
    {
        let cache_label = if cache_hit { "hit" } else { "miss" };

        counter!(
            "rag_file_search_total",
            "status" => status.to_string(),
            "cache" => cache_label.to_string()
        )
        .increment(1);

        histogram!(
            "rag_file_search_duration_seconds",
            "status" => status.to_string(),
            "cache" => cache_label.to_string()
        )
        .record(duration_secs);

        histogram!("rag_file_search_results_count").record(results_count as f64);

        histogram!("rag_file_search_vector_stores_searched").record(vector_stores_searched as f64);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (
            status,
            duration_secs,
            results_count,
            vector_stores_searched,
            cache_hit,
        );
    }
}

/// Record file search tool iteration in the middleware loop.
///
/// Tracks iteration behavior for the file_search middleware, enabling:
/// - Iteration count distribution analysis
/// - Detection of potential infinite loops
/// - Optimization of max_iterations setting
/// - Understanding of multi-turn search patterns
///
/// # Arguments
/// * `iteration` - Current iteration number (1-indexed)
/// * `is_final` - Whether this is the final iteration (hit limit or completed)
/// * `reason` - Why the iteration ended ("completed", "limit_reached", "no_tool_call", "error")
pub fn record_file_search_iteration(iteration: u32, is_final: bool, reason: &str) {
    #[cfg(feature = "prometheus")]
    {
        counter!("rag_file_search_iterations_total").increment(1);

        if is_final {
            histogram!("rag_file_search_iteration_count", "reason" => reason.to_string())
                .record(iteration as f64);

            counter!(
                "rag_file_search_completions_total",
                "reason" => reason.to_string()
            )
            .increment(1);
        }
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (iteration, is_final, reason);
    }
}

/// Record vector store operation.
///
/// Tracks vector database operations (insert, search, delete), enabling:
/// - Backend performance comparison (pgvector vs qdrant)
/// - Operation latency monitoring
/// - Error rate tracking
/// - Capacity planning
///
/// # Arguments
/// * `backend` - Vector store backend ("pgvector", "qdrant")
/// * `operation` - Operation type ("insert", "search", "delete", "upsert")
/// * `status` - Operation result ("success", "error", "timeout")
/// * `duration_secs` - Operation latency in seconds
/// * `item_count` - Number of items affected (vectors inserted, results returned, etc.)
pub fn record_vector_store_operation(
    backend: &str,
    operation: &str,
    status: &str,
    duration_secs: f64,
    item_count: u32,
) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "rag_vector_store_operations_total",
            "backend" => backend.to_string(),
            "operation" => operation.to_string(),
            "status" => status.to_string()
        )
        .increment(1);

        histogram!(
            "rag_vector_store_operation_duration_seconds",
            "backend" => backend.to_string(),
            "operation" => operation.to_string()
        )
        .record(duration_secs);

        histogram!(
            "rag_vector_store_operation_items",
            "backend" => backend.to_string(),
            "operation" => operation.to_string()
        )
        .record(item_count as f64);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (backend, operation, status, duration_secs, item_count);
    }
}

/// Update active connections gauge.
pub fn set_active_connections(count: usize) {
    #[cfg(feature = "prometheus")]
    gauge!("active_connections").set(count as f64);
    #[cfg(not(feature = "prometheus"))]
    let _ = count;
}

/// Record provider health check.
pub fn record_provider_health(provider: &str, healthy: bool, latency_secs: Option<f64>) {
    #[cfg(feature = "prometheus")]
    {
        let status = if healthy { "healthy" } else { "unhealthy" };
        gauge!("provider_health", "provider" => provider.to_string()).set(if healthy {
            1.0
        } else {
            0.0
        });

        counter!("provider_health_checks_total", "provider" => provider.to_string(), "status" => status.to_string())
            .increment(1);

        if let Some(latency) = latency_secs {
            histogram!("provider_health_check_duration_seconds", "provider" => provider.to_string())
                .record(latency);
        }
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (provider, healthy, latency_secs);
    }
}

/// Record database operation.
pub fn record_db_operation(operation: &str, table: &str, duration_secs: f64, success: bool) {
    #[cfg(feature = "prometheus")]
    {
        let status = if success { "success" } else { "error" };
        counter!("db_operations_total", "operation" => operation.to_string(), "table" => table.to_string(), "status" => status.to_string())
            .increment(1);

        histogram!("db_operation_duration_seconds", "operation" => operation.to_string(), "table" => table.to_string())
            .record(duration_secs);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (operation, table, duration_secs, success);
    }
}

/// Record circuit breaker state change.
///
/// Tracks both the current state as a gauge (0=closed, 1=open, 2=half_open)
/// and state transition events as counters.
pub fn record_circuit_breaker_state(provider: &str, state: &str) {
    #[cfg(feature = "prometheus")]
    {
        let state_value = match state {
            "closed" => 0.0,
            "open" => 1.0,
            "half_open" => 2.0,
            _ => 0.0,
        };

        gauge!("provider_circuit_breaker_state", "provider" => provider.to_string())
            .set(state_value);

        counter!("provider_circuit_breaker_transitions_total", "provider" => provider.to_string(), "state" => state.to_string())
            .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (provider, state);
    }
}

/// Record circuit breaker failure count (for monitoring approach to threshold).
pub fn record_circuit_breaker_failures(provider: &str, failure_count: u32, threshold: u32) {
    #[cfg(feature = "prometheus")]
    {
        gauge!("provider_circuit_breaker_failure_count", "provider" => provider.to_string())
            .set(failure_count as f64);

        // Track percentage toward threshold for alerting
        let ratio = if threshold > 0 {
            failure_count as f64 / threshold as f64
        } else {
            0.0
        };
        gauge!("provider_circuit_breaker_failure_ratio", "provider" => provider.to_string())
            .set(ratio);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (provider, failure_count, threshold);
    }
}

/// Record circuit breaker consecutive opens (for adaptive backoff monitoring).
///
/// Tracks how many times the circuit has opened consecutively without successful recovery.
/// Higher values indicate persistent provider issues and longer adaptive timeouts.
pub fn record_circuit_breaker_consecutive_opens(provider: &str, consecutive_opens: u32) {
    #[cfg(feature = "prometheus")]
    gauge!("provider_circuit_breaker_consecutive_opens", "provider" => provider.to_string())
        .set(consecutive_opens as f64);
    #[cfg(not(feature = "prometheus"))]
    let _ = (provider, consecutive_opens);
}

/// Record a gateway error with categorization.
///
/// Provides a unified counter for all gateway errors, enabling:
/// - Single "total errors" view for dashboards
/// - Error rate alerting across all categories
/// - SLO tracking
///
/// # Arguments
/// * `error_type` - The category of error (e.g., "auth_failure", "budget_exceeded", "rate_limited", "provider_error")
/// * `error_code` - Specific error code within the category (e.g., "invalid_api_key", "expired_token")
/// * `provider` - Optional provider name for provider-related errors
///
/// # Error Types
/// - `auth_failure`: Authentication/authorization failures
/// - `budget_exceeded`: Budget limit exceeded
/// - `rate_limited`: Rate limit exceeded (request or token)
/// - `provider_error`: Upstream provider errors
/// - `validation_error`: Request validation failures
/// - `bad_request`: Malformed request
/// - `conflict`: Resource conflict (duplicate, already exists)
/// - `not_found`: Resource not found
/// - `internal_error`: Internal server errors
pub fn record_gateway_error(error_type: &str, error_code: &str, provider: Option<&str>) {
    #[cfg(feature = "prometheus")]
    {
        let provider_label = provider.unwrap_or("none").to_string();

        counter!(
            "gateway_errors_total",
            "error_type" => error_type.to_string(),
            "error_code" => error_code.to_string(),
            "provider" => provider_label
        )
        .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (error_type, error_code, provider);
    }
}

/// Record a provider fallback attempt.
///
/// Tracks fallback attempts between providers, enabling:
/// - Visibility into fallback frequency and success rates
/// - Provider reliability comparison
/// - Alerting on excessive fallback usage
///
/// # Arguments
/// * `from_provider` - The provider that failed and triggered the fallback
/// * `to_provider` - The fallback provider being tried
/// * `from_model` - The model that failed
/// * `to_model` - The fallback model being tried
/// * `success` - Whether the fallback attempt succeeded
/// * `attempt` - The attempt number (1-indexed) within the fallback chain
pub fn record_fallback_attempt(
    from_provider: &str,
    to_provider: &str,
    from_model: &str,
    to_model: &str,
    success: bool,
    attempt: usize,
) {
    #[cfg(feature = "prometheus")]
    {
        let success_label = if success { "true" } else { "false" };

        counter!(
            "provider_fallback_attempts_total",
            "from_provider" => from_provider.to_string(),
            "to_provider" => to_provider.to_string(),
            "from_model" => from_model.to_string(),
            "to_model" => to_model.to_string(),
            "success" => success_label.to_string()
        )
        .increment(1);

        // Track which attempt in the chain succeeded/failed
        counter!(
            "provider_fallback_by_attempt_total",
            "attempt" => attempt.to_string(),
            "success" => success_label.to_string()
        )
        .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (
            from_provider,
            to_provider,
            from_model,
            to_model,
            success,
            attempt,
        );
    }
}

/// Record when fallback chain is exhausted (all fallbacks failed).
///
/// This is a critical error metric - indicates all providers are unavailable.
pub fn record_fallback_exhausted(primary_provider: &str, primary_model: &str, chain_length: usize) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "provider_fallback_exhausted_total",
            "primary_provider" => primary_provider.to_string(),
            "primary_model" => primary_model.to_string(),
            "chain_length" => chain_length.to_string()
        )
        .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (primary_provider, primary_model, chain_length);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Guardrails Metrics
// ─────────────────────────────────────────────────────────────────────────────

/// Record a guardrails evaluation.
///
/// Tracks all guardrails evaluations with their outcomes, enabling:
/// - Visibility into guardrails usage and effectiveness
/// - Latency monitoring for guardrails evaluations
/// - Blocking rate analysis
///
/// # Arguments
/// * `provider` - The guardrails provider name (e.g., "openai_moderation", "bedrock", "azure")
/// * `stage` - The evaluation stage ("input" or "output")
/// * `result` - The evaluation result ("passed", "blocked", "warned", "redacted", "error", "timeout")
/// * `latency_secs` - The evaluation latency in seconds
pub fn record_guardrails_evaluation(provider: &str, stage: &str, result: &str, latency_secs: f64) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "guardrails_evaluations_total",
            "provider" => provider.to_string(),
            "stage" => stage.to_string(),
            "result" => result.to_string()
        )
        .increment(1);

        histogram!(
            "guardrails_latency_seconds",
            "provider" => provider.to_string(),
            "stage" => stage.to_string()
        )
        .record(latency_secs);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (provider, stage, result, latency_secs);
    }
}

/// Record a guardrails violation.
///
/// Tracks individual violations detected by guardrails, enabling:
/// - Analysis of violation patterns and categories
/// - Severity distribution monitoring
/// - Action effectiveness analysis
///
/// # Arguments
/// * `provider` - The guardrails provider name
/// * `category` - The violation category (e.g., "hate", "violence", "pii_email")
/// * `severity` - The violation severity ("info", "low", "medium", "high", "critical")
/// * `action` - The action taken ("block", "warn", "log", "redact", "allow")
pub fn record_guardrails_violation(provider: &str, category: &str, severity: &str, action: &str) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "guardrails_violations_total",
            "provider" => provider.to_string(),
            "category" => category.to_string(),
            "severity" => severity.to_string(),
            "action" => action.to_string()
        )
        .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (provider, category, severity, action);
    }
}

/// Record concurrent guardrails race outcome.
///
/// Tracks which operation completes first in concurrent mode, enabling:
/// - Analysis of concurrent mode effectiveness
/// - Latency savings from concurrent execution
/// - Timeout rate monitoring
///
/// # Arguments
/// * `winner` - Which operation completed first ("guardrails_first", "llm_first", "guardrails_timed_out")
pub fn record_guardrails_concurrent_race(winner: &str) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "guardrails_concurrent_races_total",
            "winner" => winner.to_string()
        )
        .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = winner;
    }
}

/// Record a guardrails timeout.
///
/// Tracks timeout events for guardrails evaluations, enabling:
/// - Timeout rate monitoring by provider and stage
/// - Alerting on elevated timeout rates
/// - Performance troubleshooting
///
/// # Arguments
/// * `provider` - The guardrails provider name
/// * `stage` - The evaluation stage ("input" or "output")
pub fn record_guardrails_timeout(provider: &str, stage: &str) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "guardrails_timeouts_total",
            "provider" => provider.to_string(),
            "stage" => stage.to_string()
        )
        .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (provider, stage);
    }
}

/// Record a guardrails provider error.
///
/// Tracks provider errors (not timeouts or blocks), enabling:
/// - Provider reliability monitoring
/// - Error rate alerting
/// - Retry effectiveness analysis
///
/// # Arguments
/// * `provider` - The guardrails provider name
/// * `stage` - The evaluation stage ("input" or "output")
/// * `error_type` - The type of error ("auth", "rate_limited", "provider_error", "config", "parse")
pub fn record_guardrails_error(provider: &str, stage: &str, error_type: &str) {
    #[cfg(feature = "prometheus")]
    {
        counter!(
            "guardrails_errors_total",
            "provider" => provider.to_string(),
            "stage" => stage.to_string(),
            "error_type" => error_type.to_string()
        )
        .increment(1);
    }
    #[cfg(not(feature = "prometheus"))]
    {
        let _ = (provider, stage, error_type);
    }
}

/// Metrics initialization errors.
#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Failed to set up metrics: {0}")]
    Setup(String),

    #[cfg(feature = "prometheus")]
    #[error("Failed to install metrics recorder: {0}")]
    Install(#[from] metrics_exporter_prometheus::BuildError),
}
