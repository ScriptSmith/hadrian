//! Circuit breaker pattern implementation for provider resilience.
//!
//! The circuit breaker prevents hammering unhealthy providers by tracking failures
//! and temporarily rejecting requests after a threshold is exceeded.
//!
//! # States
//!
//! - **Closed**: Normal operation. Requests pass through, failures are tracked.
//! - **Open**: After threshold failures, requests are rejected immediately.
//! - **Half-Open**: After timeout, limited probe requests test recovery.
//!
//! # Usage
//!
//! ```rust,ignore
//! let breaker = CircuitBreaker::new(&config);
//!
//! // Before making request
//! breaker.check()?;
//!
//! // After request completes
//! if response.status().is_success() {
//!     breaker.record_success();
//! } else if config.is_failure_status(response.status().as_u16()) {
//!     breaker.record_failure();
//! }
//! ```

use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::Utc;
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::{
    config::CircuitBreakerConfig,
    events::{CircuitBreakerState as EventCBState, EventBus, ServerEvent},
    observability::metrics,
};

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CircuitState {
    /// Normal operation - requests pass through.
    Closed,
    /// Circuit tripped - requests are rejected.
    Open,
    /// Testing recovery - limited requests allowed.
    HalfOpen,
}

/// Error returned when the circuit breaker is open.
#[derive(Debug, Error)]
pub enum CircuitBreakerError {
    #[error(
        "Circuit breaker is open for provider '{provider}' - rejecting request (will retry at {retry_after_secs}s)"
    )]
    Open {
        provider: Arc<str>,
        retry_after_secs: u64,
    },
}

// State encoding: upper 2 bits = state, lower 30 bits = counter
const STATE_CLOSED: u32 = 0;
const STATE_OPEN: u32 = 1;
const STATE_HALF_OPEN: u32 = 2;
const STATE_SHIFT: u32 = 30;
const COUNTER_MASK: u32 = (1 << STATE_SHIFT) - 1;

/// Thread-safe circuit breaker.
///
/// Uses atomic operations for lock-free state management.
pub struct CircuitBreaker {
    /// Provider name for logging.
    provider_name: Arc<str>,
    /// Configuration.
    config: CircuitBreakerConfig,
    /// Packed state: upper 2 bits = state, lower 30 bits = failure/success count.
    state_and_counter: AtomicU32,
    /// Timestamp when the circuit was opened (millis since UNIX epoch).
    opened_at: AtomicU64,
    /// Current open timeout in milliseconds (adaptive backoff).
    current_timeout_millis: AtomicU64,
    /// Number of consecutive times the circuit has opened without successful recovery.
    /// Used for exponential backoff calculation.
    consecutive_opens: AtomicU32,
    /// Optional event bus for broadcasting state changes.
    event_bus: Option<Arc<EventBus>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(provider_name: impl Into<Arc<str>>, config: &CircuitBreakerConfig) -> Self {
        let initial_timeout_millis = config.open_timeout_secs * 1000;
        Self {
            provider_name: provider_name.into(),
            config: config.clone(),
            state_and_counter: AtomicU32::new(pack_state(STATE_CLOSED, 0)),
            opened_at: AtomicU64::new(0),
            current_timeout_millis: AtomicU64::new(initial_timeout_millis),
            consecutive_opens: AtomicU32::new(0),
            event_bus: None,
        }
    }

    /// Create a new circuit breaker with EventBus for real-time state change notifications.
    pub fn with_event_bus(
        provider_name: impl Into<Arc<str>>,
        config: &CircuitBreakerConfig,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let initial_timeout_millis = config.open_timeout_secs * 1000;
        Self {
            provider_name: provider_name.into(),
            config: config.clone(),
            state_and_counter: AtomicU32::new(pack_state(STATE_CLOSED, 0)),
            opened_at: AtomicU64::new(0),
            current_timeout_millis: AtomicU64::new(initial_timeout_millis),
            consecutive_opens: AtomicU32::new(0),
            event_bus: Some(event_bus),
        }
    }

