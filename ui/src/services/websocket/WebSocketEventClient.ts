/**
 * WebSocket Event Client
 *
 * Core client class for connecting to the Hadrian gateway WebSocket endpoint
 * at `/ws/events`. Handles connection management, automatic reconnection,
 * topic subscriptions, and event dispatching.
 *
 * ## Usage
 *
 * ```typescript
 * const client = new WebSocketEventClient({ topics: ["health"] });
 *
 * // Listen for status changes
 * client.onStatusChange((status, error) => {
 *   console.log("Status:", status, error);
 * });
 *
 * // Listen for events
 * client.onEvent("health", (event) => {
 *   console.log("Health event:", event);
 * });
 *
 * // Connect
 * client.connect();
 *
 * // Cleanup
 * client.disconnect();
 * ```
 */

import type {
  WebSocketConnectionStatus,
  WebSocketClientConfig,
  ClientMessage,
  ServerMessage,
  EventTopic,
  ServerEvent,
  StatusCallback,
  EventCallback,
} from "./types";

/** Default configuration values */
const DEFAULT_CONFIG: Required<WebSocketClientConfig> = {
  topics: ["health"],
  autoReconnect: true,
  maxReconnectAttempts: 10,
  reconnectDelayMs: 1000,
  maxReconnectDelayMs: 30000,
  reconnectBackoffMultiplier: 2,
};

/**
 * WebSocket client for real-time gateway events.
 *
 * Features:
 * - Automatic reconnection with exponential backoff
 * - Topic-based subscriptions
 * - Event listener registration with cleanup functions
 * - Session cookie authentication (same-origin)
 */
export class WebSocketEventClient {
  private config: Required<WebSocketClientConfig>;
  private ws: WebSocket | null = null;
  private status: WebSocketConnectionStatus = "disconnected";
  private error?: string;
  private reconnectAttempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private subscribedTopics: Set<EventTopic> = new Set();

  // Listener sets
  private statusListeners = new Set<StatusCallback>();
  private eventListeners = new Map<EventTopic | "all", Set<EventCallback>>();

  constructor(config: WebSocketClientConfig = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
    // Initialize subscribed topics from config
    this.subscribedTopics = new Set(this.config.topics);
  }

  // ===========================================================================
  // Connection Management
  // ===========================================================================

  /**
   * Connect to the WebSocket server.
   *
   * Builds the WebSocket URL from the current window location,
   * automatically using wss: for HTTPS pages.
   */
  connect(): void {
    if (this.ws && (this.status === "connected" || this.status === "connecting")) {
      return;
    }

    this.setStatus("connecting");

    try {
      const url = this.buildWebSocketUrl();
      this.ws = new WebSocket(url);

      this.ws.onopen = this.handleOpen.bind(this);
      this.ws.onmessage = this.handleMessage.bind(this);
      this.ws.onclose = this.handleClose.bind(this);
      this.ws.onerror = this.handleError.bind(this);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      this.setStatus("error", `Failed to create WebSocket: ${errorMsg}`);
    }
  }

  /**
   * Disconnect from the WebSocket server.
   *
   * Clears any pending reconnection timers and closes the connection.
   */
  disconnect(): void {
    // Clear reconnect timer
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    // Close the WebSocket
    if (this.ws) {
      // Prevent reconnection on intentional close
      this.config.autoReconnect = false;
      this.ws.close(1000, "Client disconnected");
      this.ws = null;
    }

    this.reconnectAttempts = 0;
    this.setStatus("disconnected");
  }

  /**
   * Get current connection status.
   */
  getStatus(): WebSocketConnectionStatus {
    return this.status;
  }

  /**
   * Get error message if status is "error".
   */
  getError(): string | undefined {
    return this.error;
  }

  /**
   * Check if connected.
   */
  isConnected(): boolean {
    return this.status === "connected";
  }

  // ===========================================================================
  // Subscription Management
  // ===========================================================================

  /**
   * Subscribe to additional topics.
   *
   * @param topics - Topics to subscribe to
   */
  subscribe(topics: EventTopic[]): void {
    const newTopics = topics.filter((t) => !this.subscribedTopics.has(t));
    if (newTopics.length === 0) return;

    newTopics.forEach((t) => this.subscribedTopics.add(t));

    if (this.isConnected()) {
      this.send({ type: "subscribe", topics: newTopics });
    }
  }

  /**
   * Unsubscribe from topics.
   *
   * @param topics - Topics to unsubscribe from
   */
  unsubscribe(topics: EventTopic[]): void {
    const existingTopics = topics.filter((t) => this.subscribedTopics.has(t));
    if (existingTopics.length === 0) return;

    existingTopics.forEach((t) => this.subscribedTopics.delete(t));

    if (this.isConnected()) {
      this.send({ type: "unsubscribe", topics: existingTopics });
    }
  }

  /**
   * Get currently subscribed topics.
   */
  getSubscribedTopics(): EventTopic[] {
    return Array.from(this.subscribedTopics);
  }

  // ===========================================================================
  // Event Listeners
  // ===========================================================================

  /**
   * Register a callback for connection status changes.
   *
   * @param callback - Function to call when status changes
   * @returns Cleanup function to remove the listener
   */
  onStatusChange(callback: StatusCallback): () => void {
    this.statusListeners.add(callback);
    return () => {
      this.statusListeners.delete(callback);
    };
  }

