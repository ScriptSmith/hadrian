/**
 * useWebSocketEvents Hook
 *
 * React hook for subscribing to real-time gateway events via WebSocket.
 * Integrates with React Query for automatic query invalidation when
 * relevant events are received.
 *
 * ## Features
 *
 * - Auto-connects on mount (configurable)
 * - Typed callbacks for specific event types
 * - Automatic React Query cache invalidation
 * - Debounced invalidation to prevent query spam
 * - Cleanup on unmount
 *
 * ## Usage
 *
 * ```typescript
 * import { useWebSocketEvents } from "@/hooks/useWebSocketEvents";
 *
 * function ProviderHealthPage() {
 *   const { status, isConnected } = useWebSocketEvents({
 *     topics: ["health"],
 *     onHealthEvent: (event) => {
 *       console.log("Provider health changed:", event);
 *     },
 *     invalidateQueries: ["providerHealth", "circuitBreakers"],
 *     invalidateDebounceMs: 100,
 *   });
 *
 *   return (
 *     <div>
 *       <span>Status: {status}</span>
 *       {isConnected && <span>Live</span>}
 *     </div>
 *   );
 * }
 * ```
 */

import { useEffect, useRef, useCallback } from "react";
import { useQueryClient } from "@tanstack/react-query";

import {
  useWebSocketStore,
  useWebSocketStatus,
  useIsWebSocketConnected,
  getWebSocketClient,
} from "@/stores/websocketStore";
import {
  type EventTopic,
  type ProviderHealthChangedEvent,
  type CircuitBreakerStateChangedEvent,
  type AuditLogCreatedEvent,
  type UsageRecordedEvent,
  type BudgetThresholdReachedEvent,
  type RateLimitWarningEvent,
  type ServerEvent,
  isProviderHealthChangedEvent,
  isCircuitBreakerStateChangedEvent,
  isAuditLogCreatedEvent,
  isUsageRecordedEvent,
  isBudgetThresholdReachedEvent,
  isRateLimitWarningEvent,
} from "@/services/websocket";

// =============================================================================
// Types
// =============================================================================

export interface UseWebSocketEventsOptions {
  /** Topics to subscribe to (default: ["health"]) */
  topics?: EventTopic[];

  /** Automatically connect on mount (default: true) */
  autoConnect?: boolean;

  /** Callback for provider health changed events */
  onHealthEvent?: (event: ProviderHealthChangedEvent) => void;

  /** Callback for circuit breaker state changed events */
  onCircuitBreakerEvent?: (event: CircuitBreakerStateChangedEvent) => void;

  /** Callback for audit log events */
  onAuditEvent?: (event: AuditLogCreatedEvent) => void;

  /** Callback for usage recorded events */
  onUsageEvent?: (event: UsageRecordedEvent) => void;

  /** Callback for budget threshold events */
  onBudgetEvent?: (event: BudgetThresholdReachedEvent) => void;

  /** Callback for rate limit warning events */
  onRateLimitEvent?: (event: RateLimitWarningEvent) => void;

  /** Callback for all events */
  onEvent?: (event: ServerEvent) => void;

  /** React Query keys to invalidate when events are received */
  invalidateQueries?: string[];

  /** Debounce time for query invalidation in ms (default: 100) */
  invalidateDebounceMs?: number;
}

export interface UseWebSocketEventsReturn {
  /** Current connection status */
  status: ReturnType<typeof useWebSocketStatus>;

  /** Whether the WebSocket is connected */
  isConnected: boolean;

  /** Connect to the WebSocket server */
  connect: (topics?: EventTopic[]) => void;

  /** Disconnect from the WebSocket server */
  disconnect: () => void;

  /** Subscribe to additional topics */
  subscribe: (topics: EventTopic[]) => void;

  /** Unsubscribe from topics */
  unsubscribe: (topics: EventTopic[]) => void;
}

// =============================================================================
// Hook Implementation
// =============================================================================

