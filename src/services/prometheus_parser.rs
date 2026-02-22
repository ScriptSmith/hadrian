//! Prometheus text exposition format parser.
//!
//! Parses metrics from Prometheus text format (from /metrics endpoint) and provides
//! utilities for calculating percentiles from histogram data.

use std::collections::HashMap;

/// Histogram bucket data from Prometheus format.
#[derive(Debug, Clone, Default)]
pub struct HistogramData {
    /// Buckets: upper bound (le) -> cumulative count
    pub buckets: Vec<(f64, f64)>,
    /// Total sum of all observations
    pub sum: f64,
    /// Total count of observations
    pub count: f64,
}

/// Parsed metrics from Prometheus text format.
#[derive(Debug, Default)]
pub struct ParsedMetrics {
    /// Histogram metrics keyed by provider
    pub latency_histograms: HashMap<String, HistogramData>,
    /// Request counters by provider (total requests)
    pub request_counts: HashMap<String, f64>,
    /// Error counters by provider (status="error")
    pub error_counts: HashMap<String, f64>,
    /// Error counts by provider and status code
    pub errors_by_status: HashMap<String, HashMap<u16, f64>>,
    /// Input tokens by provider
    pub input_tokens: HashMap<String, f64>,
    /// Output tokens by provider
    pub output_tokens: HashMap<String, f64>,
    /// Cost in microcents by provider
    pub cost_microcents: HashMap<String, f64>,
}

/// Parse Prometheus text exposition format into structured metrics.
///
/// Extracts LLM-related metrics:
/// - `llm_request_duration_seconds` histogram for latency
/// - `llm_requests_total` counter for request/error counts
/// - `llm_input_tokens_total`, `llm_output_tokens_total` for token usage
/// - `llm_cost_microcents_total` for costs
pub fn parse_prometheus_text(text: &str) -> ParsedMetrics {
    let mut metrics = ParsedMetrics::default();

    for line in text.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse histogram bucket: llm_request_duration_seconds_bucket{provider="...",le="..."}
        if let Some(rest) = line.strip_prefix("llm_request_duration_seconds_bucket{")
            && let Some((labels, value)) = parse_metric_line(rest)
            && let (Some(provider), Some(le)) = (labels.get("provider"), labels.get("le"))
            && let (Ok(le_val), Ok(count)) = (parse_le(le), value.parse::<f64>())
        {
            let histogram = metrics
                .latency_histograms
                .entry(provider.clone())
                .or_default();
            histogram.buckets.push((le_val, count));
        }
        // Parse histogram sum
        else if let Some(rest) = line.strip_prefix("llm_request_duration_seconds_sum{")
            && let Some((labels, value)) = parse_metric_line(rest)
            && let Some(provider) = labels.get("provider")
            && let Ok(sum) = value.parse::<f64>()
        {
            let histogram = metrics
                .latency_histograms
                .entry(provider.clone())
                .or_default();
            histogram.sum = sum;
        }
        // Parse histogram count
        else if let Some(rest) = line.strip_prefix("llm_request_duration_seconds_count{")
            && let Some((labels, value)) = parse_metric_line(rest)
            && let Some(provider) = labels.get("provider")
            && let Ok(count) = value.parse::<f64>()
        {
            let histogram = metrics
                .latency_histograms
                .entry(provider.clone())
                .or_default();
            histogram.count = count;
        }
        // Parse request counter: llm_requests_total{provider="...",status="..."}
        else if let Some(rest) = line.strip_prefix("llm_requests_total{")
            && let Some((labels, value)) = parse_metric_line(rest)
            && let Some(provider) = labels.get("provider")
            && let Ok(count) = value.parse::<f64>()
        {
            // Add to total requests for this provider
            *metrics.request_counts.entry(provider.clone()).or_default() += count;

            // Track errors separately
            if labels.get("status").map(|s| s.as_str()) == Some("error") {
                *metrics.error_counts.entry(provider.clone()).or_default() += count;

                // Track errors by status code if available
                if let Some(status_code) = labels.get("status_code")
                    && let Ok(code) = status_code.parse::<u16>()
                {
                    *metrics
                        .errors_by_status
                        .entry(provider.clone())
                        .or_default()
                        .entry(code)
                        .or_default() += count;
                }
            }
        }
        // Parse input tokens
        else if let Some(rest) = line.strip_prefix("llm_input_tokens_total{")
            && let Some((labels, value)) = parse_metric_line(rest)
            && let Some(provider) = labels.get("provider")
            && let Ok(tokens) = value.parse::<f64>()
        {
            *metrics.input_tokens.entry(provider.clone()).or_default() += tokens;
        }
        // Parse output tokens
        else if let Some(rest) = line.strip_prefix("llm_output_tokens_total{")
            && let Some((labels, value)) = parse_metric_line(rest)
            && let Some(provider) = labels.get("provider")
            && let Ok(tokens) = value.parse::<f64>()
        {
            *metrics.output_tokens.entry(provider.clone()).or_default() += tokens;
        }
        // Parse cost
        else if let Some(rest) = line.strip_prefix("llm_cost_microcents_total{")
            && let Some((labels, value)) = parse_metric_line(rest)
            && let Some(provider) = labels.get("provider")
            && let Ok(cost) = value.parse::<f64>()
        {
            *metrics.cost_microcents.entry(provider.clone()).or_default() += cost;
        }
    }

    // Sort histogram buckets by upper bound
    for histogram in metrics.latency_histograms.values_mut() {
        histogram
            .buckets
            .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    metrics
}

