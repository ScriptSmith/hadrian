/**
 * MCP (Model Context Protocol) Types
 *
 * Type definitions for the MCP protocol based on the specification.
 * MCP uses JSON-RPC 2.0 for message framing.
 *
 * @see https://modelcontextprotocol.io/
 */

// =============================================================================
// JSON-RPC 2.0 Base Types
// =============================================================================

/** JSON-RPC 2.0 request */
export interface JSONRPCRequest {
  jsonrpc: "2.0";
  id: string | number;
  method: string;
  params?: Record<string, unknown>;
}

/** JSON-RPC 2.0 success response */
export interface JSONRPCSuccessResponse {
  jsonrpc: "2.0";
  id: string | number;
  result: unknown;
}

/** JSON-RPC 2.0 error response */
export interface JSONRPCErrorResponse {
  jsonrpc: "2.0";
  id: string | number | null;
  error: {
    code: number;
    message: string;
    data?: unknown;
  };
}

/** JSON-RPC 2.0 response (success or error) */
export type JSONRPCResponse = JSONRPCSuccessResponse | JSONRPCErrorResponse;

/** JSON-RPC 2.0 notification (no id, no response expected) */
export interface JSONRPCNotification {
  jsonrpc: "2.0";
  method: string;
  params?: Record<string, unknown>;
}

/** Check if response is an error */
export function isJSONRPCError(response: JSONRPCResponse): response is JSONRPCErrorResponse {
  return "error" in response;
}

// =============================================================================
// MCP Protocol Types
// =============================================================================

/** MCP protocol version */
export const MCP_PROTOCOL_VERSION = "2024-11-05";

/** Client capabilities */
export interface ClientCapabilities {
  /** Client supports sampling requests from server */
  sampling?: Record<string, never>;
  /** Client supports tool execution */
  tools?: {
    /** Client can report tool progress */
    progress?: boolean;
  };
  /** Client supports resource subscriptions */
  resources?: {
    /** Client can handle resource subscription updates */
    subscribe?: boolean;
  };
}

/** Server capabilities */
export interface ServerCapabilities {
  /** Server provides tools */
  tools?: {
    /** Server supports tool list change notifications */
    listChanged?: boolean;
  };
  /** Server provides resources */
  resources?: {
    /** Server supports resource subscriptions */
    subscribe?: boolean;
    /** Server supports resource list change notifications */
    listChanged?: boolean;
  };
  /** Server provides prompts */
  prompts?: {
    /** Server supports prompt list change notifications */
    listChanged?: boolean;
  };
  /** Server supports logging */
  logging?: Record<string, never>;
}

/** Client/server info */
export interface Implementation {
  name: string;
  version: string;
}

/** Initialize request params */
export interface InitializeParams {
  protocolVersion: string;
  capabilities: ClientCapabilities;
  clientInfo: Implementation;
}

/** Initialize response result */
export interface InitializeResult {
  protocolVersion: string;
  capabilities: ServerCapabilities;
  serverInfo: Implementation;
  instructions?: string;
}

// =============================================================================
// Tools
// =============================================================================

/** JSON Schema for tool input */
export interface JSONSchema {
  type?: string;
  properties?: Record<string, JSONSchema>;
  required?: string[];
  items?: JSONSchema;
  description?: string;
  enum?: unknown[];
  default?: unknown;
  [key: string]: unknown;
}

/** MCP Tool definition */
export interface MCPToolDefinition {
  /** Unique tool name */
  name: string;
  /** Human-readable description */
  description?: string;
  /** JSON Schema for input parameters */
  inputSchema: JSONSchema;
}

/** Tools list response */
export interface ToolsListResult {
  tools: MCPToolDefinition[];
  /** Cursor for pagination */
  nextCursor?: string;
}

/** Tool content types */
export interface TextContent {
  type: "text";
  text: string;
}

export interface ImageContent {
  type: "image";
  data: string;
  mimeType: string;
}

export interface ResourceContent {
  type: "resource";
  resource: {
    uri: string;
    mimeType?: string;
    text?: string;
    blob?: string;
  };
}

