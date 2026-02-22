//! Event broadcasting system for real-time notifications via WebSocket.
//!
//! This module provides an event bus that allows services to publish events
//! that can be consumed by WebSocket subscribers for real-time monitoring.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
//! │   Services      │────>│    EventBus     │────>│   WebSocket     │
//! │ (audit, usage)  │     │  (broadcast)    │     │   Handlers      │
//! └─────────────────┘     └─────────────────┘     └─────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! // Publishing an event
//! event_bus.publish(ServerEvent::AuditLogCreated { ... });
//!
//! // Subscribing to events
//! let mut rx = event_bus.subscribe();
//! while let Ok(event) = rx.recv().await {
//!     // Handle event
//! }
//! ```

use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Default channel capacity for the event bus.
/// This determines how many events can be buffered before slow receivers
/// start missing events (lagging).
const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

/// Event topics for filtering subscriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventTopic {
    /// Audit log events (create, access, modify, delete operations)
    Audit,
    /// Usage tracking events (token counts, costs)
    Usage,
    /// Provider health events (circuit breaker state changes)
    Health,
    /// Budget events (threshold warnings, exceeded)
    Budget,
    /// Rate limiting events (warnings, exceeded)
    RateLimit,
    /// All events (wildcard subscription)
    All,
}

impl EventTopic {
    /// Check if this topic matches another topic.
    /// `All` matches everything, otherwise exact match is required.
    pub fn matches(&self, other: &EventTopic) -> bool {
        matches!(self, EventTopic::All) || matches!(other, EventTopic::All) || self == other
    }
}

/// Server events that can be broadcast to WebSocket subscribers.
///
/// Uses `event_type` as the discriminator tag to avoid collision with
/// `ServerMessage::Event` which uses `type: "event"`. This produces JSON like:
/// ```json
/// { "type": "event", "topic": "usage", "event_type": "usage_recorded", ... }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// A new audit log entry was created.
    AuditLogCreated {
        id: Uuid,
        timestamp: DateTime<Utc>,
        action: String,
        resource_type: String,
        resource_id: Option<String>,
        actor_type: String,
        actor_id: Option<Uuid>,
        org_id: Option<Uuid>,
        project_id: Option<Uuid>,
    },

    /// Usage was recorded for a request.
    UsageRecorded {
        request_id: Uuid,
        timestamp: DateTime<Utc>,
        model: String,
        provider: String,
        input_tokens: i32,
        output_tokens: i32,
        cost_microcents: Option<i64>,
        user_id: Option<Uuid>,
        org_id: Option<Uuid>,
        project_id: Option<Uuid>,
        team_id: Option<Uuid>,
        service_account_id: Option<Uuid>,
    },

    /// A provider's circuit breaker state changed.
    CircuitBreakerStateChanged {
        provider: String,
        timestamp: DateTime<Utc>,
        previous_state: CircuitBreakerState,
        new_state: CircuitBreakerState,
        failure_count: u32,
        success_count: u32,
    },

    /// A budget threshold was reached.
    BudgetThresholdReached {
        timestamp: DateTime<Utc>,
        budget_type: BudgetType,
        threshold_percent: u8,
        current_amount_microcents: i64,
        limit_microcents: i64,
        user_id: Option<Uuid>,
        org_id: Option<Uuid>,
        project_id: Option<Uuid>,
    },

    /// Rate limit warning or exceeded.
    RateLimitWarning {
        timestamp: DateTime<Utc>,
        limit_type: RateLimitType,
        current_count: u64,
        limit: u64,
        window_secs: u64,
        user_id: Option<Uuid>,
        api_key_id: Option<Uuid>,
    },

    /// Provider health status changed.
    ProviderHealthChanged {
        provider: String,
        timestamp: DateTime<Utc>,
        is_healthy: bool,
        latency_ms: Option<u64>,
        error_message: Option<String>,
    },
}

