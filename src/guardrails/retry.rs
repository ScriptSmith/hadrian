//! Retry logic for guardrails providers.
//!
//! Provides retry functionality for guardrails HTTP requests, handling transient
//! failures like timeouts, rate limits (429), and server errors.

use std::future::Future;

use tracing::{debug, warn};

use super::{GuardrailsRequest, GuardrailsResponse, GuardrailsResult};

/// Configuration for guardrails retry behavior.
#[derive(Debug, Clone)]
pub struct GuardrailsRetryConfig {
    /// Whether retry is enabled.
    pub enabled: bool,
    /// Maximum number of retry attempts (0 = no retries, 1 = one retry, etc.).
    pub max_retries: u32,
    /// Initial delay between retries in milliseconds.
    pub initial_delay_ms: u64,
    /// Maximum delay between retries in milliseconds.
    pub max_delay_ms: u64,
    /// Backoff multiplier for exponential backoff.
    pub backoff_multiplier: f64,
    /// Jitter factor (0.0-1.0) to add randomness to delays.
    pub jitter: f64,
}

impl Default for GuardrailsRetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: 2,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            jitter: 0.1,
        }
    }
}

impl GuardrailsRetryConfig {
    /// Creates a disabled retry configuration.
    #[allow(dead_code)] // Guardrail infrastructure
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Calculates the delay for a given attempt number.
    pub fn delay_for_attempt(&self, attempt: u32) -> std::time::Duration {
        let base_delay =
            self.initial_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let delay_ms = base_delay.min(self.max_delay_ms as f64);

        // Add jitter
        let jitter_range = delay_ms * self.jitter;
        let jitter = if jitter_range > 0.0 {
            (rand::random::<f64>() * 2.0 - 1.0) * jitter_range
        } else {
            0.0
        };

        let final_delay_ms = (delay_ms + jitter).max(0.0) as u64;
        std::time::Duration::from_millis(final_delay_ms)
    }
}

