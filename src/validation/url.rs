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

/// Options controlling which IP ranges are permitted in SSRF validation.
#[derive(Debug, Clone, Copy, Default)]
pub struct UrlValidationOptions {
    /// Allow loopback addresses (127.0.0.0/8, ::1).
    pub allow_loopback: bool,
    /// Allow private/internal IP ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16).
    pub allow_private: bool,
}

/// Check if an IP address is in a private/reserved range that should not be accessed
/// by server-side requests.
fn is_blocked_ip(ip: IpAddr, opts: UrlValidationOptions) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            // Loopback (127.0.0.0/8)
            if v4.is_loopback() {
                return !opts.allow_loopback;
            }
            // Cloud metadata endpoint (169.254.169.254) — always blocked
            if v4 == Ipv4Addr::new(169, 254, 169, 254) {
                return true;
            }
            // Link-local (169.254.0.0/16) — blocked unless allow_private
            // (cloud metadata 169.254.169.254 is always blocked above)
            if v4.is_link_local() {
                return !opts.allow_private;
            }
            // Private ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
            if v4.is_private() {
                return !opts.allow_private;
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
                return !opts.allow_loopback;
            }
            // Unspecified (::)
            if v6.is_unspecified() {
                return true;
            }
            // Link-local (fe80::/10) — blocked unless allow_private
            let segments = v6.segments();
            if segments[0] & 0xffc0 == 0xfe80 {
                return !opts.allow_private;
            }
            // Unique local (fc00::/7)
            if segments[0] & 0xfe00 == 0xfc00 {
                return !opts.allow_private;
            }
            // IPv4-mapped addresses (::ffff:x.x.x.x) — check the embedded IPv4
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_blocked_ip(IpAddr::V4(v4), opts);
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
/// Use `allow_loopback = true` for development (permits 127.0.0.1/::1).
/// Use `allow_private = true` for Docker/Kubernetes (permits 10.x/172.16.x/192.168.x).
pub fn validate_base_url(url: &str, allow_loopback: bool) -> Result<(), UrlValidationError> {
    validate_base_url_opts(
        url,
        UrlValidationOptions {
            allow_loopback,
            allow_private: false,
        },
    )
}

/// Validate a user-supplied URL with full options control.
pub fn validate_base_url_opts(
    url: &str,
    opts: UrlValidationOptions,
) -> Result<(), UrlValidationError> {
    let parsed = url::Url::parse(url).map_err(|e| UrlValidationError::InvalidUrl(e.to_string()))?;

    // Scheme check
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err(UrlValidationError::InvalidScheme),
    }

    // Host check
    let host = parsed.host_str().ok_or(UrlValidationError::MissingHost)?;

    // Check for localhost string variants
    if !opts.allow_loopback
        && (host.eq_ignore_ascii_case("localhost")
            || host.eq_ignore_ascii_case("localhost.localdomain"))
    {
        return Err(UrlValidationError::BlockedAddress);
    }

    // Try to parse as IP directly first
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_blocked_ip(ip, opts) {
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
        if is_blocked_ip(addr.ip(), opts) {
            tracing::warn!(
                url = %url,
                blocked_ip = %addr.ip(),
                all_resolved = ?socket_addrs.iter().map(|a| a.ip()).collect::<Vec<_>>(),
                allow_loopback = opts.allow_loopback,
                allow_private = opts.allow_private,
                "URL blocked by SSRF validation"
            );
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
    fn test_rejects_metadata_even_with_private_allowed() {
        // Cloud metadata should always be blocked, even with allow_private
        let opts = UrlValidationOptions {
            allow_loopback: true,
            allow_private: true,
        };
        assert!(matches!(
            validate_base_url_opts("http://169.254.169.254", opts),
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
    fn test_allows_link_local_with_private() {
        // Link-local (non-metadata) allowed when allow_private is set (Docker/k8s)
        let opts = UrlValidationOptions {
            allow_loopback: false,
            allow_private: true,
        };
        assert!(validate_base_url_opts("http://169.254.1.1", opts).is_ok());
    }

    #[test]
    fn test_allows_private_with_flag() {
        let opts = UrlValidationOptions {
            allow_loopback: false,
            allow_private: true,
        };
        assert!(validate_base_url_opts("http://10.0.0.1", opts).is_ok());
        assert!(validate_base_url_opts("http://172.16.0.1", opts).is_ok());
        assert!(validate_base_url_opts("http://192.168.1.1", opts).is_ok());
    }

    #[test]
    fn test_allows_ipv6_link_local_with_private() {
        let opts = UrlValidationOptions {
            allow_loopback: false,
            allow_private: true,
        };
        assert!(validate_base_url_opts("http://[fe80::1]:8080", opts).is_ok());
    }

    #[test]
    fn test_rejects_ipv6_link_local_without_private() {
        let opts = UrlValidationOptions {
            allow_loopback: true,
            allow_private: false,
        };
        assert!(matches!(
            validate_base_url_opts("http://[fe80::1]:8080", opts),
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
