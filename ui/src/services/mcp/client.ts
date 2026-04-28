import { formatApiError } from "@/utils/formatApiError";
/**
 * MCP Client - Streamable HTTP Transport
 *
 * Client for connecting to MCP (Model Context Protocol) servers using the
 * Streamable HTTP transport (protocol version 2025-03-26).
 *
 * ## Transport Overview
 *
 * The Streamable HTTP transport uses a single endpoint for all communication:
 * - POST: Send JSON-RPC requests/notifications, receive JSON or SSE responses
 * - GET: Open SSE stream for server-initiated messages
 *
 * ## Usage
 *
 * ```typescript
 * const client = new MCPClient({
 *   url: "https://mcp-server.example.com/mcp",
 *   name: "My MCP Server",
 * });
 *
 * await client.connect();
 * const tools = await client.listTools();
 * const result = await client.callTool("my_tool", { arg1: "value" });
 * client.disconnect();
 * ```
 */

import {
  type JSONRPCRequest,
  type JSONRPCResponse,
  type JSONRPCNotification,
  type InitializeParams,
  type InitializeResult,
  type ToolsListResult,
  type ToolCallParams,
  type ToolCallResult,
  type ResourcesListResult,
  type ResourceReadResult,
  type PromptsListResult,
  type PromptGetResult,
  type ServerCapabilities,
  type Implementation,
  type MCPConnectionStatus,
  type MCPNotificationType,
  MCP_PROTOCOL_VERSION,
  isJSONRPCError,
} from "./types";

/** MCP Client configuration */
export interface MCPClientConfig {
  /** Server URL (single endpoint for Streamable HTTP transport) */
  url: string;
  /** Display name */
  name?: string;
  /** Optional HTTP headers for auth */
  headers?: Record<string, string>;
  /** Request timeout in ms (default: 300000) */
  timeout?: number;
  /** Async callback to get a valid access token before each request (for OAuth).
   *  If provided and returns a string, sets Authorization: Bearer <token>.
   *  If returns null, no Authorization header is added. */
  getAccessToken?: () => Promise<string | null>;
}

/** Status change callback */
export type StatusCallback = (status: MCPConnectionStatus, error?: string) => void;

/** Notification callback */
export type NotificationCallback = (method: MCPNotificationType, params?: unknown) => void;

/** Internal message counter */
let messageId = 0;

/** Generate unique message ID */
function nextId(): number {
  return ++messageId;
}

/**
 * MCP Client using Streamable HTTP Transport
 *
 * Implements the MCP Streamable HTTP transport (2025-03-26) which uses:
 * - Single endpoint for POST (requests) and GET (server messages)
 * - Session management via Mcp-Session-Id header
 * - Content negotiation for JSON or SSE responses
 * - Resumability via event IDs
 */
export class MCPClient {
  private config: MCPClientConfig;
  private status: MCPConnectionStatus = "disconnected";
  private error?: string;
  private serverInfo?: Implementation;
  private capabilities?: ServerCapabilities;
  private statusListeners = new Set<StatusCallback>();
  private notificationListeners = new Set<NotificationCallback>();

  // Session management
  private sessionId?: string;

  // SSE stream for server notifications
  private eventSource?: EventSource;
  private lastEventId?: string;

  // SSE reconnection state
  private reconnectTimer?: ReturnType<typeof setTimeout>;
  private sseErrorCount = 0;
  private static readonly MAX_SSE_RECONNECT_ATTEMPTS = 5;
  private static readonly SSE_RECONNECT_BASE_DELAY = 1000;

  constructor(config: MCPClientConfig) {
    this.config = {
      timeout: 300000,
      ...config,
    };
  }

  // ===========================================================================
  // Status Management
  // ===========================================================================

  /** Get current connection status */
  getStatus(): MCPConnectionStatus {
    return this.status;
  }

  /** Get error message if status is "error" */
  getError(): string | undefined {
    return this.error;
  }

  /** Get server info from initialization */
  getServerInfo(): Implementation | undefined {
    return this.serverInfo;
  }

  /** Get server capabilities */
  getCapabilities(): ServerCapabilities | undefined {
    return this.capabilities;
  }

  /** Get session ID */
  getSessionId(): string | undefined {
    return this.sessionId;
  }

  /** Subscribe to status changes */
  onStatusChange(callback: StatusCallback): () => void {
    this.statusListeners.add(callback);
    return () => this.statusListeners.delete(callback);
  }

  /** Subscribe to server notifications */
  onNotification(callback: NotificationCallback): () => void {
    this.notificationListeners.add(callback);
    return () => this.notificationListeners.delete(callback);
  }

  /** Update status and notify listeners */
  private setStatus(status: MCPConnectionStatus, error?: string) {
    this.status = status;
    this.error = error;
    this.statusListeners.forEach((cb) => cb(status, error));
  }