/// Parse a metric line after the metric name prefix.
/// Returns (labels_map, value) if successful.
fn parse_metric_line(rest: &str) -> Option<(HashMap<String, String>, &str)> {
    // Find the closing brace
    let brace_end = rest.find('}')?;
    let labels_str = &rest[..brace_end];
    let value_str = rest[brace_end + 1..].trim();

    // Parse labels
    let mut labels = HashMap::new();
    for part in split_labels(labels_str) {
        if let Some((key, value)) = part.split_once('=') {
            let value = value.trim_matches('"');
            labels.insert(key.to_string(), value.to_string());
        }
    }

    Some((labels, value_str))
}

/// Split label string by commas, respecting quoted values.
fn split_labels(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_quotes = false;

    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                parts.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }

    if start < s.len() {
        parts.push(s[start..].trim());
    }

    parts
}

/// Parse the "le" (less than or equal) bound from histogram bucket.
fn parse_le(le: &str) -> Result<f64, std::num::ParseFloatError> {
    if le == "+Inf" {
        Ok(f64::INFINITY)
    } else {
        le.parse()
    }
}

/// Calculate a percentile from histogram bucket data using linear interpolation.
///
/// The histogram buckets represent cumulative counts up to each upper bound.
/// We use linear interpolation to estimate the value at the requested percentile.
///
/// # Arguments
/// * `histogram` - Histogram data with sorted buckets
/// * `percentile` - Percentile to calculate (0.0 to 1.0, e.g., 0.95 for P95)
///
/// # Returns
/// The estimated value at the given percentile in the same unit as the histogram,
/// or None if the histogram has no data.
pub fn percentile_from_histogram(histogram: &HistogramData, percentile: f64) -> Option<f64> {
    if histogram.count == 0.0 || histogram.buckets.is_empty() {
        return None;
    }

    let target_count = histogram.count * percentile;

    // Find the bucket containing the target count
    let mut prev_bound = 0.0;
    let mut prev_count = 0.0;

    for &(upper_bound, cumulative_count) in &histogram.buckets {
        if cumulative_count >= target_count {
            // Target is in this bucket - interpolate
            if cumulative_count == prev_count {
                // No observations in this bucket, return the lower bound
                return Some(prev_bound);
            }

            // Linear interpolation within the bucket
            let fraction = (target_count - prev_count) / (cumulative_count - prev_count);
            let interpolated = prev_bound + fraction * (upper_bound - prev_bound);

            // Cap at upper bound (don't extrapolate beyond +Inf bucket)
            return Some(if upper_bound.is_infinite() {
                prev_bound
            } else {
                interpolated
            });
        }

        prev_bound = upper_bound;
        prev_count = cumulative_count;
    }

    // If we get here, the percentile is beyond all buckets
    // Return the last finite bucket bound
    histogram
        .buckets
        .iter()
        .rev()
        .find(|(b, _)| b.is_finite())
        .map(|(b, _)| *b)
}

