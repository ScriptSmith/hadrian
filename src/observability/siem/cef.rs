//! CEF (Common Event Format) log formatter.
//!
//! CEF is a standardized logging format originally developed by ArcSight (now Micro Focus).
//! It is widely supported by SIEMs including ArcSight, Splunk, LogRhythm, and others.
//!
//! # Format
//!
//! CEF messages have the following structure:
//!
//! ```text
//! CEF:Version|Device Vendor|Device Product|Device Version|Signature ID|Name|Severity|Extension
//! ```
//!
//! - **Version**: CEF format version (always "0" for CEF 0.1)
//! - **Device Vendor**: Vendor name (configurable, default: "Hadrian")
//! - **Device Product**: Product name (configurable, default: "Gateway")
//! - **Device Version**: Product version (configurable or from Cargo.toml)
//! - **Signature ID**: Unique identifier for the event type (derived from target/event name)
//! - **Name**: Human-readable event name
//! - **Severity**: 0-10 scale (mapped from tracing Level)
//! - **Extension**: Key=Value pairs with additional data
//!
//! # Severity Mapping
//!
//! | Tracing Level | CEF Severity | Description |
//! |---------------|--------------|-------------|
//! | TRACE         | 0            | Debug/trace information |
//! | DEBUG         | 1            | Debug information |
//! | INFO          | 3            | Informational |
//! | WARN          | 6            | Warning |
//! | ERROR         | 9            | Error |
//!
//! # Example Output
//!
//! ```text
//! CEF:0|Hadrian|Gateway|1.0.0|api.request|API Request|3|src=192.168.1.1 spt=443 request=/v1/chat/completions msg=Request received
//! ```
//!
//! # References
//!
//! - [CEF Format Guide (Micro Focus)](https://www.microfocus.com/documentation/arcsight/)

use std::{
    collections::HashMap,
    fmt::Write as FmtWrite,
    io::{self, Write},
    sync::Mutex,
};

use tracing::{
    Event, Level, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{Layer, layer::Context};

/// Configuration for the CEF formatter.
#[derive(Debug, Clone)]
pub struct CefConfig {
    /// Device vendor name for CEF header.
    pub device_vendor: String,
    /// Device product name for CEF header.
    pub device_product: String,
    /// Device version for CEF header.
    pub device_version: String,
    /// Hostname to include in logs (optional).
    pub hostname: Option<String>,
    /// Include timestamps in output.
    pub include_timestamp: bool,
}

impl Default for CefConfig {
    fn default() -> Self {
        Self {
            device_vendor: "Hadrian".to_string(),
            device_product: "Gateway".to_string(),
            device_version: env!("CARGO_PKG_VERSION").to_string(),
            hostname: None,
            include_timestamp: true,
        }
    }
}

impl CefConfig {
    /// Create a new CEF config from SIEM config.
    pub fn from_siem_config(siem: &crate::config::SiemConfig) -> Self {
        Self {
            device_vendor: siem.device_vendor.clone(),
            device_product: siem.device_product.clone(),
            device_version: siem.get_device_version().to_string(),
            hostname: Some(siem.get_hostname()),
            include_timestamp: true,
        }
    }
}

/// A tracing layer that formats events in CEF format.
///
/// This layer writes CEF-formatted log messages to the provided writer (typically stdout).
pub struct CefLayer<W: Write + Send + 'static> {
    config: CefConfig,
    writer: Mutex<W>,
}

impl<W: Write + Send + 'static> CefLayer<W> {
    /// Create a new CEF layer with the given configuration and writer.
    pub fn new(config: CefConfig, writer: W) -> Self {
        Self {
            config,
            writer: Mutex::new(writer),
        }
    }
}

impl CefLayer<io::Stdout> {
    /// Create a new CEF layer that writes to stdout.
    pub fn stdout(config: CefConfig) -> Self {
        Self::new(config, io::stdout())
    }
}

impl CefLayer<io::Stderr> {
    /// Create a new CEF layer that writes to stderr.
    pub fn stderr(config: CefConfig) -> Self {
        Self::new(config, io::stderr())
    }
}

