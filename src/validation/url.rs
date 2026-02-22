//! URL validation for SSRF protection.
//!
//! Validates user-supplied URLs before the server makes outbound HTTP requests to them.
//! Blocks private/internal IP ranges, non-HTTP schemes, and cloud metadata endpoints.

use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};

/// Errors from URL validation.
#[derive(Debug, thiserror::Error)]
pub enum UrlValidationError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("URL scheme must be http or https")]
    InvalidScheme,

    #[error("URL must include a hostname")]
    MissingHost,

    #[error("URL resolves to a blocked address")]
    BlockedAddress,
}

/// Check if an IP address is in a private/reserved range that should not be accessed
/// by server-side requests.
fn is_blocked_ip(ip: IpAddr, allow_loopback: bool) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            // Loopback (127.0.0.0/8)
            if v4.is_loopback() {
                return !allow_loopback;
            }
            // Private ranges
            if v4.is_private() {
                return true;
            }
            // Link-local (169.254.0.0/16)
            if v4.is_link_local() {
                return true;
            }
            // Cloud metadata endpoint (169.254.169.254)
            if v4 == Ipv4Addr::new(169, 254, 169, 254) {
                return true; // Always block, even if allow_loopback
            }
            // Broadcast
            if v4.is_broadcast() {
                return true;
            }
            // Documentation ranges (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24)
            let octets = v4.octets();
            if (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
            {
                return true;
            }
            // Unspecified (0.0.0.0)
            if v4.is_unspecified() {
                return true;
            }
            false
        }
        IpAddr::V6(v6) => {
            // Loopback (::1)
            if v6.is_loopback() {
                return !allow_loopback;
            }
            // Unspecified (::)
            if v6.is_unspecified() {
                return true;
            }
            // Link-local (fe80::/10)
            let segments = v6.segments();
            if segments[0] & 0xffc0 == 0xfe80 {
                return true;
            }
            // Unique local (fc00::/7)
            if segments[0] & 0xfe00 == 0xfc00 {
                return true;
            }
            // IPv4-mapped addresses (::ffff:x.x.x.x) — check the embedded IPv4
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_blocked_ip(IpAddr::V4(v4), allow_loopback);
            }
            false
        }
    }
}

/// Validate a user-supplied URL before making server-side HTTP requests.
///
/// Rejects:
/// - Non-http(s) schemes
/// - URLs without a hostname
/// - Hostnames that resolve to private, loopback, or link-local IPs
/// - Cloud metadata endpoints (169.254.169.254)
///
/// When `allow_loopback` is true, loopback addresses (127.0.0.1, ::1) are permitted
/// (useful for development). Private ranges and metadata endpoints are always blocked.
pub fn validate_base_url(url: &str, allow_loopback: bool) -> Result<(), UrlValidationError> {
    let parsed = url::Url::parse(url).map_err(|e| UrlValidationError::InvalidUrl(e.to_string()))?;

    // Scheme check
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err(UrlValidationError::InvalidScheme),
    }

    // Host check
    let host = parsed.host_str().ok_or(UrlValidationError::MissingHost)?;

    // Check for localhost string variants
    if !allow_loopback
        && (host.eq_ignore_ascii_case("localhost")
            || host.eq_ignore_ascii_case("localhost.localdomain"))
    {
        return Err(UrlValidationError::BlockedAddress);
    }

    // Try to parse as IP directly first
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_blocked_ip(ip, allow_loopback) {
            return Err(UrlValidationError::BlockedAddress);
        }
        return Ok(());
    }

    // Resolve hostname to IP addresses and check each one
    let port = parsed.port().unwrap_or(match parsed.scheme() {
        "https" => 443,
        _ => 80,
    });

    let socket_addrs: Vec<_> = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(|e| UrlValidationError::InvalidUrl(format!("DNS resolution failed: {e}")))?
        .collect();

    if socket_addrs.is_empty() {
        return Err(UrlValidationError::InvalidUrl(
            "Hostname did not resolve to any addresses".to_string(),
        ));
    }

    // ALL resolved addresses must be non-blocked (prevents DNS rebinding with mixed results)
    for addr in &socket_addrs {
        if is_blocked_ip(addr.ip(), allow_loopback) {
            return Err(UrlValidationError::BlockedAddress);
        }
    }

    Ok(())
}

