//! Provider registry for managing shared circuit breakers.
//!
//! Circuit breakers need to persist across requests to track failures
//! and protect against unhealthy providers. This module provides a
//! registry that stores circuit breakers keyed by provider name.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serde::Serialize;

use super::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::{
    config::{CircuitBreakerConfig, ProvidersConfig},
    events::EventBus,
};

/// Registry for managing circuit breakers across providers.
///
/// Circuit breakers are created lazily on first access or eagerly from
/// configuration. The registry is thread-safe and can be cloned cheaply.
#[derive(Clone, Default)]
pub struct CircuitBreakerRegistry {
    breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
    event_bus: Option<Arc<EventBus>>,
}

impl CircuitBreakerRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            event_bus: None,
        }
    }

    /// Create a registry pre-populated from provider configuration.
    pub fn from_config(providers: &ProvidersConfig) -> Self {
        let registry = Self::new();

        for (name, config) in providers.iter() {
            let cb_config = config.circuit_breaker_config();
            if cb_config.enabled {
                let breaker = CircuitBreaker::new(name, cb_config);
                registry.register(name, breaker);
            }
        }

        registry
    }

    /// Create a registry pre-populated from provider configuration with EventBus.
    pub fn from_config_with_event_bus(
        providers: &ProvidersConfig,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let registry = Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            event_bus: Some(event_bus.clone()),
        };

        for (name, config) in providers.iter() {
            let cb_config = config.circuit_breaker_config();
            if cb_config.enabled {
                let breaker = CircuitBreaker::with_event_bus(name, cb_config, event_bus.clone());
                registry.register(name, breaker);
            }
        }

        registry
    }

    /// Register a circuit breaker for a provider.
    pub fn register(&self, provider_name: &str, breaker: CircuitBreaker) {
        let mut breakers = self.breakers.write().expect("RwLock poisoned");
        breakers.insert(provider_name.to_string(), Arc::new(breaker));
    }

    /// Get or create a circuit breaker for a provider.
    ///
    /// If the circuit breaker doesn't exist and config has it enabled,
    /// creates one. Returns None if circuit breaker is disabled.
    pub fn get_or_create(
        &self,
        provider_name: &str,
        config: &CircuitBreakerConfig,
    ) -> Option<Arc<CircuitBreaker>> {
        if !config.enabled {
            return None;
        }

        // Try read lock first
        {
            let breakers = self.breakers.read().expect("RwLock poisoned");
            if let Some(breaker) = breakers.get(provider_name) {
                return Some(breaker.clone());
            }
        }

        // Need to create - upgrade to write lock
        let mut breakers = self.breakers.write().expect("RwLock poisoned");
        // Double-check after acquiring write lock
        if let Some(breaker) = breakers.get(provider_name) {
            return Some(breaker.clone());
        }

        let breaker = if let Some(event_bus) = &self.event_bus {
            Arc::new(CircuitBreaker::with_event_bus(
                provider_name,
                config,
                event_bus.clone(),
            ))
        } else {
            Arc::new(CircuitBreaker::new(provider_name, config))
        };
        breakers.insert(provider_name.to_string(), breaker.clone());
        Some(breaker)
    }

    /// Get a circuit breaker by name if it exists.
    pub fn get(&self, provider_name: &str) -> Option<Arc<CircuitBreaker>> {
        let breakers = self.breakers.read().expect("RwLock poisoned");
        breakers.get(provider_name).cloned()
    }

    /// Get the status of all circuit breakers.
    pub fn status(&self) -> Vec<CircuitBreakerStatus> {
        let breakers = self.breakers.read().expect("RwLock poisoned");
        breakers
            .iter()
            .map(
                |(name, breaker): (&String, &Arc<CircuitBreaker>)| CircuitBreakerStatus {
                    provider: name.clone(),
                    state: breaker.state(),
                    failure_count: breaker.failure_count(),
                },
            )
            .collect()
    }

    /// Get the status of a specific circuit breaker.
    pub fn status_for(&self, provider_name: &str) -> Option<CircuitBreakerStatus> {
        let breakers = self.breakers.read().expect("RwLock poisoned");
        breakers
            .get(provider_name)
            .map(|breaker: &Arc<CircuitBreaker>| CircuitBreakerStatus {
                provider: provider_name.to_string(),
                state: breaker.state(),
                failure_count: breaker.failure_count(),
            })
    }
}

/// Status of a circuit breaker for API responses.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct CircuitBreakerStatus {
    /// Provider name.
    pub provider: String,
    /// Current state (closed, open, or half_open).
    #[cfg_attr(feature = "utoipa", schema(example = "closed"))]
    pub state: CircuitState,
    /// Number of consecutive failures (only relevant in Closed state).
    #[cfg_attr(feature = "utoipa", schema(example = 0))]
    pub failure_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(enabled: bool) -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            enabled,
            failure_threshold: 5,
            open_timeout_secs: 30,
            success_threshold: 2,
            failure_status_codes: vec![500, 502, 503, 504],
            ..Default::default()
        }
    }

    #[test]
    fn test_registry_get_or_create() {
        let registry = CircuitBreakerRegistry::new();

        // First call creates the breaker
        let breaker1 = registry.get_or_create("test-provider", &test_config(true));
        assert!(breaker1.is_some());

        // Second call returns the same breaker
        let breaker2 = registry.get_or_create("test-provider", &test_config(true));
        assert!(breaker2.is_some());

        // Same Arc (same pointer)
        assert!(Arc::ptr_eq(&breaker1.unwrap(), &breaker2.unwrap()));
    }

    #[test]
    fn test_registry_disabled_config() {
        let registry = CircuitBreakerRegistry::new();

        let breaker = registry.get_or_create("test-provider", &test_config(false));
        assert!(breaker.is_none());
    }

    #[test]
    fn test_registry_status() {
        let registry = CircuitBreakerRegistry::new();

        registry.get_or_create("provider-a", &test_config(true));
        registry.get_or_create("provider-b", &test_config(true));

        let status = registry.status();
        assert_eq!(status.len(), 2);

        // All should be closed initially
        for s in &status {
            assert_eq!(s.state, CircuitState::Closed);
            assert_eq!(s.failure_count, 0);
        }
    }

    #[test]
    fn test_registry_status_reflects_state() {
        let registry = CircuitBreakerRegistry::new();

        let config = CircuitBreakerConfig {
            enabled: true,
            failure_threshold: 2,
            open_timeout_secs: 30,
            success_threshold: 2,
            failure_status_codes: vec![500],
            ..Default::default()
        };

        let breaker = registry.get_or_create("test", &config).unwrap();

        // Record failures to open the circuit
        breaker.record_failure();
        breaker.record_failure();

        let status = registry.status_for("test").unwrap();
        assert_eq!(status.state, CircuitState::Open);
    }
}
