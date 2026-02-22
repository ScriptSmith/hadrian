//! Provider metrics service.
//!
//! Provides provider statistics by querying Prometheus (multi-node) or parsing
//! local /metrics output (single-node).

use std::collections::HashMap;
#[cfg(feature = "prometheus")]
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "prometheus")]
use super::{
    prometheus_client::{PrometheusClient, PrometheusError},
    prometheus_parser::{average_from_histogram, parse_prometheus_text, percentile_from_histogram},
};

/// Error type for provider metrics operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderMetricsError {
    #[cfg(feature = "prometheus")]
    #[error("Prometheus query failed: {0}")]
    Prometheus(#[from] PrometheusError),

    #[error("Historical data requires Prometheus to be configured")]
    HistoricalRequiresPrometheus,

    #[error("Failed to get local metrics: {0}")]
    LocalMetrics(String),
}

/// Result type for provider metrics operations.
pub type ProviderMetricsResult<T> = Result<T, ProviderMetricsError>;

/// Current aggregated statistics for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProviderStats {
    /// Provider name
    pub provider: String,

    /// 50th percentile latency in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p50_latency_ms: Option<f64>,

    /// 95th percentile latency in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95_latency_ms: Option<f64>,

    /// 99th percentile latency in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p99_latency_ms: Option<f64>,

    /// Average latency in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_latency_ms: Option<f64>,

    /// Total number of requests
    pub request_count: i64,

    /// Total number of error responses (status != success)
    pub error_count: i64,

    /// Error counts broken down by HTTP status code
    #[serde(default)]
    pub errors_by_status: HashMap<u16, i64>,

    /// Total input tokens consumed
    pub input_tokens: i64,

    /// Total output tokens generated
    pub output_tokens: i64,

    /// Total cost in microcents
    pub total_cost_microcents: i64,

    /// When these stats were last updated
    pub last_updated: DateTime<Utc>,
}

impl ProviderStats {
    /// Create a new empty stats object for a provider.
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            p50_latency_ms: None,
            p95_latency_ms: None,
            p99_latency_ms: None,
            avg_latency_ms: None,
            request_count: 0,
            error_count: 0,
            errors_by_status: HashMap::new(),
            input_tokens: 0,
            output_tokens: 0,
            total_cost_microcents: 0,
            last_updated: Utc::now(),
        }
    }

    /// Calculate the error rate as a percentage.
    pub fn error_rate(&self) -> f64 {
        if self.request_count == 0 {
            0.0
        } else {
            (self.error_count as f64 / self.request_count as f64) * 100.0
        }
    }
}

/// Historical statistics for a time bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct TimeBucketStats {
    /// Start of the time bucket
    pub bucket_start: DateTime<Utc>,

    /// Duration of the bucket in seconds
    pub bucket_duration_secs: i64,

    /// 50th percentile latency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p50_latency_ms: Option<f64>,

    /// 95th percentile latency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95_latency_ms: Option<f64>,

    /// 99th percentile latency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p99_latency_ms: Option<f64>,

    /// Average latency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_latency_ms: Option<f64>,

    /// Request count in this bucket
    pub request_count: i64,

    /// Error count in this bucket
    pub error_count: i64,

    /// Total tokens in this bucket
    pub total_tokens: i64,

    /// Total cost in this bucket
    pub total_cost_microcents: i64,
}

/// Granularity for historical stats queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum StatsGranularity {
    /// Hourly buckets
    Hour,
    /// Daily buckets
    Day,
}

impl StatsGranularity {
    /// Get the Prometheus step string for this granularity.
    pub fn prometheus_step(&self) -> &'static str {
        match self {
            StatsGranularity::Hour => "1h",
            StatsGranularity::Day => "1d",
        }
    }

    /// Get the duration of one bucket.
    pub fn duration(&self) -> Duration {
        match self {
            StatsGranularity::Hour => Duration::hours(1),
            StatsGranularity::Day => Duration::days(1),
        }
    }

    /// Get the duration in seconds.
    pub fn duration_secs(&self) -> i64 {
        match self {
            StatsGranularity::Hour => 3600,
            StatsGranularity::Day => 86400,
        }
    }
}

/// Historical stats response containing multiple time buckets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProviderStatsHistorical {
    /// Provider name
    pub provider: String,

    /// Granularity of the buckets
    pub granularity: StatsGranularity,

    /// Time series data points
    pub data: Vec<TimeBucketStats>,

    /// Whether Prometheus is configured for historical metrics.
    /// When false, the `data` array will be empty because historical
    /// stats require Prometheus to be configured.
    #[serde(default)]
    pub prometheus_configured: bool,
}

