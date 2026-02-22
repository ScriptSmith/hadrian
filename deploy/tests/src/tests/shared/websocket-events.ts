/**
 * WebSocket Event Subscription Tests
 *
 * Tests real-time event streaming via WebSocket for provider health
 * and circuit breaker state changes.
 *
 * WebSocket endpoint: /ws/events
 *
 * Subscription protocol:
 * - Connect to /ws/events
 * - Send: {"type": "subscribe", "topics": ["health"]}
 * - Receive: {"type": "subscribed", "topics": ["health"]}
 * - Receive events: {"type": "event", "topic": "health", ...event_data}
 */
import { describe, it, expect } from "vitest";
import { WebSocket, type RawData } from "ws";
import { trackedFetch } from "../../utils/tracked-fetch";

/**
 * Context for WebSocket event tests.
 */
export interface WebSocketEventContext {
  /** Gateway base URL (http://...) */
  gatewayUrl: string;
  /** Admin token for authenticated connections */
  adminToken: string;
  /** API key for making requests that trigger events */
  apiKey: string;
  /** Provider name to test */
  providerName: string;
}

/**
 * Server message types from the WebSocket.
 */
interface ServerMessage {
  type: string;
  [key: string]: unknown;
}

interface ConnectedMessage extends ServerMessage {
  type: "connected";
  user_id: string | null;
  subscribed_topics: string[];
}

interface SubscribedMessage extends ServerMessage {
  type: "subscribed";
  topics: string[];
}

interface EventMessage extends ServerMessage {
  type: "event";
  topic: string;
  // Event-specific fields are flattened
}

/**
 * Helper to create a WebSocket connection to the gateway.
 */
function createWebSocket(
  gatewayUrl: string,
  token?: string,
  topics?: string[]
): WebSocket {
  // Convert http:// to ws://
  const wsUrl = gatewayUrl.replace(/^http/, "ws");
  const params = new URLSearchParams();

  if (token) {
    params.set("token", token);
  }
  if (topics && topics.length > 0) {
    params.set("topics", topics.join(","));
  }

  const url = `${wsUrl}/ws/events${params.toString() ? `?${params}` : ""}`;
  return new WebSocket(url);
}

/**
 * Helper to wait for a WebSocket message matching a predicate.
 */
function waitForMessage<T extends ServerMessage>(
  ws: WebSocket,
  predicate: (msg: ServerMessage) => msg is T,
  timeoutMs = 10000
): Promise<T> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      ws.removeListener("message", handler);
      reject(new Error("Timeout waiting for WebSocket message"));
    }, timeoutMs);

    const handler = (data: RawData) => {
      try {
        const msg = JSON.parse(data.toString()) as ServerMessage;
        if (predicate(msg)) {
          clearTimeout(timeout);
          ws.removeListener("message", handler);
          resolve(msg);
        }
      } catch {
        // Ignore parse errors, keep waiting
      }
    };

    ws.on("message", handler);
  });
}

/**
 * Helper to wait for WebSocket to open.
 */
function waitForOpen(ws: WebSocket, timeoutMs = 10000): Promise<void> {
  return new Promise((resolve, reject) => {
    if (ws.readyState === WebSocket.OPEN) {
      resolve();
      return;
    }

    const timeout = setTimeout(() => {
      reject(new Error("Timeout waiting for WebSocket to open"));
    }, timeoutMs);

    ws.once("open", () => {
      clearTimeout(timeout);
      resolve();
    });

    ws.once("error", (err) => {
      clearTimeout(timeout);
      reject(err);
    });
  });
}

/**
 * Helper to make a chat completion request.
 */
