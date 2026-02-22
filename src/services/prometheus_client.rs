//! Prometheus HTTP API client.
//!
//! Provides a client for querying Prometheus using PromQL.
//! Used in multi-node deployments where metrics are aggregated in Prometheus.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::{config::RetryConfig, providers::retry::with_retry};

/// Error type for Prometheus client operations.
#[derive(Debug, thiserror::Error)]
pub enum PrometheusError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Prometheus API error: {0}")]
    Api(String),

    #[error("Invalid response format: {0}")]
    InvalidResponse(String),
}

/// Result type for Prometheus client operations.
pub type PrometheusResult<T> = Result<T, PrometheusError>;

/// Prometheus query result types.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultType {
    Matrix,
    Vector,
    Scalar,
    String,
}

/// A single metric value with labels.
#[derive(Debug, Clone)]
pub struct MetricValue {
    pub labels: HashMap<String, String>,
    pub value: f64,
    pub timestamp: f64,
}

/// A metric series with multiple values over time.
#[derive(Debug, Clone)]
pub struct MetricSeries {
    pub labels: HashMap<String, String>,
    pub values: Vec<(f64, f64)>, // (timestamp, value)
}

/// Result of an instant query.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub result_type: String,
    pub values: Vec<MetricValue>,
}

/// Result of a range query.
#[derive(Debug, Clone)]
pub struct RangeResult {
    pub result_type: String,
    pub series: Vec<MetricSeries>,
}

/// Internal Prometheus API response structures.
#[derive(Debug, Deserialize)]
struct PrometheusResponse<T> {
    status: String,
    data: Option<T>,
    error: Option<String>,
    #[serde(rename = "errorType")]
    #[allow(dead_code)] // Deserialization field
    error_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QueryData {
    #[serde(rename = "resultType")]
    result_type: String,
    result: Vec<VectorResult>,
}

#[derive(Debug, Deserialize)]
struct RangeData {
    #[serde(rename = "resultType")]
    result_type: String,
    result: Vec<MatrixResult>,
}

#[derive(Debug, Deserialize)]
struct VectorResult {
    metric: HashMap<String, String>,
    value: (f64, String), // (timestamp, value_string)
}

#[derive(Debug, Deserialize)]
struct MatrixResult {
    metric: HashMap<String, String>,
    values: Vec<(f64, String)>, // [(timestamp, value_string), ...]
}

/// Client for the Prometheus HTTP API.
///
/// Includes automatic retry with exponential backoff for transient failures.
///
/// # Example
/// ```ignore
/// let client = PrometheusClient::new("http://prometheus:9090").unwrap();
///
/// // Instant query
/// let result = client.query("up").await?;
///
/// // Range query
/// let result = client.query_range(
///     "rate(http_requests_total[5m])",
///     start,
///     end,
///     "1m"
/// ).await?;
///
/// // With custom retry config
/// let client = PrometheusClient::with_retry_config(
///     "http://prometheus:9090",
///     RetryConfig { max_retries: 5, ..Default::default() },
/// ).unwrap();
/// ```
#[derive(Clone)]
pub struct PrometheusClient {
    http_client: Client,
    base_url: String,
    retry_config: RetryConfig,
}

impl PrometheusClient {
    /// Create a new Prometheus client with default retry configuration.
    ///
    /// Default retry config: 3 retries, 100ms initial delay, 2x backoff,
    /// retries on 429/5xx status codes.
    ///
    /// # Arguments
    /// * `base_url` - Prometheus server URL (e.g., "http://prometheus:9090")
    pub fn new(base_url: impl Into<String>) -> PrometheusResult<Self> {
        Self::with_retry_config(base_url, RetryConfig::default())
    }

    /// Create a new Prometheus client with custom retry configuration.
    ///
    /// # Arguments
    /// * `base_url` - Prometheus server URL (e.g., "http://prometheus:9090")
    /// * `retry_config` - Configuration for retry behavior on transient failures
    pub fn with_retry_config(
        base_url: impl Into<String>,
        retry_config: RetryConfig,
    ) -> PrometheusResult<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let mut base_url = base_url.into();
        // Remove trailing slash
        if base_url.ends_with('/') {
            base_url.pop();
        }

        Ok(Self {
            http_client,
            base_url,
            retry_config,
        })
    }

