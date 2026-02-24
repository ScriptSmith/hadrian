use std::{net::IpAddr, sync::Arc, time::Duration};

use axum::{
    Json,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use ipnet::IpNet;

use crate::{
    AppState,
    auth::AuthenticatedRequest,
    cache::{Cache, CacheKeys, RateLimitResult},
    config::TrustedProxiesConfig,
    observability::metrics,
    openapi::ErrorResponse,
};

#[derive(Debug)]
pub enum RateLimitError {
    Exceeded {
        limit: u32,
        current: i64,
        window: String,
        retry_after: u64,
    },
    Internal(String),
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        let (status, code, message, error_type, rate_limit_info) = match self {
            RateLimitError::Exceeded {
                limit,
                current,
                window,
                retry_after,
            } => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limit_exceeded",
                format!("Rate limit exceeded: {} requests per {}", limit, window),
                "rate_limit_error",
                Some((limit, current, retry_after)),
            ),
            RateLimitError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                msg,
                "server_error",
                None,
            ),
        };

        // Record rate limit error metric
        metrics::record_gateway_error("rate_limited", code, None);

        let body = ErrorResponse::with_type(error_type, code, message);

        let mut response = (status, Json(body)).into_response();

        // Add standard rate limit headers for rate limit exceeded errors
        if let Some((limit, current, retry_after)) = rate_limit_info {
            let remaining = (limit as i64).saturating_sub(current).max(0) as u32;

            if let Ok(v) = HeaderValue::try_from(limit.to_string()) {
                response.headers_mut().insert("X-RateLimit-Limit", v);
            }
            if let Ok(v) = HeaderValue::try_from(remaining.to_string()) {
                response.headers_mut().insert("X-RateLimit-Remaining", v);
            }
            if let Ok(v) = HeaderValue::try_from(retry_after.to_string()) {
                response
                    .headers_mut()
                    .insert("X-RateLimit-Reset", v.clone());
                response.headers_mut().insert("Retry-After", v);
            }
        }

        response
    }
}

/// Rate limiting middleware for unauthenticated (IP-based) requests.
///
/// Authenticated requests are rate-limited in `api_middleware` using the batched
/// `check_all_limits_batch()` function for better Redis performance.
/// This middleware only handles IP-based rate limiting for unauthenticated requests.
#[allow(clippy::collapsible_if, clippy::question_mark)]
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Extract auth from request extensions if present
    let auth = req.extensions().get::<AuthenticatedRequest>().cloned();

    // Get cache (skip all rate limiting if not configured)
    let cache = match &state.cache {
        Some(c) => c,
        None => return Ok(next.run(req).await),
    };

    // Authenticated requests are rate-limited in api_middleware via check_all_limits_batch()
    // This middleware only handles IP-based rate limiting for unauthenticated requests
    if auth.is_some() {
        return Ok(next.run(req).await);
    }

    // Unauthenticated request - rate limit by IP if enabled
    let ip_config = &state.config.limits.rate_limits.ip_rate_limits;
    if !ip_config.enabled {
        return Ok(next.run(req).await);
    }

    // Extract client IP from request
    let client_ip = extract_client_ip(&req, &state.config.server.trusted_proxies);
    let client_ip_str = client_ip
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Check per-minute IP limit
    let minute_result = check_ip_rate_limit(
        cache,
        &client_ip_str,
        "minute",
        ip_config.requests_per_minute,
        Duration::from_secs(60),
    )
    .await?;

    // Check per-hour IP limit if configured
    if let Some(rph) = ip_config.requests_per_hour {
        check_ip_rate_limit(
            cache,
            &client_ip_str,
            "hour",
            rph,
            Duration::from_secs(3600),
        )
        .await?;
    }

    // Record successful rate limit check (no API key for IP-based limiting)
    metrics::record_rate_limit("allowed", None);

    // Add rate limit headers to response
    let response = next.run(req).await;
    Ok(add_rate_limit_headers(response, &minute_result))
}

