//! Virus scanning service for file uploads.
//!
//! This module provides optional antivirus scanning for uploaded files
//! before they are stored. Currently supports ClamAV via the clamd daemon.
//!
//! # Configuration
//!
//! Virus scanning is disabled by default. To enable:
//!
//! ```toml
//! [features.file_processing.virus_scan]
//! enabled = true
//! backend = "clamav"
//!
//! [features.file_processing.virus_scan.clamav]
//! host = "localhost"
//! port = 3310
//! timeout_ms = 30000
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let scanner = ClamAvScanner::new(config)?;
//! let result = scanner.scan(&file_data).await?;
//! if !result.is_clean {
//!     return Err(ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "virus_detected", result.threat_name.unwrap()));
//! }
//! ```

use std::time::Instant;

use async_trait::async_trait;
use thiserror::Error;
use tracing::{debug, info, instrument, warn};

use crate::config::ClamAvConfig;

/// Errors that can occur during virus scanning.
#[derive(Debug, Error)]
pub enum VirusScanError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Scan failed: {0}")]
    ScanFailed(String),

    #[error("Scan timed out after {0}ms")]
    Timeout(u64),

    #[error("File too large for scanning: {size} bytes exceeds maximum {max} bytes")]
    FileTooLarge { size: usize, max: usize },

    #[error("Scanner not available: {0}")]
    NotAvailable(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type VirusScanResult<T> = Result<T, VirusScanError>;

/// Result of a virus scan operation.
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// True if the file is clean (no threats detected).
    pub is_clean: bool,

    /// Name of the detected threat, if any.
    pub threat_name: Option<String>,

    /// Time taken to scan in milliseconds.
    pub scan_duration_ms: u64,

    /// Size of the scanned file in bytes.
    pub file_size: usize,
}

impl ScanResult {
    /// Create a clean scan result.
    pub fn clean(file_size: usize, scan_duration_ms: u64) -> Self {
        Self {
            is_clean: true,
            threat_name: None,
            scan_duration_ms,
            file_size,
        }
    }

    /// Create a result indicating a threat was found.
    pub fn threat(threat_name: String, file_size: usize, scan_duration_ms: u64) -> Self {
        Self {
            is_clean: false,
            threat_name: Some(threat_name),
            scan_duration_ms,
            file_size,
        }
    }
}

/// Trait for virus scanning backends.
///
/// Implementations must be `Send + Sync` to support async contexts.
#[async_trait]
pub trait VirusScanner: Send + Sync {
    /// Scan file content for viruses/malware.
    ///
    /// Returns `Ok(ScanResult)` with the scan outcome.
    /// Returns `Err` if the scan could not be performed (connection issues, timeout, etc.).
    async fn scan(&self, content: &[u8]) -> VirusScanResult<ScanResult>;

    /// Check if the scanner is available and responding.
    async fn ping(&self) -> VirusScanResult<()>;

    /// Get the scanner backend name (for logging).
    fn backend_name(&self) -> &'static str;
}

/// ClamAV virus scanner using the clamd daemon.
///
/// Connects to clamd via TCP or Unix socket and uses the INSTREAM
/// command to scan file content in memory.
pub struct ClamAvScanner {
    config: ClamAvConfig,
}

impl ClamAvScanner {
    /// Create a new ClamAV scanner with the given configuration.
    pub fn new(config: ClamAvConfig) -> VirusScanResult<Self> {
        config.validate().map_err(VirusScanError::Config)?;

        Ok(Self { config })
    }

    /// Parse ClamAV response to extract scan result.
    fn parse_response(response: &[u8], file_size: usize, scan_duration_ms: u64) -> ScanResult {
        let response_str = String::from_utf8_lossy(response);

        // ClamAV responses:
        // - Clean: "stream: OK"
        // - Infected: "stream: Eicar-Test-Signature FOUND"
        // - Error: "stream: ... ERROR"
        if response_str.contains("OK") {
            ScanResult::clean(file_size, scan_duration_ms)
        } else if response_str.contains("FOUND") {
            // Extract threat name from: "stream: <threat_name> FOUND\0"
            // ClamAV responses have a trailing null byte, so we use find() instead
            // of strip_prefix/strip_suffix which would fail on the null terminator
            let threat_name = if let Some(start) = response_str.find("stream: ") {
                let after_stream = &response_str[start + 8..]; // Skip "stream: "
                if let Some(end) = after_stream.find(" FOUND") {
                    after_stream[..end].to_string()
                } else {
                    "Unknown threat".to_string()
                }
            } else {
                "Unknown threat".to_string()
            };
            ScanResult::threat(threat_name, file_size, scan_duration_ms)
        } else {
            // Treat unexpected responses as errors during parsing
            // This shouldn't happen normally - return as threat to be safe
            warn!(response = %response_str, "Unexpected ClamAV response");
            ScanResult::threat(
                format!("Scan error: {}", response_str.trim()),
                file_size,
                scan_duration_ms,
            )
        }
    }
}

