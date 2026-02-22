//! LEEF (Log Event Extended Format) log formatter.
//!
//! LEEF is a standardized logging format developed by IBM for QRadar SIEM.
//! It is similar to CEF but uses tabs as delimiters and has different field mappings.
//!
//! # Format
//!
//! LEEF messages have the following structure:
//!
//! ## LEEF 1.0
//! ```text
//! LEEF:1.0|Vendor|Product|Version|EventID|attr1=value1<tab>attr2=value2
//! ```
//!
//! ## LEEF 2.0
//! ```text
//! LEEF:2.0|Vendor|Product|Version|EventID|delimiter|attr1=value1<delimiter>attr2=value2
//! ```
//!
//! - **Version**: LEEF format version (1.0 or 2.0)
//! - **Vendor**: Vendor name (configurable, default: "Hadrian")
//! - **Product**: Product name (configurable, default: "Gateway")
//! - **Version**: Product version (configurable or from Cargo.toml)
//! - **EventID**: Unique identifier for the event type (derived from target/event name)
//! - **delimiter**: (LEEF 2.0 only) Custom delimiter character, defaults to tab (0x09)
//! - **Attributes**: Key=Value pairs separated by delimiter
//!
//! # Severity Mapping
//!
//! LEEF uses a 0-10 severity scale (same as CEF):
//!
//! | Tracing Level | LEEF Severity | Description |
//! |---------------|---------------|-------------|
//! | TRACE         | 0             | Debug/trace information |
//! | DEBUG         | 1             | Debug information |
//! | INFO          | 3             | Informational |
//! | WARN          | 6             | Warning |
//! | ERROR         | 9             | Error |
//!
//! # Example Output
//!
//! ## LEEF 1.0
//! ```text
//! LEEF:1.0|Hadrian|Gateway|1.0.0|api.request|src=192.168.1.1<TAB>dstPort=443<TAB>resource=/v1/chat/completions
//! ```
//!
//! ## LEEF 2.0
//! ```text
//! LEEF:2.0|Hadrian|Gateway|1.0.0|api.request|0x09|src=192.168.1.1<TAB>dstPort=443<TAB>resource=/v1/chat/completions
//! ```
//!
//! # References
//!
//! - [LEEF Format Guide (IBM)](https://www.ibm.com/docs/en/dsm?topic=leef-overview)

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

use crate::config::LeefVersion;

/// Configuration for the LEEF formatter.
#[derive(Debug, Clone)]
pub struct LeefConfig {
    /// Device vendor name for LEEF header.
    pub device_vendor: String,
    /// Device product name for LEEF header.
    pub device_product: String,
    /// Device version for LEEF header.
    pub device_version: String,
    /// Hostname to include in attributes.
    pub hostname: Option<String>,
    /// Include timestamps in output.
    pub include_timestamp: bool,
    /// LEEF format version (1.0 or 2.0).
    pub version: LeefVersion,
    /// Custom delimiter for LEEF 2.0 (defaults to tab).
    pub delimiter: char,
}

impl Default for LeefConfig {
    fn default() -> Self {
        Self {
            device_vendor: "Hadrian".to_string(),
            device_product: "Gateway".to_string(),
            device_version: env!("CARGO_PKG_VERSION").to_string(),
            hostname: None,
            include_timestamp: true,
            version: LeefVersion::V2,
            delimiter: '\t',
        }
    }
}

impl LeefConfig {
    /// Create a new LEEF config from SIEM config.
    pub fn from_siem_config(siem: &crate::config::SiemConfig) -> Self {
        Self {
            device_vendor: siem.device_vendor.clone(),
            device_product: siem.device_product.clone(),
            device_version: siem.get_device_version().to_string(),
            hostname: Some(siem.get_hostname()),
            include_timestamp: true,
            version: siem.leef_version,
            delimiter: '\t',
        }
    }
}

/// A tracing layer that formats events in LEEF format.
///
/// This layer writes LEEF-formatted log messages to the provided writer (typically stdout).
pub struct LeefLayer<W: Write + Send + 'static> {
    config: LeefConfig,
    writer: Mutex<W>,
}

