//! Syslog (RFC 5424) log formatter.
//!
//! Syslog is the standard system logging protocol defined in RFC 5424.
//! It is widely supported by log aggregation systems and SIEM platforms.
//!
//! # Format (RFC 5424)
//!
//! Syslog messages have the following structure:
//!
//! ```text
//! <PRI>VERSION TIMESTAMP HOSTNAME APP-NAME PROCID MSGID [SD-ID SD-PARAMS] MSG
//! ```
//!
//! - **PRI**: Priority value = (Facility * 8) + Severity
//! - **VERSION**: Syslog protocol version (always "1" for RFC 5424)
//! - **TIMESTAMP**: ISO 8601 timestamp with timezone
//! - **HOSTNAME**: Machine hostname
//! - **APP-NAME**: Application name (configurable, default: "hadrian")
//! - **PROCID**: Process ID (or "-" if unknown)
//! - **MSGID**: Message type identifier (derived from target)
//! - **SD**: Structured data elements in [id param=value] format
//! - **MSG**: Human-readable message
//!
//! # Severity Mapping
//!
//! RFC 5424 defines 8 severity levels (0-7):
//!
//! | Tracing Level | Syslog Severity | Code | Description |
//! |---------------|-----------------|------|-------------|
//! | ERROR         | Error           | 3    | Error conditions |
//! | WARN          | Warning         | 4    | Warning conditions |
//! | INFO          | Informational   | 6    | Informational messages |
//! | DEBUG         | Debug           | 7    | Debug-level messages |
//! | TRACE         | Debug           | 7    | Debug-level messages |
//!
//! # Example Output
//!
//! ```text
//! <134>1 2025-12-08T10:30:00.000Z gateway.example.com hadrian 12345 api.request [meta@47450 request_id="abc123" method="POST"] API request received
//! ```
//!
//! # References
//!
//! - [RFC 5424 - The Syslog Protocol](https://datatracker.ietf.org/doc/html/rfc5424)

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

use crate::config::SyslogFacility;

/// IANA Private Enterprise Number for structured data.
/// Using 47450 as a placeholder - organizations should register their own PEN.
const STRUCTURED_DATA_PEN: u32 = 47450;

/// Configuration for the Syslog formatter.
#[derive(Debug, Clone)]
pub struct SyslogConfig {
    /// Syslog facility.
    pub facility: SyslogFacility,
    /// Hostname to include in syslog header.
    pub hostname: String,
    /// Application name for APP-NAME field.
    pub app_name: String,
    /// Include structured data elements.
    pub include_structured_data: bool,
    /// Include BOM (Byte Order Mark) before message.
    /// RFC 5424 recommends BOM for UTF-8 messages, but some parsers don't handle it.
    pub include_bom: bool,
}

impl Default for SyslogConfig {
    fn default() -> Self {
        Self {
            facility: SyslogFacility::Local0,
            hostname: {
                #[cfg(feature = "otlp")]
                {
                    hostname::get()
                        .map(|h| h.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| "-".to_string())
                }
                #[cfg(not(feature = "otlp"))]
                {
                    "-".to_string()
                }
            },
            app_name: "hadrian".to_string(),
            include_structured_data: true,
            include_bom: false,
        }
    }
}

impl SyslogConfig {
    /// Create a new Syslog config from SIEM config.
    pub fn from_siem_config(siem: &crate::config::SiemConfig) -> Self {
        Self {
            facility: siem.facility,
            hostname: siem.get_hostname(),
            app_name: siem.app_name.clone(),
            include_structured_data: true,
            include_bom: false,
        }
    }
}

/// A tracing layer that formats events in Syslog (RFC 5424) format.
///
/// This layer writes Syslog-formatted log messages to the provided writer (typically stdout).
pub struct SyslogLayer<W: Write + Send + 'static> {
    config: SyslogConfig,
    writer: Mutex<W>,
}

