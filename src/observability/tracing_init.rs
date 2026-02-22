//! Tracing initialization with configurable logging formats and OpenTelemetry support.
//!
//! OpenTelemetry distributed tracing can be enabled via configuration.
//! Requires the `otlp` feature for OTLP export support.

#[cfg(feature = "otlp")]
use opentelemetry::trace::TracerProvider as _;
#[cfg(feature = "otlp")]
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

// Stub types for when OTLP feature is disabled
#[cfg(not(feature = "otlp"))]
struct SdkTracerProviderStub;
#[cfg(not(feature = "otlp"))]
struct TracerStub;

#[cfg(feature = "otlp")]
use crate::config::{OtlpProtocol, PropagationFormat, SamplingStrategy};
use crate::{
    config::{LogFormat, LoggingConfig, ObservabilityConfig},
    observability::siem::{CefConfig, CefLayer, LeefConfig, LeefLayer, SyslogConfig, SyslogLayer},
};

/// Initialize the tracing subscriber with the given configuration.
///
/// This sets up:
/// - Console logging with configurable format (pretty, compact, JSON)
/// - Environment-based log filtering
/// - OpenTelemetry distributed tracing (if configured)
pub fn init_tracing(config: &ObservabilityConfig) -> Result<TracingGuard, TracingError> {
    let logging = &config.logging;
    let filter = build_env_filter(logging);

    // Build the OpenTelemetry provider if enabled (requires otlp feature)
    #[cfg(feature = "otlp")]
    let otel_provider = if config.tracing.enabled {
        Some(build_otel_provider(&config.tracing)?)
    } else {
        None
    };
    #[cfg(not(feature = "otlp"))]
    let otel_provider: Option<SdkTracerProviderStub> = {
        if config.tracing.enabled {
            tracing::warn!(
                "OpenTelemetry tracing is enabled in config but the 'otlp' feature is not compiled. \
                Rebuild with: cargo build --features otlp"
            );
        }
        None
    };

    // Get tracer from provider if available
    #[cfg(feature = "otlp")]
    let otel_tracer = otel_provider
        .as_ref()
        .map(|p| p.tracer(config.tracing.service_name.clone()));
    #[cfg(not(feature = "otlp"))]
    let otel_tracer: Option<TracerStub> = None;

    // Build the OpenTelemetry layer if we have a tracer
    // The layer needs to be added last and is generic over the subscriber type
    match (&logging.format, logging.timestamps, otel_tracer) {
        #[cfg(feature = "otlp")]
        (LogFormat::Pretty, true, Some(tracer)) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(logging.file_line)
                .with_line_number(logging.file_line);
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .with(otel_layer)
                .init();
        }
        (LogFormat::Pretty, true, None) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(logging.file_line)
                .with_line_number(logging.file_line);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .init();
        }
        #[cfg(feature = "otlp")]
        (LogFormat::Pretty, false, Some(tracer)) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(logging.file_line)
                .with_line_number(logging.file_line)
                .without_time();
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .with(otel_layer)
                .init();
        }
        (LogFormat::Pretty, false, None) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(logging.file_line)
                .with_line_number(logging.file_line)
                .without_time();
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .init();
        }
        #[cfg(feature = "otlp")]
        (LogFormat::Compact, true, Some(tracer)) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_file(logging.file_line)
                .with_line_number(logging.file_line);
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .with(otel_layer)
                .init();
        }
        (LogFormat::Compact, true, None) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_file(logging.file_line)
                .with_line_number(logging.file_line);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .init();
        }
        #[cfg(feature = "otlp")]
        (LogFormat::Compact, false, Some(tracer)) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_file(logging.file_line)
                .with_line_number(logging.file_line)
                .without_time();
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .with(otel_layer)
                .init();
        }
        (LogFormat::Compact, false, None) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_file(logging.file_line)
                .with_line_number(logging.file_line)
                .without_time();
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .init();
        }
        #[cfg(feature = "otlp")]
        (LogFormat::Json, true, Some(tracer)) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_file(logging.file_line)
                .with_line_number(logging.file_line)
                .with_current_span(logging.include_spans);
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .with(otel_layer)
                .init();
        }
        (LogFormat::Json, true, None) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_file(logging.file_line)
                .with_line_number(logging.file_line)
                .with_current_span(logging.include_spans);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .init();
        }
        #[cfg(feature = "otlp")]
        (LogFormat::Json, false, Some(tracer)) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_file(logging.file_line)
                .with_line_number(logging.file_line)
                .with_current_span(logging.include_spans)
                .without_time();
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .with(otel_layer)
                .init();
        }
        (LogFormat::Json, false, None) => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_file(logging.file_line)
                .with_line_number(logging.file_line)
                .with_current_span(logging.include_spans)
                .without_time();
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .init();
        }
        // CEF (Common Event Format) for SIEM integration
        (LogFormat::Cef, _, otel_tracer) => {
            let cef_config = CefConfig::from_siem_config(&logging.siem);
            let cef_layer = CefLayer::stdout(cef_config);
            #[cfg(feature = "otlp")]
            if let Some(tracer) = otel_tracer {
                let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
                tracing_subscriber::registry()
                    .with(filter)
                    .with(cef_layer)
                    .with(otel_layer)
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(cef_layer)
                    .init();
            }
            #[cfg(not(feature = "otlp"))]
            {
                let _ = otel_tracer; // suppress unused warning
                tracing_subscriber::registry()
                    .with(filter)
                    .with(cef_layer)
                    .init();
            }
        }
        // LEEF (Log Event Extended Format) for IBM QRadar SIEM
        (LogFormat::Leef, _, otel_tracer) => {
            let leef_config = LeefConfig::from_siem_config(&logging.siem);
            let leef_layer = LeefLayer::stdout(leef_config);
            #[cfg(feature = "otlp")]
            if let Some(tracer) = otel_tracer {
                let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
                tracing_subscriber::registry()
                    .with(filter)
                    .with(leef_layer)
                    .with(otel_layer)
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(leef_layer)
                    .init();
            }
            #[cfg(not(feature = "otlp"))]
            {
                let _ = otel_tracer;
                tracing_subscriber::registry()
                    .with(filter)
                    .with(leef_layer)
                    .init();
            }
        }
        // Syslog (RFC 5424) format for standard syslog servers
        (LogFormat::Syslog, _, otel_tracer) => {
            let syslog_config = SyslogConfig::from_siem_config(&logging.siem);
            let syslog_layer = SyslogLayer::stdout(syslog_config);
            #[cfg(feature = "otlp")]
            if let Some(tracer) = otel_tracer {
                let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
                tracing_subscriber::registry()
                    .with(filter)
                    .with(syslog_layer)
                    .with(otel_layer)
                    .init();
            } else {
                tracing_subscriber::registry()
                    .with(filter)
                    .with(syslog_layer)
                    .init();
            }
            #[cfg(not(feature = "otlp"))]
            {
                let _ = otel_tracer;
                tracing_subscriber::registry()
                    .with(filter)
                    .with(syslog_layer)
                    .init();
            }
        }
        // When otlp is disabled, Some(_) arms for Pretty/Compact/Json are compiled out.
        // This catch-all satisfies exhaustiveness (unreachable since otel_tracer is always None).
        #[cfg(not(feature = "otlp"))]
        (_, _, Some(_)) => unreachable!(),
    }

    // Set global tracer provider for context propagation if enabled
    #[cfg(feature = "otlp")]
    if let Some(ref provider) = otel_provider {
        opentelemetry::global::set_tracer_provider(provider.clone());
    }
    #[cfg(not(feature = "otlp"))]
    let _ = &otel_provider;

    // Install propagator for context propagation
    #[cfg(feature = "otlp")]
    if config.tracing.enabled {
        install_propagator(&config.tracing.propagation);
    }

    // Log OTEL status if configured
    if config.tracing.enabled {
        if config.tracing.otlp.is_some() {
            tracing::info!(
                service_name = %config.tracing.service_name,
                "OpenTelemetry tracing enabled with OTLP export"
            );
        } else {
            tracing::info!(
                service_name = %config.tracing.service_name,
                "OpenTelemetry tracing enabled (no exporter configured)"
            );
        }
    }

    Ok(TracingGuard {
        provider: otel_provider,
    })
}

