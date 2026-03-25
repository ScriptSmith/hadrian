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
 * Only server configurations are persisted (id, name, url, enabled, headers).
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
}

export type MCPStore = MCPState & MCPActions;

// =============================================================================
// Client Management (outside Zustand for reference stability)
// =============================================================================

/** Map of server ID to MCPClient instance */
const clients = new Map<string, MCPClient>();

/** Map of server ID to cleanup functions for listeners */
const listenerCleanups = new Map<string, Array<() => void>>();

/** Get or create a client for a server */
function getClient(server: MCPServerConfig): MCPClient {
  let client = clients.get(server.id);
  if (!client) {
    client = new MCPClient({
      url: server.url,
      name: server.name,
      headers: server.headers,
    });
    clients.set(server.id, client);
  }
  return client;
}

/** Remove listener subscriptions for a server */
function cleanupListeners(serverId: string): void {
  const cleanups = listenerCleanups.get(serverId);
  if (cleanups) {
    cleanups.forEach((fn) => fn());
    listenerCleanups.delete(serverId);
  }
}

/** Remove and disconnect a client */
function removeClient(serverId: string): void {
  cleanupListeners(serverId);
  const client = clients.get(serverId);
  if (client) {
    client.disconnect();
    clients.delete(serverId);
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
        set((state) => ({
          servers: state.servers.filter((s) => s.id !== serverId),
        }));
      },

      updateServer: (serverId, updates) => {
        // If URL or headers change, we need to recreate the client
        const server = get().servers.find((s) => s.id === serverId);
        if (server && (updates.url || updates.headers)) {
          removeClient(serverId);
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
              const c = clients.get(serverId);
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

          listenerCleanups.set(serverId, cleanups);

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

        // If disabling, disconnect
        if (!newEnabled && server.status === "connected") {
          get().disconnectServer(serverId);
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

/** Get MCPClient instance for a server (for making tool calls) */
export function getMCPClient(serverId: string): MCPClient | undefined {
  return clients.get(serverId);
}

/** Call a tool on an MCP server, auto-reconnecting on session expiry */
export async function callMCPTool(
  serverId: string,
  toolName: string,
  args?: Record<string, unknown>
) {
  const client = clients.get(serverId);
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
      // Retry with the (possibly new) client
      const newClient = clients.get(serverId);
      if (!newClient?.isConnected()) {
        throw new Error(`Reconnection failed for server: ${serverId}`);
      }
      return newClient.callTool(toolName, args);
    }
    throw err;
  }
}
