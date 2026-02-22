/**
 * WebSocket Store - Real-time Event Connection Management
 *
 * Manages the WebSocket connection to the gateway's event endpoint.
 * Uses a singleton client pattern (client outside Zustand) to avoid
 * recreating the connection when the store updates.
 *
 * ## Architecture
 *
 * ```
 * ┌────────────────────────────────────────────────────────────────────┐
 * │                       websocketStore                               │
 * ├────────────────────────────────────────────────────────────────────┤
 * │  status: WebSocketConnectionStatus   - Connection state            │
 * │  error?: string                      - Error message if any        │
 * │  subscribedTopics: EventTopic[]      - Active subscriptions        │
 * ├────────────────────────────────────────────────────────────────────┤
 * │  connect(topics?)  - Connect with optional initial topics          │
 * │  disconnect()      - Close connection                              │
 * │  subscribe(topics) - Add topic subscriptions                       │
 * │  unsubscribe(topics) - Remove topic subscriptions                  │
 * └────────────────────────────────────────────────────────────────────┘
 *          │
 *          ▼
 * ┌────────────────────────────────────────────────────────────────────┐
 * │              WebSocketEventClient (singleton)                      │
 * │  - Lives outside Zustand for reference stability                   │
 * │  - Handles connection, reconnection, message parsing               │
 * └────────────────────────────────────────────────────────────────────┘
 * ```
 *
 * ## Usage
 *
 * ```typescript
 * import { useWebSocketStore, useWebSocketStatus } from "@/stores/websocketStore";
 *
 * // In a component
 * const { connect, disconnect } = useWebSocketStore();
 * const status = useWebSocketStatus();
 *
 * useEffect(() => {
 *   connect(["health"]);
 *   return () => disconnect();
 * }, []);
 * ```
 */

import { create } from "zustand";

import {
  WebSocketEventClient,
  type WebSocketConnectionStatus,
  type EventTopic,
} from "@/services/websocket";

// =============================================================================
// Types
// =============================================================================

interface WebSocketState {
  /** Current connection status */
  status: WebSocketConnectionStatus;
  /** Error message if status is "error" */
  error?: string;
  /** Currently subscribed topics */
  subscribedTopics: EventTopic[];
}

interface WebSocketActions {
  /** Connect to the WebSocket server */
  connect: (topics?: EventTopic[]) => void;
  /** Disconnect from the WebSocket server */
  disconnect: () => void;
  /** Subscribe to additional topics */
  subscribe: (topics: EventTopic[]) => void;
  /** Unsubscribe from topics */
  unsubscribe: (topics: EventTopic[]) => void;
  /** Internal: Update status (called by client callbacks) */
  _setStatus: (status: WebSocketConnectionStatus, error?: string) => void;
  /** Internal: Update subscribed topics */
  _setSubscribedTopics: (topics: EventTopic[]) => void;
}

export type WebSocketStore = WebSocketState & WebSocketActions;

// =============================================================================
// Singleton Client (outside Zustand for reference stability)
// =============================================================================

let client: WebSocketEventClient | null = null;

/**
 * Get the WebSocket client instance.
 *
 * Returns the singleton client, or null if not yet initialized.
 * Use this to register event listeners directly on the client.
 */
export function getWebSocketClient(): WebSocketEventClient | null {
  return client;
}

/**
 * Create or get the WebSocket client with the given topics.
 *
 * @param topics - Initial topics to subscribe to
 * @param onStatusChange - Callback for status changes
 * @returns The WebSocket client instance
 */
function getOrCreateClient(
  topics: EventTopic[],
  onStatusChange: (status: WebSocketConnectionStatus, error?: string) => void
): WebSocketEventClient {
  if (!client) {
    client = new WebSocketEventClient({ topics });
    client.onStatusChange(onStatusChange);
  }
  return client;
}

/**
 * Destroy the current client instance.
 */
function destroyClient(): void {
  if (client) {
    client.disconnect();
    client = null;
  }
}

// =============================================================================
// Store
// =============================================================================

export const useWebSocketStore = create<WebSocketStore>((set, get) => ({
  // ===========================================================================
  // State
  // ===========================================================================
  status: "disconnected",
  error: undefined,
  subscribedTopics: [],

  // ===========================================================================
  // Actions
  // ===========================================================================

  connect: (topics = ["health"]) => {
    const currentStatus = get().status;

    // Already connected or connecting
    if (currentStatus === "connected" || currentStatus === "connecting") {
      // Just update subscriptions if different
      const currentTopics = get().subscribedTopics;
      const newTopics = topics.filter((t) => !currentTopics.includes(t));
      if (newTopics.length > 0 && client) {
        client.subscribe(newTopics);
        get()._setSubscribedTopics([...currentTopics, ...newTopics]);
      }
      return;
    }

    // Create client and connect
    const wsClient = getOrCreateClient(topics, (status, error) => {
      get()._setStatus(status, error);
    });

    set({ subscribedTopics: topics });
    wsClient.connect();
  },

  disconnect: () => {
    destroyClient();
    set({
      status: "disconnected",
      error: undefined,
      subscribedTopics: [],
    });
  },

  subscribe: (topics) => {
    if (!client) {
      console.warn("WebSocket not connected, cannot subscribe");
      return;
    }

    const currentTopics = get().subscribedTopics;
    const newTopics = topics.filter((t) => !currentTopics.includes(t));

    if (newTopics.length > 0) {
      client.subscribe(newTopics);
      get()._setSubscribedTopics([...currentTopics, ...newTopics]);
    }
  },

  unsubscribe: (topics) => {
    if (!client) {
      console.warn("WebSocket not connected, cannot unsubscribe");
      return;
    }

    const currentTopics = get().subscribedTopics;
    const removedTopics = topics.filter((t) => currentTopics.includes(t));

    if (removedTopics.length > 0) {
      client.unsubscribe(removedTopics);
      get()._setSubscribedTopics(currentTopics.filter((t) => !removedTopics.includes(t)));
    }
  },

  _setStatus: (status, error) => {
    set({ status, error });
  },

  _setSubscribedTopics: (topics) => {
    set({ subscribedTopics: topics });
  },
}));

// =============================================================================
// Selectors
// =============================================================================

/** Get current WebSocket connection status */
export const useWebSocketStatus = () => useWebSocketStore((state) => state.status);

/** Check if WebSocket is connected */
export const useIsWebSocketConnected = () =>
  useWebSocketStore((state) => state.status === "connected");

/** Check if WebSocket is connecting or reconnecting */
export const useIsWebSocketConnecting = () =>
  useWebSocketStore((state) => state.status === "connecting" || state.status === "reconnecting");

/** Get WebSocket error message */
export const useWebSocketError = () => useWebSocketStore((state) => state.error);

/** Get subscribed topics */
export const useWebSocketTopics = () => useWebSocketStore((state) => state.subscribedTopics);