/// Execute a guardrails evaluation with retry logic.
///
/// The `evaluate` function is called for each attempt. It should return
/// the guardrails response or an error.
///
/// Retryable errors:
/// - Timeout errors
/// - Rate limit errors (429)
/// - Provider errors marked as retryable
///
/// Non-retryable errors:
/// - Blocked content (this is a successful evaluation, just with violations)
/// - Configuration errors
/// - Parse errors
/// - Authentication errors
///
/// Returns the successful response if any attempt succeeds, or the last error
/// if all retries are exhausted.
#[allow(dead_code)] // Guardrail infrastructure
pub async fn with_retry<F, Fut>(
    config: &GuardrailsRetryConfig,
    provider_name: &str,
    request: &GuardrailsRequest,
    evaluate: F,
) -> GuardrailsResult<GuardrailsResponse>
where
    F: Fn(&GuardrailsRequest) -> Fut,
    Fut: Future<Output = GuardrailsResult<GuardrailsResponse>>,
{
    if !config.enabled {
        return evaluate(request).await;
    }

    let max_attempts = config.max_retries + 1; // +1 for initial attempt

    for attempt in 0..max_attempts {
        let result = evaluate(request).await;

        match &result {
            Ok(_response) => {
                if attempt > 0 {
                    debug!(
                        provider = provider_name,
                        attempt = attempt + 1,
                        "Guardrails evaluation succeeded after retry"
                    );
                }
                return result;
            }
            Err(error) => {
                // Check if error is retryable
                if error.is_retryable() && attempt < max_attempts - 1 {
                    let delay = config.delay_for_attempt(attempt);
                    warn!(
                        provider = provider_name,
                        error = %error,
                        attempt = attempt + 1,
                        max_attempts = max_attempts,
                        delay_ms = delay.as_millis(),
                        "Retryable guardrails error, will retry after delay"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }

                // Non-retryable error or last attempt
                if attempt > 0 {
                    warn!(
                        provider = provider_name,
                        error = %error,
                        attempts = attempt + 1,
                        "Guardrails evaluation failed after all retry attempts"
                    );
                }

                return result;
            }
        }
    }

    // This shouldn't be reached, but just in case
    unreachable!("Retry loop should have returned")
}

/// A wrapper type that adds retry capability to any guardrails provider.
#[allow(dead_code)] // Guardrail infrastructure
pub struct RetryingProvider<P> {
    /// The inner provider to wrap.
    pub inner: P,
    /// Retry configuration.
    pub config: GuardrailsRetryConfig,
}

impl<P> RetryingProvider<P> {
    /// Creates a new retrying provider wrapper.
    #[allow(dead_code)] // Guardrail infrastructure
    pub fn new(inner: P, config: GuardrailsRetryConfig) -> Self {
        Self { inner, config }
    }

    /// Creates a new retrying provider wrapper with default retry config.
    #[allow(dead_code)] // Guardrail infrastructure
    pub fn with_defaults(inner: P) -> Self {
        Self::new(inner, GuardrailsRetryConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    };

    use super::*;
    use crate::guardrails::GuardrailsError;

    #[test]
    fn test_default_config() {
        let config = GuardrailsRetryConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.initial_delay_ms, 100);
        assert_eq!(config.max_delay_ms, 5000);
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_disabled_config() {
        let config = GuardrailsRetryConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_delay_for_attempt() {
        let config = GuardrailsRetryConfig {
            initial_delay_ms: 100,
            max_delay_ms: 10000,
            backoff_multiplier: 2.0,
            jitter: 0.0, // Disable jitter for deterministic testing
            ..Default::default()
        };

        // Check exponential backoff
        let delay0 = config.delay_for_attempt(0);
        let delay1 = config.delay_for_attempt(1);
        let delay2 = config.delay_for_attempt(2);

        assert_eq!(delay0.as_millis(), 100);
        assert_eq!(delay1.as_millis(), 200);
        assert_eq!(delay2.as_millis(), 400);
    }

    #[test]
    fn test_delay_capped_at_max() {
        let config = GuardrailsRetryConfig {
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

    #[tokio::test]
    async fn test_retry_on_transient_error() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let config = GuardrailsRetryConfig {
            enabled: true,
            max_retries: 2,
            initial_delay_ms: 1, // Very short for testing
            jitter: 0.0,
            ..Default::default()
        };

        let request = GuardrailsRequest::user_input("test");

        let result = with_retry(&config, "test", &request, |_req| {
            let count = attempt_count_clone.clone();
            async move {
                let attempt = count.fetch_add(1, Ordering::SeqCst);
                if attempt < 2 {
                    Err(GuardrailsError::retryable_error("test", "Transient error"))
                } else {
                    Ok(GuardrailsResponse::passed())
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3); // Initial + 2 retries
    }

    #[tokio::test]
    async fn test_no_retry_on_non_retryable_error() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let config = GuardrailsRetryConfig {
            enabled: true,
            max_retries: 2,
            initial_delay_ms: 1,
            ..Default::default()
        };

        let request = GuardrailsRequest::user_input("test");

        let result = with_retry(&config, "test", &request, |_req| {
            let count = attempt_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err(GuardrailsError::auth_error("test", "Invalid credentials"))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1); // Only initial attempt
    }

    #[tokio::test]
    async fn test_retry_disabled() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let config = GuardrailsRetryConfig::disabled();

        let request = GuardrailsRequest::user_input("test");

        let result = with_retry(&config, "test", &request, |_req| {
            let count = attempt_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err(GuardrailsError::retryable_error("test", "Transient error"))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1); // Only initial attempt
    }

    #[tokio::test]
    async fn test_max_retries_exhausted() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let config = GuardrailsRetryConfig {
            enabled: true,
            max_retries: 2,
            initial_delay_ms: 1,
            jitter: 0.0,
            ..Default::default()
        };

        let request = GuardrailsRequest::user_input("test");

        let result = with_retry(&config, "test", &request, |_req| {
            let count = attempt_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err(GuardrailsError::retryable_error("test", "Always fails"))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3); // Initial + 2 retries
    }
}