impl ServerEvent {
    /// Get the topic for this event.
    pub fn topic(&self) -> EventTopic {
        match self {
            ServerEvent::AuditLogCreated { .. } => EventTopic::Audit,
            ServerEvent::UsageRecorded { .. } => EventTopic::Usage,
            ServerEvent::CircuitBreakerStateChanged { .. } => EventTopic::Health,
            ServerEvent::BudgetThresholdReached { .. } => EventTopic::Budget,
            ServerEvent::RateLimitWarning { .. } => EventTopic::RateLimit,
            ServerEvent::ProviderHealthChanged { .. } => EventTopic::Health,
        }
    }

    /// Get the event type name as a string.
    pub fn event_type(&self) -> &'static str {
        match self {
            ServerEvent::AuditLogCreated { .. } => "audit_log_created",
            ServerEvent::UsageRecorded { .. } => "usage_recorded",
            ServerEvent::CircuitBreakerStateChanged { .. } => "circuit_breaker_state_changed",
            ServerEvent::BudgetThresholdReached { .. } => "budget_threshold_reached",
            ServerEvent::RateLimitWarning { .. } => "rate_limit_warning",
            ServerEvent::ProviderHealthChanged { .. } => "provider_health_changed",
        }
    }
}

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CircuitBreakerState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are rejected
    Open,
    /// Circuit is half-open, testing if service recovered
    HalfOpen,
}

/// Budget types for threshold events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetType {
    /// Daily budget limit
    Daily,
    /// Monthly budget limit
    Monthly,
    /// Per-request budget limit
    PerRequest,
}

/// Rate limit types for warning events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitType {
    /// Requests per minute
    RequestsPerMinute,
    /// Requests per hour
    RequestsPerHour,
    /// Tokens per minute
    TokensPerMinute,
    /// Concurrent requests
    Concurrent,
}

/// Central event bus for broadcasting server events.
///
/// Uses a tokio broadcast channel to allow multiple subscribers to receive
/// the same events. Events are cloned for each subscriber.
#[derive(Debug)]
pub struct EventBus {
    sender: broadcast::Sender<ServerEvent>,
    /// Counter for total events published (for metrics)
    events_published: AtomicU64,
    /// Counter for events dropped due to no subscribers
    events_dropped: AtomicU64,
}