/// Provider metrics service.
///
/// Operates in two modes:
/// - **Prometheus mode**: When `prometheus_query_url` is configured, queries Prometheus HTTP API
/// - **Local mode**: When not configured, parses the local `/metrics` endpoint output
#[derive(Clone, Default)]
pub struct ProviderMetricsService {
    #[cfg(feature = "prometheus")]
    prometheus_client: Option<PrometheusClient>,
    #[cfg(feature = "prometheus")]
    /// Callback to get local metrics text from the Prometheus handle
    local_metrics_fn: Option<Arc<dyn Fn() -> Option<String> + Send + Sync>>,
}

impl ProviderMetricsService {
    /// Create a new service with Prometheus backend.
    #[cfg(feature = "prometheus")]
    pub fn with_prometheus(prometheus_url: &str) -> ProviderMetricsResult<Self> {
        let client = PrometheusClient::new(prometheus_url)?;
        Ok(Self {
            prometheus_client: Some(client),
            local_metrics_fn: None,
        })
    }

    /// Create a new service with local metrics backend.
    #[cfg(feature = "prometheus")]
    pub fn with_local_metrics<F>(get_metrics: F) -> Self
    where
        F: Fn() -> Option<String> + Send + Sync + 'static,
    {
        Self {
            prometheus_client: None,
            local_metrics_fn: Some(Arc::new(get_metrics)),
        }
    }

    /// Create a new service (stub when prometheus is disabled).
    #[cfg(not(feature = "prometheus"))]
    pub fn with_local_metrics<F>(_get_metrics: F) -> Self
    where
        F: Fn() -> Option<String> + Send + Sync + 'static,
    {
        Self {}
    }

    /// Create a new service that will use local metrics from a Prometheus handle.
    #[cfg(feature = "prometheus")]
    pub fn from_prometheus_handle(
        handle: Option<&'static metrics_exporter_prometheus::PrometheusHandle>,
    ) -> Self {
        match handle {
            Some(handle) => Self::with_local_metrics(move || Some(handle.render())),
            None => Self {
                prometheus_client: None,
                local_metrics_fn: None,
            },
        }
    }

    /// Create a new empty service (no prometheus feature).
    #[cfg(not(feature = "prometheus"))]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if Prometheus backend is configured.
    pub fn has_prometheus(&self) -> bool {
        #[cfg(feature = "prometheus")]
        {
            self.prometheus_client.is_some()
        }
        #[cfg(not(feature = "prometheus"))]
        false
    }

    /// Get statistics for all providers.
    pub async fn get_all_stats(&self) -> ProviderMetricsResult<Vec<ProviderStats>> {
        #[cfg(feature = "prometheus")]
        if let Some(client) = &self.prometheus_client {
            return self.get_all_stats_prometheus(client).await;
        }
        self.get_all_stats_local()
    }

    /// Get statistics for a specific provider.
    pub async fn get_stats(&self, provider: &str) -> ProviderMetricsResult<Option<ProviderStats>> {
        let all_stats = self.get_all_stats().await?;
        Ok(all_stats.into_iter().find(|s| s.provider == provider))
    }

    /// Get historical statistics for a provider.
    ///
    /// This only works in Prometheus mode. In local mode, returns an error.
    pub async fn get_historical(
        &self,
        provider: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        granularity: StatsGranularity,
    ) -> ProviderMetricsResult<ProviderStatsHistorical> {
        #[cfg(feature = "prometheus")]
        {
            let client = self
                .prometheus_client
                .as_ref()
                .ok_or(ProviderMetricsError::HistoricalRequiresPrometheus)?;

            return self
                .get_historical_prometheus(client, provider, start, end, granularity)
                .await;
        }
        #[cfg(not(feature = "prometheus"))]
        {
            let _ = (provider, start, end, granularity);
            Err(ProviderMetricsError::HistoricalRequiresPrometheus)
        }
    }