/** Tool call content */
export type ToolContent = TextContent | ImageContent | ResourceContent;

/** Tool call request params */
export interface ToolCallParams {
  name: string;
  arguments?: Record<string, unknown>;
}

/** Tool call response */
export interface ToolCallResult {
  content: ToolContent[];
  /** Whether the tool encountered an error */
  isError?: boolean;
}

// =============================================================================
// Resources
// =============================================================================

/** MCP Resource definition */
export interface MCPResourceDefinition {
  /** Resource URI */
  uri: string;
  /** Human-readable name */
  name: string;
  /** Description */
  description?: string;
  /** MIME type */
  mimeType?: string;
}

/** Resources list response */
export interface ResourcesListResult {
  resources: MCPResourceDefinition[];
  nextCursor?: string;
}

/** Resource read response */
export interface ResourceReadResult {
  contents: Array<{
    uri: string;
    mimeType?: string;
    text?: string;
    blob?: string;
  }>;
}

// =============================================================================
// Prompts
// =============================================================================

/** Prompt argument definition */
export interface PromptArgument {
  name: string;
  description?: string;
  required?: boolean;
}

/** MCP Prompt definition */
export interface MCPPromptDefinition {
  name: string;
  description?: string;
  arguments?: PromptArgument[];
}

/** Prompts list response */
export interface PromptsListResult {
  prompts: MCPPromptDefinition[];
  nextCursor?: string;
}

/** Prompt message */
export interface PromptMessage {
  role: "user" | "assistant";
  content: TextContent | ImageContent | ResourceContent;
}

/** Prompt get response */
export interface PromptGetResult {
  description?: string;
  messages: PromptMessage[];
}

// =============================================================================
// Notifications
// =============================================================================

/** Notification types that servers can send */
export type MCPNotificationType =
  | "notifications/tools/list_changed"
  | "notifications/resources/list_changed"
  | "notifications/resources/updated"
  | "notifications/prompts/list_changed"
  | "notifications/progress"
  | "notifications/message";

// =============================================================================
// MCP-UI Extensions
// =============================================================================

/** MCP-UI resource content types */
export type MCPUIContentType = "text/html" | "text/uri-list" | "application/vnd.mcp-ui.remote-dom";

/** MCP-UI resource */
export interface MCPUIResource {
  type: "ui_resource";
  contentType: MCPUIContentType;
  content: string;
  /** Sandbox permissions for iframe */
  sandbox?: string[];
}

/** Extended tool result that may include UI resources */
export interface MCPUIToolCallResult extends ToolCallResult {
  /** Optional UI resources for rich display */
  uiResources?: MCPUIResource[];
}

// =============================================================================
// Client-side Types
// =============================================================================

/** MCP Server connection status */
export type MCPConnectionStatus = "disconnected" | "connecting" | "connected" | "error";

/** MCP Server configuration */
export interface MCPServerConfig {
  /** Unique identifier */
  id: string;
  /** Display name */
  name: string;
  /** Server URL (HTTP/SSE endpoint) */
  url: string;
  /** Whether the server is enabled */
  enabled: boolean;
  /** Optional HTTP headers for authentication */
  headers?: Record<string, string>;
}

/** MCP Server state (config + runtime state) */
export interface MCPServerState extends MCPServerConfig {
  /** Current connection status */
  status: MCPConnectionStatus;
  /** Error message if status is "error" */
  error?: string;
  /** Server info from initialization */
  serverInfo?: Implementation;
  /** Server capabilities */
  capabilities?: ServerCapabilities;
  /** Discovered tools */
  tools: MCPToolDefinition[];
  /** Discovered resources */
  resources: MCPResourceDefinition[];
  /** Discovered prompts */
  prompts: MCPPromptDefinition[];
  /** Tool enable/disable state (tool name -> enabled) */
  toolsEnabled: Record<string, boolean>;
}

/** Create default server state from config */
export function createServerState(config: MCPServerConfig): MCPServerState {
  return {
    ...config,
    status: "disconnected",
    tools: [],
    resources: [],
    prompts: [],
    toolsEnabled: {},
  };
}