    /// Execute an instant query.
    ///
    /// Automatically retries on transient failures (connection errors, 5xx, 429).
    ///
    /// # Arguments
    /// * `query` - PromQL query string
    ///
    /// # Returns
    /// Query result with metric values at the current time
    pub async fn query(&self, query: &str) -> PrometheusResult<QueryResult> {
        let url = format!("{}/api/v1/query", self.base_url);
        let query_params = [("query", query)];

        let response = with_retry(&self.retry_config, "prometheus", "query", || {
            self.http_client.get(&url).query(&query_params).send()
        })
        .await?;

        let api_response: PrometheusResponse<QueryData> = response.json().await?;

        if api_response.status != "success" {
            return Err(PrometheusError::Api(
                api_response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        let data = api_response
            .data
            .ok_or_else(|| PrometheusError::InvalidResponse("Missing data in response".into()))?;

        let values = data
            .result
            .into_iter()
            .filter_map(|r| match r.value.1.parse::<f64>() {
                Ok(v) => Some(MetricValue {
                    labels: r.metric,
                    value: v,
                    timestamp: r.value.0,
                }),
                Err(e) => {
                    tracing::debug!(
                        value = %r.value.1,
                        error = %e,
                        labels = ?r.metric,
                        "Failed to parse Prometheus metric value"
                    );
                    None
                }
            })
            .collect();

        Ok(QueryResult {
            result_type: data.result_type,
            values,
        })
    }

    /// Execute a range query.
    ///
    /// Automatically retries on transient failures (connection errors, 5xx, 429).
    ///
    /// # Arguments
    /// * `query` - PromQL query string
    /// * `start` - Start time
    /// * `end` - End time
    /// * `step` - Query resolution step (e.g., "1m", "5m", "1h")
    ///
    /// # Returns
    /// Range result with time series data
    pub async fn query_range(
        &self,
        query: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        step: &str,
    ) -> PrometheusResult<RangeResult> {
        let url = format!("{}/api/v1/query_range", self.base_url);
        let start_str = start.timestamp().to_string();
        let end_str = end.timestamp().to_string();
        let query_params = [
            ("query", query),
            ("start", start_str.as_str()),
            ("end", end_str.as_str()),
            ("step", step),
        ];

        let response = with_retry(&self.retry_config, "prometheus", "query_range", || {
            self.http_client.get(&url).query(&query_params).send()
        })
        .await?;

        let api_response: PrometheusResponse<RangeData> = response.json().await?;

        if api_response.status != "success" {
            return Err(PrometheusError::Api(
                api_response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        let data = api_response
            .data
            .ok_or_else(|| PrometheusError::InvalidResponse("Missing data in response".into()))?;

        let series = data
            .result
            .into_iter()
            .map(|r| {
                let labels = r.metric.clone();
                MetricSeries {
                    labels: r.metric,
                    values: r
                        .values
                        .into_iter()
                        .filter_map(|(ts, v)| match v.parse::<f64>() {
                            Ok(val) => Some((ts, val)),
                            Err(e) => {
                                tracing::debug!(
                                    value = %v,
                                    error = %e,
                                    labels = ?labels,
                                    "Failed to parse Prometheus range value"
                                );
                                None
                            }
                        })
                        .collect(),
                }
            })
            .collect();

        Ok(RangeResult {
            result_type: data.result_type,
            series,
        })
    }

    /// Execute multiple queries in parallel and return results keyed by query.
    pub async fn query_many(&self, queries: &[&str]) -> PrometheusResult<Vec<QueryResult>> {
        let futures: Vec<_> = queries.iter().map(|q| self.query(q)).collect();

        let results = futures::future::try_join_all(futures).await?;
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = PrometheusClient::new("http://localhost:9090").unwrap();
        assert_eq!(client.base_url, "http://localhost:9090");

        // With trailing slash
        let client = PrometheusClient::new("http://localhost:9090/").unwrap();
        assert_eq!(client.base_url, "http://localhost:9090");
    }

    #[test]
    fn test_parse_vector_result() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "vector",
                "result": [
                    {
                        "metric": {"provider": "openai", "__name__": "llm_requests_total"},
                        "value": [1704067200.123, "100"]
                    }
                ]
            }
        }"#;

        let response: PrometheusResponse<QueryData> = serde_json::from_str(json).unwrap();
        assert_eq!(response.status, "success");

        let data = response.data.unwrap();
        assert_eq!(data.result_type, "vector");
        assert_eq!(data.result.len(), 1);
        assert_eq!(
            data.result[0].metric.get("provider"),
            Some(&"openai".to_string())
        );
        assert_eq!(data.result[0].value.1, "100");
    }

    #[test]
    fn test_parse_matrix_result() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": [
                    {
                        "metric": {"provider": "openai"},
                        "values": [
                            [1704067200, "100"],
                            [1704070800, "150"],
                            [1704074400, "200"]
                        ]
                    }
                ]
            }
        }"#;

        let response: PrometheusResponse<RangeData> = serde_json::from_str(json).unwrap();
        assert_eq!(response.status, "success");

        let data = response.data.unwrap();
        assert_eq!(data.result_type, "matrix");
        assert_eq!(data.result.len(), 1);
        assert_eq!(data.result[0].values.len(), 3);
    }

    #[test]
    fn test_parse_error_response() {
        let json = r#"{
            "status": "error",
            "errorType": "bad_data",
            "error": "invalid query"
        }"#;

        let response: PrometheusResponse<QueryData> = serde_json::from_str(json).unwrap();
        assert_eq!(response.status, "error");
        assert_eq!(response.error, Some("invalid query".to_string()));
    }
}
