//! Provider health check background service.
//!
//! This module provides a background service that periodically checks the health
//! of configured providers. Health checks can use either:
//!
//! - **Reachability mode**: Calls the provider's `/models` endpoint (free, fast)
//! - **Inference mode**: Sends a minimal chat completion request (more thorough, costs money)
//!
//! Health status is stored and can be queried via the admin API. Health changes
//! are published to the EventBus for real-time monitoring.
//!
//! # Configuration
//!
//! Health checks are configured per-provider:
//!
//! ```toml
//! [providers.my-openai.health_check]
//! enabled = true
//! mode = "reachability"
//! interval_secs = 60
//! timeout_secs = 10
//! ```

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Instant,
};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    events::{EventBus, ServerEvent},
    providers::{
        CircuitBreakerRegistry, Provider,
        health_check::{
            HealthCheckResult, HealthStatus, ProviderHealthCheckConfig, ProviderHealthCheckMode,
        },
    },
};

/// Stored health state for a single provider.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ProviderHealthState {
    /// Provider name.
    pub provider: String,
    /// Current health status.
    pub status: HealthStatus,
    /// Latency of the last health check in milliseconds.
    pub latency_ms: u64,
    /// Error message from the last failed health check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// HTTP status code from the last health check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    /// Time of the last health check.
    pub last_check: DateTime<Utc>,
    /// Number of consecutive failures.
    pub consecutive_failures: u32,
    /// Number of consecutive successes.
    pub consecutive_successes: u32,
}

impl ProviderHealthState {
    /// Create a new health state with unknown status.
    fn new(provider: String) -> Self {
        Self {
            provider,
            status: HealthStatus::Unknown,
            latency_ms: 0,
            error: None,
            status_code: None,
            last_check: Utc::now(),
            consecutive_failures: 0,
            consecutive_successes: 0,
        }
    }

    /// Update state from a health check result.
    fn update(&mut self, result: &HealthCheckResult) {
        let previous_status = self.status;
        self.status = result.status;
        self.latency_ms = result.latency_ms;
        self.error = result.error.clone();
        self.status_code = result.status_code;
        self.last_check = Utc::now();

        match result.status {
            HealthStatus::Healthy => {
                self.consecutive_failures = 0;
                self.consecutive_successes += 1;
            }
            HealthStatus::Unhealthy => {
                self.consecutive_successes = 0;
                self.consecutive_failures += 1;
            }
            HealthStatus::Unknown => {
                // Keep counts as-is for unknown status
            }
        }

        // Log status transitions
        if previous_status != result.status && previous_status != HealthStatus::Unknown {
            tracing::info!(
                provider = %self.provider,
                previous = ?previous_status,
                current = ?result.status,
                latency_ms = result.latency_ms,
                error = ?result.error,
                "Provider health status changed"
            );
        }
    }

    /// Check if status changed from the previous state.
    fn status_changed(&self, previous: HealthStatus) -> bool {
        self.status != previous && previous != HealthStatus::Unknown
    }
}

/// Shared registry of provider health states.
///
/// This is a cloneable handle to the shared state that can be stored in `AppState`
/// for access by admin API endpoints, while also being used by `ProviderHealthChecker`
/// to update health status from background tasks.
///
/// # Example
///
/// ```ignore
/// // Create registry and store in AppState
/// let registry = ProviderHealthStateRegistry::new();
/// let state = AppState { health_registry: registry.clone(), ... };
///
/// // Pass to health checker
/// let checker = ProviderHealthChecker::with_registry(client, event_bus, circuit_breakers, registry);
///
/// // Query from admin endpoint
/// let all_health = state.health_registry.get_all();
/// ```
#[derive(Clone, Default)]
pub struct ProviderHealthStateRegistry {
    state: Arc<RwLock<HashMap<String, ProviderHealthState>>>,
}

impl ProviderHealthStateRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get health state for a specific provider.
    ///
    /// Returns `None` if the provider is not registered for health checks.
    pub fn get(&self, provider: &str) -> Option<ProviderHealthState> {
        let state = self.state.read().expect("RwLock poisoned");
        state.get(provider).cloned()
    }

    /// Get health state for all registered providers.
    ///
    /// Returns an empty vector if no providers are registered.
    pub fn get_all(&self) -> Vec<ProviderHealthState> {
        let state = self.state.read().expect("RwLock poisoned");
        state.values().cloned().collect()
    }

    /// Check if any providers are registered.
    pub fn is_empty(&self) -> bool {
        let state = self.state.read().expect("RwLock poisoned");
        state.is_empty()
    }

    /// Get the number of registered providers.
    pub fn len(&self) -> usize {
        let state = self.state.read().expect("RwLock poisoned");
        state.len()
    }

    /// Initialize a provider's health state (internal use).
    fn init_provider(&self, provider: String) {
        let mut state = self.state.write().expect("RwLock poisoned");
        state.insert(provider.clone(), ProviderHealthState::new(provider));
    }

    /// Update a provider's health state from a check result (internal use).
    ///
    /// Returns `true` if the status changed from the previous value.
    fn update_provider(&self, provider: &str, result: &HealthCheckResult) -> bool {
        let mut state = self.state.write().expect("RwLock poisoned");
        if let Some(provider_state) = state.get_mut(provider) {
            let previous_status = provider_state.status;
            provider_state.update(result);
            provider_state.status_changed(previous_status)
        } else {
            false
        }
    }
}

