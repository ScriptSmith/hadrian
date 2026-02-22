/**
 * WebSocket Event Types
 *
 * TypeScript types matching the backend WebSocket protocol defined in:
 * - src/routes/ws.rs (ClientMessage, ServerMessage)
 * - src/events/mod.rs (EventTopic, ServerEvent, CircuitBreakerState)
 */

// =============================================================================
// Connection Status
// =============================================================================

/** WebSocket connection status */
export type WebSocketConnectionStatus =
  | "disconnected"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "error";

// =============================================================================
// Event Topics (matches backend EventTopic)
// =============================================================================

/** Available event topics for subscription */
export type EventTopic = "audit" | "usage" | "health" | "budget" | "rate_limit" | "all";

// =============================================================================
// Client Messages (matches backend ClientMessage)
// =============================================================================

/** Subscribe to event topics */
export interface SubscribeMessage {
  type: "subscribe";
  topics: string[];
}

/** Unsubscribe from event topics */
export interface UnsubscribeMessage {
  type: "unsubscribe";
  topics: string[];
}

/** Client ping for keepalive */
export interface PingMessage {
  type: "ping";
}

/** Union of all client-to-server messages */
export type ClientMessage = SubscribeMessage | UnsubscribeMessage | PingMessage;

// =============================================================================
// Server Messages (matches backend ServerMessage)
// =============================================================================

/** Connection established message */
export interface ConnectedMessage {
  type: "connected";
  user_id: string | null;
  subscribed_topics: string[];
}

/** Subscription confirmed message */
export interface SubscribedMessage {
  type: "subscribed";
  topics: string[];
}

/** Unsubscription confirmed message */
export interface UnsubscribedMessage {
  type: "unsubscribed";
  topics: string[];
}

/** Server event message (flattened event payload) */
export interface EventMessage {
  type: "event";
  topic: EventTopic;
  // Event-specific fields are flattened into this object
  [key: string]: unknown;
}

/** Error message */
export interface ErrorMessage {
  type: "error";
  code: string;
  message: string;
}

/** Pong response to client ping */
export interface PongMessage {
  type: "pong";
}

/** Union of all server-to-client messages */
export type ServerMessage =
  | ConnectedMessage
  | SubscribedMessage
  | UnsubscribedMessage
  | EventMessage
  | ErrorMessage
  | PongMessage;

// =============================================================================
// Circuit Breaker State (matches backend CircuitBreakerState)
// =============================================================================

/** Circuit breaker states */
export type CircuitBreakerState = "closed" | "open" | "half_open";

// =============================================================================
// Server Events (matches backend ServerEvent)
// =============================================================================

// Note: Events use `event_type` as the discriminator field instead of `type`
// to avoid collision with ServerMessage's `type: "event"` field when the
// event is flattened into the message. The resulting JSON looks like:
// { "type": "event", "topic": "health", "event_type": "provider_health_changed", ... }

/** Provider health status changed event */
export interface ProviderHealthChangedEvent {
  event_type: "provider_health_changed";
  provider: string;
  timestamp: string;
  is_healthy: boolean;
  latency_ms?: number;
  error_message?: string;
}

/** Circuit breaker state changed event */
export interface CircuitBreakerStateChangedEvent {
  event_type: "circuit_breaker_state_changed";
  provider: string;
  timestamp: string;
  previous_state: CircuitBreakerState;
  new_state: CircuitBreakerState;
  failure_count: number;
  success_count: number;
}

/** Audit log created event */
export interface AuditLogCreatedEvent {
  event_type: "audit_log_created";
  id: string;
  timestamp: string;
  action: string;
  resource_type: string;
  resource_id?: string;
  actor_type: string;
  actor_id?: string;
  org_id?: string;
  project_id?: string;
}

/** Usage recorded event */
export interface UsageRecordedEvent {
  event_type: "usage_recorded";
  request_id: string;
  timestamp: string;
  model: string;
  provider: string;
  input_tokens: number;
  output_tokens: number;
  cost_microcents?: number;
  user_id?: string;
  org_id?: string;
  project_id?: string;
}

/** Budget threshold reached event */
export interface BudgetThresholdReachedEvent {
  event_type: "budget_threshold_reached";
  timestamp: string;
  budget_type: "daily" | "monthly" | "per_request";
  threshold_percent: number;
  current_amount_microcents: number;
  limit_microcents: number;
  user_id?: string;
  org_id?: string;
  project_id?: string;
}

/** Rate limit warning event */
export interface RateLimitWarningEvent {
  event_type: "rate_limit_warning";
  timestamp: string;
  limit_type: "requests_per_minute" | "requests_per_hour" | "tokens_per_minute" | "concurrent";
  current_count: number;
  limit: number;
  window_secs: number;
  user_id?: string;
  api_key_id?: string;
}

/** Union of all health-related events */
export type HealthEvent = ProviderHealthChangedEvent | CircuitBreakerStateChangedEvent;

/** Union of all server events */
export type ServerEvent =
  | ProviderHealthChangedEvent
  | CircuitBreakerStateChangedEvent
  | AuditLogCreatedEvent
  | UsageRecordedEvent
  | BudgetThresholdReachedEvent
  | RateLimitWarningEvent;

// =============================================================================
// Type Guards
// =============================================================================

/** Check if an event is a provider health changed event */
export function isProviderHealthChangedEvent(
  event: ServerEvent
): event is ProviderHealthChangedEvent {
  return event.event_type === "provider_health_changed";
}

/** Check if an event is a circuit breaker state changed event */
export function isCircuitBreakerStateChangedEvent(
  event: ServerEvent
): event is CircuitBreakerStateChangedEvent {
  return event.event_type === "circuit_breaker_state_changed";
}

/** Check if an event is a health event */
export function isHealthEvent(event: ServerEvent): event is HealthEvent {
  return isProviderHealthChangedEvent(event) || isCircuitBreakerStateChangedEvent(event);
}

/** Check if an event is an audit log created event */
export function isAuditLogCreatedEvent(event: ServerEvent): event is AuditLogCreatedEvent {
  return event.event_type === "audit_log_created";
}

/** Check if an event is a usage recorded event */
export function isUsageRecordedEvent(event: ServerEvent): event is UsageRecordedEvent {
  return event.event_type === "usage_recorded";
}

/** Check if an event is a budget threshold reached event */
export function isBudgetThresholdReachedEvent(
  event: ServerEvent
): event is BudgetThresholdReachedEvent {
  return event.event_type === "budget_threshold_reached";
}

/** Check if an event is a rate limit warning event */
export function isRateLimitWarningEvent(event: ServerEvent): event is RateLimitWarningEvent {
  return event.event_type === "rate_limit_warning";
}

// =============================================================================
// Client Configuration
// =============================================================================

/** WebSocket client configuration */
export interface WebSocketClientConfig {
  /** Initial topics to subscribe to (default: ["health"]) */
  topics?: EventTopic[];
  /** Enable automatic reconnection (default: true) */
  autoReconnect?: boolean;
  /** Maximum reconnection attempts (default: 10) */
  maxReconnectAttempts?: number;
  /** Initial reconnect delay in ms (default: 1000) */
  reconnectDelayMs?: number;
  /** Maximum reconnect delay in ms (default: 30000) */
  maxReconnectDelayMs?: number;
  /** Reconnect backoff multiplier (default: 2) */
  reconnectBackoffMultiplier?: number;
}

/** Callback for status changes */
export type StatusCallback = (status: WebSocketConnectionStatus, error?: string) => void;

/** Callback for server events */
export type EventCallback = (event: ServerEvent) => void;
