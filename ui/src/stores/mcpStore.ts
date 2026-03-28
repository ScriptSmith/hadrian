/**
 * MCP Store - Model Context Protocol Server Management
 *
 * Manages MCP server connections, discovered tools, and connection state.
 * Server configurations are persisted to localStorage while runtime state
 * (connection status, discovered tools) is ephemeral.
 *
 * ## Architecture
 *
 * ```
 * ┌─────────────────────────────────────────────────────────────────┐
 * │                         mcpStore                                │
 * ├─────────────────────────────────────────────────────────────────┤
 * │  servers: MCPServerState[]     - Server configs + runtime state │
 * │  clients: Map<string, MCPClient> - Active client instances      │
 * ├─────────────────────────────────────────────────────────────────┤
 * │  addServer()      - Add new server config                       │
 * │  removeServer()   - Remove server and disconnect                │
 * │  updateServer()   - Update server config                        │
 * │  connectServer()  - Connect and discover tools                  │
 * │  disconnectServer() - Disconnect from server                    │
 * │  toggleServerEnabled() - Enable/disable server                  │
 * │  setToolEnabled() - Enable/disable individual tool              │
 * └─────────────────────────────────────────────────────────────────┘
 * ```
 *
 * ## Persistence
 *
 * Only server configurations are persisted (id, name, url, enabled, headers, timeout).
 * Runtime state (status, tools, resources) is reset on page reload.
 */

import { create } from "zustand";
import { persist } from "zustand/middleware";

import {
  MCPClient,
  type MCPServerConfig,
  type MCPServerState,
  type MCPConnectionStatus,
  type MCPToolDefinition,
  createServerState,
} from "@/services/mcp";

// =============================================================================
// Types
// =============================================================================

interface MCPState {
  /** Server states (config + runtime) */
  servers: MCPServerState[];
}

interface MCPActions {
  /** Add a new MCP server */
  addServer: (config: Omit<MCPServerConfig, "id">) => string;
  /** Remove an MCP server by ID */
  removeServer: (serverId: string) => void;
  /** Update server configuration */
  updateServer: (serverId: string, updates: Partial<Omit<MCPServerConfig, "id">>) => void;
  /** Connect to an MCP server and discover its tools */
  connectServer: (serverId: string) => Promise<void>;
  /** Disconnect from an MCP server */
  disconnectServer: (serverId: string) => void;
  /** Toggle server enabled state */
  toggleServerEnabled: (serverId: string) => void;
  /** Enable or disable a specific tool on a server */
  setToolEnabled: (serverId: string, toolName: string, enabled: boolean) => void;
  /** Update server connection status (internal) */
  _setServerStatus: (serverId: string, status: MCPConnectionStatus, error?: string) => void;
  /** Update server tools after discovery (internal) */
  _setServerTools: (serverId: string, tools: MCPToolDefinition[]) => void;
  /** Get all enabled tools across all connected servers */
  getEnabledTools: () => Array<{ server: MCPServerState; tool: MCPToolDefinition }>;
  /** Connect all enabled servers that aren't already connected/connecting */
  ensureConnected: () => Promise<void>;
  /** Disconnect all servers */
  disconnectAll: () => void;
  /** Disconnect all per-conversation MCP sessions for a conversation */
  disconnectConversation: (conversationId: string) => void;
}

export type MCPStore = MCPState & MCPActions;

// =============================================================================
// Client Management (outside Zustand for reference stability)
// =============================================================================

/** Global clients: Map of server ID to MCPClient instance (for discovery/status) */
const globalClients = new Map<string, MCPClient>();

/** Global listener cleanups: Map of server ID to cleanup functions */
const globalListenerCleanups = new Map<string, Array<() => void>>();

/** Per-conversation clients: Map of `${serverId}::${conversationId}` to MCPClient */
const conversationClients = new Map<string, MCPClient>();

/** Dedup map to prevent concurrent lazy-connect calls for the same key */
const connectingPromises = new Map<string, Promise<MCPClient>>();

/** Composite key for conversation clients */
function clientKey(serverId: string, conversationId: string): string {
  return `${serverId}::${conversationId}`;
}

/** Get or create a global client for a server */
function getClient(server: MCPServerConfig): MCPClient {
  let client = globalClients.get(server.id);
  if (!client) {
    client = new MCPClient({
      url: server.url,
      name: server.name,
      headers: server.headers,
      timeout: server.timeout,
    });
    globalClients.set(server.id, client);
  }
  return client;
}

/** Create a new client for a per-conversation session */
function getConversationClient(server: MCPServerConfig, conversationId: string): MCPClient {
  const key = clientKey(server.id, conversationId);
  let client = conversationClients.get(key);
  if (!client) {
    client = new MCPClient({
      url: server.url,
      name: server.name,
      headers: server.headers,
      timeout: server.timeout,
    });
    conversationClients.set(key, client);
  }
  return client;
}