/// Extract the client IP address from the request.
///
/// This respects the trusted proxy configuration to correctly extract
/// the real client IP when behind reverse proxies/load balancers.
///
/// **Security:** Proxy headers are only trusted when:
/// 1. `dangerously_trust_all` is explicitly enabled (use only in isolated environments), OR
/// 2. The connecting IP is within one of the configured trusted CIDR ranges
///
/// When proxy headers are trusted, X-Forwarded-For is parsed right-to-left,
/// skipping IPs within trusted CIDRs, to find the first untrusted (client) IP.
/// This prevents proxy spoofing attacks where an attacker adds fake IPs to
/// the beginning of X-Forwarded-For.
#[allow(clippy::collapsible_if)]
pub fn extract_client_ip(req: &Request, trusted_proxies: &TrustedProxiesConfig) -> Option<IpAddr> {
    // Get the direct connecting IP (from TCP connection)
    let connecting_ip = req
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip());

    // If no proxy trust is configured, just return the connecting IP
    if !trusted_proxies.is_configured() {
        return connecting_ip;
    }

    // Parse CIDRs once for this request
    let parsed_cidrs = trusted_proxies.parsed_cidrs();

    // Check if we should trust proxy headers from this connecting IP
    let should_trust_headers = match connecting_ip {
        Some(ip) => trusted_proxies.is_trusted_ip(ip, &parsed_cidrs),
        None => {
            // No connecting IP available - only trust if dangerously_trust_all is set
            // (this can happen in certain test scenarios)
            trusted_proxies.dangerously_trust_all
        }
    };

    if !should_trust_headers {
        // Connecting IP is not from a trusted proxy - don't trust headers
        // Log this as it may indicate an attack attempt or misconfiguration
        if let Some(ip) = connecting_ip {
            if req.headers().contains_key(&trusted_proxies.real_ip_header) {
                tracing::debug!(
                    connecting_ip = %ip,
                    header = %trusted_proxies.real_ip_header,
                    "Ignoring proxy header from untrusted IP"
                );
            }
        }
        return connecting_ip;
    }

    // We trust the proxy - extract client IP from headers

    // Try the configured header (default: X-Forwarded-For)
    if let Some(client_ip) = extract_ip_from_xff(req, trusted_proxies, &parsed_cidrs) {
        return Some(client_ip);
    }

    // Try X-Real-IP as fallback (common with nginx)
    // X-Real-IP contains a single IP, so no right-to-left parsing needed
    if let Some(header_value) = req.headers().get("X-Real-IP")
        && let Ok(header_str) = header_value.to_str()
        && let Ok(ip) = header_str.trim().parse::<IpAddr>()
    {
        return Some(ip);
    }

    // Fall back to connection info
    connecting_ip
}

/// Extract client IP from X-Forwarded-For header using right-to-left parsing.
///
/// X-Forwarded-For format: "client, proxy1, proxy2, ..., proxyN"
/// Each proxy appends the IP it received the request from.
///
/// To find the real client IP, we parse right-to-left, skipping IPs that are
/// within trusted proxy CIDRs. The first untrusted IP is the client.
///
/// This prevents attacks where an attacker sends:
/// `X-Forwarded-For: fake-ip, attacker-ip`
/// and the proxy appends their real IP, resulting in:
/// `X-Forwarded-For: fake-ip, attacker-ip, proxy-ip`
///
/// With right-to-left parsing, we skip proxy-ip (trusted), then return
/// attacker-ip (first untrusted), not fake-ip.
fn extract_ip_from_xff(
    req: &Request,
    trusted_proxies: &TrustedProxiesConfig,
    parsed_cidrs: &[IpNet],
) -> Option<IpAddr> {
    let header_value = req.headers().get(&trusted_proxies.real_ip_header)?;
    let header_str = header_value.to_str().ok()?;

    // Parse all IPs from the header
    let ips: Vec<IpAddr> = header_str
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if ips.is_empty() {
        return None;
    }

    // If dangerously_trust_all is set, just return the leftmost (first) IP
    // This maintains backwards compatibility for users who explicitly trust all
    if trusted_proxies.dangerously_trust_all {
        return ips.into_iter().next();
    }

    // Parse right-to-left, skipping trusted proxy IPs
    // Find the first untrusted IP - this is the client
    ips.into_iter()
        .rev()
        .find(|&ip| !trusted_proxies.is_trusted_ip(ip, parsed_cidrs))
}

