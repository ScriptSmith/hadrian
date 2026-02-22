//! Usage sink abstraction for pluggable usage data destinations.
//!
//! This module provides a trait-based abstraction for where usage data is sent.
//! Multiple sinks can be enabled simultaneously (e.g., database + OTLP).
//!
//! ## Available Sinks
//!
//! - **DatabaseSink**: Writes usage records to the configured database (SQLite/PostgreSQL)
//! - **OtlpSink**: Exports usage records as OTLP log records to any OpenTelemetry-compatible backend
//!
//! ## Configuration
//!
//! ```toml
//! [observability.usage]
//! database = true  # Enable database logging (default)
//!
//! [observability.usage.otlp]
//! enabled = true
//! endpoint = "http://localhost:4317"  # or inherit from tracing.otlp
//! ```

use std::sync::Arc;
#[cfg(feature = "otlp")]
use std::time::Duration;

use async_trait::async_trait;
#[cfg(feature = "otlp")]
use opentelemetry::logs::LoggerProvider;

#[cfg(feature = "otlp")]
use crate::config::{OtlpProtocol, TracingConfig, UsageOtlpConfig};
use crate::{
    db::DbPool,
    dlq::{DeadLetterQueue, DlqEntry},
    models::UsageLogEntry,
    observability::metrics,
};

/// Trait for usage data sinks.
///
/// Implementations can write usage data to various backends.
#[async_trait]
pub trait UsageSink: Send + Sync {
    /// Write a batch of usage entries.
    ///
    /// Returns the number of entries successfully written.
    async fn write_batch(&self, entries: &[UsageLogEntry]) -> Result<usize, UsageSinkError>;

    /// Get the sink name for logging/metrics.
    fn name(&self) -> &'static str;
}

/// Errors from usage sinks.
#[derive(Debug, thiserror::Error)]
pub enum UsageSinkError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("OTLP export error: {0}")]
    Otlp(String),

    #[error("Sink not configured")]
    NotConfigured,
}

// ─────────────────────────────────────────────────────────────────────────────
// Database Sink
// ─────────────────────────────────────────────────────────────────────────────

/// Database sink that writes usage records to SQLite/PostgreSQL.
pub struct DatabaseSink {
    db: Arc<DbPool>,
    dlq: Option<Arc<dyn DeadLetterQueue>>,
}

impl DatabaseSink {
    pub fn new(db: Arc<DbPool>, dlq: Option<Arc<dyn DeadLetterQueue>>) -> Self {
        Self { db, dlq }
    }
}

#[async_trait]
impl UsageSink for DatabaseSink {
    async fn write_batch(&self, entries: &[UsageLogEntry]) -> Result<usize, UsageSinkError> {
        if entries.is_empty() {
            return Ok(0);
        }

        let start = std::time::Instant::now();
        match self.db.usage().log_batch(entries.to_vec()).await {
            Ok(inserted) => {
                let duration = start.elapsed().as_secs_f64();
                metrics::record_db_operation("batch_insert", "usage_log", duration, true);
                tracing::debug!(
                    inserted = inserted,
                    total = entries.len(),
                    duration_ms = duration * 1000.0,
                    "Usage log batch insert successful"
                );
                Ok(inserted)
            }
            Err(e) => {
                let duration = start.elapsed().as_secs_f64();
                metrics::record_db_operation("batch_insert", "usage_log", duration, false);
                tracing::error!(
                    error = %e,
                    count = entries.len(),
                    "Failed to batch insert usage logs"
                );

                // Fall back to DLQ for failed entries
                if let Some(dlq) = &self.dlq {
                    for entry in entries {
                        if let Ok(json) = serde_json::to_string(entry) {
                            let mut dlq_entry = DlqEntry::new("usage_log", json, e.to_string())
                                .with_metadata("model", entry.model.clone());
                            if let Some(api_key_id) = entry.api_key_id {
                                dlq_entry =
                                    dlq_entry.with_metadata("api_key_id", api_key_id.to_string());
                            }

                            if let Err(dlq_err) = dlq.push(dlq_entry).await {
                                tracing::error!(
                                    error = %dlq_err,
                                    "Failed to write usage entry to DLQ"
                                );
                            } else {
                                metrics::record_dlq_operation("push", "usage_log");
                            }
                        }
                    }
                }

                Err(UsageSinkError::Database(e.to_string()))
            }
        }
    }