/** Get or connect a per-conversation client, deduplicating concurrent calls */
async function ensureConversationClient(
  serverId: string,
  conversationId: string
): Promise<MCPClient> {
  const key = clientKey(serverId, conversationId);
  const existing = conversationClients.get(key);
  if (existing?.isConnected()) return existing;

  const pending = connectingPromises.get(key);
  if (pending) return pending;

  const promise = (async () => {
    const server = useMCPStore.getState().servers.find((s) => s.id === serverId);
    if (!server) throw new Error(`Server not found: ${serverId}`);
    const client = getConversationClient(server, conversationId);
    await client.connect();
    return client;
  })();

  connectingPromises.set(key, promise);
  try {
    return await promise;
  } finally {
    connectingPromises.delete(key);
  }
}

/** Disconnect and remove a single per-conversation client */
function removeConversationClient(serverId: string, conversationId: string): void {
  const key = clientKey(serverId, conversationId);
  const client = conversationClients.get(key);
  if (client) {
    client.disconnect();
    conversationClients.delete(key);
  }
}

/** Disconnect all per-conversation clients for a given conversation */
function removeAllClientsForConversation(conversationId: string): void {
  const suffix = `::${conversationId}`;
  for (const [key, client] of conversationClients) {
    if (key.endsWith(suffix)) {
      client.disconnect();
      conversationClients.delete(key);
    }
  }
}

/** Disconnect all per-conversation clients for a given server */
function removeAllConversationClientsForServer(serverId: string): void {
  const prefix = `${serverId}::`;
  for (const [key, client] of conversationClients) {
    if (key.startsWith(prefix)) {
      client.disconnect();
      conversationClients.delete(key);
    }
  }
}

/** Remove listener subscriptions for a server */
function cleanupListeners(serverId: string): void {
  const cleanups = globalListenerCleanups.get(serverId);
  if (cleanups) {
    cleanups.forEach((fn) => fn());
    globalListenerCleanups.delete(serverId);
  }
}

/** Remove and disconnect a global client */
function removeClient(serverId: string): void {
  cleanupListeners(serverId);
  const client = globalClients.get(serverId);
  if (client) {
    client.disconnect();
    globalClients.delete(serverId);
  }
}