/// Build the OpenTelemetry tracer provider.
#[cfg(feature = "otlp")]
fn build_otel_provider(
    config: &crate::config::TracingConfig,
) -> Result<SdkTracerProvider, TracingError> {
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::Resource;

    // Build resource attributes
    let mut resource_attrs = vec![KeyValue::new("service.name", config.service_name.clone())];

    if let Some(version) = &config.service_version {
        resource_attrs.push(KeyValue::new("service.version", version.clone()));
    }

    if let Some(env) = &config.environment {
        resource_attrs.push(KeyValue::new("deployment.environment", env.clone()));
    }

    // Add custom resource attributes
    for (key, value) in &config.resource_attributes {
        resource_attrs.push(KeyValue::new(key.clone(), value.clone()));
    }

    let resource = Resource::builder().with_attributes(resource_attrs).build();

    // Build sampler
    let sampler = build_sampler(&config.sampling);

    // Build tracer provider
    let provider = if let Some(otlp) = &config.otlp {
        let exporter = build_otlp_exporter(otlp)?;
        SdkTracerProvider::builder()
            .with_resource(resource)
            .with_sampler(sampler)
            .with_batch_exporter(exporter)
            .build()
    } else {
        // No exporter - create a provider without export (spans will be dropped)
        SdkTracerProvider::builder()
            .with_resource(resource)
            .with_sampler(sampler)
            .build()
    };

    Ok(provider)
}