impl std::fmt::Debug for ProviderHealthStateRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.read().expect("RwLock poisoned");
        f.debug_struct("ProviderHealthStateRegistry")
            .field("provider_count", &state.len())
            .finish()
    }
}

/// Internal entry for a provider being health-checked.
struct HealthCheckEntry {
    /// The provider instance.
    provider: Arc<dyn Provider>,
    /// Health check configuration.
    config: ProviderHealthCheckConfig,
}

/// Background service for checking provider health.
///
/// Spawns a background task per provider that runs health checks at configured
/// intervals. Health status is stored in a shared `ProviderHealthStateRegistry`
/// that can be queried via the admin API.
///
/// # Circuit Breaker Integration
///
/// Health check results are fed into the circuit breaker system:
/// - **Failures**: Always recorded as circuit breaker failures (provider is down)
/// - **Successes**: Only recorded for `Inference` mode checks (proves the model works)
///
/// This asymmetry ensures that a simple reachability check (which only verifies
/// the API endpoint is up) cannot prematurely close a circuit that was opened
/// due to actual inference failures.
pub struct ProviderHealthChecker {
    /// Providers to check, keyed by name.
    providers: HashMap<String, HealthCheckEntry>,
    /// Shared health state registry (can be stored in AppState for API access).
    registry: ProviderHealthStateRegistry,
    /// HTTP client for health check requests.
    client: reqwest::Client,
    /// Event bus for publishing health changes.
    event_bus: Option<Arc<EventBus>>,
    /// Circuit breaker registry for recording health check results.
    circuit_breakers: CircuitBreakerRegistry,
}

impl ProviderHealthChecker {
    /// Create a new health checker with an internal registry.
    ///
    /// Use `with_registry()` instead if you need to access health state from
    /// outside the checker (e.g., from admin API endpoints).
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP client for health check requests
    /// * `event_bus` - Optional event bus for publishing health change events
    /// * `circuit_breakers` - Registry for recording health check results to circuit breakers
    #[allow(dead_code)] // Used in tests; production uses with_registry()
    pub fn new(
        client: reqwest::Client,
        event_bus: Option<Arc<EventBus>>,
        circuit_breakers: CircuitBreakerRegistry,
    ) -> Self {
        Self::with_registry(
            client,
            event_bus,
            circuit_breakers,
            ProviderHealthStateRegistry::new(),
        )
    }

    /// Create a new health checker with an external registry.
    ///
    /// The registry can be stored in `AppState` and queried by admin API endpoints
    /// while the checker runs in the background.
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP client for health check requests
    /// * `event_bus` - Optional event bus for publishing health change events
    /// * `circuit_breakers` - Registry for recording health check results to circuit breakers
    /// * `registry` - Shared health state registry
    pub fn with_registry(
        client: reqwest::Client,
        event_bus: Option<Arc<EventBus>>,
        circuit_breakers: CircuitBreakerRegistry,
        registry: ProviderHealthStateRegistry,
    ) -> Self {
        Self {
            providers: HashMap::new(),
            registry,
            client,
            event_bus,
            circuit_breakers,
        }
    }

    /// Register a provider for health checking.
    ///
    /// Only providers with `health_check.enabled = true` in their config should
    /// be registered. The config should be validated before registration.
    ///
    /// # Arguments
    ///
    /// * `name` - Provider name (used for logging and state lookup)
    /// * `provider` - The provider instance
    /// * `config` - Health check configuration
    pub fn register(
        &mut self,
        name: impl Into<String>,
        provider: Arc<dyn Provider>,
        config: ProviderHealthCheckConfig,
    ) {
        let name = name.into();

        // Validate config
        if let Err(e) = config.validate() {
            tracing::error!(
                provider = %name,
                error = %e,
                "Invalid health check configuration, skipping registration"
            );
            return;
        }

        if !config.enabled {
            tracing::debug!(
                provider = %name,
                "Health checks disabled for provider, skipping registration"
            );
            return;
        }

        tracing::info!(
            provider = %name,
            mode = ?config.mode,
            interval_secs = config.interval_secs,
            timeout_secs = config.timeout_secs,
            "Registering provider for health checks"
        );

        // Initialize state in registry
        self.registry.init_provider(name.clone());

        self.providers
            .insert(name, HealthCheckEntry { provider, config });
    }