/// Extract client IP address from request headers and connection info.
///
/// Like [`extract_client_ip`] but works with already-extracted components
/// instead of a full `Request`, for use in route handlers where the request
/// has already been decomposed.
#[cfg(feature = "sso")]
pub fn extract_client_ip_from_parts(
    headers: &axum::http::HeaderMap,
    connecting_addr: Option<std::net::SocketAddr>,
    trusted_proxies: &TrustedProxiesConfig,
) -> Option<IpAddr> {
    let connecting_ip = connecting_addr.map(|addr| addr.ip());

    if !trusted_proxies.is_configured() {
        return connecting_ip;
    }

    let parsed_cidrs = trusted_proxies.parsed_cidrs();

    let should_trust_headers = match connecting_ip {
        Some(ip) => trusted_proxies.is_trusted_ip(ip, &parsed_cidrs),
        None => trusted_proxies.dangerously_trust_all,
    };

    if !should_trust_headers {
        return connecting_ip;
    }

    if let Some(header_value) = headers.get(&trusted_proxies.real_ip_header)
        && let Ok(header_str) = header_value.to_str()
    {
        let ips: Vec<IpAddr> = header_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if !ips.is_empty() {
            if trusted_proxies.dangerously_trust_all {
                return ips.into_iter().next();
            }

            if let Some(ip) = ips
                .into_iter()
                .rev()
                .find(|&ip| !trusted_proxies.is_trusted_ip(ip, &parsed_cidrs))
            {
                return Some(ip);
            }
        }
    }

    if let Some(header_value) = headers.get("X-Real-IP")
        && let Ok(header_str) = header_value.to_str()
        && let Ok(ip) = header_str.trim().parse::<IpAddr>()
    {
        return Some(ip);
    }

    connecting_ip
}

async fn check_ip_rate_limit(
    cache: &std::sync::Arc<dyn Cache>,
    client_ip: &str,
    window: &str,
    limit: u32,
    ttl: Duration,
) -> Result<RateLimitResult, RateLimitError> {
    let cache_key = CacheKeys::rate_limit_ip(client_ip, window);

    // Atomically check and increment - only increments if under limit
    let result = cache
        .check_and_incr_rate_limit(&cache_key, limit, ttl.as_secs())
        .await
        .map_err(|e| RateLimitError::Internal(e.to_string()))?;

    if !result.allowed {
        metrics::record_rate_limit("limited", None);
        return Err(RateLimitError::Exceeded {
            limit,
            current: result.current,
            window: format!("{} (IP)", window),
            retry_after: result.reset_secs,
        });
    }

    Ok(result)
}