    /// Get all stats from Prometheus.
    #[cfg(feature = "prometheus")]
    async fn get_all_stats_prometheus(
        &self,
        client: &PrometheusClient,
    ) -> ProviderMetricsResult<Vec<ProviderStats>> {
        // Execute multiple queries in parallel
        let queries = [
            // Request counts by provider
            "sum by (provider) (llm_requests_total)",
            // Error counts by provider
            "sum by (provider) (llm_requests_total{status=\"error\"})",
            // P50 latency
            "histogram_quantile(0.50, sum by (provider, le) (rate(llm_request_duration_seconds_bucket[5m])))",
            // P95 latency
            "histogram_quantile(0.95, sum by (provider, le) (rate(llm_request_duration_seconds_bucket[5m])))",
            // P99 latency
            "histogram_quantile(0.99, sum by (provider, le) (rate(llm_request_duration_seconds_bucket[5m])))",
            // Average latency
            "sum by (provider) (rate(llm_request_duration_seconds_sum[5m])) / sum by (provider) (rate(llm_request_duration_seconds_count[5m]))",
            // Input tokens
            "sum by (provider) (llm_input_tokens_total)",
            // Output tokens
            "sum by (provider) (llm_output_tokens_total)",
            // Cost
            "sum by (provider) (llm_cost_microcents_total)",
        ];

        let results = client.query_many(&queries).await?;

        // Build a map of provider -> stats
        let mut stats_map: HashMap<String, ProviderStats> = HashMap::new();
        let now = Utc::now();

        // Process request counts
        for mv in &results[0].values {
            if let Some(provider) = mv.labels.get("provider") {
                let stats = stats_map
                    .entry(provider.clone())
                    .or_insert_with(|| ProviderStats::new(provider));
                stats.request_count = mv.value as i64;
                stats.last_updated = now;
            }
        }

        // Process error counts
        for mv in &results[1].values {
            if let Some(provider) = mv.labels.get("provider")
                && let Some(stats) = stats_map.get_mut(provider)
            {
                stats.error_count = mv.value as i64;
            }
        }

        // Process P50 latency (convert seconds to ms)
        for mv in &results[2].values {
            if let Some(provider) = mv.labels.get("provider")
                && let Some(stats) = stats_map.get_mut(provider)
                && mv.value.is_finite()
            {
                stats.p50_latency_ms = Some(mv.value * 1000.0);
            }
        }

        // Process P95 latency
        for mv in &results[3].values {
            if let Some(provider) = mv.labels.get("provider")
                && let Some(stats) = stats_map.get_mut(provider)
                && mv.value.is_finite()
            {
                stats.p95_latency_ms = Some(mv.value * 1000.0);
            }
        }

        // Process P99 latency
        for mv in &results[4].values {
            if let Some(provider) = mv.labels.get("provider")
                && let Some(stats) = stats_map.get_mut(provider)
                && mv.value.is_finite()
            {
                stats.p99_latency_ms = Some(mv.value * 1000.0);
            }
        }

        // Process average latency
        for mv in &results[5].values {
            if let Some(provider) = mv.labels.get("provider")
                && let Some(stats) = stats_map.get_mut(provider)
                && mv.value.is_finite()
            {
                stats.avg_latency_ms = Some(mv.value * 1000.0);
            }
        }

        // Process input tokens
        for mv in &results[6].values {
            if let Some(provider) = mv.labels.get("provider")
                && let Some(stats) = stats_map.get_mut(provider)
            {
                stats.input_tokens = mv.value as i64;
            }
        }

        // Process output tokens
        for mv in &results[7].values {
            if let Some(provider) = mv.labels.get("provider")
                && let Some(stats) = stats_map.get_mut(provider)
            {
                stats.output_tokens = mv.value as i64;
            }
        }

        // Process cost
        for mv in &results[8].values {
            if let Some(provider) = mv.labels.get("provider")
                && let Some(stats) = stats_map.get_mut(provider)
            {
                stats.total_cost_microcents = mv.value as i64;
            }
        }

        // Fetch errors by status code
        let errors_query =
            "sum by (provider, status_code) (llm_requests_total{status_code=~\"4..|5..\"})";
        if let Ok(errors_result) = client.query(errors_query).await {
            for mv in errors_result.values {
                if let (Some(provider), Some(status_code)) =
                    (mv.labels.get("provider"), mv.labels.get("status_code"))
                    && let (Some(stats), Ok(code)) =
                        (stats_map.get_mut(provider), status_code.parse::<u16>())
                {
                    stats.errors_by_status.insert(code, mv.value as i64);
                }
            }
        }

        let mut stats: Vec<_> = stats_map.into_values().collect();
        stats.sort_by(|a, b| a.provider.cmp(&b.provider));

        Ok(stats)
    }