#[async_trait]
impl VirusScanner for ClamAvScanner {
    #[instrument(skip(self, content), fields(size = content.len()))]
    async fn scan(&self, content: &[u8]) -> VirusScanResult<ScanResult> {
        let file_size = content.len();

        // Check file size limit
        let max_size = self.config.max_file_size_bytes() as usize;
        if file_size > max_size {
            return Err(VirusScanError::FileTooLarge {
                size: file_size,
                max: max_size,
            });
        }

        let start = Instant::now();

        // Use socket if configured, otherwise TCP
        let response = if let Some(ref socket_path) = self.config.socket_path {
            debug!(socket_path, "Scanning via Unix socket");
            let socket = clamav_client::tokio::Socket {
                socket_path: socket_path.as_str(),
            };
            let timeout_ms = self.config.timeout_ms as usize;
            clamav_client::tokio::scan_buffer(content, socket, Some(timeout_ms))
                .await
                .map_err(|e| VirusScanError::ScanFailed(e.to_string()))?
        } else {
            let address = self.config.tcp_address();
            debug!(address = %address, "Scanning via TCP");
            let tcp = clamav_client::tokio::Tcp {
                host_address: address.as_str(),
            };
            let timeout_ms = self.config.timeout_ms as usize;
            clamav_client::tokio::scan_buffer(content, tcp, Some(timeout_ms))
                .await
                .map_err(|e| VirusScanError::ScanFailed(e.to_string()))?
        };

        let scan_duration_ms = start.elapsed().as_millis() as u64;
        let result = Self::parse_response(&response, file_size, scan_duration_ms);

        if result.is_clean {
            info!(file_size, scan_duration_ms, "File scan completed: clean");
        } else {
            warn!(
                file_size,
                scan_duration_ms,
                threat_name = ?result.threat_name,
                "File scan completed: threat detected"
            );
        }

        Ok(result)
    }

    #[instrument(skip(self))]
    async fn ping(&self) -> VirusScanResult<()> {
        let result = if let Some(ref socket_path) = self.config.socket_path {
            let socket = clamav_client::tokio::Socket {
                socket_path: socket_path.as_str(),
            };
            clamav_client::tokio::ping(socket).await
        } else {
            let address = self.config.tcp_address();
            let tcp = clamav_client::tokio::Tcp {
                host_address: address.as_str(),
            };
            clamav_client::tokio::ping(tcp).await
        };

        result.map_err(|e| VirusScanError::ConnectionFailed(e.to_string()))?;

        debug!("ClamAV ping successful");
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "clamav"
    }
}

/// A no-op scanner that always returns clean.
/// Used when virus scanning is disabled.
pub struct NoOpScanner;

#[async_trait]
impl VirusScanner for NoOpScanner {
    async fn scan(&self, content: &[u8]) -> VirusScanResult<ScanResult> {
        Ok(ScanResult::clean(content.len(), 0))
    }