    /// Check if a request is allowed through the circuit breaker.
    ///
    /// Returns `Ok(())` if the request can proceed, or `Err` if the circuit is open.
    pub fn check(&self) -> Result<(), CircuitBreakerError> {
        if !self.config.enabled {
            return Ok(());
        }

        let packed = self.state_and_counter.load(Ordering::Acquire);
        let (state, _) = unpack_state(packed);

        match state {
            STATE_CLOSED => Ok(()),
            STATE_OPEN => {
                // Check if timeout has elapsed (using adaptive timeout)
                let opened_at = self.opened_at.load(Ordering::Acquire);
                let now = current_time_millis();
                let timeout_millis = self.current_timeout_millis.load(Ordering::Acquire);

                if now >= opened_at + timeout_millis {
                    // Transition to half-open
                    self.transition_to_half_open();
                    Ok(())
                } else {
                    let retry_after = (opened_at + timeout_millis - now) / 1000;
                    Err(CircuitBreakerError::Open {
                        provider: self.provider_name.clone(),
                        retry_after_secs: retry_after,
                    })
                }
            }
            STATE_HALF_OPEN => {
                // Allow request through for testing
                Ok(())
            }
            _ => Ok(()), // Unknown state, allow through
        }
    }

    /// Record a successful request.
    pub fn record_success(&self) {
        if !self.config.enabled {
            return;
        }

        loop {
            let packed = self.state_and_counter.load(Ordering::Acquire);
            let (state, counter) = unpack_state(packed);

            match state {
                STATE_CLOSED => {
                    // Reset failure counter on success
                    if counter > 0 {
                        let new_packed = pack_state(STATE_CLOSED, 0);
                        if self
                            .state_and_counter
                            .compare_exchange_weak(
                                packed,
                                new_packed,
                                Ordering::Release,
                                Ordering::Relaxed,
                            )
                            .is_ok()
                        {
                            debug!(
                                provider = %self.provider_name,
                                "Circuit breaker: failure counter reset after success"
                            );
                            // Reset failure count metric
                            metrics::record_circuit_breaker_failures(
                                &self.provider_name,
                                0,
                                self.config.failure_threshold,
                            );
                        }
                        // If CAS failed, another thread modified state - that's fine, retry if needed
                    }
                    return;
                }
                STATE_HALF_OPEN => {
                    let new_counter = counter + 1;
                    if new_counter >= self.config.success_threshold {
                        // Enough successes, close the circuit
                        self.transition_to_closed();
                        return;
                    }
                    // Increment success counter atomically
                    let new_packed = pack_state(STATE_HALF_OPEN, new_counter);
                    if self
                        .state_and_counter
                        .compare_exchange_weak(
                            packed,
                            new_packed,
                            Ordering::Release,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                    {
                        debug!(
                            provider = %self.provider_name,
                            successes = new_counter,
                            threshold = self.config.success_threshold,
                            "Circuit breaker: successful probe"
                        );
                        return;
                    }
                    // CAS failed, retry
                    std::hint::spin_loop();
                }
                _ => return,
            }
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        if !self.config.enabled {
            return;
        }

        loop {
            let packed = self.state_and_counter.load(Ordering::Acquire);
            let (state, counter) = unpack_state(packed);

            match state {
                STATE_CLOSED => {
                    let new_counter = counter + 1;
                    if new_counter >= self.config.failure_threshold {
                        // Threshold exceeded, open the circuit
                        self.transition_to_open();
                        return;
                    }
                    // Increment failure counter atomically
                    let new_packed = pack_state(STATE_CLOSED, new_counter);
                    if self
                        .state_and_counter
                        .compare_exchange_weak(
                            packed,
                            new_packed,
                            Ordering::Release,
                            Ordering::Relaxed,
                        )
                        .is_ok()
                    {
                        debug!(
                            provider = %self.provider_name,
                            failures = new_counter,
                            threshold = self.config.failure_threshold,
                            "Circuit breaker: failure recorded"
                        );
                        // Track failure count approaching threshold
                        metrics::record_circuit_breaker_failures(
                            &self.provider_name,
                            new_counter,
                            self.config.failure_threshold,
                        );
                        return;
                    }
                    // CAS failed, retry
                    std::hint::spin_loop();
                }
                STATE_HALF_OPEN => {
                    // Any failure in half-open state reopens the circuit
                    self.transition_to_open();
                    return;
                }
                _ => return,
            }
        }
    }

    /// Get the current state of the circuit breaker.
    pub fn state(&self) -> CircuitState {
        if !self.config.enabled {
            return CircuitState::Closed;
        }

        let packed = self.state_and_counter.load(Ordering::Acquire);
        let (state, _) = unpack_state(packed);

        // Check for state transition on read (using adaptive timeout)
        if state == STATE_OPEN {
            let opened_at = self.opened_at.load(Ordering::Acquire);
            let now = current_time_millis();
            let timeout_millis = self.current_timeout_millis.load(Ordering::Acquire);

            if now >= opened_at + timeout_millis {
                return CircuitState::HalfOpen;
            }
        }

        match state {
            STATE_CLOSED => CircuitState::Closed,
            STATE_OPEN => CircuitState::Open,
            STATE_HALF_OPEN => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }

    /// Get the number of consecutive opens (for metrics/debugging).
    pub fn consecutive_opens(&self) -> u32 {
        self.consecutive_opens.load(Ordering::Acquire)
    }

    /// Get the current open timeout in seconds (for metrics/debugging).
    pub fn current_timeout_secs(&self) -> u64 {
        self.current_timeout_millis.load(Ordering::Acquire) / 1000
    }

    /// Get the failure count (for metrics/debugging).
    pub fn failure_count(&self) -> u32 {
        let packed = self.state_and_counter.load(Ordering::Acquire);
        let (state, counter) = unpack_state(packed);
        if state == STATE_CLOSED { counter } else { 0 }
    }

    fn transition_to_open(&self) {
        let previous_state = self.state();

        // Increment consecutive opens for adaptive backoff
        let consecutive = self.consecutive_opens.fetch_add(1, Ordering::AcqRel);

        // Calculate adaptive timeout based on consecutive opens
        let timeout_secs = self.config.calculate_open_timeout_secs(consecutive);
        let timeout_millis = timeout_secs * 1000;
        self.current_timeout_millis
            .store(timeout_millis, Ordering::Release);

        self.opened_at
            .store(current_time_millis(), Ordering::Release);
        self.state_and_counter
            .store(pack_state(STATE_OPEN, 0), Ordering::Release);

        warn!(
            provider = %self.provider_name,
            timeout_secs = timeout_secs,
            consecutive_opens = consecutive + 1,
            base_timeout_secs = self.config.open_timeout_secs,
            "Circuit breaker OPENED - provider marked unhealthy"
        );
        metrics::record_circuit_breaker_state(&self.provider_name, "open");
        metrics::record_circuit_breaker_consecutive_opens(&self.provider_name, consecutive + 1);
        // Reset failure count metric when circuit opens
        metrics::record_circuit_breaker_failures(
            &self.provider_name,
            0,
            self.config.failure_threshold,
        );
        // Publish state change event
        self.publish_state_change(previous_state, CircuitState::Open);
    }

    fn transition_to_half_open(&self) {
        let previous_state = self.state();
        self.state_and_counter
            .store(pack_state(STATE_HALF_OPEN, 0), Ordering::Release);
        info!(
            provider = %self.provider_name,
            "Circuit breaker HALF-OPEN - testing recovery"
        );
        metrics::record_circuit_breaker_state(&self.provider_name, "half_open");
        // Publish state change event
        self.publish_state_change(previous_state, CircuitState::HalfOpen);
    }

    fn transition_to_closed(&self) {
        let previous_state = self.state();

        // Reset adaptive backoff state on successful recovery
        let previous_consecutive = self.consecutive_opens.swap(0, Ordering::AcqRel);
        let initial_timeout_millis = self.config.open_timeout_secs * 1000;
        self.current_timeout_millis
            .store(initial_timeout_millis, Ordering::Release);

        self.state_and_counter
            .store(pack_state(STATE_CLOSED, 0), Ordering::Release);

        info!(
            provider = %self.provider_name,
            previous_consecutive_opens = previous_consecutive,
            "Circuit breaker CLOSED - provider recovered"
        );
        metrics::record_circuit_breaker_state(&self.provider_name, "closed");
        metrics::record_circuit_breaker_consecutive_opens(&self.provider_name, 0);
        // Reset failure count metric when circuit closes
        metrics::record_circuit_breaker_failures(
            &self.provider_name,
            0,
            self.config.failure_threshold,
        );
        // Publish state change event
        self.publish_state_change(previous_state, CircuitState::Closed);
    }

    /// Publish a state change event to the EventBus.
    fn publish_state_change(&self, previous_state: CircuitState, new_state: CircuitState) {
        if let Some(event_bus) = &self.event_bus {
            let packed = self.state_and_counter.load(Ordering::Acquire);
            let (_, counter) = unpack_state(packed);

            event_bus.publish(ServerEvent::CircuitBreakerStateChanged {
                provider: self.provider_name.to_string(),
                timestamp: Utc::now(),
                previous_state: circuit_state_to_event(previous_state),
                new_state: circuit_state_to_event(new_state),
                failure_count: counter,
                success_count: if new_state == CircuitState::Closed {
                    counter
                } else {
                    0
                },
            });
        }
    }
}

/// Convert local CircuitState to events module CircuitBreakerState.
fn circuit_state_to_event(state: CircuitState) -> EventCBState {
    match state {
        CircuitState::Closed => EventCBState::Closed,
        CircuitState::Open => EventCBState::Open,
        CircuitState::HalfOpen => EventCBState::HalfOpen,
    }
}

fn pack_state(state: u32, counter: u32) -> u32 {
    (state << STATE_SHIFT) | (counter & COUNTER_MASK)
}

fn unpack_state(packed: u32) -> (u32, u32) {
    let state = packed >> STATE_SHIFT;
    let counter = packed & COUNTER_MASK;
    (state, counter)
}

fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 3,
            open_timeout_secs: 1, // Short timeout for tests
            success_threshold: 2,
            failure_status_codes: vec![500, 502, 503, 504],
            backoff_multiplier: 2.0,
            max_open_timeout_secs: 300,
        }
    }