    /// Get all stats from local metrics.
    #[cfg(feature = "prometheus")]
    fn get_all_stats_local(&self) -> ProviderMetricsResult<Vec<ProviderStats>> {
        // Return empty stats if metrics are not available (consistent with "no data" case)
        let Some(metrics_text) = self.local_metrics_fn.as_ref().and_then(|f| f()) else {
            return Ok(vec![]);
        };

        let parsed = parse_prometheus_text(&metrics_text);
        let now = Utc::now();

        // Build stats from parsed metrics
        let mut stats_map: HashMap<String, ProviderStats> = HashMap::new();

        // Get all provider names from various metrics
        let all_providers: std::collections::HashSet<_> = parsed
            .request_counts
            .keys()
            .chain(parsed.latency_histograms.keys())
            .chain(parsed.input_tokens.keys())
            .collect();

        for provider in all_providers {
            let mut stats = ProviderStats::new(provider);
            stats.last_updated = now;

            // Request and error counts
            if let Some(&count) = parsed.request_counts.get(provider) {
                stats.request_count = count as i64;
            }
            if let Some(&count) = parsed.error_counts.get(provider) {
                stats.error_count = count as i64;
            }

            // Errors by status
            if let Some(errors) = parsed.errors_by_status.get(provider) {
                for (&code, &count) in errors {
                    stats.errors_by_status.insert(code, count as i64);
                }
            }

            // Latency percentiles from histogram
            if let Some(histogram) = parsed.latency_histograms.get(provider) {
                // Convert from seconds to milliseconds
                stats.p50_latency_ms =
                    percentile_from_histogram(histogram, 0.50).map(|v| v * 1000.0);
                stats.p95_latency_ms =
                    percentile_from_histogram(histogram, 0.95).map(|v| v * 1000.0);
                stats.p99_latency_ms =
                    percentile_from_histogram(histogram, 0.99).map(|v| v * 1000.0);
                stats.avg_latency_ms = average_from_histogram(histogram).map(|v| v * 1000.0);
            }

            // Tokens
            if let Some(&tokens) = parsed.input_tokens.get(provider) {
                stats.input_tokens = tokens as i64;
            }
            if let Some(&tokens) = parsed.output_tokens.get(provider) {
                stats.output_tokens = tokens as i64;
            }

            // Cost
            if let Some(&cost) = parsed.cost_microcents.get(provider) {
                stats.total_cost_microcents = cost as i64;
            }

            stats_map.insert(provider.clone(), stats);
        }

        let mut stats: Vec<_> = stats_map.into_values().collect();
        stats.sort_by(|a, b| a.provider.cmp(&b.provider));

        Ok(stats)
    }

    /// Get all stats (stub without prometheus).
    #[cfg(not(feature = "prometheus"))]
    fn get_all_stats_local(&self) -> ProviderMetricsResult<Vec<ProviderStats>> {
        Ok(vec![])
    }