impl<W: Write + Send + 'static> LeefLayer<W> {
    /// Create a new LEEF layer with the given configuration and writer.
    pub fn new(config: LeefConfig, writer: W) -> Self {
        Self {
            config,
            writer: Mutex::new(writer),
        }
    }
}

impl LeefLayer<io::Stdout> {
    /// Create a new LEEF layer that writes to stdout.
    pub fn stdout(config: LeefConfig) -> Self {
        Self::new(config, io::stdout())
    }
}

impl LeefLayer<io::Stderr> {
    /// Create a new LEEF layer that writes to stderr.
    pub fn stderr(config: LeefConfig) -> Self {
        Self::new(config, io::stderr())
    }
}

impl<S, W> Layer<S> for LeefLayer<W>
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    W: Write + Send + 'static,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        // Extract event fields
        let mut visitor = LeefFieldVisitor::new();
        event.record(&mut visitor);

        // Build LEEF message
        let leef_message = format_leef_message(
            &self.config,
            metadata.level(),
            metadata.target(),
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
                    leef_message
                )
            } else {
                writeln!(writer, "{}", leef_message)
            };
        }
    }
}

/// Visitor that collects fields from a tracing event.
struct LeefFieldVisitor {
    fields: HashMap<String, String>,
    message: Option<String>,
}

impl LeefFieldVisitor {
    fn new() -> Self {
        Self {
            fields: HashMap::new(),
            message: None,
        }
    }
}

