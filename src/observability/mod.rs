//! Observability module providing logging, tracing, and metrics.
//!
//! This module initializes and configures:
//! - Structured logging with configurable formats (pretty, compact, JSON, CEF, LEEF, Syslog)
//! - OpenTelemetry distributed tracing with OTLP export
//! - Prometheus metrics with custom histograms for latency and tokens
//! - SIEM integration for enterprise security monitoring

pub mod metrics;
pub mod siem;
mod tracing_init;

pub use tracing_init::*;

/// Set the current span's OpenTelemetry status to `Ok`.
///
/// Expands to a real `set_status(Status::Ok)` call when `otlp` is enabled,
/// and to nothing otherwise.
macro_rules! otel_span_ok {
    () => {
        #[cfg(feature = "otlp")]
        {
            use ::tracing_opentelemetry::OpenTelemetrySpanExt as _;
            ::tracing::Span::current().set_status(::opentelemetry::trace::Status::Ok);
        }
    };
}

/// Set the current span's OpenTelemetry status to an error with a formatted message.
///
/// Accepts `format!`-style arguments. Expands to a real `set_status(Status::error(...))`
/// call when `otlp` is enabled, and to nothing otherwise.
macro_rules! otel_span_error {
    ($($arg:tt)*) => {
        #[cfg(feature = "otlp")]
        {
            use ::tracing_opentelemetry::OpenTelemetrySpanExt as _;
            ::tracing::Span::current()
                .set_status(::opentelemetry::trace::Status::error(format!($($arg)*)));
        }
    };
}

pub(crate) use otel_span_error;
pub(crate) use otel_span_ok;