    #[test]
    fn test_disabled_circuit_breaker() {
        let config = CircuitBreakerConfig {
            enabled: false,
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", &config);

        // Should always allow requests
        assert!(breaker.check().is_ok());
        assert_eq!(breaker.state(), CircuitState::Closed);

        // Record many failures - should not affect state
        for _ in 0..100 {
            breaker.record_failure();
        }
        assert!(breaker.check().is_ok());
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_closed_state_allows_requests() {
        let breaker = CircuitBreaker::new("test", &test_config());

        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.check().is_ok());
    }

    #[test]
    fn test_failures_open_circuit() {
        let breaker = CircuitBreaker::new("test", &test_config());

        // Record failures up to threshold
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert_eq!(breaker.failure_count(), 1);

        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert_eq!(breaker.failure_count(), 2);

        breaker.record_failure(); // This should trip the circuit
        assert_eq!(breaker.state(), CircuitState::Open);

        // Should reject requests
        let result = breaker.check();
        assert!(result.is_err());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let breaker = CircuitBreaker::new("test", &test_config());

        breaker.record_failure();
        breaker.record_failure();
        assert_eq!(breaker.failure_count(), 2);

        breaker.record_success();
        assert_eq!(breaker.failure_count(), 0);
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_open_to_half_open_transition() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 1, // Short but non-zero timeout for test
            success_threshold: 1,
            failure_status_codes: vec![500],
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", &config);

        // Open the circuit
        breaker.record_failure();

        // State should be Open immediately after opening (check internal state directly)
        let packed = breaker.state_and_counter.load(Ordering::Acquire);
        let (state, _) = unpack_state(packed);
        assert_eq!(state, STATE_OPEN);

        // Wait for timeout to elapse
        std::thread::sleep(std::time::Duration::from_millis(1100));

        // Check should transition to half-open and allow request
        assert!(breaker.check().is_ok());
        // State should now be half-open
        let packed = breaker.state_and_counter.load(Ordering::Acquire);
        let (state, _) = unpack_state(packed);
        assert_eq!(state, STATE_HALF_OPEN);
    }

    #[test]
    fn test_half_open_success_closes_circuit() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 0,
            success_threshold: 2,
            failure_status_codes: vec![500],
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", &config);

        // Open and transition to half-open
        breaker.record_failure();
        std::thread::sleep(std::time::Duration::from_millis(10));
        breaker.check().unwrap(); // Transitions to half-open

        // Record successes
        breaker.record_success();
        let packed = breaker.state_and_counter.load(Ordering::Acquire);
        let (state, _) = unpack_state(packed);
        assert_eq!(state, STATE_HALF_OPEN);

        breaker.record_success(); // Should close circuit
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_failure_reopens_circuit() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 60, // Long timeout so state doesn't auto-transition
            success_threshold: 2,
            failure_status_codes: vec![500],
            ..Default::default()
        };
        let breaker = CircuitBreaker::new("test", &config);

