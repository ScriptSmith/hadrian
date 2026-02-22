//! SIEM (Security Information and Event Management) log formatters.
//!
//! This module provides log formatters for enterprise SIEM integration:
//!
//! - **CEF (Common Event Format)**: ArcSight-originated format supported by most SIEMs
//! - **LEEF (Log Event Extended Format)**: IBM QRadar format
//! - **Syslog (RFC 5424)**: Standard syslog format for any syslog server
//!
//! # Architecture
//!
//! Each formatter is implemented as a `tracing_subscriber::Layer` that intercepts
//! tracing events and formats them according to the respective SIEM format specification.
//!
//! ```text
//! tracing::Event -> SiemLayer -> Formatted Output -> stdout/stderr
//!                      |
//!                      +-> Field Mapping
//!                      +-> Severity Mapping
//!                      +-> Header Generation
//!                      +-> Extension Encoding
//! ```
//!
//! # Common Field Mappings
//!
//! The formatters attempt to map common tracing fields to SIEM-specific fields:
//!
//! | Tracing Field | CEF Extension | LEEF Attribute | Description |
//! |---------------|---------------|----------------|-------------|
//! | `src_ip`      | `src`         | `src`          | Source IP address |
//! | `dst_ip`      | `dst`         | `dst`          | Destination IP address |
//! | `src_port`    | `spt`         | `srcPort`      | Source port |
//! | `dst_port`    | `dpt`         | `dstPort`      | Destination port |
//! | `user`        | `suser`       | `usrName`      | Username |
//! | `request_id`  | `externalId`  | `externalId`   | Request correlation ID |
//! | `method`      | `requestMethod` | `proto`      | HTTP method |
//! | `path`        | `request`     | `resource`     | Request path |
//! | `status`      | `outcome`     | `action`       | HTTP status code |

pub mod cef;
pub mod leef;
pub mod syslog;

pub use cef::{CefConfig, CefLayer};
pub use leef::{LeefConfig, LeefLayer};
pub use syslog::{SyslogConfig, SyslogLayer};