  /**
   * Register a callback for events on a specific topic.
   *
   * Use "all" to receive all events regardless of topic.
   *
   * @param topic - Topic to listen for, or "all" for all events
   * @param callback - Function to call when an event is received
   * @returns Cleanup function to remove the listener
   */
  onEvent(topic: EventTopic | "all", callback: EventCallback): () => void {
    let listeners = this.eventListeners.get(topic);
    if (!listeners) {
      listeners = new Set();
      this.eventListeners.set(topic, listeners);
    }
    listeners.add(callback);

    return () => {
      const l = this.eventListeners.get(topic);
      if (l) {
        l.delete(callback);
        if (l.size === 0) {
          this.eventListeners.delete(topic);
        }
      }
    };
  }

  // ===========================================================================
  // Private: Connection Handlers
  // ===========================================================================

  private buildWebSocketUrl(): string {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const host = window.location.host;

    // Build query string with initial topics
    const params = new URLSearchParams();
    if (this.subscribedTopics.size > 0) {
      params.set("topics", Array.from(this.subscribedTopics).join(","));
    }

    const queryString = params.toString();
    const url = `${protocol}//${host}/ws/events${queryString ? `?${queryString}` : ""}`;

    return url;
  }

  private handleOpen(): void {
    this.reconnectAttempts = 0;
    this.setStatus("connected");
  }

  private handleMessage(event: MessageEvent): void {
    try {
      const message = JSON.parse(event.data as string) as ServerMessage;
      this.processMessage(message);
    } catch (err) {
      console.warn("Failed to parse WebSocket message:", err);
    }
  }

  private handleClose(event: CloseEvent): void {
    this.ws = null;

    // Don't reconnect on clean close or if reconnection is disabled
    if (event.code === 1000 || !this.config.autoReconnect) {
      this.setStatus("disconnected");
      return;
    }

    // Schedule reconnection
    this.scheduleReconnect();
  }

  private handleError(): void {
    // The close event will follow, handle reconnection there
    this.setStatus("error", "WebSocket connection error");
  }

  // ===========================================================================
  // Private: Message Processing
  // ===========================================================================

  private processMessage(message: ServerMessage): void {
    switch (message.type) {
      case "connected":
        // Server confirmed connection - we may get updated topic list
        break;

      case "subscribed":
        // Subscription confirmed
        break;

      case "unsubscribed":
        // Unsubscription confirmed
        break;

      case "event":
        this.dispatchEvent(message);
        break;

      case "error":
        console.error(`WebSocket error from server: [${message.code}] ${message.message}`);
        break;

      case "pong":
        // Pong received - keepalive confirmed
        break;
    }
  }

  private dispatchEvent(message: ServerMessage & { type: "event" }): void {
    // Extract the event from the flattened message
    // The backend flattens ServerEvent into EventMessage, so we need to
    // reconstruct it by picking the known event fields
    const event = this.extractEventFromMessage(message);
    if (!event) return;

    // Dispatch to topic-specific listeners
    const topicListeners = this.eventListeners.get(message.topic);
    if (topicListeners) {
      topicListeners.forEach((callback) => callback(event));
    }

    // Dispatch to "all" listeners
    const allListeners = this.eventListeners.get("all");
    if (allListeners) {
      allListeners.forEach((callback) => callback(event));
    }
  }

  private extractEventFromMessage(message: ServerMessage & { type: "event" }): ServerEvent | null {
    // The backend now uses `event_type` as the discriminator for ServerEvent to avoid
    // collision with ServerMessage's `type: "event"`. The message looks like:
    // { "type": "event", "topic": "health", "event_type": "provider_health_changed", ... }
    const eventType = (message as Record<string, unknown>)["event_type"] as string | undefined;

    if (!eventType) {
      console.warn("Event message missing event_type:", message);
      return null;
    }

    // Create the event object, excluding the wrapper fields
    const baseEvent = { ...message };
    delete (baseEvent as Record<string, unknown>)["topic"];
    delete (baseEvent as Record<string, unknown>)["type"];

    return baseEvent as unknown as ServerEvent;
  }

  // ===========================================================================
  // Private: Reconnection
  // ===========================================================================

  private scheduleReconnect(): void {
    if (this.reconnectAttempts >= this.config.maxReconnectAttempts) {
      this.setStatus(
        "error",
        `Max reconnection attempts (${this.config.maxReconnectAttempts}) reached`
      );
      return;
    }

    this.setStatus("reconnecting");
    this.reconnectAttempts++;

    // Calculate delay with exponential backoff and jitter
    const baseDelay =
      this.config.reconnectDelayMs *
      Math.pow(this.config.reconnectBackoffMultiplier, this.reconnectAttempts - 1);
    const delay = Math.min(baseDelay, this.config.maxReconnectDelayMs);
    // Add ±20% jitter
    const jitter = delay * 0.2 * (Math.random() * 2 - 1);
    const finalDelay = Math.round(delay + jitter);

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, finalDelay);
  }

  // ===========================================================================
  // Private: Utilities
  // ===========================================================================

  private setStatus(status: WebSocketConnectionStatus, error?: string): void {
    this.status = status;
    this.error = error;
    this.statusListeners.forEach((callback) => callback(status, error));
  }

  private send(message: ClientMessage): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    }
  }
}