/// Build the OTLP span exporter.
#[cfg(feature = "otlp")]
fn build_otlp_exporter(
    config: &crate::config::OtlpConfig,
) -> Result<opentelemetry_otlp::SpanExporter, TracingError> {
    use std::time::Duration;

    use opentelemetry_otlp::{WithExportConfig, WithHttpConfig, WithTonicConfig};

    match config.protocol {
        OtlpProtocol::Grpc => {
            let mut builder = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(&config.endpoint)
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
                TracingError::Init(format!("Failed to create gRPC OTLP exporter: {}", e))
            })
        }
        OtlpProtocol::Http => {
            let mut builder = opentelemetry_otlp::SpanExporter::builder()
                .with_http()
                .with_endpoint(&config.endpoint)
                .with_timeout(Duration::from_secs(config.timeout_secs));

            // Add headers if configured
            if !config.headers.is_empty() {
                let headers: std::collections::HashMap<String, String> = config.headers.clone();
                builder = builder.with_headers(headers);
            }

            builder.build().map_err(|e| {
                TracingError::Init(format!("Failed to create HTTP OTLP exporter: {}", e))
            })
        }
    }
}

/// Build the sampler from config.
#[cfg(feature = "otlp")]
fn build_sampler(config: &crate::config::SamplingConfig) -> Sampler {
    match config.strategy {
        SamplingStrategy::AlwaysOn => Sampler::AlwaysOn,
        SamplingStrategy::AlwaysOff => Sampler::AlwaysOff,
        SamplingStrategy::Ratio => Sampler::TraceIdRatioBased(config.rate),
        SamplingStrategy::ParentBased => {
            Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(config.rate)))
        }
    }
}

/// Install the context propagator.
#[cfg(feature = "otlp")]
fn install_propagator(format: &PropagationFormat) {
    use opentelemetry::propagation::TextMapCompositePropagator;
    use opentelemetry_sdk::propagation::{BaggagePropagator, TraceContextPropagator};

    match format {
        PropagationFormat::TraceContext => {
            opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
        }
        PropagationFormat::B3 | PropagationFormat::Jaeger => {
            // B3 and Jaeger propagators require additional crates
            // Fall back to TraceContext for now
            tracing::warn!(
                format = ?format,
                "Propagation format not yet supported, falling back to TraceContext"
            );
            opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
        }
        PropagationFormat::Multi => {
            // Composite propagator with TraceContext and Baggage
            let propagator = TextMapCompositePropagator::new(vec![
                Box::new(TraceContextPropagator::new()),
                Box::new(BaggagePropagator::new()),
            ]);
            opentelemetry::global::set_text_map_propagator(propagator);
        }
    }
}

/// Build the environment filter from logging config.
fn build_env_filter(config: &LoggingConfig) -> EnvFilter {
    // Start with the configured level
    let base_level = match config.level {
        crate::config::LogLevel::Trace => "trace",
        crate::config::LogLevel::Debug => "debug",
        crate::config::LogLevel::Info => "info",
        crate::config::LogLevel::Warn => "warn",
        crate::config::LogLevel::Error => "error",
    };

    // Check for RUST_LOG environment variable first
    if let Ok(env_filter) = std::env::var("RUST_LOG") {
        EnvFilter::try_new(env_filter).unwrap_or_else(|_| EnvFilter::new(base_level))
    } else if let Some(filter) = &config.filter {
        // Use config filter if provided
        let combined = format!("{},{}", base_level, filter);
        EnvFilter::try_new(combined).unwrap_or_else(|_| EnvFilter::new(base_level))
    } else {
        // Default filter that quiets noisy crates
        EnvFilter::new(format!(
            "{},hyper=warn,h2=warn,tower=info,sqlx=warn,reqwest=warn",
            base_level
        ))
    }
}

/// Guard that ensures OpenTelemetry is properly shut down.
pub struct TracingGuard {
    #[cfg(feature = "otlp")]
    provider: Option<SdkTracerProvider>,
    #[cfg(not(feature = "otlp"))]
    #[allow(dead_code)] // Tracing guard keeps provider alive
    provider: Option<SdkTracerProviderStub>,
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        // Shutdown the tracer provider to flush pending spans
        #[cfg(feature = "otlp")]
        if let Some(provider) = &self.provider
            && let Err(e) = provider.shutdown()
        {
            eprintln!("Error shutting down OpenTelemetry tracer provider: {:?}", e);
        }
    }
}

/// Tracing initialization errors.
#[derive(Debug, thiserror::Error)]
pub enum TracingError {
    #[error("Failed to initialize tracing: {0}")]
    Init(String),
}
