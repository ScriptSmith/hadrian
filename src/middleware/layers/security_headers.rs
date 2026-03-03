//! Security headers middleware.
//!
//! Adds standard security headers to all responses to protect against
//! common web vulnerabilities like clickjacking, MIME-sniffing, and
//! protocol downgrade attacks.

use axum::{
    body::Body,
    extract::State,
    http::{Request, header::HeaderValue},
    middleware::Next,
    response::Response,
};

use crate::AppState;

/// Middleware that adds security headers to all responses.
pub async fn security_headers_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let config = &state.config.server.security_headers;

    if !config.enabled {
        return next.run(request).await;
    }

    // Check if this is a secure connection (for HSTS)
    let is_secure = is_secure_connection(&request);

    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // X-Content-Type-Options: nosniff
    if let Some(value) = try_header_value(&config.content_type_options) {
        headers.insert("x-content-type-options", value);
    }

    // X-Frame-Options: DENY or SAMEORIGIN
    if let Some(value) = config.frame_options.as_deref().and_then(try_header_value) {
        headers.insert("x-frame-options", value);
    }

    // Strict-Transport-Security (only on HTTPS connections)
    if config.hsts.enabled && is_secure {
        let hsts_value = build_hsts_header(&config.hsts);
        if let Some(value) = try_header_value(&hsts_value) {
            headers.insert("strict-transport-security", value);
        }
    }

    // Content-Security-Policy
    if let Some(value) = config
        .content_security_policy
        .as_deref()
        .and_then(try_header_value)
    {
        headers.insert("content-security-policy", value);
    }

    // X-XSS-Protection (legacy, but still useful for older browsers)
    if let Some(value) = config.xss_protection.as_deref().and_then(try_header_value) {
        headers.insert("x-xss-protection", value);
    }

    // Referrer-Policy
    if let Some(value) = config.referrer_policy.as_deref().and_then(try_header_value) {
        headers.insert("referrer-policy", value);
    }

    // Permissions-Policy
    if let Some(value) = config
        .permissions_policy
        .as_deref()
        .and_then(try_header_value)
    {
        headers.insert("permissions-policy", value);
    }

    response
}

/// Try to convert a string to a header value, returning None if empty or invalid.
fn try_header_value(s: &str) -> Option<HeaderValue> {
    if s.is_empty() {
        return None;
    }
    HeaderValue::try_from(s).ok()
}

/// Build the Strict-Transport-Security header value.
fn build_hsts_header(config: &crate::config::HstsConfig) -> String {
    let mut parts = vec![format!("max-age={}", config.max_age_secs)];

    if config.include_subdomains {
        parts.push("includeSubDomains".to_string());
    }

    if config.preload {
        parts.push("preload".to_string());
    }

    parts.join("; ")
}

/// Check if the request came over a secure connection.
///
/// This checks for:
/// - X-Forwarded-Proto: https (from reverse proxy)
/// - The request URI scheme (if available)
fn is_secure_connection<B>(request: &Request<B>) -> bool {
    // Check X-Forwarded-Proto header (set by reverse proxies)
    let forwarded_https = request
        .headers()
        .get("x-forwarded-proto")
        .is_some_and(|proto| proto.as_bytes().eq_ignore_ascii_case(b"https"));

    // Check the URI scheme directly
    let scheme_https = request
        .uri()
        .scheme_str()
        .is_some_and(|s| s.eq_ignore_ascii_case("https"));

    forwarded_https || scheme_https
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HstsConfig;

    #[test]
    fn test_build_hsts_header_basic() {
        let config = HstsConfig {
            enabled: true,
            max_age_secs: 31536000,
            include_subdomains: false,
            preload: false,
        };

        assert_eq!(build_hsts_header(&config), "max-age=31536000");
    }

    #[test]
    fn test_build_hsts_header_with_subdomains() {
        let config = HstsConfig {
            enabled: true,
            max_age_secs: 31536000,
            include_subdomains: true,
            preload: false,
        };

        assert_eq!(
            build_hsts_header(&config),
            "max-age=31536000; includeSubDomains"
        );
    }

    #[test]
    fn test_build_hsts_header_full() {
        let config = HstsConfig {
            enabled: true,
            max_age_secs: 63072000,
            include_subdomains: true,
            preload: true,
        };

        assert_eq!(
            build_hsts_header(&config),
            "max-age=63072000; includeSubDomains; preload"
        );
    }
}