impl<W: Write + Send + 'static> SyslogLayer<W> {
    /// Create a new Syslog layer with the given configuration and writer.
    pub fn new(config: SyslogConfig, writer: W) -> Self {
        Self {
            config,
            writer: Mutex::new(writer),
        }
    }
}

impl SyslogLayer<io::Stdout> {
    /// Create a new Syslog layer that writes to stdout.
    pub fn stdout(config: SyslogConfig) -> Self {
        Self::new(config, io::stdout())
    }
}

impl SyslogLayer<io::Stderr> {
    /// Create a new Syslog layer that writes to stderr.
    pub fn stderr(config: SyslogConfig) -> Self {
        Self::new(config, io::stderr())
    }
}

impl<S, W> Layer<S> for SyslogLayer<W>
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    W: Write + Send + 'static,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        // Extract event fields
        let mut visitor = SyslogFieldVisitor::new();
        event.record(&mut visitor);

        // Build Syslog message
        let syslog_message = format_syslog_message(
            &self.config,
            metadata.level(),
            metadata.target(),
            &visitor.fields,
            &visitor.message,
        );

        // Write to output
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writeln!(writer, "{}", syslog_message);
        }
    }
}

/// Visitor that collects fields from a tracing event.
struct SyslogFieldVisitor {
    fields: HashMap<String, String>,
    message: Option<String>,
}

impl SyslogFieldVisitor {
    fn new() -> Self {
        Self {
            fields: HashMap::new(),
            message: None,
        }
    }
}