    /// Get historical stats from Prometheus.
    #[cfg(feature = "prometheus")]
    async fn get_historical_prometheus(
        &self,
        client: &PrometheusClient,
        provider: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        granularity: StatsGranularity,
    ) -> ProviderMetricsResult<ProviderStatsHistorical> {
        let step = granularity.prometheus_step();
        let bucket_duration = granularity.duration_secs();

        // Queries for historical data
        let requests_query = format!(
            "sum(increase(llm_requests_total{{provider=\"{}\"}}[{}]))",
            provider, step
        );
        let errors_query = format!(
            "sum(increase(llm_requests_total{{provider=\"{}\",status=\"error\"}}[{}]))",
            provider, step
        );
        let tokens_query = format!(
            "sum(increase(llm_input_tokens_total{{provider=\"{}\"}}[{}])) + sum(increase(llm_output_tokens_total{{provider=\"{}\"}}[{}]))",
            provider, step, provider, step
        );
        let cost_query = format!(
            "sum(increase(llm_cost_microcents_total{{provider=\"{}\"}}[{}]))",
            provider, step
        );

        // Execute range queries
        let (requests_result, errors_result, tokens_result, cost_result) = tokio::try_join!(
            client.query_range(&requests_query, start, end, step),
            client.query_range(&errors_query, start, end, step),
            client.query_range(&tokens_query, start, end, step),
            client.query_range(&cost_query, start, end, step),
        )?;

        // Build time series data
        let mut buckets: HashMap<i64, TimeBucketStats> = HashMap::new();

        // Process request counts
        for series in &requests_result.series {
            for &(ts, value) in &series.values {
                let bucket_start = DateTime::from_timestamp(ts as i64, 0).unwrap_or(start);
                let bucket = buckets.entry(ts as i64).or_insert_with(|| TimeBucketStats {
                    bucket_start,
                    bucket_duration_secs: bucket_duration,
                    p50_latency_ms: None,
                    p95_latency_ms: None,
                    p99_latency_ms: None,
                    avg_latency_ms: None,
                    request_count: 0,
                    error_count: 0,
                    total_tokens: 0,
                    total_cost_microcents: 0,
                });
                bucket.request_count = value as i64;
            }
        }

        // Process error counts
        for series in &errors_result.series {
            for &(ts, value) in &series.values {
                if let Some(bucket) = buckets.get_mut(&(ts as i64)) {
                    bucket.error_count = value as i64;
                }
            }
        }

        // Process tokens
        for series in &tokens_result.series {
            for &(ts, value) in &series.values {
                if let Some(bucket) = buckets.get_mut(&(ts as i64)) {
                    bucket.total_tokens = value as i64;
                }
            }
        }

        // Process cost
        for series in &cost_result.series {
            for &(ts, value) in &series.values {
                if let Some(bucket) = buckets.get_mut(&(ts as i64)) {
                    bucket.total_cost_microcents = value as i64;
                }
            }
        }

        // Sort by timestamp
        let mut data: Vec<_> = buckets.into_values().collect();
        data.sort_by_key(|a| a.bucket_start);

        Ok(ProviderStatsHistorical {
            provider: provider.to_string(),
            granularity,
            data,
            prometheus_configured: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_stats_new() {
        let stats = ProviderStats::new("openai");
        assert_eq!(stats.provider, "openai");
        assert_eq!(stats.request_count, 0);
        assert_eq!(stats.error_count, 0);
        assert!(stats.p50_latency_ms.is_none());
    }

    #[test]
    fn test_error_rate() {
        let mut stats = ProviderStats::new("test");
        assert_eq!(stats.error_rate(), 0.0);

        stats.request_count = 100;
        stats.error_count = 5;
        assert!((stats.error_rate() - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_stats_granularity() {
        assert_eq!(StatsGranularity::Hour.prometheus_step(), "1h");
        assert_eq!(StatsGranularity::Day.prometheus_step(), "1d");
        assert_eq!(StatsGranularity::Hour.duration_secs(), 3600);
        assert_eq!(StatsGranularity::Day.duration_secs(), 86400);
    }

    #[cfg(feature = "prometheus")]
    #[test]
    fn test_local_metrics() {
        let metrics_text = r#"
# HELP llm_requests_total Total LLM requests
# TYPE llm_requests_total counter
llm_requests_total{provider="openai",model="gpt-4",status="success"} 100
llm_requests_total{provider="openai",model="gpt-4",status="error",status_code="500"} 5

# HELP llm_request_duration_seconds LLM request duration histogram
# TYPE llm_request_duration_seconds histogram
llm_request_duration_seconds_bucket{provider="openai",model="gpt-4",le="0.1"} 10
llm_request_duration_seconds_bucket{provider="openai",model="gpt-4",le="0.5"} 50
llm_request_duration_seconds_bucket{provider="openai",model="gpt-4",le="1.0"} 90
llm_request_duration_seconds_bucket{provider="openai",model="gpt-4",le="+Inf"} 105
llm_request_duration_seconds_sum{provider="openai",model="gpt-4"} 52.5
llm_request_duration_seconds_count{provider="openai",model="gpt-4"} 105

llm_input_tokens_total{provider="openai",model="gpt-4"} 10000
llm_output_tokens_total{provider="openai",model="gpt-4"} 5000
llm_cost_microcents_total{provider="openai",model="gpt-4"} 150000
"#;

        let service =
            ProviderMetricsService::with_local_metrics(move || Some(metrics_text.to_string()));

        // Run synchronously in test
        let stats = service.get_all_stats_local().unwrap();

        assert_eq!(stats.len(), 1);
        let openai_stats = &stats[0];
        assert_eq!(openai_stats.provider, "openai");
        assert_eq!(openai_stats.request_count, 105);
        assert_eq!(openai_stats.error_count, 5);
        assert_eq!(openai_stats.input_tokens, 10000);
        assert_eq!(openai_stats.output_tokens, 5000);
        assert_eq!(openai_stats.total_cost_microcents, 150000);

        // Check latency percentiles are calculated
        assert!(openai_stats.p50_latency_ms.is_some());
        assert!(openai_stats.p95_latency_ms.is_some());
        assert!(openai_stats.avg_latency_ms.is_some());

        // Check errors by status
        assert_eq!(openai_stats.errors_by_status.get(&500), Some(&5));
    }
}