impl<S, W> Layer<S> for CefLayer<W>
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    W: Write + Send + 'static,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        // Extract event fields
        let mut visitor = CefFieldVisitor::new();
        event.record(&mut visitor);

        // Build CEF message
        let cef_message = format_cef_message(
            &self.config,
            metadata.level(),
            metadata.target(),
            metadata.name(),
            &visitor.fields,
            &visitor.message,
        );

        // Write to output
        if let Ok(mut writer) = self.writer.lock() {
            let _ = if self.config.include_timestamp {
                writeln!(
                    writer,
                    "{} {}",
                    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                    cef_message
                )
            } else {
                writeln!(writer, "{}", cef_message)
            };
        }
    }
}

/// Visitor that collects fields from a tracing event.
struct CefFieldVisitor {
    fields: HashMap<String, String>,
    message: Option<String>,
}

impl CefFieldVisitor {
    fn new() -> Self {
        Self {
            fields: HashMap::new(),
            message: None,
        }
    }
}

impl Visit for CefFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        let value_str = format!("{:?}", value);

        if name == "message" {
            self.message = Some(value_str.trim_matches('"').to_string());
        } else {
            self.fields.insert(name.to_string(), value_str);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let name = field.name();

        if name == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(name.to_string(), value.to_string());
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.fields
            .insert(field.name().to_string(), value.to_string());
    }
}

/// Format a CEF message from the event data.
fn format_cef_message(
    config: &CefConfig,
    level: &Level,
    target: &str,
    name: &str,
    fields: &HashMap<String, String>,
    message: &Option<String>,
) -> String {
    let mut output = String::with_capacity(256);

    // CEF Header: CEF:Version|Vendor|Product|Version|SignatureID|Name|Severity|
    let _ = write!(
        output,
        "CEF:0|{}|{}|{}|{}|{}|{}|",
        escape_cef_header(&config.device_vendor),
        escape_cef_header(&config.device_product),
        escape_cef_header(&config.device_version),
        escape_cef_header(target),
        escape_cef_header(name),
        level_to_cef_severity(level)
    );

    // Build extension fields
    let mut extensions = Vec::new();

    // Add hostname if configured
    if let Some(ref hostname) = config.hostname {
        extensions.push(format!("dvchost={}", escape_cef_extension(hostname)));
    }

    // Map common tracing fields to CEF extension keys
    for (key, value) in fields {
        let cef_key = map_field_to_cef_key(key);
        extensions.push(format!("{}={}", cef_key, escape_cef_extension(value)));
    }

    // Add message as 'msg' if present
    if let Some(msg) = message {
        extensions.push(format!("msg={}", escape_cef_extension(msg)));
    }

    // Join extensions with spaces
    output.push_str(&extensions.join(" "));

    output
}

/// Map tracing field names to standard CEF extension keys.
///
/// CEF defines standard extension keys for common fields. This function
/// maps common tracing field names to their CEF equivalents.
fn map_field_to_cef_key(field: &str) -> &str {
    match field {
        // Network fields
        "src_ip" | "source_ip" | "client_ip" | "remote_addr" => "src",
        "dst_ip" | "dest_ip" | "destination_ip" | "server_ip" => "dst",
        "src_port" | "source_port" | "client_port" | "remote_port" => "spt",
        "dst_port" | "dest_port" | "destination_port" | "server_port" | "port" => "dpt",
        "protocol" | "proto" => "proto",

        // User fields
        "user" | "username" | "user_name" | "user_id" => "suser",
        "target_user" | "dst_user" => "duser",

        // Request fields
        "method" | "http_method" => "requestMethod",
        "path" | "uri" | "url" | "request_path" => "request",
        "request_id" | "trace_id" | "correlation_id" => "externalId",
        "status" | "status_code" | "http_status" => "outcome",

        // Time fields
        "latency" | "duration" | "latency_ms" | "duration_ms" => "cn1",
        "latency_label" => "cn1Label",

        // Size fields
        "request_size" | "bytes_in" => "in",
        "response_size" | "bytes_out" => "out",

        // Application fields
        "app" | "application" | "service" => "app",
        "action" | "event_type" => "act",
        "reason" | "error" | "error_message" => "reason",
        "file" | "file_path" | "filename" => "fname",

        // Authentication fields
        "api_key_id" | "key_id" => "cs1",
        "api_key_label" => "cs1Label",
        "org_id" | "organization_id" => "cs2",
        "org_label" => "cs2Label",
        "project_id" => "cs3",
        "project_label" => "cs3Label",

        // Model/AI specific fields
        "model" | "model_name" => "cs4",
        "model_label" => "cs4Label",
        "provider" | "provider_name" => "cs5",
        "provider_label" => "cs5Label",
        "tokens" | "total_tokens" => "cn2",
        "tokens_label" => "cn2Label",
        "cost" | "cost_cents" => "cn3",
        "cost_label" => "cn3Label",

        // Default: use original field name with 'flexString' prefix pattern
        _ => field,
    }
}