    async fn ping(&self) -> VirusScanResult<()> {
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "noop"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_response_clean() {
        let response = b"stream: OK";
        let result = ClamAvScanner::parse_response(response, 1024, 50);

        assert!(result.is_clean);
        assert!(result.threat_name.is_none());
        assert_eq!(result.file_size, 1024);
        assert_eq!(result.scan_duration_ms, 50);
    }

    #[test]
    fn test_parse_response_threat() {
        let response = b"stream: Eicar-Test-Signature FOUND";
        let result = ClamAvScanner::parse_response(response, 68, 100);

        assert!(!result.is_clean);
        assert_eq!(result.threat_name, Some("Eicar-Test-Signature".to_string()));
        assert_eq!(result.file_size, 68);
        assert_eq!(result.scan_duration_ms, 100);
    }

    #[test]
    fn test_parse_response_complex_threat() {
        let response = b"stream: Win.Trojan.Agent-123456 FOUND";
        let result = ClamAvScanner::parse_response(response, 50000, 200);

        assert!(!result.is_clean);
        assert_eq!(
            result.threat_name,
            Some("Win.Trojan.Agent-123456".to_string())
        );
    }

    #[test]
    fn test_scan_result_clean() {
        let result = ScanResult::clean(1000, 25);

        assert!(result.is_clean);
        assert!(result.threat_name.is_none());
        assert_eq!(result.file_size, 1000);
        assert_eq!(result.scan_duration_ms, 25);
    }

    #[test]
    fn test_scan_result_threat() {
        let result = ScanResult::threat("TestVirus".to_string(), 500, 10);

        assert!(!result.is_clean);
        assert_eq!(result.threat_name, Some("TestVirus".to_string()));
        assert_eq!(result.file_size, 500);
        assert_eq!(result.scan_duration_ms, 10);
    }

    #[test]
    fn test_clamav_config_tcp_address() {
        let config = ClamAvConfig {
            host: "scanner.local".to_string(),
            port: 3311,
            ..Default::default()
        };

        assert_eq!(config.tcp_address(), "scanner.local:3311");
    }

    #[test]
    fn test_clamav_config_max_size_bytes() {
        let config = ClamAvConfig {
            max_file_size_mb: 50,
            ..Default::default()
        };

        assert_eq!(config.max_file_size_bytes(), 50 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_noop_scanner_always_clean() {
        let scanner = NoOpScanner;
        let data = b"some test data";

        let result = scanner.scan(data).await.unwrap();

        assert!(result.is_clean);
        assert!(result.threat_name.is_none());
        assert_eq!(result.file_size, data.len());
        assert_eq!(result.scan_duration_ms, 0);
    }

    #[tokio::test]
    async fn test_noop_scanner_ping() {
        let scanner = NoOpScanner;
        assert!(scanner.ping().await.is_ok());
    }

    #[test]
    fn test_noop_scanner_backend_name() {
        let scanner = NoOpScanner;
        assert_eq!(scanner.backend_name(), "noop");
    }
}

/// Integration tests using testcontainers with a real ClamAV instance.
///
/// These tests are marked #[ignore] by default because:
/// - They require Docker to be running
/// - ClamAV container takes 30-60 seconds to start (loading virus database)
/// - They're slow compared to unit tests
///
/// Run with: `cargo test clamav_integration --ignored`
#[cfg(test)]
mod clamav_integration_tests {
    use std::sync::OnceLock;

    use testcontainers_modules::testcontainers::{
        ContainerAsync, GenericImage,
        core::{ContainerPort, WaitFor},
        runners::AsyncRunner,
    };
    use tokio::sync::OnceCell;

    use super::*;

    /// EICAR test file - standard AV test pattern that all antivirus software should detect.
    /// This is NOT a real virus, just a test signature agreed upon by the AV industry.
    /// See: https://www.eicar.org/download-anti-malware-testfile/
    const EICAR_TEST_STRING: &[u8] =
        b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*";

    /// Shared ClamAV container state - initialized once per test run
    struct SharedClamAvContainer {
        #[allow(dead_code)] // Test infrastructure: keeps container alive
        container: ContainerAsync<GenericImage>,
        host: String,
        port: u16,
    }

    /// Global shared container - lazily initialized on first use
    static SHARED_CONTAINER: OnceLock<OnceCell<SharedClamAvContainer>> = OnceLock::new();

    /// Get or initialize the shared ClamAV container
    async fn get_shared_container() -> &'static SharedClamAvContainer {
        let cell = SHARED_CONTAINER.get_or_init(OnceCell::new);
        cell.get_or_init(|| async {
            // ClamAV container - takes a while to start due to loading virus databases
            let container = GenericImage::new("clamav/clamav", "stable")
                .with_exposed_port(ContainerPort::Tcp(3310))
                // ClamAV logs "socket found, clamd started." when ready
                .with_wait_for(WaitFor::message_on_stdout("socket found, clamd started."))
                .start()
                .await
                .expect("Failed to start ClamAV container");

            let host = container
                .get_host()
                .await
                .expect("Failed to get ClamAV host")
                .to_string();
            let port = container
                .get_host_port_ipv4(3310)
                .await
                .expect("Failed to get ClamAV port");

            SharedClamAvContainer {
                container,
                host,
                port,
            }
        })
        .await
    }

    /// Create a scanner connected to the testcontainers ClamAV instance
    async fn create_test_scanner() -> ClamAvScanner {
        let shared = get_shared_container().await;
        let config = ClamAvConfig {
            host: shared.host.clone(),
            port: shared.port,
            timeout_ms: 60000, // 60 second timeout for slow scans
            max_file_size_mb: 25,
            socket_path: None,
        };
        ClamAvScanner::new(config).expect("Failed to create scanner")
    }

    #[tokio::test]
    #[ignore = "Requires Docker and takes ~60s to start ClamAV container"]
    async fn test_clamav_ping() {
        let scanner = create_test_scanner().await;

        let result = scanner.ping().await;
        assert!(result.is_ok(), "ClamAV ping failed: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore = "Requires Docker and takes ~60s to start ClamAV container"]
    async fn test_clamav_scan_clean_file() {
        let scanner = create_test_scanner().await;

        // Regular text content - should be clean
        let clean_data = b"Hello, this is a perfectly normal text file with no malware.";

        let result = scanner.scan(clean_data).await;
        assert!(result.is_ok(), "Scan failed: {:?}", result.err());

        let scan_result = result.unwrap();
        assert!(
            scan_result.is_clean,
            "Clean file should not be detected as threat: {:?}",
            scan_result.threat_name
        );
        assert!(scan_result.threat_name.is_none());
        assert_eq!(scan_result.file_size, clean_data.len());
        assert!(scan_result.scan_duration_ms > 0);
    }

    #[tokio::test]
    #[ignore = "Requires Docker and takes ~60s to start ClamAV container"]
    async fn test_clamav_scan_eicar_test_file() {
        let scanner = create_test_scanner().await;

        let result = scanner.scan(EICAR_TEST_STRING).await;
        assert!(result.is_ok(), "Scan failed: {:?}", result.err());

        let scan_result = result.unwrap();
        assert!(
            !scan_result.is_clean,
            "EICAR test file should be detected as threat"
        );
        assert!(
            scan_result.threat_name.is_some(),
            "Threat name should be present"
        );

        // ClamAV detects EICAR as "Win.Test.EICAR_HDB-1" or similar
        let threat_name = scan_result.threat_name.unwrap();
        assert!(
            threat_name.contains("Eicar") || threat_name.contains("EICAR"),
            "Expected EICAR detection, got: {}",
            threat_name
        );

        assert_eq!(scan_result.file_size, EICAR_TEST_STRING.len());
    }

    #[tokio::test]
    #[ignore = "Requires Docker and takes ~60s to start ClamAV container"]
    async fn test_clamav_scan_binary_data() {
        let scanner = create_test_scanner().await;

        // Random binary data - should be clean
        let binary_data: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();

        let result = scanner.scan(&binary_data).await;
        assert!(result.is_ok(), "Scan failed: {:?}", result.err());

        let scan_result = result.unwrap();
        assert!(
            scan_result.is_clean,
            "Random binary data should be clean: {:?}",
            scan_result.threat_name
        );
    }

    #[tokio::test]
    #[ignore = "Requires Docker and takes ~60s to start ClamAV container"]
    async fn test_clamav_scan_empty_file() {
        let scanner = create_test_scanner().await;

        let result = scanner.scan(b"").await;
        assert!(result.is_ok(), "Scan failed: {:?}", result.err());

        let scan_result = result.unwrap();
        assert!(scan_result.is_clean, "Empty file should be clean");
        assert_eq!(scan_result.file_size, 0);
    }

    #[tokio::test]
    async fn test_clamav_connection_failure() {
        // Connect to a port that's definitely not running ClamAV
        let config = ClamAvConfig {
            host: "127.0.0.1".to_string(),
            port: 1, // Privileged port, won't have ClamAV
            timeout_ms: 1000,
            max_file_size_mb: 25,
            socket_path: None,
        };
        let scanner = ClamAvScanner::new(config).expect("Failed to create scanner");

        let result = scanner.ping().await;
        assert!(
            result.is_err(),
            "Should fail to connect to non-existent server"
        );

        match result {
            Err(VirusScanError::ConnectionFailed(_)) => (),
            Err(e) => panic!("Expected ConnectionFailed, got: {:?}", e),
            Ok(_) => panic!("Expected error, got success"),
        }
    }

    #[tokio::test]
    async fn test_clamav_file_too_large() {
        let config = ClamAvConfig {
            host: "localhost".to_string(),
            port: 3310,
            timeout_ms: 1000,
            max_file_size_mb: 1, // 1 MB limit
            socket_path: None,
        };
        let scanner = ClamAvScanner::new(config).expect("Failed to create scanner");

        // Create data larger than the limit (2 MB)
        let large_data = vec![0u8; 2 * 1024 * 1024];

        let result = scanner.scan(&large_data).await;
        assert!(result.is_err(), "Should reject file larger than limit");

        match result {
            Err(VirusScanError::FileTooLarge { size, max }) => {
                assert_eq!(size, 2 * 1024 * 1024);
                assert_eq!(max, 1024 * 1024);
            }
            Err(e) => panic!("Expected FileTooLarge, got: {:?}", e),
            Ok(_) => panic!("Expected error, got success"),
        }
    }
}