  /** Emit notification to listeners */
  private emitNotification(method: MCPNotificationType, params?: unknown) {
    this.notificationListeners.forEach((cb) => cb(method, params));
  }

  // ===========================================================================
  // Connection Management
  // ===========================================================================

  /**
   * Connect to the MCP server and perform initialization handshake
   */
  async connect(): Promise<void> {
    if (this.status === "connected") {
      return;
    }

    // Clean up any stale state from previous connection attempts
    this.closeEventSource();
    this.clearReconnectTimer();
    this.sessionId = undefined;
    this.sseErrorCount = 0;

    this.setStatus("connecting");

    try {
      // Perform initialization handshake
      const initParams: InitializeParams = {
        protocolVersion: MCP_PROTOCOL_VERSION,
        capabilities: {
          tools: { progress: false },
          resources: { subscribe: false },
        },
        clientInfo: {
          name: "hadrian-gateway-ui",
          version: "1.0.0",
        },
      };

      const { result, sessionId } = await this.sendRequest<InitializeResult>(
        "initialize",
        initParams as unknown as Record<string, unknown>
      );

      // Store session ID if provided
      if (sessionId) {
        this.sessionId = sessionId;
      }

      this.serverInfo = result.serverInfo;
      this.capabilities = result.capabilities;

      // Send initialized notification
      await this.sendNotification("notifications/initialized");

      // Set up SSE for server notifications if supported
      this.setupServerMessageStream();

      this.setStatus("connected");
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : formatApiError(err);
      this.setStatus("error", errorMsg);
      throw err;
    }
  }

  /**
   * Disconnect from the server
   */
  async disconnect(): Promise<void> {
    this.closeEventSource();
    this.clearReconnectTimer();

    // Send DELETE to terminate session if we have a session ID
    if (this.sessionId) {
      try {
        const headers: Record<string, string> = {
          "Mcp-Session-Id": this.sessionId,
          ...this.config.headers,
        };
        if (this.config.getAccessToken) {
          try {
            const token = await this.config.getAccessToken();
            if (token) headers["Authorization"] = `Bearer ${token}`;
          } catch {
            // Best effort during disconnect
          }
        }
        await fetch(this.config.url, { method: "DELETE", headers });
      } catch {
        // Ignore errors during disconnect
      }
    }

    this.sessionId = undefined;
    this.lastEventId = undefined;
    this.serverInfo = undefined;
    this.capabilities = undefined;
    this.sseErrorCount = 0;
    this.setStatus("disconnected");
  }

  /** Close the SSE EventSource if open */
  private closeEventSource(): void {
    if (this.eventSource) {
      this.eventSource.onmessage = null;
      this.eventSource.onerror = null;
      this.eventSource.close();
      this.eventSource = undefined;
    }
  }