/// Convert tracing Level to CEF severity (0-10 scale).
///
/// CEF severity scale:
/// - 0-3: Low (informational)
/// - 4-6: Medium (warnings)
/// - 7-8: High (errors)
/// - 9-10: Very High (critical)
fn level_to_cef_severity(level: &Level) -> u8 {
    match *level {
        Level::TRACE => 0,
        Level::DEBUG => 1,
        Level::INFO => 3,
        Level::WARN => 6,
        Level::ERROR => 9,
    }
}

/// Escape special characters in CEF header fields.
///
/// Header fields use | as delimiter, so | and \ must be escaped.
fn escape_cef_header(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '|' => result.push_str("\\|"),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            _ => result.push(c),
        }
    }
    result
}

/// Escape special characters in CEF extension values.
///
/// Extension values use = and space as delimiters, so they must be escaped.
/// Also escapes newlines and backslashes.
fn escape_cef_extension(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '=' => result.push_str("\\="),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_to_cef_severity() {
        assert_eq!(level_to_cef_severity(&Level::TRACE), 0);
        assert_eq!(level_to_cef_severity(&Level::DEBUG), 1);
        assert_eq!(level_to_cef_severity(&Level::INFO), 3);
        assert_eq!(level_to_cef_severity(&Level::WARN), 6);
        assert_eq!(level_to_cef_severity(&Level::ERROR), 9);
    }

    #[test]
    fn test_escape_cef_header() {
        assert_eq!(escape_cef_header("simple"), "simple");
        assert_eq!(escape_cef_header("with|pipe"), "with\\|pipe");
        assert_eq!(escape_cef_header("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_cef_header("multi|pipe|test"), "multi\\|pipe\\|test");
        assert_eq!(escape_cef_header("new\nline"), "new\\nline");
        assert_eq!(escape_cef_header("carriage\rreturn"), "carriage\\rreturn");
    }

    #[test]
    fn test_escape_cef_extension() {
        assert_eq!(escape_cef_extension("simple"), "simple");
        assert_eq!(escape_cef_extension("with=equals"), "with\\=equals");
        assert_eq!(escape_cef_extension("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_cef_extension("key=value=test"), "key\\=value\\=test");
        assert_eq!(escape_cef_extension("new\nline"), "new\\nline");
    }

    #[test]
    fn test_map_field_to_cef_key() {
        // Network fields
        assert_eq!(map_field_to_cef_key("src_ip"), "src");
        assert_eq!(map_field_to_cef_key("client_ip"), "src");
        assert_eq!(map_field_to_cef_key("dst_ip"), "dst");
        assert_eq!(map_field_to_cef_key("src_port"), "spt");
        assert_eq!(map_field_to_cef_key("dst_port"), "dpt");

        // User fields
        assert_eq!(map_field_to_cef_key("user"), "suser");
        assert_eq!(map_field_to_cef_key("username"), "suser");

        // Request fields
        assert_eq!(map_field_to_cef_key("method"), "requestMethod");
        assert_eq!(map_field_to_cef_key("path"), "request");
        assert_eq!(map_field_to_cef_key("request_id"), "externalId");
        assert_eq!(map_field_to_cef_key("status"), "outcome");

        // Unknown field returns as-is
        assert_eq!(map_field_to_cef_key("custom_field"), "custom_field");
    }

    #[test]
    fn test_format_cef_message_basic() {
        let config = CefConfig::default();
        let fields = HashMap::new();
        let message = Some("Test message".to_string());

        let output = format_cef_message(
            &config,
            &Level::INFO,
            "test.target",
            "event test",
            &fields,
            &message,
        );

        assert!(output.starts_with("CEF:0|Hadrian|Gateway|"));
        assert!(output.contains("|test.target|event test|3|"));
        assert!(output.contains("msg=Test message"));
    }

    #[test]
    fn test_format_cef_message_with_fields() {
        let config = CefConfig::default();
        let mut fields = HashMap::new();
        fields.insert("src_ip".to_string(), "192.168.1.1".to_string());
        fields.insert("dst_port".to_string(), "443".to_string());
        fields.insert("method".to_string(), "POST".to_string());
        let message = Some("API request".to_string());

        let output = format_cef_message(
            &config,
            &Level::INFO,
            "api.request",
            "Request",
            &fields,
            &message,
        );

        assert!(output.contains("src=192.168.1.1"));
        assert!(output.contains("dpt=443"));
        assert!(output.contains("requestMethod=POST"));
        assert!(output.contains("msg=API request"));
    }

    #[test]
    fn test_format_cef_message_with_hostname() {
        let config = CefConfig {
            hostname: Some("gateway-prod-1".to_string()),
            ..Default::default()
        };
        let fields = HashMap::new();
        let message = None;

        let output = format_cef_message(&config, &Level::INFO, "test", "event", &fields, &message);

        assert!(output.contains("dvchost=gateway-prod-1"));
    }

    #[test]
    fn test_format_cef_message_escaping() {
        let config = CefConfig {
            device_vendor: "Test|Vendor".to_string(),
            device_product: "Test\\Product".to_string(),
            ..Default::default()
        };
        let mut fields = HashMap::new();
        fields.insert("data".to_string(), "value=with=equals".to_string());
        let message = Some("Message with\nnewline".to_string());

        let output = format_cef_message(&config, &Level::WARN, "test", "event", &fields, &message);

        // Header escaping
        assert!(output.contains("Test\\|Vendor"));
        assert!(output.contains("Test\\\\Product"));
        // Extension escaping
        assert!(output.contains("data=value\\=with\\=equals"));
        assert!(output.contains("msg=Message with\\nnewline"));
    }

    #[test]
    fn test_format_cef_message_severity_levels() {
        let config = CefConfig::default();
        let fields = HashMap::new();
        let message = None;

        // Test each severity level appears in the message
        let trace_msg =
            format_cef_message(&config, &Level::TRACE, "test", "event", &fields, &message);
        assert!(trace_msg.contains("|0|"));

        let debug_msg =
            format_cef_message(&config, &Level::DEBUG, "test", "event", &fields, &message);
        assert!(debug_msg.contains("|1|"));

        let info_msg =
            format_cef_message(&config, &Level::INFO, "test", "event", &fields, &message);
        assert!(info_msg.contains("|3|"));

        let warn_msg =
            format_cef_message(&config, &Level::WARN, "test", "event", &fields, &message);
        assert!(warn_msg.contains("|6|"));

        let error_msg =
            format_cef_message(&config, &Level::ERROR, "test", "event", &fields, &message);
        assert!(error_msg.contains("|9|"));
    }

    #[test]
    fn test_cef_config_default() {
        let config = CefConfig::default();
        assert_eq!(config.device_vendor, "Hadrian");
        assert_eq!(config.device_product, "Gateway");
        assert!(!config.device_version.is_empty());
        assert!(config.hostname.is_none());
        assert!(config.include_timestamp);
    }

    #[test]
    fn test_cef_field_visitor_initialization() {
        let visitor = CefFieldVisitor::new();

        // Visitor should start with empty fields and no message
        assert!(visitor.fields.is_empty());
        assert!(visitor.message.is_none());
    }
}