export function useWebSocketEvents(
  options: UseWebSocketEventsOptions = {}
): UseWebSocketEventsReturn {
  const {
    topics = ["health"],
    autoConnect = true,
    onHealthEvent,
    onCircuitBreakerEvent,
    onAuditEvent,
    onUsageEvent,
    onBudgetEvent,
    onRateLimitEvent,
    onEvent,
    invalidateQueries = [],
    invalidateDebounceMs = 100,
  } = options;

  const queryClient = useQueryClient();
  const { connect, disconnect, subscribe, unsubscribe } = useWebSocketStore();
  const status = useWebSocketStatus();
  const isConnected = useIsWebSocketConnected();

  // Use refs for callbacks to avoid re-subscribing when callbacks change
  const callbacksRef = useRef({
    onHealthEvent,
    onCircuitBreakerEvent,
    onAuditEvent,
    onUsageEvent,
    onBudgetEvent,
    onRateLimitEvent,
    onEvent,
  });

  // Update refs when callbacks change
  useEffect(() => {
    callbacksRef.current = {
      onHealthEvent,
      onCircuitBreakerEvent,
      onAuditEvent,
      onUsageEvent,
      onBudgetEvent,
      onRateLimitEvent,
      onEvent,
    };
  }, [
    onHealthEvent,
    onCircuitBreakerEvent,
    onAuditEvent,
    onUsageEvent,
    onBudgetEvent,
    onRateLimitEvent,
    onEvent,
  ]);

  // Debounced query invalidation
  const invalidationTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingInvalidationsRef = useRef<Set<string>>(new Set());

  const scheduleInvalidation = useCallback(
    (keys: string[]) => {
      if (keys.length === 0) return;

      // Add to pending
      keys.forEach((key) => pendingInvalidationsRef.current.add(key));

      // Clear existing timer
      if (invalidationTimerRef.current) {
        clearTimeout(invalidationTimerRef.current);
      }

      // Schedule invalidation
      invalidationTimerRef.current = setTimeout(() => {
        const toInvalidate = Array.from(pendingInvalidationsRef.current);
        pendingInvalidationsRef.current.clear();

        // Invalidate each query key
        toInvalidate.forEach((key) => {
          queryClient.invalidateQueries({ queryKey: [key] });
        });

        invalidationTimerRef.current = null;
      }, invalidateDebounceMs);
    },
    [queryClient, invalidateDebounceMs]
  );

  // Event handler that dispatches to appropriate callbacks
  const handleEvent = useCallback(
    (event: ServerEvent) => {
      const callbacks = callbacksRef.current;

      // Call generic event callback
      callbacks.onEvent?.(event);

      // Call type-specific callbacks
      if (isProviderHealthChangedEvent(event)) {
        callbacks.onHealthEvent?.(event);
      } else if (isCircuitBreakerStateChangedEvent(event)) {
        callbacks.onCircuitBreakerEvent?.(event);
      } else if (isAuditLogCreatedEvent(event)) {
        callbacks.onAuditEvent?.(event);
      } else if (isUsageRecordedEvent(event)) {
        callbacks.onUsageEvent?.(event);
      } else if (isBudgetThresholdReachedEvent(event)) {
        callbacks.onBudgetEvent?.(event);
      } else if (isRateLimitWarningEvent(event)) {
        callbacks.onRateLimitEvent?.(event);
      }

      // Schedule query invalidation
      if (invalidateQueries.length > 0) {
        scheduleInvalidation(invalidateQueries);
      }
    },
    [invalidateQueries, scheduleInvalidation]
  );

  // Auto-connect and set up event listener
  useEffect(() => {
    // Declare cleanupRef BEFORE it's used in the setTimeout callback
    // to prevent race condition where cleanup runs before listener setup
    const cleanupRef: { current: () => void } = { current: () => {} };

    if (autoConnect) {
      connect(topics);
    }

    // Set up event listener when client is available
    const setupListener = () => {
      const client = getWebSocketClient();
      if (!client) return () => {};

      // Register for all events on our subscribed topics
      // We use "all" and filter in handleEvent for simplicity
      return client.onEvent("all", handleEvent);
    };

    // We need to wait for the client to be created
    // Use a small delay to ensure connection is initiated
    const timerId = setTimeout(() => {
      cleanupRef.current = setupListener();
    }, 50);

    return () => {
      clearTimeout(timerId);
      cleanupRef.current();

      // Clear any pending invalidation timers
      if (invalidationTimerRef.current) {
        clearTimeout(invalidationTimerRef.current);
        invalidationTimerRef.current = null;
      }
    };
  }, [autoConnect, topics, connect, handleEvent]);

  return {
    status,
    isConnected,
    connect,
    disconnect,
    subscribe,
    unsubscribe,
  };
}

export default useWebSocketEvents;