    fn name(&self) -> &'static str {
        "database"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OTLP Sink (requires 'otlp' feature)
// ─────────────────────────────────────────────────────────────────────────────

/// OTLP sink that exports usage records as OpenTelemetry log records.
///
/// This allows usage data to be sent to any OTLP-compatible backend:
/// - Grafana Cloud / Loki
/// - Datadog
/// - Honeycomb
/// - Elastic/OpenSearch
/// - ClickHouse (via OTEL collector)
/// - Any OpenTelemetry Collector
///
/// Requires the `otlp` feature.
#[cfg(feature = "otlp")]
pub struct OtlpSink {
    logger_provider: opentelemetry_sdk::logs::SdkLoggerProvider,
    logger: opentelemetry_sdk::logs::SdkLogger,
}

#[cfg(feature = "otlp")]
impl OtlpSink {
    /// Create a new OTLP sink from configuration.
    pub fn new(
        config: &UsageOtlpConfig,
        tracing_config: &TracingConfig,
    ) -> Result<Self, UsageSinkError> {
        use opentelemetry::KeyValue;
        use opentelemetry_sdk::Resource;

        // Build resource attributes
        let service_name = config
            .service_name
            .clone()
            .unwrap_or_else(|| tracing_config.service_name.clone());

        let mut resource_attrs = vec![KeyValue::new("service.name", service_name.clone())];

        if let Some(version) = &tracing_config.service_version {
            resource_attrs.push(KeyValue::new("service.version", version.clone()));
        }

        if let Some(env) = &tracing_config.environment {
            resource_attrs.push(KeyValue::new("deployment.environment", env.clone()));
        }

        let resource = Resource::builder().with_attributes(resource_attrs).build();

        // Build the log exporter
        let exporter = Self::build_exporter(config, tracing_config)?;

        // Build the logger provider
        let provider = opentelemetry_sdk::logs::SdkLoggerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(exporter)
            .build();

        let logger = provider.logger("hadrian.usage");

        Ok(Self {
            logger_provider: provider,
            logger,
        })
    }