/// Calculate average latency from histogram data.
pub fn average_from_histogram(histogram: &HistogramData) -> Option<f64> {
    if histogram.count == 0.0 {
        return None;
    }
    Some(histogram.sum / histogram.count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prometheus_text_basic() {
        let text = r#"
# HELP llm_requests_total Total LLM requests
# TYPE llm_requests_total counter
llm_requests_total{provider="openai",model="gpt-4",status="success"} 100
llm_requests_total{provider="openai",model="gpt-4",status="error",status_code="500"} 5
llm_requests_total{provider="anthropic",model="claude-3",status="success"} 50

# HELP llm_request_duration_seconds LLM request duration histogram
# TYPE llm_request_duration_seconds histogram
llm_request_duration_seconds_bucket{provider="openai",model="gpt-4",le="0.1"} 10
llm_request_duration_seconds_bucket{provider="openai",model="gpt-4",le="0.5"} 50
llm_request_duration_seconds_bucket{provider="openai",model="gpt-4",le="1.0"} 90
llm_request_duration_seconds_bucket{provider="openai",model="gpt-4",le="+Inf"} 105
llm_request_duration_seconds_sum{provider="openai",model="gpt-4"} 52.5
llm_request_duration_seconds_count{provider="openai",model="gpt-4"} 105

# HELP llm_input_tokens_total Total input tokens
# TYPE llm_input_tokens_total counter
llm_input_tokens_total{provider="openai",model="gpt-4"} 10000
llm_output_tokens_total{provider="openai",model="gpt-4"} 5000
llm_cost_microcents_total{provider="openai",model="gpt-4"} 150000
"#;

        let metrics = parse_prometheus_text(text);

        // Check request counts
        assert_eq!(metrics.request_counts.get("openai"), Some(&105.0));
        assert_eq!(metrics.request_counts.get("anthropic"), Some(&50.0));

        // Check error counts
        assert_eq!(metrics.error_counts.get("openai"), Some(&5.0));

        // Check errors by status
        assert_eq!(
            metrics
                .errors_by_status
                .get("openai")
                .and_then(|m| m.get(&500)),
            Some(&5.0)
        );

        // Check histogram
        let histogram = metrics.latency_histograms.get("openai").unwrap();
        assert_eq!(histogram.count, 105.0);
        assert_eq!(histogram.sum, 52.5);
        assert_eq!(histogram.buckets.len(), 4);

        // Check tokens
        assert_eq!(metrics.input_tokens.get("openai"), Some(&10000.0));
        assert_eq!(metrics.output_tokens.get("openai"), Some(&5000.0));
        assert_eq!(metrics.cost_microcents.get("openai"), Some(&150000.0));
    }

    #[test]
    fn test_percentile_from_histogram() {
        let histogram = HistogramData {
            buckets: vec![
                (0.1, 10.0),
                (0.5, 50.0),
                (1.0, 90.0),
                (f64::INFINITY, 100.0),
            ],
            sum: 50.0,
            count: 100.0,
        };

        // P50 should be around 0.5 (50th observation is at count 50)
        let p50 = percentile_from_histogram(&histogram, 0.5).unwrap();
        assert!((p50 - 0.5).abs() < 0.01, "P50 = {}", p50);

        // P90 should be around 1.0 (90th observation is at count 90)
        let p90 = percentile_from_histogram(&histogram, 0.9).unwrap();
        assert!((p90 - 1.0).abs() < 0.01, "P90 = {}", p90);

        // P10 should be around 0.1 (10th observation is at count 10)
        let p10 = percentile_from_histogram(&histogram, 0.1).unwrap();
        assert!((p10 - 0.1).abs() < 0.01, "P10 = {}", p10);
    }

    #[test]
    fn test_percentile_empty_histogram() {
        let histogram = HistogramData::default();
        assert!(percentile_from_histogram(&histogram, 0.5).is_none());
    }

    #[test]
    fn test_average_from_histogram() {
        let histogram = HistogramData {
            buckets: vec![(1.0, 100.0)],
            sum: 50.0,
            count: 100.0,
        };

        let avg = average_from_histogram(&histogram).unwrap();
        assert!((avg - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_split_labels() {
        let labels = r#"provider="openai",model="gpt-4",status="success""#;
        let parts = split_labels(labels);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], r#"provider="openai""#);
        assert_eq!(parts[1], r#"model="gpt-4""#);
        assert_eq!(parts[2], r#"status="success""#);
    }

    #[test]
    fn test_parse_le() {
        assert_eq!(parse_le("0.1").unwrap(), 0.1);
        assert_eq!(parse_le("1.0").unwrap(), 1.0);
        assert!(parse_le("+Inf").unwrap().is_infinite());
    }
}
