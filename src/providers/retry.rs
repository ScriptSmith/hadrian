//! Provider retry logic with exponential backoff and circuit breaker integration.
//!
//! Provides retry functionality for provider HTTP requests, handling transient
//! failures like 5xx errors, rate limits (429), and connection issues.
//!
//! Also integrates with the circuit breaker pattern to prevent hammering
//! unhealthy providers.

use std::future::Future;

use reqwest::StatusCode;
use tracing::{debug, warn};

use crate::{
    config::{CircuitBreakerConfig, RetryConfig},
    providers::circuit_breaker::{CircuitBreaker, CircuitBreakerError},
};

/// Determines if a reqwest error is retryable.
///
/// Connection errors, timeouts, and other transient issues are retryable.
pub fn is_retryable_error(error: &reqwest::Error) -> bool {
    // Connection errors, timeouts, and other transient issues
    error.is_connect()
        || error.is_timeout()
        || error.is_request()
        // Status errors where we got a response but it was a server error
        || error
            .status()
            .map(|s| s.is_server_error() || s == StatusCode::TOO_MANY_REQUESTS)
            .unwrap_or(false)
}

/// Execute an async operation with retry logic.
///
/// The `make_request` function is called for each attempt. It should return
/// a future that produces a `reqwest::Response` or `reqwest::Error`.
///
/// Returns the successful response if any attempt succeeds, or the last error
/// if all retries are exhausted.
pub async fn with_retry<F, Fut>(
    config: &RetryConfig,
    provider_name: &str,
    operation: &str,
    make_request: F,
) -> Result<reqwest::Response, reqwest::Error>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    if !config.enabled {
        return make_request().await;
    }

    let max_attempts = config.max_retries + 1; // +1 for initial attempt

    for attempt in 0..max_attempts {
        let result = make_request().await;

        match result {
            Ok(response) => {
                let status = response.status();

                // Check if we should retry based on status code
                if config.should_retry_status(status.as_u16()) && attempt < max_attempts - 1 {
                    let delay = config.delay_for_attempt(attempt);
                    warn!(
                        provider = provider_name,
                        operation = operation,
                        status = %status,
                        attempt = attempt + 1,
                        max_attempts = max_attempts,
                        delay_ms = delay.as_millis(),
                        "Retryable status code, will retry after delay"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }

                if attempt > 0 {
                    debug!(
                        provider = provider_name,
                        operation = operation,
                        status = %status,
                        attempt = attempt + 1,
                        "Request succeeded after retry"
                    );
                }

                return Ok(response);
            }
            Err(error) => {
                // Check if error is retryable
                if is_retryable_error(&error) && attempt < max_attempts - 1 {
                    let delay = config.delay_for_attempt(attempt);
                    warn!(
                        provider = provider_name,
                        operation = operation,
                        error = %error,
                        attempt = attempt + 1,
                        max_attempts = max_attempts,
                        delay_ms = delay.as_millis(),
                        "Retryable error, will retry after delay"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }

                // Non-retryable error or last attempt
                if attempt > 0 {
                    warn!(
                        provider = provider_name,
                        operation = operation,
                        error = %error,
                        attempts = attempt + 1,
                        "Request failed after all retry attempts"
                    );
                }

                return Err(error);
            }
        }
    }

    // This shouldn't be reached, but just in case
    unreachable!("Retry loop should have returned")
}

/// Error type for provider requests with circuit breaker support.
#[derive(Debug, thiserror::Error)]
pub enum ProviderRequestError {
    #[error(transparent)]
    CircuitBreakerOpen(#[from] CircuitBreakerError),

    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

/// Execute an async operation with circuit breaker and retry logic.
///
/// This function combines circuit breaker protection with retry logic:
/// 1. Check circuit breaker - if open, reject immediately
/// 2. If closed/half-open, proceed with retry logic
/// 3. After all retries complete, record result to circuit breaker
///
/// # Arguments
///
/// * `circuit_breaker` - Optional circuit breaker instance
/// * `circuit_breaker_config` - Configuration for determining failure status codes
/// * `retry_config` - Retry configuration
/// * `provider_name` - Provider name for logging
/// * `operation` - Operation name for logging
/// * `make_request` - Function that creates the request future
pub async fn with_circuit_breaker_and_retry<F, Fut>(
    circuit_breaker: Option<&CircuitBreaker>,
    circuit_breaker_config: &CircuitBreakerConfig,
    retry_config: &RetryConfig,
    provider_name: &str,
    operation: &str,
    make_request: F,
) -> Result<reqwest::Response, ProviderRequestError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    // Check circuit breaker before attempting request
    if let Some(cb) = circuit_breaker {
        cb.check()?;
    }

    // Execute with retry logic
    let result = with_retry(retry_config, provider_name, operation, make_request).await;

    // Record result to circuit breaker
    if let Some(cb) = circuit_breaker {
        match &result {
            Ok(response) => {
                let status = response.status().as_u16();
                if circuit_breaker_config.is_failure_status(status) {
                    cb.record_failure();
                } else {
                    cb.record_success();
                }
            }
            Err(_) => {
                // Connection errors, timeouts, etc. count as failures
                cb.record_failure();
            }
        }
    }

    result.map_err(ProviderRequestError::Request)
}

/// Error type for generic operations with circuit breaker support.
#[derive(Debug, thiserror::Error)]
pub enum GenericRequestError<E> {
    #[error(transparent)]
    CircuitBreakerOpen(#[from] CircuitBreakerError),

    #[error(transparent)]
    Operation(E),
}

/// Execute a generic async operation with circuit breaker and retry logic.
///
/// This function combines circuit breaker protection with retry logic for any
/// async operation, not just HTTP requests. Useful for database operations,
/// vector store calls, and other non-HTTP operations.
///
/// # Arguments
///
/// * `circuit_breaker` - Optional circuit breaker instance
/// * `circuit_breaker_config` - Configuration for determining failure status
/// * `retry_config` - Retry configuration (max retries, delays, backoff)
/// * `service_name` - Service name for logging (e.g., "qdrant", "pgvector")
/// * `operation_name` - Operation name for logging (e.g., "search", "store_chunk")
/// * `is_retryable` - Predicate that returns true if the error should trigger a retry
/// * `is_failure` - Predicate that returns true if the result should count as a circuit breaker failure
/// * `operation` - Async function that produces the result
///
/// # Returns
///
/// The successful result if any attempt succeeds, or an error if all retries
/// are exhausted or the circuit breaker is open.
///
/// # Example
///
/// ```ignore
/// let result = with_circuit_breaker_and_retry_generic(
///     circuit_breaker.as_ref(),
///     &circuit_breaker_config,
///     &retry_config,
///     "qdrant",
///     "search_vector_stores",
///     |e| is_retryable_database_error(&e.to_string()),
///     |_result| false, // Success is never a failure
///     || async { vector_store.search(&query).await },
/// ).await;
/// ```
pub async fn with_circuit_breaker_and_retry_generic<F, Fut, T, E, P, Q>(
    circuit_breaker: Option<&CircuitBreaker>,
    retry_config: &RetryConfig,
    service_name: &str,
    operation_name: &str,
    is_retryable: P,
    is_failure: Q,
    operation: F,
) -> Result<T, GenericRequestError<E>>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
    P: Fn(&E) -> bool,
    Q: Fn(&T) -> bool,
{
    // Check circuit breaker before attempting operation
    if let Some(cb) = circuit_breaker {
        cb.check()?;
    }

    // Execute with retry logic
    let result = with_retry_generic(retry_config, operation_name, &is_retryable, &operation).await;

    // Record result to circuit breaker
    if let Some(cb) = circuit_breaker {
        match &result {
            Ok(value) => {
                if is_failure(value) {
                    debug!(
                        service = service_name,
                        operation = operation_name,
                        "Recording failure to circuit breaker (success with failure condition)"
                    );
                    cb.record_failure();
                } else {
                    cb.record_success();
                }
            }
            Err(_) => {
                debug!(
                    service = service_name,
                    operation = operation_name,
                    "Recording failure to circuit breaker (operation error)"
                );
                cb.record_failure();
            }
        }
    }

    result.map_err(GenericRequestError::Operation)
}

/// Execute a generic async operation with retry logic.
///
/// This function retries any async operation that returns a Result, using a
/// custom predicate to determine if an error is retryable. Useful for database
/// operations, vector store calls, and other non-HTTP operations.
///
/// # Arguments
///
/// * `config` - Retry configuration (max retries, delays, backoff)
/// * `operation_name` - Operation name for logging
/// * `is_retryable` - Predicate that returns true if the error should trigger a retry
/// * `operation` - Async function that produces the result
///
/// # Returns
///
/// The successful result if any attempt succeeds, or the last error if all retries
/// are exhausted.
///
/// # Example
///
/// ```ignore
/// let result = with_retry_generic(
///     &retry_config,
///     "store_chunks",
///     |e| e.is_connection_error() || e.is_deadlock(),
///     || async { db.insert_chunks(&chunks).await },
/// ).await;
/// ```
pub async fn with_retry_generic<F, Fut, T, E, P>(
    config: &RetryConfig,
    operation_name: &str,
    is_retryable: P,
    operation: F,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
    P: Fn(&E) -> bool,
{
    if !config.enabled {
        return operation().await;
    }

    let max_attempts = config.max_retries + 1; // +1 for initial attempt

    for attempt in 0..max_attempts {
        let result = operation().await;

        match result {
            Ok(value) => {
                if attempt > 0 {
                    debug!(
                        operation = operation_name,
                        attempt = attempt + 1,
                        "Operation succeeded after retry"
                    );
                }
                return Ok(value);
            }
            Err(error) => {
                // Check if error is retryable and we have attempts remaining
                if is_retryable(&error) && attempt < max_attempts - 1 {
                    let delay = config.delay_for_attempt(attempt);
                    warn!(
                        operation = operation_name,
                        error = %error,
                        attempt = attempt + 1,
                        max_attempts = max_attempts,
                        delay_ms = delay.as_millis(),
                        "Retryable error, will retry after delay"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }

                // Non-retryable error or last attempt
                if attempt > 0 {
                    warn!(
                        operation = operation_name,
                        error = %error,
                        attempts = attempt + 1,
                        "Operation failed after all retry attempts"
                    );
                }

                return Err(error);
            }
        }
    }

    // This shouldn't be reached, but just in case
    unreachable!("Retry loop should have returned")
}

/// Determines if a database/vector store error is potentially retryable.
///
/// Retryable errors include:
/// - Connection errors (temporary network issues)
/// - Deadlocks (transaction conflicts)
/// - Serialization failures (concurrent updates)
/// - Too many connections (temporary resource exhaustion)
/// - HTTP 5xx server errors (service unavailable, internal error)
/// - Rate limiting (429 Too Many Requests)
///
/// Non-retryable errors include:
/// - Constraint violations (data error)
/// - Syntax errors (programming error)
/// - Authentication failures (config error)
/// - Unknown table/column (schema error)
/// - HTTP 4xx client errors (except 429)
pub fn is_retryable_database_error(error_msg: &str) -> bool {
    let error_lower = error_msg.to_lowercase();

    // Connection errors
    if error_lower.contains("connection")
        || error_lower.contains("connect")
        || error_lower.contains("timeout")
        || error_lower.contains("timed out")
        || error_lower.contains("broken pipe")
        || error_lower.contains("reset by peer")
        || error_lower.contains("closed")
    {
        return true;
    }

    // Deadlock and serialization errors
    if error_lower.contains("deadlock")
        || error_lower.contains("serialization")
        || error_lower.contains("could not serialize")
        || error_lower.contains("concurrent update")
        || error_lower.contains("lock timeout")
    {
        return true;
    }

    // Resource exhaustion
    if error_lower.contains("too many connections")
        || error_lower.contains("too many clients")
        || error_lower.contains("resource temporarily unavailable")
    {
        return true;
    }

    // Transient I/O errors
    if error_lower.contains("io error") || error_lower.contains("temporary failure") {
        return true;
    }

    // HTTP server errors (5xx) - common in Qdrant and other HTTP backends
    if error_lower.contains("service unavailable")
        || error_lower.contains("internal server error")
        || error_lower.contains("bad gateway")
        || error_lower.contains("gateway timeout")
        || error_lower.contains("503")
        || error_lower.contains("502")
        || error_lower.contains("504")
    {
        return true;
    }

    // Rate limiting
    if error_lower.contains("rate limit")
        || error_lower.contains("too many requests")
        || error_lower.contains("429")
    {
        return true;
    }

    // Qdrant-specific transient errors
    if error_lower.contains("overloaded")
        || error_lower.contains("temporarily unavailable")
        || error_lower.contains("retry")
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_retry_config() {
        let config = RetryConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 100);
        assert_eq!(config.max_delay_ms, 10_000);
        assert_eq!(config.backoff_multiplier, 2.0);
        assert!(config.retryable_status_codes.contains(&429));
        assert!(config.retryable_status_codes.contains(&500));
        assert!(config.retryable_status_codes.contains(&502));
        assert!(config.retryable_status_codes.contains(&503));
        assert!(config.retryable_status_codes.contains(&504));
    }

    #[test]
    fn test_should_retry_status() {
        let config = RetryConfig::default();

        // Should retry server errors and rate limits
        assert!(config.should_retry_status(429));
        assert!(config.should_retry_status(500));
        assert!(config.should_retry_status(502));
        assert!(config.should_retry_status(503));
        assert!(config.should_retry_status(504));

        // Should not retry client errors
        assert!(!config.should_retry_status(400));
        assert!(!config.should_retry_status(401));
        assert!(!config.should_retry_status(403));
        assert!(!config.should_retry_status(404));

        // Should not retry success
        assert!(!config.should_retry_status(200));
        assert!(!config.should_retry_status(201));
    }

    #[test]
    fn test_should_retry_status_disabled() {
        let config = RetryConfig {
            enabled: false,
            ..Default::default()
        };

        // Should not retry anything when disabled
        assert!(!config.should_retry_status(429));
        assert!(!config.should_retry_status(500));
    }

    #[test]
    fn test_delay_for_attempt() {
        let config = RetryConfig {
            initial_delay_ms: 100,
            max_delay_ms: 10_000,
            backoff_multiplier: 2.0,
            jitter: 0.0, // Disable jitter for deterministic testing
            ..Default::default()
        };

        // Check exponential backoff
        let delay0 = config.delay_for_attempt(0);
        let delay1 = config.delay_for_attempt(1);
        let delay2 = config.delay_for_attempt(2);
        let delay3 = config.delay_for_attempt(3);

        assert_eq!(delay0.as_millis(), 100);
        assert_eq!(delay1.as_millis(), 200);
        assert_eq!(delay2.as_millis(), 400);
        assert_eq!(delay3.as_millis(), 800);
    }

    #[test]
    fn test_delay_capped_at_max() {
        let config = RetryConfig {
            initial_delay_ms: 1000,
            max_delay_ms: 5000,
            backoff_multiplier: 10.0,
            jitter: 0.0,
            ..Default::default()
        };

        // After a few attempts, should be capped at max
        let delay = config.delay_for_attempt(5);
        assert_eq!(delay.as_millis(), 5000);
    }

    #[test]
    fn test_delay_with_jitter() {
        let config = RetryConfig {
            initial_delay_ms: 1000,
            max_delay_ms: 10_000,
            backoff_multiplier: 2.0,
            jitter: 0.2, // 20% jitter
            ..Default::default()
        };

        // With jitter, delays should vary but be within expected range
        let delays: Vec<_> = (0..10).map(|_| config.delay_for_attempt(0)).collect();

        for delay in delays {
            let ms = delay.as_millis();
            // Should be within +/- 20% of 1000ms
            assert!((800..=1200).contains(&ms), "Delay {} out of range", ms);
        }
    }

    #[test]
    fn test_custom_retryable_status_codes() {
        let config = RetryConfig {
            retryable_status_codes: vec![418], // I'm a teapot
            ..Default::default()
        };

        assert!(config.should_retry_status(418));
        assert!(!config.should_retry_status(429));
        assert!(!config.should_retry_status(500));
    }

    // Tests for is_retryable_database_error

    #[test]
    fn test_retryable_connection_errors() {
        assert!(is_retryable_database_error("connection refused"));
        assert!(is_retryable_database_error("Connection reset by peer"));
        assert!(is_retryable_database_error("connect timeout"));
        assert!(is_retryable_database_error("broken pipe"));
        assert!(is_retryable_database_error("connection closed"));
    }

    #[test]
    fn test_retryable_deadlock_errors() {
        assert!(is_retryable_database_error("deadlock detected"));
        assert!(is_retryable_database_error(
            "could not serialize access due to concurrent update"
        ));
        assert!(is_retryable_database_error("lock timeout exceeded"));
        assert!(is_retryable_database_error("serialization failure"));
    }

    #[test]
    fn test_retryable_resource_errors() {
        assert!(is_retryable_database_error("too many connections"));
        assert!(is_retryable_database_error("too many clients already"));
        assert!(is_retryable_database_error(
            "resource temporarily unavailable"
        ));
    }

    #[test]
    fn test_non_retryable_errors() {
        // Constraint violations
        assert!(!is_retryable_database_error("unique constraint violation"));
        assert!(!is_retryable_database_error("foreign key constraint"));

        // Syntax errors
        assert!(!is_retryable_database_error("syntax error at position 42"));

        // Auth errors
        assert!(!is_retryable_database_error(
            "password authentication failed"
        ));

        // Schema errors
        assert!(!is_retryable_database_error(
            "relation \"foo\" does not exist"
        ));
        assert!(!is_retryable_database_error(
            "column \"bar\" does not exist"
        ));

        // Generic errors
        assert!(!is_retryable_database_error("invalid input syntax"));
    }

    #[test]
    fn test_retryable_http_server_errors() {
        // HTTP 5xx errors
        assert!(is_retryable_database_error("Service Unavailable"));
        assert!(is_retryable_database_error("internal server error"));
        assert!(is_retryable_database_error("Bad Gateway"));
        assert!(is_retryable_database_error("Gateway Timeout"));
        assert!(is_retryable_database_error("HTTP 503"));
        assert!(is_retryable_database_error("HTTP 502"));
        assert!(is_retryable_database_error("HTTP 504"));
    }

    #[test]
    fn test_retryable_rate_limit_errors() {
        assert!(is_retryable_database_error("rate limit exceeded"));
        assert!(is_retryable_database_error("Too Many Requests"));
        assert!(is_retryable_database_error("HTTP 429"));
    }

    #[test]
    fn test_retryable_qdrant_errors() {
        // Qdrant-specific transient errors
        assert!(is_retryable_database_error("server is overloaded"));
        assert!(is_retryable_database_error("temporarily unavailable"));
        assert!(is_retryable_database_error("please retry the request"));
    }

    // Tests for with_retry_generic

    #[tokio::test]
    async fn test_retry_generic_succeeds_first_attempt() {
        let config = RetryConfig {
            enabled: true,
            max_retries: 3,
            initial_delay_ms: 10,
            ..Default::default()
        };

        let attempt_count = std::sync::atomic::AtomicU32::new(0);

        let result: Result<i32, String> = with_retry_generic(
            &config,
            "test_op",
            |_| true, // All errors retryable
            || {
                attempt_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async { Ok(42) }
            },
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_generic_succeeds_after_retry() {
        let config = RetryConfig {
            enabled: true,
            max_retries: 3,
            initial_delay_ms: 10,
            max_delay_ms: 100,
            backoff_multiplier: 2.0,
            jitter: 0.0,
            ..Default::default()
        };

        let attempt_count = std::sync::atomic::AtomicU32::new(0);

        let result: Result<i32, String> = with_retry_generic(
            &config,
            "test_op",
            |_| true,
            || {
                let count = attempt_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async move {
                    if count < 2 {
                        Err("transient error".to_string())
                    } else {
                        Ok(42)
                    }
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_generic_exhausts_retries() {
        let config = RetryConfig {
            enabled: true,
            max_retries: 2,
            initial_delay_ms: 10,
            max_delay_ms: 100,
            backoff_multiplier: 2.0,
            jitter: 0.0,
            ..Default::default()
        };

        let attempt_count = std::sync::atomic::AtomicU32::new(0);

        let result: Result<i32, String> = with_retry_generic(
            &config,
            "test_op",
            |_| true,
            || {
                attempt_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async { Err("permanent error".to_string()) }
            },
        )
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "permanent error");
        // max_retries=2 means 3 total attempts (initial + 2 retries)
        assert_eq!(attempt_count.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_generic_non_retryable_error() {
        let config = RetryConfig {
            enabled: true,
            max_retries: 3,
            initial_delay_ms: 10,
            ..Default::default()
        };

        let attempt_count = std::sync::atomic::AtomicU32::new(0);

        let result: Result<i32, String> = with_retry_generic(
            &config,
            "test_op",
            |e: &String| !e.contains("permanent"), // Only retry non-permanent errors
            || {
                attempt_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async { Err("permanent failure".to_string()) }
            },
        )
        .await;

        assert!(result.is_err());
        // Should fail immediately without retries
        assert_eq!(attempt_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_generic_disabled() {
        let config = RetryConfig {
            enabled: false,
            max_retries: 3,
            initial_delay_ms: 10,
            ..Default::default()
        };

        let attempt_count = std::sync::atomic::AtomicU32::new(0);

        let result: Result<i32, String> = with_retry_generic(
            &config,
            "test_op",
            |_| true,
            || {
                attempt_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async { Err("error".to_string()) }
            },
        )
        .await;

        assert!(result.is_err());
        // Should not retry when disabled
        assert_eq!(attempt_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