    fn build_exporter(
        config: &UsageOtlpConfig,
        tracing_config: &TracingConfig,
    ) -> Result<opentelemetry_otlp::LogExporter, UsageSinkError> {
        use opentelemetry_otlp::{WithExportConfig, WithHttpConfig, WithTonicConfig};

        // Use config endpoint or fall back to tracing endpoint
        let endpoint = config
            .endpoint
            .clone()
            .or_else(|| tracing_config.otlp.as_ref().map(|o| o.endpoint.clone()))
            .ok_or_else(|| {
                UsageSinkError::Otlp("No OTLP endpoint configured for usage logging".to_string())
            })?;

        match config.protocol {
            OtlpProtocol::Grpc => {
                let mut builder = opentelemetry_otlp::LogExporter::builder()
                    .with_tonic()
                    .with_endpoint(&endpoint)
                    .with_timeout(Duration::from_secs(config.timeout_secs));

                // Add headers if configured
                if !config.headers.is_empty() {
                    let metadata = tonic::metadata::MetadataMap::from_headers(
                        config
                            .headers
                            .iter()
                            .filter_map(|(k, v)| {
                                let key = http::header::HeaderName::try_from(k).ok()?;
                                let value = http::header::HeaderValue::try_from(v).ok()?;
                                Some((key, value))
                            })
                            .collect(),
                    );
                    builder = builder.with_metadata(metadata);
                }

                // Enable compression if configured
                if config.compression {
                    builder = builder.with_compression(opentelemetry_otlp::Compression::Gzip);
                }

                builder.build().map_err(|e| {
                    UsageSinkError::Otlp(format!("Failed to create gRPC OTLP log exporter: {}", e))
                })
            }
            OtlpProtocol::Http => {
                let mut builder = opentelemetry_otlp::LogExporter::builder()
                    .with_http()
                    .with_endpoint(&endpoint)
                    .with_timeout(Duration::from_secs(config.timeout_secs));

                // Add headers if configured
                if !config.headers.is_empty() {
                    let headers: std::collections::HashMap<String, String> = config.headers.clone();
                    builder = builder.with_headers(headers);
                }

                builder.build().map_err(|e| {
                    UsageSinkError::Otlp(format!("Failed to create HTTP OTLP log exporter: {}", e))
                })
            }
        }
    }
}

#[cfg(feature = "otlp")]
#[async_trait]
impl UsageSink for OtlpSink {
    async fn write_batch(&self, entries: &[UsageLogEntry]) -> Result<usize, UsageSinkError> {
        use opentelemetry::{
            Key,
            logs::{LogRecord, Logger, Severity},
        };

        if entries.is_empty() {
            return Ok(0);
        }

        let start = std::time::Instant::now();
        let mut success_count = 0;

        for entry in entries {
            // Create a log record for this usage entry
            let mut record = self.logger.create_log_record();

            // Set severity and body
            record.set_severity_number(Severity::Info);
            record.set_body(
                format!(
                    "LLM usage: {} tokens, {} microcents",
                    entry.input_tokens + entry.output_tokens,
                    entry.cost_microcents.unwrap_or(0)
                )
                .into(),
            );

            // Add all usage attributes using direct types that implement Into<AnyValue>
            record.add_attribute(
                Key::from_static_str("hadrian.request_id"),
                entry.request_id.clone(),
            );
            if let Some(api_key_id) = entry.api_key_id {
                record.add_attribute(
                    Key::from_static_str("hadrian.api_key_id"),
                    api_key_id.to_string(),
                );
            }
            if let Some(user_id) = entry.user_id {
                record.add_attribute(Key::from_static_str("hadrian.user_id"), user_id.to_string());
            }
            if let Some(org_id) = entry.org_id {
                record.add_attribute(Key::from_static_str("hadrian.org_id"), org_id.to_string());
            }
            if let Some(project_id) = entry.project_id {
                record.add_attribute(
                    Key::from_static_str("hadrian.project_id"),
                    project_id.to_string(),
                );
            }
            if let Some(team_id) = entry.team_id {
                record.add_attribute(Key::from_static_str("hadrian.team_id"), team_id.to_string());
            }
            if let Some(service_account_id) = entry.service_account_id {
                record.add_attribute(
                    Key::from_static_str("hadrian.service_account_id"),
                    service_account_id.to_string(),
                );
            }
            record.add_attribute(Key::from_static_str("hadrian.model"), entry.model.clone());
            record.add_attribute(
                Key::from_static_str("hadrian.provider"),
                entry.provider.clone(),
            );
            record.add_attribute(
                Key::from_static_str("hadrian.input_tokens"),
                entry.input_tokens as i64,
            );
            record.add_attribute(
                Key::from_static_str("hadrian.output_tokens"),
                entry.output_tokens as i64,
            );
            record.add_attribute(
                Key::from_static_str("hadrian.total_tokens"),
                (entry.input_tokens + entry.output_tokens) as i64,
            );

            if let Some(cost) = entry.cost_microcents {
                record.add_attribute(Key::from_static_str("hadrian.cost_microcents"), cost);
                // Also add cost in dollars for easier querying
                record.add_attribute(
                    Key::from_static_str("hadrian.cost_dollars"),
                    cost as f64 / 100_000_000.0,
                );
            }

            if let Some(referer) = &entry.http_referer {
                record.add_attribute(
                    Key::from_static_str("hadrian.http_referer"),
                    referer.clone(),
                );
            }

            record.add_attribute(Key::from_static_str("hadrian.streamed"), entry.streamed);

            if entry.cached_tokens > 0 {
                record.add_attribute(
                    Key::from_static_str("hadrian.cached_tokens"),
                    entry.cached_tokens as i64,
                );
            }

            if entry.reasoning_tokens > 0 {
                record.add_attribute(
                    Key::from_static_str("hadrian.reasoning_tokens"),
                    entry.reasoning_tokens as i64,
                );
            }

            if let Some(finish_reason) = &entry.finish_reason {
                record.add_attribute(
                    Key::from_static_str("hadrian.finish_reason"),
                    finish_reason.clone(),
                );
            }

            if let Some(latency_ms) = entry.latency_ms {
                record.add_attribute(
                    Key::from_static_str("hadrian.latency_ms"),
                    latency_ms as i64,
                );
            }

            record.add_attribute(Key::from_static_str("hadrian.cancelled"), entry.cancelled);

            if let Some(status_code) = entry.status_code {
                record.add_attribute(
                    Key::from_static_str("hadrian.status_code"),
                    status_code as i64,
                );
            }

            record.add_attribute(
                Key::from_static_str("hadrian.pricing_source"),
                entry.pricing_source.as_str().to_string(),
            );

            if let Some(image_count) = entry.image_count {
                record.add_attribute(
                    Key::from_static_str("hadrian.image_count"),
                    image_count as i64,
                );
            }
            if let Some(audio_seconds) = entry.audio_seconds {
                record.add_attribute(
                    Key::from_static_str("hadrian.audio_seconds"),
                    audio_seconds as i64,
                );
            }
            if let Some(character_count) = entry.character_count {
                record.add_attribute(
                    Key::from_static_str("hadrian.character_count"),
                    character_count as i64,
                );
            }

            // Emit the log record
            self.logger.emit(record);
            success_count += 1;
        }

        let duration = start.elapsed().as_secs_f64();
        tracing::debug!(
            count = success_count,
            duration_ms = duration * 1000.0,
            "OTLP usage logs emitted"
        );

        Ok(success_count)
    }