/** Generate unique server ID */
function generateServerId(): string {
  return `mcp-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

// =============================================================================
// Store
// =============================================================================

export const useMCPStore = create<MCPStore>()(
  persist(
    (set, get) => ({
      // =========================================================================
      // State
      // =========================================================================
      servers: [],

      // =========================================================================
      // Actions
      // =========================================================================

      addServer: (config) => {
        const id = generateServerId();
        const serverConfig: MCPServerConfig = {
          ...config,
          id,
          enabled: config.enabled ?? true,
        };
        const serverState = createServerState(serverConfig);

        set((state) => ({
          servers: [...state.servers, serverState],
        }));

        // Auto-connect if enabled
        if (serverConfig.enabled) {
          get()
            .connectServer(id)
            .catch((err) => {
              console.debug("MCP auto-connect failed:", err);
            });
        }

        return id;
      },

      removeServer: (serverId) => {
        removeClient(serverId);
        removeAllConversationClientsForServer(serverId);
        set((state) => ({
          servers: state.servers.filter((s) => s.id !== serverId),
        }));
      },

      updateServer: (serverId, updates) => {
        // If URL or headers change, we need to recreate the client
        const server = get().servers.find((s) => s.id === serverId);
        if (server && (updates.url || updates.headers || updates.timeout)) {
          removeClient(serverId);
          removeAllConversationClientsForServer(serverId);
        }

        set((state) => ({
          servers: state.servers.map((s) => (s.id === serverId ? { ...s, ...updates } : s)),
        }));
      },

      connectServer: async (serverId) => {
        const server = get().servers.find((s) => s.id === serverId);
        if (!server) {
          throw new Error(`Server not found: ${serverId}`);
        }

        // Clean up any existing listeners before (re)connecting
        cleanupListeners(serverId);

        // Update status to connecting
        get()._setServerStatus(serverId, "connecting");

        try {
          const client = getClient(server);

          // Set up status listener (tracked for cleanup)
          const cleanups: Array<() => void> = [];

          cleanups.push(
            client.onStatusChange((status, error) => {
              get()._setServerStatus(serverId, status, error);
            })
          );

          // Set up notification listener so tool/resource/prompt changes
          // are automatically re-discovered
          cleanups.push(
            client.onNotification(async (method) => {
              const c = globalClients.get(serverId);
              if (!c?.isConnected()) return;

              try {
                if (method === "notifications/tools/list_changed") {
                  const tools = await c.listAllTools();
                  get()._setServerTools(serverId, tools);
                }
                // Future: handle resources/prompts list_changed similarly
              } catch (err) {
                console.debug("MCP notification handler error:", err);
              }
            })
          );

          globalListenerCleanups.set(serverId, cleanups);

          // Connect to server
          await client.connect();

          // Discover tools
          const tools = await client.listAllTools();
          get()._setServerTools(serverId, tools);

          // Enable all tools by default (preserve existing preferences)
          set((state) => ({
            servers: state.servers.map((s) => {
              if (s.id !== serverId) return s;
              const toolsEnabled: Record<string, boolean> = { ...s.toolsEnabled };
              for (const t of tools) {
                // Only set default for tools we haven't seen before
                if (!(t.name in toolsEnabled)) {
                  toolsEnabled[t.name] = true;
                }
              }
              return { ...s, toolsEnabled };
            }),
          }));
        } catch (err) {
          const errorMsg = err instanceof Error ? err.message : String(err);
          get()._setServerStatus(serverId, "error", errorMsg);
          throw err;
        }
      },

      disconnectServer: (serverId) => {
        removeClient(serverId);
        set((state) => ({
          servers: state.servers.map((s) =>
            s.id === serverId
              ? {
                  ...s,
                  status: "disconnected" as MCPConnectionStatus,
                  error: undefined,
                  tools: [],
                  resources: [],
                  prompts: [],
                  serverInfo: undefined,
                  capabilities: undefined,
                }
              : s
          ),
        }));
      },

      toggleServerEnabled: (serverId) => {
        const server = get().servers.find((s) => s.id === serverId);
        if (!server) return;

        const newEnabled = !server.enabled;

        // If disabling, disconnect global + all conversation clients
        if (!newEnabled && server.status === "connected") {
          get().disconnectServer(serverId);
          removeAllConversationClientsForServer(serverId);
        }

        set((state) => ({
          servers: state.servers.map((s) =>
            s.id === serverId ? { ...s, enabled: newEnabled } : s
          ),
        }));

        // If enabling, auto-connect
        if (newEnabled) {
          get()
            .connectServer(serverId)
            .catch((err) => {
              console.debug("MCP auto-connect failed:", err);
            });
        }
      },

      setToolEnabled: (serverId, toolName, enabled) => {
        set((state) => ({
          servers: state.servers.map((s) =>
            s.id === serverId
              ? {
                  ...s,
                  toolsEnabled: {
                    ...s.toolsEnabled,
                    [toolName]: enabled,
                  },
                }
              : s
          ),
        }));
      },

      _setServerStatus: (serverId, status, error) => {
        set((state) => ({
          servers: state.servers.map((s) => (s.id === serverId ? { ...s, status, error } : s)),
        }));
      },

      _setServerTools: (serverId, tools) => {
        set((state) => ({
          servers: state.servers.map((s) => (s.id === serverId ? { ...s, tools } : s)),
        }));
      },

      getEnabledTools: () => {
        const { servers } = get();
        const result: Array<{ server: MCPServerState; tool: MCPToolDefinition }> = [];

        for (const server of servers) {
          if (!server.enabled || server.status !== "connected") continue;

          for (const tool of server.tools) {
            if (server.toolsEnabled[tool.name] !== false) {
              result.push({ server, tool });
            }
          }
        }

        return result;
      },

      ensureConnected: async () => {
        const { servers } = get();
        const needsConnect = servers.filter(
          (s) => s.enabled && s.status !== "connected" && s.status !== "connecting"
        );
        await Promise.all(
          needsConnect.map((s) =>
            get()
              .connectServer(s.id)
              .catch((err) => {
                console.debug(`MCP auto-connect failed for ${s.name}:`, err);
              })
          )
        );
      },

      disconnectAll: () => {
        const { servers } = get();
        for (const server of servers) {
          if (server.status !== "disconnected") {
            removeClient(server.id);
          }
        }
        // Also disconnect all per-conversation clients
        for (const [, client] of conversationClients) {
          client.disconnect();
        }
        conversationClients.clear();
        set((state) => ({
          servers: state.servers.map((s) => ({
            ...s,
            status: "disconnected" as MCPConnectionStatus,
            error: undefined,
            tools: [],
            resources: [],
            prompts: [],
            serverInfo: undefined,
            capabilities: undefined,
          })),
        }));
      },

      disconnectConversation: (conversationId) => {
        removeAllClientsForConversation(conversationId);
      },
    }),
    {
      name: "hadrian-mcp-servers",
      // Only persist config fields, not runtime state
      partialize: (state) => ({
        servers: state.servers.map((s) => ({
          id: s.id,
          name: s.name,
          url: s.url,
          enabled: s.enabled,
          headers: s.headers,
          timeout: s.timeout,
          // Persist tool enable/disable preferences
          toolsEnabled: s.toolsEnabled,
        })),
      }),
      // Rehydrate with full state structure
      onRehydrateStorage: () => (state) => {
        if (state) {
          // Convert persisted configs back to full server states
          state.servers = state.servers.map((s) => ({
            ...createServerState(s as MCPServerConfig),
            toolsEnabled: s.toolsEnabled || {},
          }));
        }
      },
    }
  )
);

// =============================================================================
// Selectors
// =============================================================================

/** Get all MCP servers */
export const useMCPServers = () => useMCPStore((state) => state.servers);

/** Get a specific server by ID */
export const useMCPServer = (serverId: string) =>
  useMCPStore((state) => state.servers.find((s) => s.id === serverId));

/** Get connection status for a server */
export const useMCPConnectionStatus = (serverId: string) =>
  useMCPStore((state) => {
    const server = state.servers.find((s) => s.id === serverId);
    return server?.status ?? "disconnected";
  });

/** Get all enabled tools across all connected servers */
export const useEnabledMCPTools = () =>
  useMCPStore((state) => {
    const result: Array<{ server: MCPServerState; tool: MCPToolDefinition }> = [];

    for (const server of state.servers) {
      if (!server.enabled || server.status !== "connected") continue;

      for (const tool of server.tools) {
        if (server.toolsEnabled[tool.name] !== false) {
          result.push({ server, tool });
        }
      }
    }

    return result;
  });

/** Get count of connected servers */
export const useConnectedServerCount = () =>
  useMCPStore((state) => state.servers.filter((s) => s.status === "connected").length);

/** Get count of enabled servers */
export const useEnabledServerCount = () =>
  useMCPStore((state) => state.servers.filter((s) => s.enabled).length);

/** Get total tool count across all connected servers */
export const useMCPToolCount = () =>
  useMCPStore((state) =>
    state.servers
      .filter((s) => s.enabled && s.status === "connected")
      .reduce((sum, s) => sum + s.tools.length, 0)
  );

/** Check if any server has an error */
export const useHasMCPError = () =>
  useMCPStore((state) => state.servers.some((s) => s.status === "error"));

/** Get all server errors */
export const useMCPErrors = () =>
  useMCPStore((state) =>
    state.servers
      .filter((s) => s.status === "error" && s.error)
      .map((s) => ({ serverId: s.id, name: s.name, error: s.error! }))
  );

// =============================================================================
// Utility Functions
// =============================================================================

/** Get MCPClient instance for a server (global or per-conversation) */
export function getMCPClient(serverId: string, conversationId?: string): MCPClient | undefined {
  if (conversationId) {
    return conversationClients.get(clientKey(serverId, conversationId));
  }
  return globalClients.get(serverId);
}

/**
 * Call a tool on an MCP server, auto-reconnecting on session expiry.
 * If conversationId is provided, uses a per-conversation session (lazily created).
 * Otherwise falls back to the global client.
 */
export async function callMCPTool(
  serverId: string,
  toolName: string,
  args?: Record<string, unknown>,
  conversationId?: string
) {
  if (conversationId) {
    return callMCPToolWithConversationClient(serverId, toolName, args, conversationId);
  }
  return callMCPToolWithGlobalClient(serverId, toolName, args);
}

/** Call a tool using the global client (existing behavior) */
async function callMCPToolWithGlobalClient(
  serverId: string,
  toolName: string,
  args?: Record<string, unknown>
) {
  const client = globalClients.get(serverId);
  if (!client) {
    throw new Error(`No client for server: ${serverId}`);
  }
  if (!client.isConnected()) {
    throw new Error(`Server not connected: ${serverId}`);
  }

  try {
    return await client.callTool(toolName, args);
  } catch (err) {
    // Auto-reconnect once on session expiry, then retry the call
    if (err instanceof Error && err.message.includes("session expired")) {
      console.debug("MCP session expired during tool call, reconnecting…");
      const store = useMCPStore.getState();
      await store.connectServer(serverId);
      const newClient = globalClients.get(serverId);
      if (!newClient?.isConnected()) {
        throw new Error(`Reconnection failed for server: ${serverId}`);
      }
      return newClient.callTool(toolName, args);
    }
    throw err;
  }
}

/** Call a tool using a per-conversation client (lazy init, auto-reconnect) */
async function callMCPToolWithConversationClient(
  serverId: string,
  toolName: string,
  args: Record<string, unknown> | undefined,
  conversationId: string
) {
  let client = await ensureConversationClient(serverId, conversationId);

  try {
    return await client.callTool(toolName, args);
  } catch (err) {
    if (err instanceof Error && err.message.includes("session expired")) {
      console.debug("MCP conversation session expired, reconnecting…");
      removeConversationClient(serverId, conversationId);
      client = await ensureConversationClient(serverId, conversationId);
      return client.callTool(toolName, args);
    }
    throw err;
  }
}