        // Open the circuit
        breaker.record_failure();

        // Manually transition to half-open (simulate timeout elapsed)
        breaker
            .state_and_counter
            .store(pack_state(STATE_HALF_OPEN, 0), Ordering::Release);

        // Record one success then a failure
        breaker.record_success();
        breaker.record_failure(); // Should reopen circuit

        // Check internal state directly
        let packed = breaker.state_and_counter.load(Ordering::Acquire);
        let (state, _) = unpack_state(packed);
        assert_eq!(state, STATE_OPEN);
    }

    #[test]
    fn test_pack_unpack_state() {
        let packed = pack_state(STATE_OPEN, 42);
        let (state, counter) = unpack_state(packed);
        assert_eq!(state, STATE_OPEN);
        assert_eq!(counter, 42);

        let packed = pack_state(STATE_CLOSED, 0);
        let (state, counter) = unpack_state(packed);
        assert_eq!(state, STATE_CLOSED);
        assert_eq!(counter, 0);

        // Test max counter value
        let max_counter = COUNTER_MASK;
        let packed = pack_state(STATE_HALF_OPEN, max_counter);
        let (state, counter) = unpack_state(packed);
        assert_eq!(state, STATE_HALF_OPEN);
        assert_eq!(counter, max_counter);
    }

    #[test]
    fn test_is_failure_status() {
        let config = test_config();

        assert!(config.is_failure_status(500));
        assert!(config.is_failure_status(502));
        assert!(config.is_failure_status(503));
        assert!(config.is_failure_status(504));

        // 429 is NOT a failure status (rate limiting is expected)
        assert!(!config.is_failure_status(429));
        assert!(!config.is_failure_status(200));
        assert!(!config.is_failure_status(400));
        assert!(!config.is_failure_status(401));
    }

    #[test]
    fn test_concurrent_access() {
        use std::{sync::Arc, thread};

        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 100,
            open_timeout_secs: 30,
            success_threshold: 10,
            failure_status_codes: vec![500],
            ..Default::default()
        };
        let breaker = Arc::new(CircuitBreaker::new("test", &config));

        let mut handles = vec![];

        // Spawn threads that record failures
        for _ in 0..10 {
            let b = breaker.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    b.record_failure();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have opened the circuit (100 failures)
        assert_eq!(breaker.state(), CircuitState::Open);
    }

    #[test]
    fn test_adaptive_backoff_timeout_calculation() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 10,
            success_threshold: 1,
            failure_status_codes: vec![500],
            backoff_multiplier: 2.0,
            max_open_timeout_secs: 100,
        };

        // First open: 10s
        assert_eq!(config.calculate_open_timeout_secs(0), 10);
        // Second open: 10 * 2 = 20s
        assert_eq!(config.calculate_open_timeout_secs(1), 20);
        // Third open: 10 * 4 = 40s
        assert_eq!(config.calculate_open_timeout_secs(2), 40);
        // Fourth open: 10 * 8 = 80s
        assert_eq!(config.calculate_open_timeout_secs(3), 80);
        // Fifth open: 10 * 16 = 160s, capped at 100s
        assert_eq!(config.calculate_open_timeout_secs(4), 100);
        // Sixth open: still capped at 100s
        assert_eq!(config.calculate_open_timeout_secs(5), 100);
    }

    #[test]
    fn test_adaptive_backoff_disabled_when_multiplier_one() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 30,
            success_threshold: 1,
            failure_status_codes: vec![500],
            backoff_multiplier: 1.0, // Disables adaptive backoff
            max_open_timeout_secs: 300,
        };

        // All opens should use base timeout
        assert_eq!(config.calculate_open_timeout_secs(0), 30);
        assert_eq!(config.calculate_open_timeout_secs(1), 30);
        assert_eq!(config.calculate_open_timeout_secs(5), 30);
        assert_eq!(config.calculate_open_timeout_secs(10), 30);
    }

    #[test]
    fn test_consecutive_opens_increment_on_failure() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 60, // Long timeout so we can test state manually
            success_threshold: 1,
            failure_status_codes: vec![500],
            backoff_multiplier: 2.0,
            max_open_timeout_secs: 300,
        };
        let breaker = CircuitBreaker::new("test", &config);

        // Initially zero consecutive opens
        assert_eq!(breaker.consecutive_opens(), 0);
        assert_eq!(breaker.current_timeout_secs(), 60);

        // First failure opens circuit
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
        assert_eq!(breaker.consecutive_opens(), 1);
        assert_eq!(breaker.current_timeout_secs(), 60); // 60 * 2^0 = 60

        // Manually transition to half-open
        breaker
            .state_and_counter
            .store(pack_state(STATE_HALF_OPEN, 0), Ordering::Release);

        // Failure in half-open reopens with increased timeout
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
        assert_eq!(breaker.consecutive_opens(), 2);
        assert_eq!(breaker.current_timeout_secs(), 120); // 60 * 2^1 = 120

        // Manually transition to half-open again
        breaker
            .state_and_counter
            .store(pack_state(STATE_HALF_OPEN, 0), Ordering::Release);

        // Another failure
        breaker.record_failure();
        assert_eq!(breaker.consecutive_opens(), 3);
        assert_eq!(breaker.current_timeout_secs(), 240); // 60 * 2^2 = 240
    }

    #[test]
    fn test_consecutive_opens_reset_on_recovery() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 60,
            success_threshold: 1,
            failure_status_codes: vec![500],
            backoff_multiplier: 2.0,
            max_open_timeout_secs: 300,
        };
        let breaker = CircuitBreaker::new("test", &config);

        // Open circuit multiple times to build up consecutive opens
        breaker.record_failure();
        assert_eq!(breaker.consecutive_opens(), 1);

        // Manually transition to half-open and fail again
        breaker
            .state_and_counter
            .store(pack_state(STATE_HALF_OPEN, 0), Ordering::Release);
        breaker.record_failure();
        assert_eq!(breaker.consecutive_opens(), 2);
        assert_eq!(breaker.current_timeout_secs(), 120);

        // Manually transition to half-open
        breaker
            .state_and_counter
            .store(pack_state(STATE_HALF_OPEN, 0), Ordering::Release);

        // Successful recovery
        breaker.record_success();
        assert_eq!(breaker.state(), CircuitState::Closed);

        // Consecutive opens and timeout should be reset
        assert_eq!(breaker.consecutive_opens(), 0);
        assert_eq!(breaker.current_timeout_secs(), 60); // Back to base timeout

        // Next failure should start fresh
        breaker.record_failure();
        assert_eq!(breaker.consecutive_opens(), 1);
        assert_eq!(breaker.current_timeout_secs(), 60); // 60 * 2^0 = 60
    }

    #[test]
    fn test_adaptive_timeout_capped_at_max() {
        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 30,
            success_threshold: 1,
            failure_status_codes: vec![500],
            backoff_multiplier: 3.0, // Aggressive multiplier
            max_open_timeout_secs: 120,
        };
        let breaker = CircuitBreaker::new("test", &config);

        // Open circuit: 30s
        breaker.record_failure();
        assert_eq!(breaker.current_timeout_secs(), 30);

        // Half-open and fail: 30 * 3 = 90s
        breaker
            .state_and_counter
            .store(pack_state(STATE_HALF_OPEN, 0), Ordering::Release);
        breaker.record_failure();
        assert_eq!(breaker.current_timeout_secs(), 90);

        // Half-open and fail: 30 * 9 = 270s, capped at 120s
        breaker
            .state_and_counter
            .store(pack_state(STATE_HALF_OPEN, 0), Ordering::Release);
        breaker.record_failure();
        assert_eq!(breaker.current_timeout_secs(), 120);

        // Further failures stay capped
        breaker
            .state_and_counter
            .store(pack_state(STATE_HALF_OPEN, 0), Ordering::Release);
        breaker.record_failure();
        assert_eq!(breaker.current_timeout_secs(), 120);
    }
}