    fn name(&self) -> &'static str {
        "otlp"
    }
}

#[cfg(feature = "otlp")]
impl Drop for OtlpSink {
    fn drop(&mut self) {
        // Ensure pending logs are flushed
        if let Err(e) = self.logger_provider.shutdown() {
            tracing::warn!(error = %e, "Error shutting down OTLP usage logger");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Composite Sink
// ─────────────────────────────────────────────────────────────────────────────

/// Composite sink that writes to multiple backends.
///
/// Writes are attempted to all configured sinks. Failures in one sink
/// don't prevent writes to other sinks.
pub struct CompositeSink {
    sinks: Vec<Arc<dyn UsageSink>>,
}

impl CompositeSink {
    pub fn new(sinks: Vec<Arc<dyn UsageSink>>) -> Self {
        Self { sinks }
    }

    #[allow(dead_code)] // OTLP export configuration
    pub fn is_empty(&self) -> bool {
        self.sinks.is_empty()
    }
}

#[async_trait]
impl UsageSink for CompositeSink {
    async fn write_batch(&self, entries: &[UsageLogEntry]) -> Result<usize, UsageSinkError> {
        if entries.is_empty() {
            return Ok(0);
        }

        let mut max_written = 0;
        let mut last_error = None;

        for sink in &self.sinks {
            match sink.write_batch(entries).await {
                Ok(written) => {
                    max_written = max_written.max(written);
                    tracing::debug!(sink = sink.name(), written, "Usage sink write successful");
                }
                Err(e) => {
                    tracing::error!(sink = sink.name(), error = %e, "Usage sink write failed");
                    last_error = Some(e);
                }
            }
        }

        // Return success if at least one sink succeeded
        if max_written > 0 {
            Ok(max_written)
        } else {
            Err(last_error.unwrap_or(UsageSinkError::NotConfigured))
        }
    }

    fn name(&self) -> &'static str {
        "composite"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composite_sink_empty() {
        let sink = CompositeSink::new(vec![]);
        assert!(sink.is_empty());
    }
}
