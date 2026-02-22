/**
 * WebSocket Event Service
 *
 * Provides real-time event streaming from the Hadrian gateway via WebSocket.
 * Connect to `/ws/events` to receive health, usage, audit, budget, and
 * rate limit events.
 *
 * @example
 * ```typescript
 * import { WebSocketEventClient, type ProviderHealthChangedEvent } from "@/services/websocket";
 *
 * const client = new WebSocketEventClient({ topics: ["health"] });
 *
 * client.onStatusChange((status) => console.log("Status:", status));
 * client.onEvent("health", (event) => console.log("Event:", event));
 *
 * client.connect();
 * ```
 */

// Client
export { WebSocketEventClient } from "./WebSocketEventClient";

// Types - Connection
export type {
  WebSocketConnectionStatus,
  WebSocketClientConfig,
  StatusCallback,
  EventCallback,
} from "./types";

// Types - Messages
export type {
  EventTopic,
  ClientMessage,
  SubscribeMessage,
  UnsubscribeMessage,
  PingMessage,
  ServerMessage,
  ConnectedMessage,
  SubscribedMessage,
  UnsubscribedMessage,
  EventMessage,
  ErrorMessage,
  PongMessage,
} from "./types";

// Types - Events
export type {
  CircuitBreakerState,
  ServerEvent,
  HealthEvent,
  ProviderHealthChangedEvent,
  CircuitBreakerStateChangedEvent,
  AuditLogCreatedEvent,
  UsageRecordedEvent,
  BudgetThresholdReachedEvent,
  RateLimitWarningEvent,
} from "./types";

// Type Guards
export {
  isProviderHealthChangedEvent,
  isCircuitBreakerStateChangedEvent,
  isHealthEvent,
  isAuditLogCreatedEvent,
  isUsageRecordedEvent,
  isBudgetThresholdReachedEvent,
  isRateLimitWarningEvent,
} from "./types";