impl Visit for SyslogFieldVisitor {
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

/// Format a Syslog message from the event data.
fn format_syslog_message(
    config: &SyslogConfig,
    level: &Level,
    target: &str,
    fields: &HashMap<String, String>,
    message: &Option<String>,
) -> String {
    let mut output = String::with_capacity(512);

    // Calculate PRI value: (Facility * 8) + Severity
    let severity = level_to_syslog_severity(level);
    let pri = (config.facility.code() as u16 * 8) + severity as u16;

    // Version is always 1 for RFC 5424
    let version = 1;

    // Timestamp in RFC 5424 format (ISO 8601 with microseconds and timezone)
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ");

    // Hostname (max 255 chars, no spaces)
    let hostname = sanitize_syslog_field(&config.hostname, 255);

    // App name (max 48 chars)
    let app_name = sanitize_syslog_field(&config.app_name, 48);

    // Process ID
    let procid = std::process::id();

    // Message ID (derived from target, max 32 chars)
    let msgid = sanitize_syslog_field(target, 32);

    // Build header
    let _ = write!(output, "<{}>{}", pri, version);

    // Add space and timestamp
    let _ = write!(output, " {}", timestamp);

    // Add space and hostname
    let _ = write!(output, " {}", hostname);

    // Add space and app-name
    let _ = write!(output, " {}", app_name);

    // Add space and procid
    let _ = write!(output, " {}", procid);

    // Add space and msgid
    let _ = write!(output, " {}", msgid);

    // Add structured data
    if config.include_structured_data && !fields.is_empty() {
        let _ = write!(output, " {}", format_structured_data(fields));
    } else {
        // NILVALUE if no structured data
        output.push_str(" -");
    }

    // Add message (with optional BOM)
    if let Some(msg) = message {
        if config.include_bom {
            // UTF-8 BOM: EF BB BF
            output.push_str(" \u{FEFF}");
            output.push_str(msg);
        } else {
            output.push(' ');
            output.push_str(msg);
        }
    }

    output
}

/// Format structured data elements.
///
/// Structured data format: [SD-ID param="value" param="value"]
/// SD-ID format: name@PEN (e.g., meta@47450)
fn format_structured_data(fields: &HashMap<String, String>) -> String {
    if fields.is_empty() {
        return "-".to_string();
    }

    let mut output = String::with_capacity(256);
    output.push('[');

    // Use "meta@PEN" as the SD-ID
    let _ = write!(output, "meta@{}", STRUCTURED_DATA_PEN);

    // Add parameters
    for (key, value) in fields {
        // Sanitize key (SD-PARAM-NAME): only ASCII printable, no =, ], "
        let safe_key = sanitize_sd_name(key);
        // Escape value (SD-PARAM-VALUE): escape \, ", ]
        let safe_value = escape_sd_value(value);
        let _ = write!(output, " {}=\"{}\"", safe_key, safe_value);
    }

    output.push(']');
    output
}

/// Convert tracing Level to Syslog severity (0-7 scale).
///
/// RFC 5424 severity levels:
/// - 0: Emergency (not used)
/// - 1: Alert (not used)
/// - 2: Critical (not used)
/// - 3: Error
/// - 4: Warning
/// - 5: Notice (not used)
/// - 6: Informational
/// - 7: Debug
fn level_to_syslog_severity(level: &Level) -> u8 {
    match *level {
        Level::ERROR => 3, // Error
        Level::WARN => 4,  // Warning
        Level::INFO => 6,  // Informational
        Level::DEBUG => 7, // Debug
        Level::TRACE => 7, // Debug (Syslog doesn't have trace)
    }
}

/// Sanitize a syslog header field.
///
/// RFC 5424 requires:
/// - Only printable US-ASCII characters (33-126)
/// - No spaces
/// - Maximum length varies by field
fn sanitize_syslog_field(s: &str, max_len: usize) -> String {
    let sanitized: String = s
        .chars()
        .filter(|c| *c >= '\x21' && *c <= '\x7e') // Printable ASCII, no space
        .take(max_len)
        .collect();

    if sanitized.is_empty() {
        "-".to_string() // NILVALUE
    } else {
        sanitized
    }
}

/// Sanitize a structured data parameter name.
///
/// SD-PARAM-NAME can only contain:
/// - Printable US-ASCII (33-126)
/// - Excluding: =, space, ], "
fn sanitize_sd_name(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .filter(|c| {
            *c >= '\x21' && *c <= '\x7e' // Printable ASCII
                && *c != '='
                && *c != ']'
                && *c != '"'
                && *c != ' '
        })
        .take(32) // Reasonable max length for param names
        .collect();

    if sanitized.is_empty() {
        "param".to_string()
    } else {
        sanitized
    }
}

/// Escape a structured data parameter value.
///
/// SD-PARAM-VALUE requires escaping:
/// - \ (backslash) -> \\
/// - " (double quote) -> \"
/// - ] (closing bracket) -> \]
fn escape_sd_value(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            ']' => result.push_str("\\]"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_to_syslog_severity() {
        assert_eq!(level_to_syslog_severity(&Level::ERROR), 3);
        assert_eq!(level_to_syslog_severity(&Level::WARN), 4);
        assert_eq!(level_to_syslog_severity(&Level::INFO), 6);
        assert_eq!(level_to_syslog_severity(&Level::DEBUG), 7);
        assert_eq!(level_to_syslog_severity(&Level::TRACE), 7);
    }

    #[test]
    fn test_sanitize_syslog_field() {
        assert_eq!(sanitize_syslog_field("simple", 255), "simple");
        assert_eq!(sanitize_syslog_field("with space", 255), "withspace");
        assert_eq!(sanitize_syslog_field("with\ttab", 255), "withtab");
        assert_eq!(sanitize_syslog_field("", 255), "-");
        assert_eq!(sanitize_syslog_field("   ", 255), "-"); // All spaces -> empty -> NILVALUE
        assert_eq!(sanitize_syslog_field("toolong", 4), "tool");
    }

    #[test]
    fn test_sanitize_sd_name() {
        assert_eq!(sanitize_sd_name("simple"), "simple");
        assert_eq!(sanitize_sd_name("with=equals"), "withequals");
        assert_eq!(sanitize_sd_name("with]bracket"), "withbracket");
        assert_eq!(sanitize_sd_name("with\"quote"), "withquote");
        assert_eq!(sanitize_sd_name(""), "param");
    }

    #[test]
    fn test_escape_sd_value() {
        assert_eq!(escape_sd_value("simple"), "simple");
        assert_eq!(escape_sd_value("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_sd_value("with\"quote"), "with\\\"quote");
        assert_eq!(escape_sd_value("with]bracket"), "with\\]bracket");
        assert_eq!(escape_sd_value("all\\\"three]"), "all\\\\\\\"three\\]");
    }

    #[test]
    fn test_format_structured_data_empty() {
        let fields = HashMap::new();
        assert_eq!(format_structured_data(&fields), "-");
    }

    #[test]
    fn test_format_structured_data_with_fields() {
        let mut fields = HashMap::new();
        fields.insert("key1".to_string(), "value1".to_string());

        let sd = format_structured_data(&fields);
        assert!(sd.starts_with("[meta@47450"));
        assert!(sd.contains("key1=\"value1\""));
        assert!(sd.ends_with(']'));
    }

    #[test]
    fn test_format_structured_data_escaping() {
        let mut fields = HashMap::new();
        fields.insert("key".to_string(), "value with \"quotes\"".to_string());

        let sd = format_structured_data(&fields);
        assert!(sd.contains("key=\"value with \\\"quotes\\\"\""));
    }

    #[test]
    fn test_format_syslog_message_basic() {
        let config = SyslogConfig {
            facility: SyslogFacility::Local0,
            hostname: "testhost".to_string(),
            app_name: "testapp".to_string(),
            include_structured_data: false,
            include_bom: false,
        };
        let fields = HashMap::new();
        let message = Some("Test message".to_string());

        let output = format_syslog_message(&config, &Level::INFO, "test.target", &fields, &message);

        // PRI for Local0 (16) + INFO (6) = 16*8 + 6 = 134
        assert!(output.starts_with("<134>1 "));
        assert!(output.contains("testhost"));
        assert!(output.contains("testapp"));
        assert!(output.contains("test.target"));
        assert!(output.contains(" - ")); // NILVALUE for no structured data
        assert!(output.ends_with("Test message"));
    }

    #[test]
    fn test_format_syslog_message_with_structured_data() {
        let config = SyslogConfig {
            facility: SyslogFacility::Auth,
            hostname: "gateway".to_string(),
            app_name: "hadrian".to_string(),
            include_structured_data: true,
            include_bom: false,
        };
        let mut fields = HashMap::new();
        fields.insert("user".to_string(), "admin".to_string());
        fields.insert("action".to_string(), "login".to_string());
        let message = Some("User logged in".to_string());

        let output = format_syslog_message(&config, &Level::INFO, "auth.login", &fields, &message);

        // PRI for Auth (4) + INFO (6) = 4*8 + 6 = 38
        assert!(output.starts_with("<38>1 "));
        assert!(output.contains("[meta@47450"));
        assert!(output.ends_with("User logged in"));
    }

    #[test]
    fn test_format_syslog_message_severity_levels() {
        let config = SyslogConfig::default();
        let fields = HashMap::new();
        let message = None;

        // ERROR: Local0 (16) + 3 = 131
        let error_msg = format_syslog_message(&config, &Level::ERROR, "test", &fields, &message);
        assert!(error_msg.starts_with("<131>1 "));

        // WARN: Local0 (16) + 4 = 132
        let warn_msg = format_syslog_message(&config, &Level::WARN, "test", &fields, &message);
        assert!(warn_msg.starts_with("<132>1 "));

        // INFO: Local0 (16) + 6 = 134
        let info_msg = format_syslog_message(&config, &Level::INFO, "test", &fields, &message);
        assert!(info_msg.starts_with("<134>1 "));

        // DEBUG: Local0 (16) + 7 = 135
        let debug_msg = format_syslog_message(&config, &Level::DEBUG, "test", &fields, &message);
        assert!(debug_msg.starts_with("<135>1 "));

        // TRACE: Local0 (16) + 7 = 135
        let trace_msg = format_syslog_message(&config, &Level::TRACE, "test", &fields, &message);
        assert!(trace_msg.starts_with("<135>1 "));
    }

    #[test]
    fn test_format_syslog_message_different_facilities() {
        let fields = HashMap::new();
        let message = Some("Test".to_string());

        // Kern (0) + INFO (6) = 6
        let config_kern = SyslogConfig {
            facility: SyslogFacility::Kern,
            ..Default::default()
        };
        let kern_msg = format_syslog_message(&config_kern, &Level::INFO, "test", &fields, &message);
        assert!(kern_msg.starts_with("<6>1 "));

        // User (1) + INFO (6) = 14
        let config_user = SyslogConfig {
            facility: SyslogFacility::User,
            ..Default::default()
        };
        let user_msg = format_syslog_message(&config_user, &Level::INFO, "test", &fields, &message);
        assert!(user_msg.starts_with("<14>1 "));

        // Local7 (23) + ERROR (3) = 187
        let config_local7 = SyslogConfig {
            facility: SyslogFacility::Local7,
            ..Default::default()
        };
        let local7_msg =
            format_syslog_message(&config_local7, &Level::ERROR, "test", &fields, &message);
        assert!(local7_msg.starts_with("<187>1 "));
    }

    #[test]
    fn test_format_syslog_message_with_bom() {
        let config = SyslogConfig {
            include_bom: true,
            include_structured_data: false,
            ..Default::default()
        };
        let fields = HashMap::new();
        let message = Some("Test message".to_string());

        let output = format_syslog_message(&config, &Level::INFO, "test", &fields, &message);

        // Should contain BOM (U+FEFF) before message
        assert!(output.contains(" \u{FEFF}Test message"));
    }

    #[test]
    fn test_syslog_config_default() {
        let config = SyslogConfig::default();
        assert_eq!(config.facility, SyslogFacility::Local0);
        assert!(!config.hostname.is_empty());
        assert_eq!(config.app_name, "hadrian");
        assert!(config.include_structured_data);
        assert!(!config.include_bom);
    }

    #[test]
    fn test_syslog_field_visitor_initialization() {
        let visitor = SyslogFieldVisitor::new();

        // Visitor should start with empty fields and no message
        assert!(visitor.fields.is_empty());
        assert!(visitor.message.is_none());
    }

    #[test]
    fn test_pri_calculation() {
        // PRI = Facility * 8 + Severity
        // Verify our format produces correct PRI values

        let test_cases = [
            (SyslogFacility::Kern, Level::ERROR, 3),     // 0*8 + 3 = 3
            (SyslogFacility::User, Level::WARN, 12),     // 1*8 + 4 = 12
            (SyslogFacility::Mail, Level::INFO, 22),     // 2*8 + 6 = 22
            (SyslogFacility::Daemon, Level::DEBUG, 31),  // 3*8 + 7 = 31
            (SyslogFacility::Auth, Level::ERROR, 35),    // 4*8 + 3 = 35
            (SyslogFacility::Local0, Level::INFO, 134),  // 16*8 + 6 = 134
            (SyslogFacility::Local7, Level::ERROR, 187), // 23*8 + 3 = 187
        ];

        for (facility, level, expected_pri) in test_cases {
            let config = SyslogConfig {
                facility,
                include_structured_data: false,
                ..Default::default()
            };
            let fields = HashMap::new();
            let message = None;

            let output = format_syslog_message(&config, &level, "test", &fields, &message);
            let expected_prefix = format!("<{}>1 ", expected_pri);
            assert!(
                output.starts_with(&expected_prefix),
                "Expected PRI {} for facility {:?} and level {:?}, got: {}",
                expected_pri,
                facility,
                level,
                &output[..20.min(output.len())]
            );
        }
    }
}