/// Validate that a URL uses HTTPS scheme.
#[cfg(feature = "saml")]
pub fn require_https(url: &str) -> Result<(), UrlValidationError> {
    let parsed = url::Url::parse(url).map_err(|e| UrlValidationError::InvalidUrl(e.to_string()))?;
    if parsed.scheme() != "https" {
        return Err(UrlValidationError::InvalidScheme);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_https_url() {
        assert!(validate_base_url("https://api.openai.com/v1", false).is_ok());
    }

    #[test]
    fn test_valid_http_url() {
        // Use a direct IP to avoid DNS resolution in test environments
        assert!(validate_base_url("http://93.184.215.14", false).is_ok());
    }

    #[test]
    fn test_rejects_ftp_scheme() {
        assert!(matches!(
            validate_base_url("ftp://example.com", false),
            Err(UrlValidationError::InvalidScheme)
        ));
    }

    #[test]
    fn test_rejects_file_scheme() {
        assert!(matches!(
            validate_base_url("file:///etc/passwd", false),
            Err(UrlValidationError::InvalidScheme)
        ));
    }

    #[test]
    fn test_rejects_private_10() {
        assert!(matches!(
            validate_base_url("http://10.0.0.1", false),
            Err(UrlValidationError::BlockedAddress)
        ));
    }

    #[test]
    fn test_rejects_private_172() {
        assert!(matches!(
            validate_base_url("http://172.16.0.1", false),
            Err(UrlValidationError::BlockedAddress)
        ));
    }

    #[test]
    fn test_rejects_private_192() {
        assert!(matches!(
            validate_base_url("http://192.168.1.1", false),
            Err(UrlValidationError::BlockedAddress)
        ));
    }

    #[test]
    fn test_rejects_loopback() {
        assert!(matches!(
            validate_base_url("http://127.0.0.1", false),
            Err(UrlValidationError::BlockedAddress)
        ));
    }

    #[test]
    fn test_rejects_localhost() {
        assert!(matches!(
            validate_base_url("http://localhost", false),
            Err(UrlValidationError::BlockedAddress)
        ));
    }

    #[test]
    fn test_allows_loopback_when_flag_set() {
        assert!(validate_base_url("http://127.0.0.1:8080", true).is_ok());
    }

    #[test]
    fn test_allows_localhost_when_flag_set() {
        assert!(validate_base_url("http://localhost:8080", true).is_ok());
    }

    #[test]
    fn test_rejects_metadata_endpoint() {
        assert!(matches!(
            validate_base_url("http://169.254.169.254/latest/meta-data/", false),
            Err(UrlValidationError::BlockedAddress)
        ));
    }

    #[test]
    fn test_rejects_metadata_even_with_loopback_allowed() {
        // Cloud metadata should always be blocked
        assert!(matches!(
            validate_base_url("http://169.254.169.254", true),
            Err(UrlValidationError::BlockedAddress)
        ));
    }

    #[test]
    fn test_rejects_link_local() {
        assert!(matches!(
            validate_base_url("http://169.254.1.1", false),
            Err(UrlValidationError::BlockedAddress)
        ));
    }

    #[test]
    fn test_rejects_ipv6_loopback() {
        assert!(matches!(
            validate_base_url("http://[::1]", false),
            Err(UrlValidationError::BlockedAddress)
        ));
    }

    #[test]
    fn test_allows_ipv6_loopback_when_flag_set() {
        assert!(validate_base_url("http://[::1]:8080", true).is_ok());
    }

    #[test]
    fn test_rejects_unspecified() {
        assert!(matches!(
            validate_base_url("http://0.0.0.0", false),
            Err(UrlValidationError::BlockedAddress)
        ));
    }

    #[test]
    fn test_rejects_invalid_url() {
        assert!(matches!(
            validate_base_url("not a url", false),
            Err(UrlValidationError::InvalidUrl(_))
        ));
    }

    #[cfg(feature = "saml")]
    #[test]
    fn test_require_https_rejects_http() {
        assert!(matches!(
            require_https("http://example.com"),
            Err(UrlValidationError::InvalidScheme)
        ));
    }

    #[cfg(feature = "saml")]
    #[test]
    fn test_require_https_allows_https() {
        assert!(require_https("https://example.com").is_ok());
    }
}