impl EventBus {
    /// Create a new event bus with the default channel capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CHANNEL_CAPACITY)
    }

    /// Create a new event bus with a custom channel capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            events_published: AtomicU64::new(0),
            events_dropped: AtomicU64::new(0),
        }
    }

    /// Publish an event to all subscribers.
    ///
    /// Returns the number of subscribers that received the event.
    /// If there are no subscribers, the event is dropped and 0 is returned.
    pub fn publish(&self, event: ServerEvent) -> usize {
        self.events_published.fetch_add(1, Ordering::Relaxed);

        match self.sender.send(event) {
            Ok(count) => count,
            Err(_) => {
                // No active subscribers, event is dropped
                self.events_dropped.fetch_add(1, Ordering::Relaxed);
                0
            }
        }
    }

    /// Subscribe to events from this bus.
    ///
    /// Returns a receiver that can be used to receive events.
    /// If the receiver falls behind, it will receive `RecvError::Lagged`
    /// indicating how many events were missed.
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.sender.subscribe()
    }

    /// Get the current number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Get the total number of events published.
    pub fn events_published(&self) -> u64 {
        self.events_published.load(Ordering::Relaxed)
    }

    /// Get the number of events dropped (no subscribers).
    pub fn events_dropped(&self) -> u64 {
        self.events_dropped.load(Ordering::Relaxed)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        // Clone shares the same underlying broadcast channel
        Self {
            sender: self.sender.clone(),
            events_published: AtomicU64::new(self.events_published.load(Ordering::Relaxed)),
            events_dropped: AtomicU64::new(self.events_dropped.load(Ordering::Relaxed)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_topic_matches() {
        assert!(EventTopic::All.matches(&EventTopic::Audit));
        assert!(EventTopic::All.matches(&EventTopic::Usage));
        assert!(EventTopic::All.matches(&EventTopic::All));

        assert!(EventTopic::Audit.matches(&EventTopic::All));
        assert!(EventTopic::Audit.matches(&EventTopic::Audit));
        assert!(!EventTopic::Audit.matches(&EventTopic::Usage));

        assert!(EventTopic::Usage.matches(&EventTopic::Usage));
        assert!(!EventTopic::Usage.matches(&EventTopic::Audit));
    }

    #[test]
    fn test_server_event_topic() {
        let audit_event = ServerEvent::AuditLogCreated {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            action: "create".to_string(),
            resource_type: "user".to_string(),
            resource_id: Some("123".to_string()),
            actor_type: "user".to_string(),
            actor_id: Some(Uuid::new_v4()),
            org_id: None,
            project_id: None,
        };
        assert_eq!(audit_event.topic(), EventTopic::Audit);
        assert_eq!(audit_event.event_type(), "audit_log_created");

        let usage_event = ServerEvent::UsageRecorded {
            request_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cost_microcents: Some(1000),
            user_id: None,
            org_id: None,
            project_id: None,
            team_id: None,
            service_account_id: None,
        };
        assert_eq!(usage_event.topic(), EventTopic::Usage);
        assert_eq!(usage_event.event_type(), "usage_recorded");

        let health_event = ServerEvent::CircuitBreakerStateChanged {
            provider: "openai".to_string(),
            timestamp: Utc::now(),
            previous_state: CircuitBreakerState::Closed,
            new_state: CircuitBreakerState::Open,
            failure_count: 5,
            success_count: 0,
        };
        assert_eq!(health_event.topic(), EventTopic::Health);
        assert_eq!(health_event.event_type(), "circuit_breaker_state_changed");
    }

    #[test]
    fn test_event_bus_new() {
        let bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);
        assert_eq!(bus.events_published(), 0);
        assert_eq!(bus.events_dropped(), 0);
    }

    #[test]
    fn test_event_bus_publish_no_subscribers() {
        let bus = EventBus::new();
        let event = ServerEvent::AuditLogCreated {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            action: "create".to_string(),
            resource_type: "user".to_string(),
            resource_id: None,
            actor_type: "system".to_string(),
            actor_id: None,
            org_id: None,
            project_id: None,
        };

        let count = bus.publish(event);
        assert_eq!(count, 0);
        assert_eq!(bus.events_published(), 1);
        assert_eq!(bus.events_dropped(), 1);
    }

    #[tokio::test]
    async fn test_event_bus_subscribe_and_receive() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 1);

        let event = ServerEvent::UsageRecorded {
            request_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cost_microcents: Some(1000),
            user_id: None,
            org_id: None,
            project_id: None,
            team_id: None,
            service_account_id: None,
        };

        let count = bus.publish(event.clone());
        assert_eq!(count, 1);
        assert_eq!(bus.events_published(), 1);
        assert_eq!(bus.events_dropped(), 0);

        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_type(), "usage_recorded");
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();
        let mut rx3 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 3);

        let event = ServerEvent::BudgetThresholdReached {
            timestamp: Utc::now(),
            budget_type: BudgetType::Daily,
            threshold_percent: 80,
            current_amount_microcents: 8000,
            limit_microcents: 10000,
            user_id: None,
            org_id: None,
            project_id: None,
        };

        let count = bus.publish(event);
        assert_eq!(count, 3);

        // All subscribers should receive the event
        let r1 = rx1.recv().await.unwrap();
        let r2 = rx2.recv().await.unwrap();
        let r3 = rx3.recv().await.unwrap();

        assert_eq!(r1.event_type(), "budget_threshold_reached");
        assert_eq!(r2.event_type(), "budget_threshold_reached");
        assert_eq!(r3.event_type(), "budget_threshold_reached");
    }

    #[tokio::test]
    async fn test_event_bus_subscriber_drop() {
        let bus = EventBus::new();
        let rx1 = bus.subscribe();
        let rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        drop(rx1);
        assert_eq!(bus.subscriber_count(), 1);

        drop(rx2);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_event_bus_clone() {
        let bus1 = EventBus::new();
        let _rx = bus1.subscribe();

        let bus2 = bus1.clone();

        // Both should see the same subscriber count (shared channel)
        assert_eq!(bus1.subscriber_count(), 1);
        assert_eq!(bus2.subscriber_count(), 1);

        // Publishing on either should reach all subscribers
        let event = ServerEvent::ProviderHealthChanged {
            provider: "anthropic".to_string(),
            timestamp: Utc::now(),
            is_healthy: false,
            latency_ms: None,
            error_message: Some("Connection timeout".to_string()),
        };

        let count = bus2.publish(event);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_server_event_serialization() {
        let event = ServerEvent::RateLimitWarning {
            timestamp: Utc::now(),
            limit_type: RateLimitType::RequestsPerMinute,
            current_count: 58,
            limit: 60,
            window_secs: 60,
            user_id: Some(Uuid::new_v4()),
            api_key_id: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        // Uses event_type tag to avoid collision with ServerMessage's type tag
        assert!(json.contains("\"event_type\":\"rate_limit_warning\""));
        assert!(json.contains("\"limit_type\":\"requests_per_minute\""));

        // Deserialize back
        let parsed: ServerEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type(), "rate_limit_warning");
    }

    #[test]
    fn test_circuit_breaker_state_serialization() {
        let state = CircuitBreakerState::HalfOpen;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"half_open\"");

        let parsed: CircuitBreakerState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, CircuitBreakerState::HalfOpen);
    }

    #[test]
    fn test_budget_type_serialization() {
        let budget = BudgetType::Monthly;
        let json = serde_json::to_string(&budget).unwrap();
        assert_eq!(json, "\"monthly\"");
    }

    #[test]
    fn test_rate_limit_type_serialization() {
        let limit = RateLimitType::TokensPerMinute;
        let json = serde_json::to_string(&limit).unwrap();
        assert_eq!(json, "\"tokens_per_minute\"");
    }

    #[test]
    fn test_event_bus_with_capacity() {
        let bus = EventBus::with_capacity(10);
        assert_eq!(bus.subscriber_count(), 0);
        assert_eq!(bus.events_published(), 0);
        assert_eq!(bus.events_dropped(), 0);
    }

    #[tokio::test]
    async fn test_event_bus_lagged_subscriber() {
        // Create a small capacity bus to force lagging
        let bus = EventBus::with_capacity(2);
        let mut rx = bus.subscribe();

        // Publish more events than capacity
        for i in 0..5 {
            let event = ServerEvent::UsageRecorded {
                request_id: Uuid::new_v4(),
                timestamp: Utc::now(),
                model: format!("model-{}", i),
                provider: "test".to_string(),
                input_tokens: 100,
                output_tokens: 50,
                cost_microcents: Some(1000),
                user_id: None,
                org_id: None,
                project_id: None,
                team_id: None,
                service_account_id: None,
            };
            bus.publish(event);
        }

        // First receive should report lagged
        let result = rx.recv().await;
        assert!(matches!(
            result,
            Err(broadcast::error::RecvError::Lagged(_))
        ));

        // Should still be able to receive remaining events
        let result = rx.recv().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_event_topic_all_variants() {
        // Test all EventTopic variants for serialization round-trip
        let topics = [
            EventTopic::Audit,
            EventTopic::Usage,
            EventTopic::Health,
            EventTopic::Budget,
            EventTopic::RateLimit,
            EventTopic::All,
        ];

        for topic in topics {
            let json = serde_json::to_string(&topic).unwrap();
            let parsed: EventTopic = serde_json::from_str(&json).unwrap();
            assert_eq!(topic, parsed);
        }
    }

    #[test]
    fn test_server_event_all_variants_serialization() {
        // Test all ServerEvent variants can be serialized and deserialized
        let events = vec![
            ServerEvent::AuditLogCreated {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                action: "create".to_string(),
                resource_type: "user".to_string(),
                resource_id: Some("123".to_string()),
                actor_type: "user".to_string(),
                actor_id: Some(Uuid::new_v4()),
                org_id: Some(Uuid::new_v4()),
                project_id: Some(Uuid::new_v4()),
            },
            ServerEvent::UsageRecorded {
                request_id: Uuid::new_v4(),
                timestamp: Utc::now(),
                model: "gpt-4".to_string(),
                provider: "openai".to_string(),
                input_tokens: 100,
                output_tokens: 50,
                cost_microcents: Some(1000),
                user_id: Some(Uuid::new_v4()),
                org_id: Some(Uuid::new_v4()),
                project_id: Some(Uuid::new_v4()),
                team_id: Some(Uuid::new_v4()),
                service_account_id: None,
            },
            ServerEvent::CircuitBreakerStateChanged {
                provider: "anthropic".to_string(),
                timestamp: Utc::now(),
                previous_state: CircuitBreakerState::Closed,
                new_state: CircuitBreakerState::Open,
                failure_count: 5,
                success_count: 0,
            },
            ServerEvent::BudgetThresholdReached {
                timestamp: Utc::now(),
                budget_type: BudgetType::Monthly,
                threshold_percent: 90,
                current_amount_microcents: 9000,
                limit_microcents: 10000,
                user_id: Some(Uuid::new_v4()),
                org_id: None,
                project_id: None,
            },
            ServerEvent::RateLimitWarning {
                timestamp: Utc::now(),
                limit_type: RateLimitType::RequestsPerHour,
                current_count: 950,
                limit: 1000,
                window_secs: 3600,
                user_id: None,
                api_key_id: Some(Uuid::new_v4()),
            },
            ServerEvent::ProviderHealthChanged {
                provider: "vertex".to_string(),
                timestamp: Utc::now(),
                is_healthy: true,
                latency_ms: Some(150),
                error_message: None,
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let parsed: ServerEvent = serde_json::from_str(&json).unwrap();
            // Verify event type is preserved
            assert_eq!(event.event_type(), parsed.event_type());
            assert_eq!(event.topic(), parsed.topic());
        }
    }

    #[test]
    fn test_circuit_breaker_state_all_variants() {
        let states = [
            CircuitBreakerState::Closed,
            CircuitBreakerState::Open,
            CircuitBreakerState::HalfOpen,
        ];

        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let parsed: CircuitBreakerState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, parsed);
        }
    }

    #[test]
    fn test_budget_type_all_variants() {
        let types = [
            BudgetType::Daily,
            BudgetType::Monthly,
            BudgetType::PerRequest,
        ];

        for budget_type in types {
            let json = serde_json::to_string(&budget_type).unwrap();
            let parsed: BudgetType = serde_json::from_str(&json).unwrap();
            assert_eq!(budget_type, parsed);
        }
    }

    #[test]
    fn test_rate_limit_type_all_variants() {
        let types = [
            RateLimitType::RequestsPerMinute,
            RateLimitType::RequestsPerHour,
            RateLimitType::TokensPerMinute,
            RateLimitType::Concurrent,
        ];

        for limit_type in types {
            let json = serde_json::to_string(&limit_type).unwrap();
            let parsed: RateLimitType = serde_json::from_str(&json).unwrap();
            assert_eq!(limit_type, parsed);
        }
    }

    #[test]
    fn test_event_bus_default() {
        let bus = EventBus::default();
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_event_bus_publish_increments_counter() {
        let bus = EventBus::new();
        let _rx = bus.subscribe();

        for i in 0..5 {
            let event = ServerEvent::AuditLogCreated {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                action: format!("action-{}", i),
                resource_type: "test".to_string(),
                resource_id: None,
                actor_type: "system".to_string(),
                actor_id: None,
                org_id: None,
                project_id: None,
            };
            bus.publish(event);
        }

        assert_eq!(bus.events_published(), 5);
        assert_eq!(bus.events_dropped(), 0);
    }

    #[test]
    fn test_provider_health_changed_event() {
        // Test ProviderHealthChanged with error message
        let event = ServerEvent::ProviderHealthChanged {
            provider: "openai".to_string(),
            timestamp: Utc::now(),
            is_healthy: false,
            latency_ms: None,
            error_message: Some("Connection refused".to_string()),
        };

        assert_eq!(event.topic(), EventTopic::Health);
        assert_eq!(event.event_type(), "provider_health_changed");

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"is_healthy\":false"));
        assert!(json.contains("\"error_message\":\"Connection refused\""));
    }
}