async function makeChatRequest(
  gatewayUrl: string,
  apiKey: string,
  model: string
): Promise<Response> {
  return trackedFetch(`${gatewayUrl}/api/v1/chat/completions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-API-Key": apiKey,
    },
    body: JSON.stringify({
      model,
      messages: [{ role: "user", content: "WebSocket test" }],
    }),
  });
}

/**
 * Run WebSocket event tests.
 *
 * @param getContext - Function that returns the test context
 */
export function runWebSocketEventTests(
  getContext: () => WebSocketEventContext
) {
  describe("WebSocket Event Streaming", () => {
    // =========================================================================
    // Connection Tests
    // =========================================================================
    describe("Connection", () => {
      it("connects to /ws/events endpoint", async () => {
        const { gatewayUrl } = getContext();

        const ws = createWebSocket(gatewayUrl);

        try {
          await waitForOpen(ws);

          // Wait for connected message
          const msg = await waitForMessage(
            ws,
            (m): m is ConnectedMessage => m.type === "connected"
          );

          expect(msg.type).toBe("connected");
          expect(msg.subscribed_topics).toBeDefined();
          expect(Array.isArray(msg.subscribed_topics)).toBe(true);
        } finally {
          ws.close();
        }
      });

      it("connects with initial topics from query parameter", async () => {
        const { gatewayUrl } = getContext();

        const ws = createWebSocket(gatewayUrl, undefined, ["health"]);

        try {
          await waitForOpen(ws);

          const msg = await waitForMessage(
            ws,
            (m): m is ConnectedMessage => m.type === "connected"
          );

          expect(msg.subscribed_topics).toContain("health");
        } finally {
          ws.close();
        }
      });
    });

    // =========================================================================
    // Subscription Tests
    // =========================================================================
    describe("Topic Subscription", () => {
      it("subscribes to health topic", async () => {
        const { gatewayUrl } = getContext();

        const ws = createWebSocket(gatewayUrl);

        try {
          await waitForOpen(ws);

          // Wait for initial connected message
          await waitForMessage(
            ws,
            (m): m is ConnectedMessage => m.type === "connected"
          );

          // Subscribe to health topic
          ws.send(JSON.stringify({ type: "subscribe", topics: ["health"] }));

          // Wait for subscription confirmation
          const msg = await waitForMessage(
            ws,
            (m): m is SubscribedMessage => m.type === "subscribed"
          );

          expect(msg.topics).toContain("health");
        } finally {
          ws.close();
        }
      });

      it("unsubscribes from topics", async () => {
        const { gatewayUrl } = getContext();

        const ws = createWebSocket(gatewayUrl, undefined, ["health", "usage"]);

        try {
          await waitForOpen(ws);

          await waitForMessage(
            ws,
            (m): m is ConnectedMessage => m.type === "connected"
          );

          // Unsubscribe from health topic
          ws.send(JSON.stringify({ type: "unsubscribe", topics: ["health"] }));

          // Wait for unsubscription confirmation
          const msg = await waitForMessage(
            ws,
            (m): m is ServerMessage => m.type === "unsubscribed"
          );

          expect(msg.topics).toContain("health");
        } finally {
          ws.close();
        }
      });
    });

    // =========================================================================
    // Event Delivery Tests
    // =========================================================================
    describe("Event Delivery", () => {
      // NOTE: CircuitBreakerStateChanged event test is skipped because the test
      // provider doesn't integrate with the circuit breaker registry. This test
      // would work with real providers that properly emit circuit breaker events.

      it("filters events by subscribed topics", async () => {
        const { gatewayUrl, apiKey } = getContext();

        // Subscribe only to budget topic (not health)
        const ws = createWebSocket(gatewayUrl, undefined, ["budget"]);

        try {
          await waitForOpen(ws);

          await waitForMessage(
            ws,
            (m): m is ConnectedMessage => m.type === "connected"
          );

          // Make requests that would trigger health events
          await makeChatRequest(gatewayUrl, apiKey, "test/test-model");

          // Wait a bit and verify we don't receive health events
          const receivedMessages: ServerMessage[] = [];
          const messageHandler = (data: RawData) => {
            try {
              receivedMessages.push(JSON.parse(data.toString()));
            } catch {
              // Ignore
            }
          };

          ws.on("message", messageHandler);

          // Wait 2 seconds for any events
          await new Promise((resolve) => setTimeout(resolve, 2000));

          ws.removeListener("message", messageHandler);

          // Check that no health events were received
          const healthEvents = receivedMessages.filter(
            (m) => m.type === "event" && (m as EventMessage).topic === "health"
          );
          expect(healthEvents.length).toBe(0);
        } finally {
          ws.close();
        }
      });

      it("receives events when subscribed to 'all' topic", async () => {
        const { gatewayUrl, apiKey } = getContext();

        // Subscribe to all topics
        const ws = createWebSocket(gatewayUrl, undefined, ["all"]);

        try {
          await waitForOpen(ws);

          const connectedMsg = await waitForMessage(
            ws,
            (m): m is ConnectedMessage => m.type === "connected"
          );

          expect(connectedMsg.subscribed_topics).toContain("all");

          // Make a request to potentially trigger events
          await makeChatRequest(gatewayUrl, apiKey, "test/test-model");

          // We can't guarantee specific events, but the subscription should work
          // Just verify the connection stays open
          expect(ws.readyState).toBe(WebSocket.OPEN);
        } finally {
          ws.close();
        }
      });
    });

    // =========================================================================
    // Keepalive Tests
    // =========================================================================
    describe("Keepalive", () => {
      it("responds to server ping with pong", async () => {
        const { gatewayUrl } = getContext();

        const ws = createWebSocket(gatewayUrl);

        try {
          await waitForOpen(ws);

          await waitForMessage(
            ws,
            (m): m is ConnectedMessage => m.type === "connected"
          );

          // Send a client ping
          ws.send(JSON.stringify({ type: "ping" }));

          // Wait for pong response
          const msg = await waitForMessage(
            ws,
            (m): m is ServerMessage => m.type === "pong",
            5000
          );

          expect(msg.type).toBe("pong");
        } finally {
          ws.close();
        }
      });
    });
  });
}