  /** Clear any pending reconnection timer */
  private clearReconnectTimer(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = undefined;
    }
  }

  /**
   * Check if connected
   */
  isConnected(): boolean {
    return this.status === "connected";
  }

  // ===========================================================================
  // Server Message Stream (GET endpoint)
  // ===========================================================================

  /**
   * Set up SSE stream for server-initiated messages
   *
   * In Streamable HTTP transport, clients can GET the endpoint to open
   * an SSE stream for receiving server notifications and requests.
   *
   * We disable the browser's native EventSource auto-reconnect and handle
   * reconnection ourselves with exponential backoff so we can detect when
   * the session has expired and perform a full re-initialization.
   */
  private setupServerMessageStream(): void {
    // Only set up if server supports notifications
    const supportsNotifications =
      this.capabilities?.tools?.listChanged ||
      this.capabilities?.resources?.listChanged ||
      this.capabilities?.prompts?.listChanged;

    if (!supportsNotifications) {
      return;
    }

    // Close existing EventSource before opening a new one
    this.closeEventSource();

    try {
      const url = new URL(this.config.url);
      if (this.sessionId) {
        url.searchParams.set("sessionId", this.sessionId);
      }
      if (this.lastEventId) {
        url.searchParams.set("lastEventId", this.lastEventId);
      }

      const es = new EventSource(url.toString());
      this.eventSource = es;

      es.onopen = () => {
        // Reset error count on successful connection
        this.sseErrorCount = 0;
      };

      es.onmessage = (event) => {
        if (event.lastEventId) {
          this.lastEventId = event.lastEventId;
        }

        try {
          const message = JSON.parse(event.data) as JSONRPCNotification;
          if (message.method) {
            this.emitNotification(message.method as MCPNotificationType, message.params);
          }
        } catch {
          // Ignore parse errors
        }
      };

      es.onerror = () => {
        // Close the EventSource immediately to prevent browser auto-reconnect
        // We manage reconnection ourselves to handle session expiry properly
        this.closeEventSource();

        this.sseErrorCount++;

        if (this.sseErrorCount > MCPClient.MAX_SSE_RECONNECT_ATTEMPTS) {
          console.warn(
            `MCP SSE: giving up after ${MCPClient.MAX_SSE_RECONNECT_ATTEMPTS} reconnect attempts`
          );
          // SSE is not critical — the connection is still usable for requests.
          // Just stop trying to reopen the notification stream.
          return;
        }

        // Exponential backoff: 1s, 2s, 4s, 8s, 16s
        const delay = MCPClient.SSE_RECONNECT_BASE_DELAY * Math.pow(2, this.sseErrorCount - 1);
        console.debug(`MCP SSE: reconnecting in ${delay}ms (attempt ${this.sseErrorCount})`);

        this.clearReconnectTimer();
        this.reconnectTimer = setTimeout(() => {
          // Only reconnect if we're still supposed to be connected
          if (this.status === "connected") {
            this.setupServerMessageStream();
          }
        }, delay);
      };
    } catch {
      console.warn("Failed to set up MCP server message stream");
    }
  }

  // ===========================================================================
  // JSON-RPC Transport (Streamable HTTP)
  // ===========================================================================

  /**
   * Send a JSON-RPC request and wait for response
   *
   * Uses Streamable HTTP transport:
   * - POST with Accept: application/json, text/event-stream
   * - Handles both JSON and SSE responses based on Content-Type
   * - Includes Mcp-Session-Id header if session established
   */
  private async sendRequest<T>(
    method: string,
    params?: Record<string, unknown>
  ): Promise<{ result: T; sessionId?: string }> {
    const request: JSONRPCRequest = {
      jsonrpc: "2.0",
      id: nextId(),
      method,
      params,
    };

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.config.timeout);

    try {
      const headers: Record<string, string> = {
        "Content-Type": "application/json",
        Accept: "application/json,text/event-stream",
        ...this.config.headers,
      };

      // Inject OAuth token if provider is configured
      if (this.config.getAccessToken) {
        const token = await this.config.getAccessToken();
        if (token) headers["Authorization"] = `Bearer ${token}`;
      }

      // Include session ID if we have one
      if (this.sessionId) {
        headers["Mcp-Session-Id"] = this.sessionId;
      }

      const response = await fetch(this.config.url, {
        method: "POST",
        headers,
        body: JSON.stringify(request),
        signal: controller.signal,
      });

      if (!response.ok) {
        // 404/410 means the session expired — the caller should reconnect
        if (response.status === 404 || response.status === 410) {
          this.sessionId = undefined;
          this.closeEventSource();
          this.setStatus("disconnected");
          throw new Error("MCP session expired — please reconnect");
        }
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      // Get session ID from response header
      const responseSessionId = response.headers.get("Mcp-Session-Id") || undefined;

      // Handle response based on Content-Type
      const contentType = response.headers.get("Content-Type") || "";

      if (contentType.includes("text/event-stream")) {
        // SSE response - parse the stream for our response
        const result = await this.parseSSEResponse<T>(response, request.id);
        return { result, sessionId: responseSessionId };
      } else {
        // JSON response
        const jsonResponse = (await response.json()) as JSONRPCResponse;

        if (isJSONRPCError(jsonResponse)) {
          throw new Error(`MCP Error ${jsonResponse.error.code}: ${jsonResponse.error.message}`);
        }

        return { result: jsonResponse.result as T, sessionId: responseSessionId };
      }
    } catch (err) {
      if (err instanceof Error && err.name === "AbortError") {
        throw new Error(`MCP request timed out after ${this.config.timeout}ms`);
      }
      throw err;
    } finally {
      clearTimeout(timeoutId);
    }
  }

  /**
   * Parse SSE response stream for a specific request ID
   */
  private async parseSSEResponse<T>(response: Response, requestId: string | number): Promise<T> {
    const reader = response.body?.getReader();
    if (!reader) {
      throw new Error("No response body for SSE stream");
    }

    const decoder = new TextDecoder();
    let buffer = "";

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          // Parse SSE event
          if (line.startsWith("data: ")) {
            const data = line.slice(6).trim();
            if (!data) continue;

            try {
              const message = JSON.parse(data) as JSONRPCResponse | JSONRPCNotification;

              // Check if this is the response we're waiting for
              if ("id" in message && message.id === requestId) {
                if (isJSONRPCError(message)) {
                  throw new Error(`MCP Error ${message.error.code}: ${message.error.message}`);
                }
                return message.result as T;
              }

              // Handle notifications received during response
              if ("method" in message && !("id" in message)) {
                this.emitNotification(
                  message.method as MCPNotificationType,
                  (message as JSONRPCNotification).params
                );
              }
            } catch (parseErr) {
              if (parseErr instanceof Error && parseErr.message.startsWith("MCP Error")) {
                throw parseErr;
              }
              // Ignore other parse errors, continue reading
            }
          } else if (line.startsWith("id: ")) {
            // Track event ID for resumability
            this.lastEventId = line.slice(4).trim();
          }
        }
      }
    } finally {
      reader.releaseLock();
    }

    throw new Error("SSE stream ended without response");
  }

  /**
   * Send a JSON-RPC notification (no response expected)
   *
   * For notifications-only POST, server should return 202 Accepted
   */
  private async sendNotification(method: string, params?: Record<string, unknown>): Promise<void> {
    const notification: JSONRPCNotification = {
      jsonrpc: "2.0",
      method,
      params,
    };

    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      Accept: "application/json,text/event-stream",
      ...this.config.headers,
    };

    // Inject OAuth token if provider is configured
    if (this.config.getAccessToken) {
      try {
        const token = await this.config.getAccessToken();
        if (token) headers["Authorization"] = `Bearer ${token}`;
      } catch {
        // Best effort for notifications
      }
    }

    if (this.sessionId) {
      headers["Mcp-Session-Id"] = this.sessionId;
    }

    // Fire and forget - expect 202 Accepted
    try {
      await fetch(this.config.url, {
        method: "POST",
        headers,
        body: JSON.stringify(notification),
      });
    } catch {
      // Ignore notification send failures
    }
  }

  // ===========================================================================
  // MCP Protocol Methods
  // ===========================================================================

  /**
   * List available tools
   */
  async listTools(cursor?: string): Promise<ToolsListResult> {
    this.ensureConnected();
    const { result } = await this.sendRequest<ToolsListResult>(
      "tools/list",
      cursor ? { cursor } : undefined
    );
    return result;
  }

  /**
   * List all tools (handles pagination)
   */
  async listAllTools(): Promise<ToolsListResult["tools"]> {
    const tools: ToolsListResult["tools"] = [];
    let cursor: string | undefined;

    do {
      const result = await this.listTools(cursor);
      tools.push(...result.tools);
      cursor = result.nextCursor;
    } while (cursor);

    return tools;
  }

  /**
   * Call a tool
   */
  async callTool(name: string, args?: Record<string, unknown>): Promise<ToolCallResult> {
    this.ensureConnected();
    const params: ToolCallParams = {
      name,
      arguments: args,
    };
    const { result } = await this.sendRequest<ToolCallResult>(
      "tools/call",
      params as unknown as Record<string, unknown>
    );
    return result;
  }

  /**
   * List available resources
   */
  async listResources(cursor?: string): Promise<ResourcesListResult> {
    this.ensureConnected();
    const { result } = await this.sendRequest<ResourcesListResult>(
      "resources/list",
      cursor ? { cursor } : undefined
    );
    return result;
  }

  /**
   * List all resources (handles pagination)
   */
  async listAllResources(): Promise<ResourcesListResult["resources"]> {
    const resources: ResourcesListResult["resources"] = [];
    let cursor: string | undefined;

    do {
      const result = await this.listResources(cursor);
      resources.push(...result.resources);
      cursor = result.nextCursor;
    } while (cursor);

    return resources;
  }

  /**
   * Read a resource
   */
  async readResource(uri: string): Promise<ResourceReadResult> {
    this.ensureConnected();
    const { result } = await this.sendRequest<ResourceReadResult>("resources/read", { uri });
    return result;
  }

  /**
   * List available prompts
   */
  async listPrompts(cursor?: string): Promise<PromptsListResult> {
    this.ensureConnected();
    const { result } = await this.sendRequest<PromptsListResult>(
      "prompts/list",
      cursor ? { cursor } : undefined
    );
    return result;
  }

  /**
   * List all prompts (handles pagination)
   */
  async listAllPrompts(): Promise<PromptsListResult["prompts"]> {
    const prompts: PromptsListResult["prompts"] = [];
    let cursor: string | undefined;

    do {
      const result = await this.listPrompts(cursor);
      prompts.push(...result.prompts);
      cursor = result.nextCursor;
    } while (cursor);

    return prompts;
  }

  /**
   * Get a prompt with arguments
   */
  async getPrompt(name: string, args?: Record<string, string>): Promise<PromptGetResult> {
    this.ensureConnected();
    const { result } = await this.sendRequest<PromptGetResult>("prompts/get", {
      name,
      arguments: args,
    });
    return result;
  }

  /**
   * Ping the server to check connectivity
   */
  async ping(): Promise<void> {
    this.ensureConnected();
    await this.sendRequest("ping");
  }

  // ===========================================================================
  // Helpers
  // ===========================================================================

  private ensureConnected(): void {
    if (this.status !== "connected") {
      throw new Error("MCP client not connected. Call connect() first.");
    }
  }
}