impl Visit for LeefFieldVisitor {
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

/// Format a LEEF message from the event data.
fn format_leef_message(
    config: &LeefConfig,
    level: &Level,
    target: &str,
    fields: &HashMap<String, String>,
    message: &Option<String>,
) -> String {
    let mut output = String::with_capacity(256);

    // LEEF Header: LEEF:Version|Vendor|Product|Version|EventID|[delimiter]
    match config.version {
        LeefVersion::V1 => {
            let _ = write!(
                output,
                "LEEF:1.0|{}|{}|{}|{}|",
                escape_leef_header(&config.device_vendor),
                escape_leef_header(&config.device_product),
                escape_leef_header(&config.device_version),
                escape_leef_header(target),
            );
        }
        LeefVersion::V2 => {
            // LEEF 2.0 includes delimiter specification as hex
            let delimiter_hex = format!("0x{:02x}", config.delimiter as u8);
            let _ = write!(
                output,
                "LEEF:2.0|{}|{}|{}|{}|{}|",
                escape_leef_header(&config.device_vendor),
                escape_leef_header(&config.device_product),
                escape_leef_header(&config.device_version),
                escape_leef_header(target),
                delimiter_hex,
            );
        }
    }

    // Build attribute fields
    let mut attributes = Vec::new();
    let delimiter = config.delimiter;

    // Add standard LEEF attributes
    // sev (severity) is a standard LEEF attribute
    attributes.push(format!("sev={}", level_to_leef_severity(level)));

    // Add hostname if configured
    if let Some(ref hostname) = config.hostname {
        attributes.push(format!(
            "devName={}",
            escape_leef_value(hostname, delimiter)
        ));
    }

    // Add devTime (timestamp in LEEF format)
    let timestamp = chrono::Utc::now().format("%b %d %Y %H:%M:%S").to_string();
    attributes.push(format!(
        "devTime={}",
        escape_leef_value(&timestamp, delimiter)
    ));

    // Map common tracing fields to LEEF attribute names
    for (key, value) in fields {
        let leef_key = map_field_to_leef_key(key);
        attributes.push(format!(
            "{}={}",
            leef_key,
            escape_leef_value(value, delimiter)
        ));
    }

    // Add message as 'msg' if present
    if let Some(msg) = message {
        attributes.push(format!("msg={}", escape_leef_value(msg, delimiter)));
    }

    // Join attributes with the delimiter
    let delimiter_str = delimiter.to_string();
    output.push_str(&attributes.join(&delimiter_str));

    output
}

/// Map tracing field names to standard LEEF attribute names.
///
/// LEEF defines standard attribute names for common fields. This function
/// maps common tracing field names to their LEEF equivalents.
fn map_field_to_leef_key(field: &str) -> &str {
    match field {
        // Network fields
        "src_ip" | "source_ip" | "client_ip" | "remote_addr" => "src",
        "dst_ip" | "dest_ip" | "destination_ip" | "server_ip" => "dst",
        "src_port" | "source_port" | "client_port" | "remote_port" => "srcPort",
        "dst_port" | "dest_port" | "destination_port" | "server_port" | "port" => "dstPort",
        "protocol" | "proto" => "proto",
        "src_mac" | "source_mac" => "srcMAC",
        "dst_mac" | "dest_mac" => "dstMAC",

        // User fields
        "user" | "username" | "user_name" => "usrName",
        "user_id" | "uid" => "identSrc",
        "target_user" | "dst_user" => "identDst",
        "group" | "group_name" => "identGrp",

        // Request fields
        "method" | "http_method" => "proto",
        "path" | "uri" | "url" | "request_path" => "resource",
        "request_id" | "trace_id" | "correlation_id" => "externalId",
        "status" | "status_code" | "http_status" => "action",

        // Size fields
        "request_size" | "bytes_in" | "src_bytes" => "srcBytes",
        "response_size" | "bytes_out" | "dst_bytes" => "dstBytes",
        "total_bytes" | "bytes" => "totalBytes",
        "src_packets" | "packets_in" => "srcPackets",
        "dst_packets" | "packets_out" => "dstPackets",

        // Device fields
        "hostname" | "host" | "device_name" => "devName",
        "device_type" => "devType",

        // Time fields
        "latency" | "duration" | "latency_ms" | "duration_ms" => "devTimeFormat",

        // Application fields
        "app" | "application" | "service" => "application",
        "action" | "event_type" => "action",
        "reason" | "error" | "error_message" => "reason",
        "category" | "event_category" => "cat",
        "policy" | "policy_name" => "policy",

        // AI/Model specific fields (custom extensions)
        "api_key_id" | "key_id" => "apiKeyId",
        "org_id" | "organization_id" => "orgId",
        "project_id" => "projectId",
        "model" | "model_name" => "aiModel",
        "provider" | "provider_name" => "aiProvider",
        "tokens" | "total_tokens" => "aiTokens",
        "cost" | "cost_cents" => "aiCost",

        // Default: use original field name
        _ => field,
    }
}

/// Convert tracing Level to LEEF severity (0-10 scale).
///
/// LEEF severity scale (same as CEF):
/// - 0-3: Low (informational)
/// - 4-6: Medium (warnings)
/// - 7-8: High (errors)
/// - 9-10: Very High (critical)
fn level_to_leef_severity(level: &Level) -> u8 {
    match *level {
        Level::TRACE => 0,
        Level::DEBUG => 1,
        Level::INFO => 3,
        Level::WARN => 6,
        Level::ERROR => 9,
    }
}

/// Escape special characters in LEEF header fields.
///
/// Header fields use | as delimiter, so | must be escaped with \.
fn escape_leef_header(s: &str) -> String {
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

/// Escape special characters in LEEF attribute values.
///
/// Attribute values must escape the delimiter, =, and special characters.
fn escape_leef_value(s: &str, delimiter: char) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c == delimiter {
            // Escape the delimiter
            result.push_str(&format!("\\x{:02x}", c as u8));
        } else {
            match c {
                '\\' => result.push_str("\\\\"),
                '=' => result.push_str("\\="),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                _ => result.push(c),
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_to_leef_severity() {
        assert_eq!(level_to_leef_severity(&Level::TRACE), 0);
        assert_eq!(level_to_leef_severity(&Level::DEBUG), 1);
        assert_eq!(level_to_leef_severity(&Level::INFO), 3);
        assert_eq!(level_to_leef_severity(&Level::WARN), 6);
        assert_eq!(level_to_leef_severity(&Level::ERROR), 9);
    }

    #[test]
    fn test_escape_leef_header() {
        assert_eq!(escape_leef_header("simple"), "simple");
        assert_eq!(escape_leef_header("with|pipe"), "with\\|pipe");
        assert_eq!(escape_leef_header("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_leef_header("multi|pipe|test"), "multi\\|pipe\\|test");
        assert_eq!(escape_leef_header("new\nline"), "new\\nline");
        assert_eq!(escape_leef_header("carriage\rreturn"), "carriage\\rreturn");
    }

    #[test]
    fn test_escape_leef_value() {
        let delimiter = '\t';
        assert_eq!(escape_leef_value("simple", delimiter), "simple");
        assert_eq!(escape_leef_value("with=equals", delimiter), "with\\=equals");
        assert_eq!(
            escape_leef_value("with\\backslash", delimiter),
            "with\\\\backslash"
        );
        assert_eq!(
            escape_leef_value("key=value=test", delimiter),
            "key\\=value\\=test"
        );
        assert_eq!(escape_leef_value("new\nline", delimiter), "new\\nline");
        // Tab delimiter should be escaped
        assert_eq!(escape_leef_value("with\ttab", delimiter), "with\\x09tab");
    }

    #[test]
    fn test_escape_leef_value_custom_delimiter() {
        // Test with custom delimiter (|)
        let delimiter = '|';
        assert_eq!(escape_leef_value("simple", delimiter), "simple");
        assert_eq!(escape_leef_value("with|pipe", delimiter), "with\\x7cpipe");
        assert_eq!(escape_leef_value("with\ttab", delimiter), "with\ttab"); // Tab not escaped
    }

    #[test]
    fn test_map_field_to_leef_key() {
        // Network fields
        assert_eq!(map_field_to_leef_key("src_ip"), "src");
        assert_eq!(map_field_to_leef_key("client_ip"), "src");
        assert_eq!(map_field_to_leef_key("dst_ip"), "dst");
        assert_eq!(map_field_to_leef_key("src_port"), "srcPort");
        assert_eq!(map_field_to_leef_key("dst_port"), "dstPort");

        // User fields
        assert_eq!(map_field_to_leef_key("user"), "usrName");
        assert_eq!(map_field_to_leef_key("username"), "usrName");

        // Request fields
        assert_eq!(map_field_to_leef_key("path"), "resource");
        assert_eq!(map_field_to_leef_key("request_id"), "externalId");
        assert_eq!(map_field_to_leef_key("status"), "action");

        // AI-specific fields
        assert_eq!(map_field_to_leef_key("model"), "aiModel");
        assert_eq!(map_field_to_leef_key("provider"), "aiProvider");
        assert_eq!(map_field_to_leef_key("tokens"), "aiTokens");

        // Unknown field returns as-is
        assert_eq!(map_field_to_leef_key("custom_field"), "custom_field");
    }

    #[test]
    fn test_format_leef_message_v1_basic() {
        let config = LeefConfig {
            version: LeefVersion::V1,
            hostname: None,
            include_timestamp: false,
            ..Default::default()
        };
        let fields = HashMap::new();
        let message = Some("Test message".to_string());

        let output = format_leef_message(&config, &Level::INFO, "test.target", &fields, &message);

        assert!(output.starts_with("LEEF:1.0|Hadrian|Gateway|"));
        assert!(output.contains("|test.target|"));
        assert!(output.contains("sev=3"));
        assert!(output.contains("msg=Test message"));
        // V1 should not have the delimiter field in header
        assert!(!output.contains("0x09|"));
    }

    #[test]
    fn test_format_leef_message_v2_basic() {
        let config = LeefConfig {
            version: LeefVersion::V2,
            hostname: None,
            include_timestamp: false,
            ..Default::default()
        };
        let fields = HashMap::new();
        let message = Some("Test message".to_string());

        let output = format_leef_message(&config, &Level::INFO, "test.target", &fields, &message);

        assert!(output.starts_with("LEEF:2.0|Hadrian|Gateway|"));
        assert!(output.contains("|test.target|0x09|"));
        assert!(output.contains("sev=3"));
        assert!(output.contains("msg=Test message"));
    }

    #[test]
    fn test_format_leef_message_with_fields() {
        let config = LeefConfig {
            version: LeefVersion::V2,
            hostname: None,
            include_timestamp: false,
            ..Default::default()
        };
        let mut fields = HashMap::new();
        fields.insert("src_ip".to_string(), "192.168.1.1".to_string());
        fields.insert("dst_port".to_string(), "443".to_string());
        fields.insert("method".to_string(), "POST".to_string());
        let message = Some("API request".to_string());

        let output = format_leef_message(&config, &Level::INFO, "api.request", &fields, &message);

        assert!(output.contains("src=192.168.1.1"));
        assert!(output.contains("dstPort=443"));
        assert!(output.contains("msg=API request"));
    }

    #[test]
    fn test_format_leef_message_with_hostname() {
        let config = LeefConfig {
            version: LeefVersion::V2,
            hostname: Some("gateway-prod-1".to_string()),
            include_timestamp: false,
            ..Default::default()
        };
        let fields = HashMap::new();
        let message = None;

        let output = format_leef_message(&config, &Level::INFO, "test", &fields, &message);

        assert!(output.contains("devName=gateway-prod-1"));
    }

    #[test]
    fn test_format_leef_message_escaping() {
        let config = LeefConfig {
            version: LeefVersion::V2,
            device_vendor: "Test|Vendor".to_string(),
            device_product: "Test\\Product".to_string(),
            hostname: None,
            include_timestamp: false,
            ..Default::default()
        };
        let mut fields = HashMap::new();
        fields.insert("data".to_string(), "value=with=equals".to_string());
        let message = Some("Message with\nnewline".to_string());

        let output = format_leef_message(&config, &Level::WARN, "test", &fields, &message);

        // Header escaping
        assert!(output.contains("Test\\|Vendor"));
        assert!(output.contains("Test\\\\Product"));
        // Value escaping
        assert!(output.contains("data=value\\=with\\=equals"));
        assert!(output.contains("msg=Message with\\nnewline"));
    }

    #[test]
    fn test_format_leef_message_severity_levels() {
        let config = LeefConfig {
            version: LeefVersion::V2,
            hostname: None,
            include_timestamp: false,
            ..Default::default()
        };
        let fields = HashMap::new();
        let message = None;

        let trace_msg = format_leef_message(&config, &Level::TRACE, "test", &fields, &message);
        assert!(trace_msg.contains("sev=0"));

        let debug_msg = format_leef_message(&config, &Level::DEBUG, "test", &fields, &message);
        assert!(debug_msg.contains("sev=1"));

        let info_msg = format_leef_message(&config, &Level::INFO, "test", &fields, &message);
        assert!(info_msg.contains("sev=3"));

        let warn_msg = format_leef_message(&config, &Level::WARN, "test", &fields, &message);
        assert!(warn_msg.contains("sev=6"));

        let error_msg = format_leef_message(&config, &Level::ERROR, "test", &fields, &message);
        assert!(error_msg.contains("sev=9"));
    }

    #[test]
    fn test_leef_config_default() {
        let config = LeefConfig::default();
        assert_eq!(config.device_vendor, "Hadrian");
        assert_eq!(config.device_product, "Gateway");
        assert!(!config.device_version.is_empty());
        assert!(config.hostname.is_none());
        assert!(config.include_timestamp);
        assert_eq!(config.version, LeefVersion::V2);
        assert_eq!(config.delimiter, '\t');
    }

    #[test]
    fn test_leef_field_visitor_initialization() {
        let visitor = LeefFieldVisitor::new();

        // Visitor should start with empty fields and no message
        assert!(visitor.fields.is_empty());
        assert!(visitor.message.is_none());
    }
}