pub fn add_rate_limit_headers(mut response: Response, rate_limit: &RateLimitResult) -> Response {
    let remaining = (rate_limit.limit as i64)
        .saturating_sub(rate_limit.current)
        .max(0) as u32;

    let limit_value = HeaderValue::try_from(rate_limit.limit.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0"));
    let remaining_value = HeaderValue::try_from(remaining.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0"));
    let reset_value = HeaderValue::try_from(rate_limit.reset_secs.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0"));

    let headers = response.headers_mut();
    headers.insert("X-RateLimit-Limit", limit_value);
    headers.insert("X-RateLimit-Remaining", remaining_value);
    headers.insert("X-RateLimit-Reset", reset_value);

    response
}

/// Result of token rate limit check with reservation info for later adjustment
#[derive(Debug, Clone)]
pub struct TokenRateLimitCheckResult {
    /// Per-minute reservation info
    pub minute_reservation: TokenReservation,
    /// Per-day reservation info (if configured)
    pub day_reservation: Option<TokenReservation>,
}

/// Info about a token reservation for a single window
#[derive(Debug, Clone)]
pub struct TokenReservation {
    /// Cache key used for this window
    pub cache_key: String,
    /// Estimated tokens that were reserved
    pub reserved_tokens: i64,
    /// Current token count after reservation
    pub current_tokens: i64,
    /// The limit for this window
    pub limit: u32,
    /// TTL for the cache entry
    pub ttl_secs: u64,
}

/// Legacy result for headers (computed from reservations)
#[derive(Debug, Clone)]
pub struct TokenRateLimitResult {
    /// Per-minute current usage
    pub minute_current: i64,
    /// Per-minute limit
    pub minute_limit: u32,
    /// Per-day current usage (if configured)
    pub day_current: Option<i64>,
    /// Per-day limit (if configured)
    pub day_limit: Option<u32>,
}

impl From<&TokenRateLimitCheckResult> for TokenRateLimitResult {
    fn from(check: &TokenRateLimitCheckResult) -> Self {
        Self {
            minute_current: check.minute_reservation.current_tokens,
            minute_limit: check.minute_reservation.limit,
            day_current: check.day_reservation.as_ref().map(|r| r.current_tokens),
            day_limit: check.day_reservation.as_ref().map(|r| r.limit),
        }
    }
}

/// Adjust the token reservation after actual token count is known.
///
/// This should be called after the request completes to replace the estimated
/// tokens with the actual count. If actual tokens are higher than estimated,
/// the overage is added. If lower, the difference is credited back.
///
/// Uses retry with exponential backoff to handle transient cache failures.
/// Returns true if all adjustments succeeded, false if any failed.
///
/// For failed requests, pass `actual_tokens = 0` to refund the entire reservation.
pub async fn adjust_token_reservation(
    cache: &Arc<dyn Cache>,
    reservation: &TokenRateLimitCheckResult,
    actual_tokens: i64,
) -> bool {
    const MAX_RETRIES: u32 = 3;
    const INITIAL_BACKOFF_MS: u64 = 10;

    let mut all_succeeded = true;

    // Adjust minute reservation
    let minute_adjustment = actual_tokens - reservation.minute_reservation.reserved_tokens;
    if minute_adjustment != 0 {
        let mut last_error = None;
        for attempt in 0..MAX_RETRIES {
            match cache
                .incr_by(
                    &reservation.minute_reservation.cache_key,
                    minute_adjustment,
                    Duration::from_secs(reservation.minute_reservation.ttl_secs),
                )
                .await
            {
                Ok(_) => {
                    last_error = None;
                    break;
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < MAX_RETRIES - 1 {
                        tokio::time::sleep(Duration::from_millis(
                            INITIAL_BACKOFF_MS * (1 << attempt),
                        ))
                        .await;
                    }
                }
            }
        }
        if let Some(e) = last_error {
            tracing::error!(
                cache_key = %reservation.minute_reservation.cache_key,
                adjustment = minute_adjustment,
                error = %e,
                "Failed to adjust token reservation (per-minute) after {} retries",
                MAX_RETRIES
            );
            all_succeeded = false;
        }
    }

    // Adjust day reservation if present
    if let Some(day_reservation) = &reservation.day_reservation {
        let day_adjustment = actual_tokens - day_reservation.reserved_tokens;
        if day_adjustment != 0 {
            let mut last_error = None;
            for attempt in 0..MAX_RETRIES {
                match cache
                    .incr_by(
                        &day_reservation.cache_key,
                        day_adjustment,
                        Duration::from_secs(day_reservation.ttl_secs),
                    )
                    .await
                {
                    Ok(_) => {
                        last_error = None;
                        break;
                    }
                    Err(e) => {
                        last_error = Some(e);
                        if attempt < MAX_RETRIES - 1 {
                            tokio::time::sleep(Duration::from_millis(
                                INITIAL_BACKOFF_MS * (1 << attempt),
                            ))
                            .await;
                        }
                    }
                }
            }
            if let Some(e) = last_error {
                tracing::error!(
                    cache_key = %day_reservation.cache_key,
                    adjustment = day_adjustment,
                    error = %e,
                    "Failed to adjust token reservation (per-day) after {} retries",
                    MAX_RETRIES
                );
                all_succeeded = false;
            }
        }
    }

    all_succeeded
}

/// Add token rate limit headers to response
pub fn add_token_rate_limit_headers(
    mut response: Response,
    token_limit: &TokenRateLimitResult,
) -> Response {
    let remaining = (token_limit.minute_limit as i64)
        .saturating_sub(token_limit.minute_current)
        .max(0);

    let limit_value = HeaderValue::try_from(token_limit.minute_limit.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0"));
    let remaining_value = HeaderValue::try_from(remaining.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0"));
    let used_value = HeaderValue::try_from(token_limit.minute_current.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static("0"));

    let headers = response.headers_mut();
    headers.insert("X-TokenRateLimit-Limit", limit_value);
    headers.insert("X-TokenRateLimit-Remaining", remaining_value);
    headers.insert("X-TokenRateLimit-Used", used_value);

    // Add daily limit headers if configured
    if let (Some(day_limit), Some(day_current)) = (token_limit.day_limit, token_limit.day_current) {
        let day_remaining = (day_limit as i64).saturating_sub(day_current).max(0);

        if let Ok(v) = HeaderValue::try_from(day_limit.to_string()) {
            headers.insert("X-TokenRateLimit-Day-Limit", v);
        }
        if let Ok(v) = HeaderValue::try_from(day_remaining.to_string()) {
            headers.insert("X-TokenRateLimit-Day-Remaining", v);
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use axum::body::Body;
    use http::Request as HttpRequest;

    use super::*;

    fn make_request_with_headers(headers: Vec<(&str, &str)>) -> Request {
        let mut builder = HttpRequest::builder().method("GET").uri("/test");

        for (name, value) in headers {
            builder = builder.header(name, value);
        }

        builder.body(Body::empty()).unwrap()
    }

    /// Create a request with headers and a simulated connecting IP
    fn make_request_with_connect_info(
        headers: Vec<(&str, &str)>,
        connecting_ip: &str,
    ) -> Request<Body> {
        let mut builder = HttpRequest::builder().method("GET").uri("/test");

        for (name, value) in headers {
            builder = builder.header(name, value);
        }

        let mut req = builder.body(Body::empty()).unwrap();

        // Add ConnectInfo extension to simulate a TCP connection
        // Parse IP first to handle both IPv4 and IPv6
        let ip: IpAddr = connecting_ip.parse().unwrap();
        let addr = SocketAddr::new(ip, 12345);
        req.extensions_mut().insert(ConnectInfo(addr));

        req
    }

    // ========== dangerously_trust_all tests ==========

    #[test]
    fn test_trust_all_extracts_first_xff_ip() {
        let config = TrustedProxiesConfig {
            dangerously_trust_all: true,
            cidrs: vec![],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // With dangerously_trust_all, the first (leftmost) IP should be returned
        let req = make_request_with_headers(vec![("X-Forwarded-For", "192.168.1.100")]);
        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, Some("192.168.1.100".parse().unwrap()));
    }

    #[test]
    fn test_trust_all_with_xff_chain_returns_first() {
        let config = TrustedProxiesConfig {
            dangerously_trust_all: true,
            cidrs: vec![],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // X-Forwarded-For format: "client, proxy1, proxy2"
        // With dangerously_trust_all, returns leftmost (client) IP
        let req = make_request_with_headers(vec![(
            "X-Forwarded-For",
            "10.0.0.1, 172.16.0.1, 192.168.1.1",
        )]);

        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, Some("10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn test_trust_all_x_real_ip_fallback() {
        let config = TrustedProxiesConfig {
            dangerously_trust_all: true,
            cidrs: vec![],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // No X-Forwarded-For, but X-Real-IP is present
        let req = make_request_with_headers(vec![("X-Real-IP", "203.0.113.50")]);

        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, Some("203.0.113.50".parse().unwrap()));
    }

    #[test]
    fn test_trust_all_custom_header() {
        let config = TrustedProxiesConfig {
            dangerously_trust_all: true,
            cidrs: vec![],
            real_ip_header: "CF-Connecting-IP".to_string(),
        };

        let req = make_request_with_headers(vec![("CF-Connecting-IP", "198.51.100.25")]);

        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, Some("198.51.100.25".parse().unwrap()));
    }

    #[test]
    fn test_trust_all_ipv6() {
        let config = TrustedProxiesConfig {
            dangerously_trust_all: true,
            cidrs: vec![],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        let req = make_request_with_headers(vec![("X-Forwarded-For", "2001:db8::1")]);

        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, Some("2001:db8::1".parse().unwrap()));
    }

    #[test]
    fn test_trust_all_invalid_header() {
        let config = TrustedProxiesConfig {
            dangerously_trust_all: true,
            cidrs: vec![],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        let req = make_request_with_headers(vec![("X-Forwarded-For", "not-an-ip")]);

        // Should fall back to trying X-Real-IP, then ConnectInfo (both None here)
        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, None);
    }

    // ========== No proxy trust configured ==========

    #[test]
    fn test_no_trust_ignores_xff_header() {
        // When no proxies are trusted, headers are ignored
        let config = TrustedProxiesConfig {
            dangerously_trust_all: false,
            cidrs: vec![],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        let req = make_request_with_headers(vec![("X-Forwarded-For", "192.168.1.100")]);

        // Should return None since no proxies are trusted and no ConnectInfo
        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, None);
    }

    #[test]
    fn test_no_trust_returns_connecting_ip() {
        let config = TrustedProxiesConfig {
            dangerously_trust_all: false,
            cidrs: vec![],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // Even with XFF header, should return connecting IP when no trust configured
        let req = make_request_with_connect_info(vec![("X-Forwarded-For", "1.2.3.4")], "10.0.0.1");

        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, Some("10.0.0.1".parse().unwrap()));
    }

    // ========== CIDR-based trust (SECURITY CRITICAL) ==========

    #[test]
    fn test_cidr_trust_validates_connecting_ip() {
        // Trust proxies from 10.0.0.0/8
        let config = TrustedProxiesConfig {
            dangerously_trust_all: false,
            cidrs: vec!["10.0.0.0/8".to_string()],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // Request from trusted proxy (10.0.0.1) - should extract from XFF
        let req =
            make_request_with_connect_info(vec![("X-Forwarded-For", "192.168.1.100")], "10.0.0.1");
        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, Some("192.168.1.100".parse().unwrap()));
    }

    #[test]
    fn test_cidr_trust_rejects_untrusted_connecting_ip() {
        // Trust proxies from 10.0.0.0/8 only
        let config = TrustedProxiesConfig {
            dangerously_trust_all: false,
            cidrs: vec!["10.0.0.0/8".to_string()],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // Request from UNTRUSTED IP (192.168.1.1) - should NOT trust XFF header
        // This is the key security fix - prevents IP spoofing
        let req = make_request_with_connect_info(
            vec![("X-Forwarded-For", "1.2.3.4")],
            "192.168.1.1", // Not in 10.0.0.0/8
        );
        let ip = extract_client_ip(&req, &config);
        // Should return the connecting IP, NOT the spoofed XFF header
        assert_eq!(ip, Some("192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn test_cidr_trust_right_to_left_xff_parsing() {
        // Trust proxies from 10.0.0.0/8
        let config = TrustedProxiesConfig {
            dangerously_trust_all: false,
            cidrs: vec!["10.0.0.0/8".to_string()],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // Chain: "fake-client, real-client, trusted-proxy"
        // The trusted proxy (10.0.0.50) added real-client (203.0.113.50) to XFF
        // An attacker prepended fake-client (1.1.1.1)
        // Right-to-left parsing should skip 10.0.0.50 (trusted) and return 203.0.113.50
        let req = make_request_with_connect_info(
            vec![("X-Forwarded-For", "1.1.1.1, 203.0.113.50, 10.0.0.50")],
            "10.0.0.1", // Connecting from trusted proxy
        );
        let ip = extract_client_ip(&req, &config);
        // Should be 203.0.113.50 (first untrusted from the right), NOT 1.1.1.1 (spoofed)
        assert_eq!(ip, Some("203.0.113.50".parse().unwrap()));
    }

    #[test]
    fn test_cidr_trust_multiple_cidrs() {
        // Trust proxies from multiple ranges
        let config = TrustedProxiesConfig {
            dangerously_trust_all: false,
            cidrs: vec!["10.0.0.0/8".to_string(), "172.16.0.0/12".to_string()],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // Request from 172.16.0.1 (trusted via second CIDR)
        let req =
            make_request_with_connect_info(vec![("X-Forwarded-For", "8.8.8.8")], "172.16.0.1");
        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, Some("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn test_cidr_trust_ipv6() {
        // Trust IPv6 proxy range
        let config = TrustedProxiesConfig {
            dangerously_trust_all: false,
            cidrs: vec!["fd00::/8".to_string()],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // Request from trusted IPv6 proxy
        let req = make_request_with_connect_info(
            vec![("X-Forwarded-For", "2001:db8::1")],
            "fd12:3456:789a::1",
        );
        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, Some("2001:db8::1".parse().unwrap()));
    }

    #[test]
    fn test_cidr_without_connect_info_returns_none() {
        // CIDR configured but no ConnectInfo - can't validate connecting IP
        let config = TrustedProxiesConfig {
            dangerously_trust_all: false,
            cidrs: vec!["10.0.0.0/8".to_string()],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // No ConnectInfo - we don't know if the connection is from a trusted proxy
        let req = make_request_with_headers(vec![("X-Forwarded-For", "192.168.1.100")]);
        let ip = extract_client_ip(&req, &config);
        // Should return None since we can't validate the connecting IP
        assert_eq!(ip, None);
    }

    // ========== Edge cases ==========

    #[test]
    fn test_all_xff_ips_trusted_falls_back_to_connecting() {
        // Edge case: all IPs in XFF chain are trusted proxies
        let config = TrustedProxiesConfig {
            dangerously_trust_all: false,
            cidrs: vec!["10.0.0.0/8".to_string()],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // All IPs in XFF are in trusted range
        let req = make_request_with_connect_info(
            vec![("X-Forwarded-For", "10.0.0.1, 10.0.0.2, 10.0.0.3")],
            "10.0.0.4",
        );
        let ip = extract_client_ip(&req, &config);
        // No untrusted IP found in XFF - falls back to connecting IP
        // This is a reasonable fallback for internal-only traffic
        assert_eq!(ip, Some("10.0.0.4".parse().unwrap()));
    }

    #[test]
    fn test_invalid_cidr_skipped() {
        // Invalid CIDRs should be skipped without breaking functionality
        let config = TrustedProxiesConfig {
            dangerously_trust_all: false,
            cidrs: vec!["not-a-cidr".to_string(), "10.0.0.0/8".to_string()],
            real_ip_header: "X-Forwarded-For".to_string(),
        };

        // Should still work with the valid CIDR
        let req =
            make_request_with_connect_info(vec![("X-Forwarded-For", "192.168.1.100")], "10.0.0.1");
        let ip = extract_client_ip(&req, &config);
        assert_eq!(ip, Some("192.168.1.100".parse().unwrap()));
    }

    // ========== Cache key tests ==========

    #[test]
    fn test_cache_key_ip_rate_limit() {
        let key = CacheKeys::rate_limit_ip("192.168.1.100", "minute");
        assert_eq!(key, "gw:ratelimit:ip:192.168.1.100:minute");

        let key = CacheKeys::rate_limit_ip("::1", "hour");
        assert_eq!(key, "gw:ratelimit:ip:::1:hour");
    }

    #[test]
    fn test_ip_rate_limit_config_defaults() {
        use crate::config::IpRateLimitConfig;

        let config = IpRateLimitConfig::default();
        assert!(config.enabled);
        assert_eq!(config.requests_per_minute, 120);
        assert_eq!(config.requests_per_hour, None);
    }
}