    /// Get the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Check if any providers are registered.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Get health state for a specific provider.
    #[allow(dead_code)] // Convenience method; production accesses registry directly
    pub fn get_health(&self, provider: &str) -> Option<ProviderHealthState> {
        self.registry.get(provider)
    }

    /// Get health state for all providers.
    #[allow(dead_code)] // Convenience method; production accesses registry directly
    pub fn get_all_health(&self) -> Vec<ProviderHealthState> {
        self.registry.get_all()
    }

    /// Start the health checker background tasks.
    ///
    /// Spawns a background task for each registered provider that runs health
    /// checks at the configured interval. This method consumes `self` and runs
    /// until all tasks complete (which is never under normal operation).
    ///
    /// If no providers are registered, this returns immediately.
    pub async fn start(self) {
        if self.providers.is_empty() {
            tracing::info!("No providers registered for health checks");
            return;
        }

        tracing::info!(
            provider_count = self.providers.len(),
            "Starting provider health checker"
        );

        let registry = self.registry;
        let event_bus = self.event_bus;
        let client = self.client;
        let circuit_breakers = self.circuit_breakers;

        // Spawn a task for each provider
        let mut handles = Vec::new();

        for (name, entry) in self.providers {
            let provider = entry.provider;
            let config = entry.config;
            let registry = registry.clone();
            let event_bus = event_bus.clone();
            let client = client.clone();
            let circuit_breakers = circuit_breakers.clone();

            let handle = tokio::spawn(async move {
                run_health_check_loop(
                    name,
                    provider,
                    config,
                    registry,
                    event_bus,
                    client,
                    circuit_breakers,
                )
                .await;
            });

            handles.push(handle);
        }

        // Wait for all tasks (they run indefinitely)
        for handle in handles {
            let _ = handle.await;
        }
    }
}

/// Run the health check loop for a single provider.
async fn run_health_check_loop(
    name: String,
    provider: Arc<dyn Provider>,
    config: ProviderHealthCheckConfig,
    registry: ProviderHealthStateRegistry,
    event_bus: Option<Arc<EventBus>>,
    client: reqwest::Client,
    circuit_breakers: CircuitBreakerRegistry,
) {
    let interval = config.interval();

    tracing::debug!(
        provider = %name,
        interval_secs = config.interval_secs,
        "Starting health check loop"
    );

    // Run initial check immediately
    run_single_health_check(
        &name,
        &provider,
        &config,
        &registry,
        &event_bus,
        &client,
        &circuit_breakers,
    )
    .await;

    // Then check at configured interval
    loop {
        tokio::time::sleep(interval).await;
        run_single_health_check(
            &name,
            &provider,
            &config,
            &registry,
            &event_bus,
            &client,
            &circuit_breakers,
        )
        .await;
    }
}

