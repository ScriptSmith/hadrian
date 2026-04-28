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
import { EventEmitter } from "node:events";
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
 * Per-WebSocket FIFO of messages parsed by the synchronous collector installed
 * in `createWebSocket`. Without this buffer, the server's `connected` reply
 * (sent immediately on upgrade) races the test's `await waitForOpen` and gets
 * dropped on the floor because no listener is registered yet.
 *
 * The collector is the *only* `message` listener; `waitForMessage` consumes
 * from the FIFO and uses an inner emitter to be notified of new arrivals.
 */
interface WsState {
  buffer: ServerMessage[];
  emitter: EventEmitter;
}
const wsState = new WeakMap<WebSocket, WsState>();

/**
 * Helper to create a WebSocket connection to the gateway.
 *
 * Installs a synchronous `message` listener at construction so that messages
 * arriving between socket open and the first `waitForMessage` call are
 * buffered rather than dropped.
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
  const ws = new WebSocket(url);

  const state: WsState = { buffer: [], emitter: new EventEmitter() };
  // Unbounded subscribers are fine here — tests register at most one waiter
  // at a time, but `setMaxListeners(0)` keeps node from warning if a future
  // test adds parallel waiters.
  state.emitter.setMaxListeners(0);
  wsState.set(ws, state);
  ws.on("message", (data: RawData) => {
    try {
      state.buffer.push(JSON.parse(data.toString()) as ServerMessage);
      state.emitter.emit("message");
    } catch {
      // Non-JSON frames are ignored — same behavior as the previous handler.
    }
  });

  return ws;
}

/**
 * Helper to wait for a WebSocket message matching a predicate.
 *
 * Consumes buffered messages first (see `wsState`) before subscribing to new
 * arrivals, so callers can `await waitForOpen()` and then call this helper
 * without racing the server's initial `connected` frame.
 */
function waitForMessage<T extends ServerMessage>(
  ws: WebSocket,
  predicate: (msg: ServerMessage) => msg is T,
  timeoutMs = 10000
): Promise<T> {
  const state = wsState.get(ws);
  if (!state) {
    return Promise.reject(
      new Error("waitForMessage: WebSocket was not created via createWebSocket")
    );
  }

  // Scan the buffer for a matching message, removing it on hit so a follow-up
  // call doesn't re-deliver the same frame.
  const drain = (): T | undefined => {
    for (let i = 0; i < state.buffer.length; i++) {
      const msg = state.buffer[i];
      if (predicate(msg)) {
        state.buffer.splice(i, 1);
        return msg;
      }
    }
    return undefined;
  };

  return new Promise((resolve, reject) => {
    const found = drain();
    if (found) {
      resolve(found);
      return;
    }

    const timeout = setTimeout(() => {
      state.emitter.removeListener("message", onMessage);
      reject(new Error("Timeout waiting for WebSocket message"));
    }, timeoutMs);

    const onMessage = () => {
      const found = drain();
      if (found) {
        clearTimeout(timeout);
        state.emitter.removeListener("message", onMessage);
        resolve(found);
      }
    };

    state.emitter.on("message", onMessage);
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
