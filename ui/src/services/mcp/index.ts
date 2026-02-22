/**
 * MCP (Model Context Protocol) Service
 *
 * Client library for connecting to MCP servers via HTTP/SSE transport.
 *
 * @example
 * ```typescript
 * import { MCPClient } from "@/services/mcp";
 *
 * const client = new MCPClient({
 *   url: "https://mcp-server.example.com",
 *   name: "My Server",
 * });
 *
 * await client.connect();
 * const tools = await client.listAllTools();
 * ```
 */

export {
  MCPClient,
  type MCPClientConfig,
  type StatusCallback,
  type NotificationCallback,
} from "./client";

export {
  // JSON-RPC types
  type JSONRPCRequest,
  type JSONRPCResponse,
  type JSONRPCSuccessResponse,
  type JSONRPCErrorResponse,
  type JSONRPCNotification,
  isJSONRPCError,
  // Protocol constants
  MCP_PROTOCOL_VERSION,
  // Capabilities
  type ClientCapabilities,
  type ServerCapabilities,
  type Implementation,
  // Initialize
  type InitializeParams,
  type InitializeResult,
  // Tools
  type JSONSchema,
  type MCPToolDefinition,
  type ToolsListResult,
  type ToolContent,
  type TextContent,
  type ImageContent,
  type ResourceContent,
  type ToolCallParams,
  type ToolCallResult,
  // Resources
  type MCPResourceDefinition,
  type ResourcesListResult,
  type ResourceReadResult,
  // Prompts
  type PromptArgument,
  type MCPPromptDefinition,
  type PromptsListResult,
  type PromptMessage,
  type PromptGetResult,
  // Notifications
  type MCPNotificationType,
  // MCP-UI
  type MCPUIContentType,
  type MCPUIResource,
  type MCPUIToolCallResult,
  // Client-side types
  type MCPConnectionStatus,
  type MCPServerConfig,
  type MCPServerState,
  createServerState,
} from "./types";