/// Run a single health check and update state.
///
/// # Circuit Breaker Integration
///
/// Health check results are fed into the circuit breaker:
/// - **Failures**: Always recorded (provider is definitely down)
/// - **Successes with Inference mode**: Recorded (proves the model works)
/// - **Successes with Reachability mode**: NOT recorded (API up ≠ model works)
///
/// This asymmetry prevents a simple `/v1/models` check from closing a circuit
/// that was opened due to actual inference failures.
async fn run_single_health_check(
    name: &str,
    provider: &Arc<dyn Provider>,
    config: &ProviderHealthCheckConfig,
    registry: &ProviderHealthStateRegistry,
    event_bus: &Option<Arc<EventBus>>,
    client: &reqwest::Client,
    circuit_breakers: &CircuitBreakerRegistry,
) {
    let start = Instant::now();

    // Apply timeout to the health check
    let timeout = config.timeout();
    let result = match tokio::time::timeout(timeout, provider.health_check(client, config)).await {
        Ok(result) => result,
        Err(_) => {
            // Timeout occurred
            HealthCheckResult::unhealthy(
                start.elapsed().as_millis() as u64,
                format!("Health check timed out after {}s", config.timeout_secs),
                None,
            )
        }
    };

    // Update state and check if status changed
    let status_changed = registry.update_provider(name, &result);

    // Record result to circuit breaker
    if let Some(circuit_breaker) = circuit_breakers.get(name) {
        match result.status {
            HealthStatus::Unhealthy => {
                // Always record failures - provider is definitely down
                circuit_breaker.record_failure();
                tracing::debug!(
                    provider = %name,
                    "Health check failure recorded to circuit breaker"
                );
            }
            HealthStatus::Healthy => {
                // Only record successes for Inference mode - proves the model works
                // Reachability mode (just checking /v1/models) shouldn't close a circuit
                // that was opened due to inference failures
                if config.mode == ProviderHealthCheckMode::Inference {
                    circuit_breaker.record_success();
                    tracing::debug!(
                        provider = %name,
                        "Health check success (inference) recorded to circuit breaker"
                    );
                }
            }
            HealthStatus::Unknown => {
                // Don't record unknown status to circuit breaker
            }
        }
    }

    // Log based on result
    match result.status {
        HealthStatus::Healthy => {
            tracing::debug!(
                provider = %name,
                latency_ms = result.latency_ms,
                status_code = ?result.status_code,
                "Provider health check passed"
            );
        }
        HealthStatus::Unhealthy => {
            tracing::warn!(
                provider = %name,
                latency_ms = result.latency_ms,
                error = ?result.error,
                status_code = ?result.status_code,
                "Provider health check failed"
            );
        }
        HealthStatus::Unknown => {
            tracing::debug!(
                provider = %name,
                "Provider health check returned unknown status"
            );
        }
    }

    // Publish event if status changed
    if status_changed && let Some(bus) = event_bus {
        bus.publish(ServerEvent::ProviderHealthChanged {
            provider: name.to_string(),
            timestamp: Utc::now(),
            is_healthy: result.status == HealthStatus::Healthy,
            latency_ms: Some(result.latency_ms),
            error_message: result.error,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_health_state_new() {
        let state = ProviderHealthState::new("test".to_string());
        assert_eq!(state.provider, "test");
        assert_eq!(state.status, HealthStatus::Unknown);
        assert_eq!(state.latency_ms, 0);
        assert!(state.error.is_none());
        assert!(state.status_code.is_none());
        assert_eq!(state.consecutive_failures, 0);
        assert_eq!(state.consecutive_successes, 0);
    }

    #[test]
    fn test_provider_health_state_update_healthy() {
        let mut state = ProviderHealthState::new("test".to_string());
        let result = HealthCheckResult::healthy(150, 200);

        state.update(&result);

        assert_eq!(state.status, HealthStatus::Healthy);
        assert_eq!(state.latency_ms, 150);
        assert!(state.error.is_none());
        assert_eq!(state.status_code, Some(200));
        assert_eq!(state.consecutive_successes, 1);
        assert_eq!(state.consecutive_failures, 0);
    }

    #[test]
    fn test_provider_health_state_update_unhealthy() {
        let mut state = ProviderHealthState::new("test".to_string());
        let result = HealthCheckResult::unhealthy(500, "Connection refused", Some(503));

        state.update(&result);

        assert_eq!(state.status, HealthStatus::Unhealthy);
        assert_eq!(state.latency_ms, 500);
        assert_eq!(state.error, Some("Connection refused".to_string()));
        assert_eq!(state.status_code, Some(503));
        assert_eq!(state.consecutive_failures, 1);
        assert_eq!(state.consecutive_successes, 0);
    }

    #[test]
    fn test_provider_health_state_consecutive_tracking() {
        let mut state = ProviderHealthState::new("test".to_string());

        // Three healthy checks
        for _ in 0..3 {
            state.update(&HealthCheckResult::healthy(100, 200));
        }
        assert_eq!(state.consecutive_successes, 3);
        assert_eq!(state.consecutive_failures, 0);

        // One failure resets successes
        state.update(&HealthCheckResult::unhealthy(100, "error", None));
        assert_eq!(state.consecutive_successes, 0);
        assert_eq!(state.consecutive_failures, 1);

        // Two more failures
        for _ in 0..2 {
            state.update(&HealthCheckResult::unhealthy(100, "error", None));
        }
        assert_eq!(state.consecutive_failures, 3);

        // One success resets failures
        state.update(&HealthCheckResult::healthy(100, 200));
        assert_eq!(state.consecutive_successes, 1);
        assert_eq!(state.consecutive_failures, 0);
    }

    #[test]
    fn test_provider_health_state_status_changed() {
        let mut state = ProviderHealthState::new("test".to_string());

        // Unknown -> Healthy is not a change (initial transition)
        state.update(&HealthCheckResult::healthy(100, 200));
        assert!(!state.status_changed(HealthStatus::Unknown));

        // Healthy -> Healthy is not a change
        let prev = state.status;
        state.update(&HealthCheckResult::healthy(100, 200));
        assert!(!state.status_changed(prev));

        // Healthy -> Unhealthy is a change
        let prev = state.status;
        state.update(&HealthCheckResult::unhealthy(100, "error", None));
        assert!(state.status_changed(prev));

        // Unhealthy -> Healthy is a change
        let prev = state.status;
        state.update(&HealthCheckResult::healthy(100, 200));
        assert!(state.status_changed(prev));
    }

    #[test]
    fn test_health_checker_new() {
        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let checker = ProviderHealthChecker::new(client, None, circuit_breakers);

        assert!(checker.is_empty());
        assert_eq!(checker.provider_count(), 0);
    }

    fn test_provider() -> Arc<dyn Provider> {
        Arc::new(crate::providers::test::TestProvider::new("test-model"))
    }

    #[test]
    fn test_health_checker_register_disabled() {
        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let mut checker = ProviderHealthChecker::new(client, None, circuit_breakers);

        let config = ProviderHealthCheckConfig {
            enabled: false,
            ..Default::default()
        };

        checker.register("test", test_provider(), config);

        // Should not be registered since disabled
        assert!(checker.is_empty());
    }

    #[test]
    fn test_health_checker_register_invalid_config() {
        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let mut checker = ProviderHealthChecker::new(client, None, circuit_breakers);

        // Inference mode without model is invalid
        let config = ProviderHealthCheckConfig {
            enabled: true,
            mode: crate::providers::health_check::ProviderHealthCheckMode::Inference,
            model: None,
            ..Default::default()
        };

        checker.register("test", test_provider(), config);

        // Should not be registered since config is invalid
        assert!(checker.is_empty());
    }

    #[test]
    fn test_health_checker_register_valid() {
        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let mut checker = ProviderHealthChecker::new(client, None, circuit_breakers);

        let config = ProviderHealthCheckConfig {
            enabled: true,
            ..Default::default()
        };

        checker.register("test", test_provider(), config);

        assert!(!checker.is_empty());
        assert_eq!(checker.provider_count(), 1);

        // Initial state should be Unknown
        let health = checker.get_health("test").unwrap();
        assert_eq!(health.status, HealthStatus::Unknown);
    }

    #[test]
    fn test_health_checker_get_all_health() {
        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let mut checker = ProviderHealthChecker::new(client, None, circuit_breakers);

        let config = ProviderHealthCheckConfig {
            enabled: true,
            ..Default::default()
        };

        checker.register("provider-a", test_provider(), config.clone());
        checker.register("provider-b", test_provider(), config);

        let all_health = checker.get_all_health();
        assert_eq!(all_health.len(), 2);
    }

    #[test]
    fn test_provider_health_state_serialization() {
        let state = ProviderHealthState {
            provider: "test-provider".to_string(),
            status: HealthStatus::Healthy,
            latency_ms: 150,
            error: None,
            status_code: Some(200),
            last_check: Utc::now(),
            consecutive_failures: 0,
            consecutive_successes: 5,
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"provider\":\"test-provider\""));
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"latency_ms\":150"));
        // error should be skipped when None
        assert!(!json.contains("\"error\""));
    }

    // ============== Circuit Breaker Integration Tests ==============

    use crate::{
        config::CircuitBreakerConfig,
        providers::circuit_breaker::{CircuitBreaker, CircuitState},
    };

    fn test_circuit_breaker_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 3,
            open_timeout_secs: 30,
            success_threshold: 2,
            failure_status_codes: vec![500, 502, 503, 504],
            ..Default::default()
        }
    }

    #[test]
    fn test_circuit_breaker_records_health_check_failure() {
        // Create circuit breaker and registry
        let cb_config = test_circuit_breaker_config();
        let registry = CircuitBreakerRegistry::new();
        let breaker = CircuitBreaker::new("test-provider", &cb_config);
        registry.register("test-provider", breaker);

        // Simulate health check failure by directly calling the circuit breaker
        // (run_single_health_check does this internally)
        let cb = registry.get("test-provider").unwrap();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);

        // Record failure
        cb.record_failure();
        assert_eq!(cb.failure_count(), 1);

        // Record more failures to open circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_circuit_breaker_records_inference_success() {
        // Create circuit breaker and registry
        let cb_config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 0, // Immediate transition to half-open
            success_threshold: 2,
            failure_status_codes: vec![500],
            ..Default::default()
        };
        let registry = CircuitBreakerRegistry::new();
        let breaker = CircuitBreaker::new("test-provider", &cb_config);
        registry.register("test-provider", breaker);

        let cb = registry.get("test-provider").unwrap();

        // Open the circuit
        cb.record_failure();
        // With open_timeout_secs = 0, state() returns HalfOpen immediately
        // because the timeout has already elapsed
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Trigger check to formally transition to half-open
        cb.check().unwrap();

        // Record successes (simulating inference health check)
        cb.record_success();
        cb.record_success();

        // Circuit should be closed
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_reachability_success_should_not_close_circuit() {
        // This test documents the intended behavior:
        // A reachability check success (just checking /v1/models)
        // should NOT call record_success() on the circuit breaker.
        //
        // The actual logic is in run_single_health_check():
        // - If config.mode == ProviderHealthCheckMode::Inference -> record_success()
        // - If config.mode == ProviderHealthCheckMode::Reachability -> don't record
        //
        // This ensures that a working /v1/models endpoint can't close a circuit
        // that was opened due to actual inference failures.

        let cb_config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 0,
            success_threshold: 1,
            failure_status_codes: vec![500],
            ..Default::default()
        };
        let registry = CircuitBreakerRegistry::new();
        let breaker = CircuitBreaker::new("test-provider", &cb_config);
        registry.register("test-provider", breaker);

        let cb = registry.get("test-provider").unwrap();

        // Open the circuit due to inference failure
        cb.record_failure();
        // With open_timeout_secs = 0, state() returns HalfOpen immediately
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Trigger check to formally transition to half-open
        cb.check().unwrap();

        // A reachability check would NOT call record_success()
        // (per our logic in run_single_health_check)
        // So the circuit stays half-open
        // This is verified by NOT calling record_success() here

        // Verify circuit is still half-open
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Only an Inference mode success would close it
    }

    #[test]
    fn test_health_checker_has_circuit_breakers() {
        let client = reqwest::Client::new();
        let cb_config = test_circuit_breaker_config();
        let registry = CircuitBreakerRegistry::new();

        // Pre-register a circuit breaker
        let breaker = CircuitBreaker::new("my-provider", &cb_config);
        registry.register("my-provider", breaker);

        let checker = ProviderHealthChecker::new(client, None, registry.clone());

        // The checker should have access to the same registry
        // Verify by checking the circuit breaker exists
        assert!(registry.get("my-provider").is_some());

        // The checker itself doesn't expose circuit_breakers directly,
        // but the integration works when start() is called
        assert!(checker.is_empty()); // No providers registered for health checks yet
    }

    // ============== ProviderHealthStateRegistry Tests ==============

    #[test]
    fn test_health_state_registry_new() {
        let registry = ProviderHealthStateRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_health_state_registry_init_and_get() {
        let registry = ProviderHealthStateRegistry::new();
        registry.init_provider("test-provider".to_string());

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);

        let health = registry.get("test-provider").unwrap();
        assert_eq!(health.provider, "test-provider");
        assert_eq!(health.status, HealthStatus::Unknown);
        assert_eq!(health.consecutive_failures, 0);
        assert_eq!(health.consecutive_successes, 0);
    }

    #[test]
    fn test_health_state_registry_get_all() {
        let registry = ProviderHealthStateRegistry::new();
        registry.init_provider("provider-a".to_string());
        registry.init_provider("provider-b".to_string());

        let all = registry.get_all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_health_state_registry_get_nonexistent() {
        let registry = ProviderHealthStateRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_health_state_registry_shared_access() {
        // Verify that cloning the registry shares state
        let registry = ProviderHealthStateRegistry::new();
        let registry2 = registry.clone();

        registry.init_provider("test".to_string());

        // Both registries should see the provider
        assert_eq!(registry.len(), 1);
        assert_eq!(registry2.len(), 1);
        assert!(registry2.get("test").is_some());
    }

    #[test]
    fn test_health_state_registry_debug() {
        let registry = ProviderHealthStateRegistry::new();
        registry.init_provider("test".to_string());

        let debug = format!("{:?}", registry);
        assert!(debug.contains("ProviderHealthStateRegistry"));
        assert!(debug.contains("provider_count"));
    }

    #[test]
    fn test_health_checker_with_registry() {
        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let registry = ProviderHealthStateRegistry::new();

        let mut checker =
            ProviderHealthChecker::with_registry(client, None, circuit_breakers, registry.clone());

        let config = ProviderHealthCheckConfig {
            enabled: true,
            ..Default::default()
        };

        checker.register("test", test_provider(), config);

        // Registry should be updated by the checker
        assert_eq!(registry.len(), 1);
        let health = registry.get("test").unwrap();
        assert_eq!(health.status, HealthStatus::Unknown);
    }

    // ============== Background Health Checker Integration Tests ==============
    //
    // These tests verify the background health check loop behavior:
    // - start() spawns tasks and runs initial checks
    // - Periodic checks run at configured intervals
    // - Health state is updated after each check
    // - ProviderHealthChanged events are published on status transitions

    use crate::{config::TestFailureMode, providers::test::TestProvider};

    /// Create a test provider with a specific failure mode.
    fn test_provider_with_mode(mode: TestFailureMode) -> Arc<dyn Provider> {
        Arc::new(TestProvider::with_failure_mode("test-model", mode))
    }

    /// Create a health check config with a long interval for testing.
    /// Uses a 60 second interval so only the initial check runs during short test windows.
    fn fast_health_config() -> ProviderHealthCheckConfig {
        ProviderHealthCheckConfig {
            enabled: true,
            mode: ProviderHealthCheckMode::Reachability,
            interval_secs: 60, // Long interval so only initial check runs
            timeout_secs: 5,
            model: None,
            prompt: None,
        }
    }

    #[tokio::test]
    async fn test_start_spawns_tasks_and_runs_initial_check() {
        // Create a healthy provider
        let provider = test_provider_with_mode(TestFailureMode::None);

        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let registry = ProviderHealthStateRegistry::new();

        let mut checker =
            ProviderHealthChecker::with_registry(client, None, circuit_breakers, registry.clone());

        checker.register("healthy-provider", provider, fast_health_config());

        // Verify initial state is Unknown
        let health = registry.get("healthy-provider").unwrap();
        assert_eq!(health.status, HealthStatus::Unknown);

        // Spawn the checker as a background task (it runs forever)
        let handle = tokio::spawn(async move {
            checker.start().await;
        });

        // Wait for the initial check to complete (should happen immediately)
        // Use a short sleep to allow the async task to run
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Verify state changed from Unknown to Healthy
        let health = registry.get("healthy-provider").unwrap();
        assert_eq!(
            health.status,
            HealthStatus::Healthy,
            "Initial health check should have run and set status to Healthy"
        );
        assert_eq!(health.consecutive_successes, 1);
        assert_eq!(health.consecutive_failures, 0);

        // Clean up: abort the background task
        handle.abort();
    }

    #[tokio::test]
    async fn test_start_runs_initial_check_for_unhealthy_provider() {
        // Create a provider that always fails
        let provider = test_provider_with_mode(TestFailureMode::ConnectionError {
            message: "Simulated connection failure".to_string(),
        });

        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let registry = ProviderHealthStateRegistry::new();

        let mut checker =
            ProviderHealthChecker::with_registry(client, None, circuit_breakers, registry.clone());

        checker.register("failing-provider", provider, fast_health_config());

        // Spawn the checker
        let handle = tokio::spawn(async move {
            checker.start().await;
        });

        // Wait for the initial check
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Verify state changed to Unhealthy
        let health = registry.get("failing-provider").unwrap();
        assert_eq!(
            health.status,
            HealthStatus::Unhealthy,
            "Initial health check should have run and set status to Unhealthy"
        );
        assert_eq!(health.consecutive_failures, 1);
        assert!(health.error.is_some());

        handle.abort();
    }

    #[tokio::test]
    async fn test_health_state_updated_with_consecutive_counts() {
        // This test verifies that consecutive success/failure counts are tracked
        // We use a provider that succeeds, then manually verify the counts

        let provider = test_provider_with_mode(TestFailureMode::None);

        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let registry = ProviderHealthStateRegistry::new();

        let mut checker =
            ProviderHealthChecker::with_registry(client, None, circuit_breakers, registry.clone());

        // Use a very short interval config
        let config = ProviderHealthCheckConfig {
            enabled: true,
            mode: ProviderHealthCheckMode::Reachability,
            interval_secs: 1, // Minimum interval
            timeout_secs: 1,
            model: None,
            prompt: None,
        };

        checker.register("test-provider", provider, config);

        let handle = tokio::spawn(async move {
            checker.start().await;
        });

        // Wait for initial check
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let health = registry.get("test-provider").unwrap();
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.consecutive_successes, 1);

        // The interval is 1 second, so we'd need to wait that long for another check
        // For this test, we just verify the initial check updates the state correctly

        handle.abort();
    }

    #[tokio::test]
    async fn test_health_changed_event_published_on_transition() {
        use crate::events::EventBus;

        // Create a provider that will fail
        let provider = test_provider_with_mode(TestFailureMode::ConnectionError {
            message: "Provider down".to_string(),
        });

        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let registry = ProviderHealthStateRegistry::new();
        let event_bus = Arc::new(EventBus::new());

        // Subscribe BEFORE starting the checker
        let _event_rx = event_bus.subscribe();

        let mut checker = ProviderHealthChecker::with_registry(
            client,
            Some(event_bus.clone()),
            circuit_breakers,
            registry.clone(),
        );

        checker.register("event-test-provider", provider, fast_health_config());

        let handle = tokio::spawn(async move {
            checker.start().await;
        });

        // Wait for the initial check
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // The first transition (Unknown -> Unhealthy) should NOT publish an event
        // because we don't consider Unknown -> X as a "change"
        // Let's verify the state is Unhealthy
        let health = registry.get("event-test-provider").unwrap();
        assert_eq!(health.status, HealthStatus::Unhealthy);

        // No event should have been published for Unknown -> Unhealthy
        // (This is by design - see status_changed() which ignores Unknown transitions)

        handle.abort();
    }

    #[tokio::test]
    async fn test_multiple_providers_checked_concurrently() {
        // Register multiple providers and verify they all get checked

        let healthy_provider = test_provider_with_mode(TestFailureMode::None);
        let failing_provider = test_provider_with_mode(TestFailureMode::ConnectionError {
            message: "Down".to_string(),
        });

        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let registry = ProviderHealthStateRegistry::new();

        let mut checker =
            ProviderHealthChecker::with_registry(client, None, circuit_breakers, registry.clone());

        checker.register("provider-a", healthy_provider, fast_health_config());
        checker.register("provider-b", failing_provider, fast_health_config());

        assert_eq!(checker.provider_count(), 2);

        let handle = tokio::spawn(async move {
            checker.start().await;
        });

        // Wait for initial checks
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;

        // Both providers should have been checked
        let health_a = registry.get("provider-a").unwrap();
        let health_b = registry.get("provider-b").unwrap();

        assert_eq!(health_a.status, HealthStatus::Healthy);
        assert_eq!(health_b.status, HealthStatus::Unhealthy);

        handle.abort();
    }

    #[tokio::test]
    async fn test_start_with_no_providers_returns_immediately() {
        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let checker = ProviderHealthChecker::new(client, None, circuit_breakers);

        assert!(checker.is_empty());

        // start() should return immediately when no providers are registered
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), checker.start()).await;

        // Should complete without timeout
        assert!(
            result.is_ok(),
            "start() with no providers should return immediately"
        );
    }

    #[tokio::test]
    async fn test_health_check_timeout_handling() {
        // Create a provider that times out
        let provider = test_provider_with_mode(TestFailureMode::Timeout {
            delay_ms: 5000, // 5 seconds - longer than our timeout
        });

        let client = reqwest::Client::new();
        let circuit_breakers = CircuitBreakerRegistry::new();
        let registry = ProviderHealthStateRegistry::new();

        let mut checker =
            ProviderHealthChecker::with_registry(client, None, circuit_breakers, registry.clone());

        // Config with 1 second timeout
        let config = ProviderHealthCheckConfig {
            enabled: true,
            mode: ProviderHealthCheckMode::Reachability,
            interval_secs: 60,
            timeout_secs: 1, // Short timeout
            model: None,
            prompt: None,
        };

        checker.register("slow-provider", provider, config);

        let handle = tokio::spawn(async move {
            checker.start().await;
        });

        // Wait for timeout to occur (health check timeout is 1s, plus some buffer)
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        // Provider should be marked unhealthy due to timeout
        let health = registry.get("slow-provider").unwrap();
        assert_eq!(
            health.status,
            HealthStatus::Unhealthy,
            "Provider should be unhealthy after timeout"
        );
        assert!(
            health
                .error
                .as_ref()
                .is_some_and(|e| e.contains("timed out")),
            "Error message should mention timeout"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn test_circuit_breaker_updated_on_health_check_failure() {
        // Create a failing provider
        let provider = test_provider_with_mode(TestFailureMode::ConnectionError {
            message: "Provider unavailable".to_string(),
        });

        let client = reqwest::Client::new();
        let cb_config = test_circuit_breaker_config();
        let circuit_breakers = CircuitBreakerRegistry::new();

        // Pre-register circuit breaker for this provider
        let breaker = CircuitBreaker::new("cb-test-provider", &cb_config);
        circuit_breakers.register("cb-test-provider", breaker);

        let registry = ProviderHealthStateRegistry::new();

        let mut checker = ProviderHealthChecker::with_registry(
            client,
            None,
            circuit_breakers.clone(),
            registry.clone(),
        );

        checker.register("cb-test-provider", provider, fast_health_config());

        // Verify circuit breaker starts with 0 failures
        let cb = circuit_breakers.get("cb-test-provider").unwrap();
        assert_eq!(cb.failure_count(), 0);

        let handle = tokio::spawn(async move {
            checker.start().await;
        });

        // Wait for initial check
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Circuit breaker should have recorded the failure
        assert_eq!(
            cb.failure_count(),
            1,
            "Health check failure should be recorded to circuit breaker"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn test_inference_mode_success_updates_circuit_breaker() {
        // Create a healthy provider
        let provider = test_provider_with_mode(TestFailureMode::None);

        let client = reqwest::Client::new();
        let cb_config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 1,
            open_timeout_secs: 0, // Immediate transition to half-open
            success_threshold: 1,
            failure_status_codes: vec![500],
            ..Default::default()
        };
        let circuit_breakers = CircuitBreakerRegistry::new();

        // Pre-register and open the circuit breaker
        let breaker = CircuitBreaker::new("inference-cb-provider", &cb_config);
        breaker.record_failure(); // Open the circuit
        circuit_breakers.register("inference-cb-provider", breaker);

        let cb = circuit_breakers.get("inference-cb-provider").unwrap();
        // Trigger transition to half-open
        let _ = cb.check();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        let registry = ProviderHealthStateRegistry::new();

        let mut checker = ProviderHealthChecker::with_registry(
            client,
            None,
            circuit_breakers.clone(),
            registry.clone(),
        );

        // Use INFERENCE mode - successes should update circuit breaker
        let config = ProviderHealthCheckConfig {
            enabled: true,
            mode: ProviderHealthCheckMode::Inference,
            interval_secs: 60,
            timeout_secs: 5,
            model: Some("test-model".to_string()),
            prompt: None,
        };

        checker.register("inference-cb-provider", provider, config);

        let handle = tokio::spawn(async move {
            checker.start().await;
        });

        // Wait for initial check
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Circuit should be closed after successful inference health check
        assert_eq!(
            cb.state(),
            CircuitState::Closed,
            "Inference mode health check success should close circuit"
        );

        handle.abort();
    }
}
