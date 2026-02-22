/**
 * MCPUIRenderer - Renders MCP-UI resources from tool responses
 *
 * Wraps @mcp-ui/client's UIResourceRenderer to integrate with the chat UI.
 * Handles UI actions (tool calls, prompts, links) by routing them appropriately.
 *
 * ## Supported Content Types
 *
 * - HTML (`text/html`) - Rendered in sandboxed iframe
 * - External URL (`text/uri-list`) - Website embedding
 * - Remote DOM (`application/vnd.mcp-ui.remote-dom`) - Server-generated components
 */

import { useCallback } from "react";
import { UIResourceRenderer, type UIActionResult } from "@mcp-ui/client";
import { cn } from "@/utils/cn";

/** MCP-UI Resource type (matches @mcp-ui/client expectations) */
export interface MCPUIResource {
  /** Resource URI */
  uri: string;
  /** MIME type determining rendering strategy */
  mimeType: string;
  /** Resource content (HTML, URL, or Remote DOM script) */
  text?: string;
  /** Binary content as base64 */
  blob?: string;
}

/** Callback for handling UI actions from rendered content */
export interface MCPUIActionHandler {
  /** Handle a tool call from the UI */
  onToolCall?: (toolName: string, params: Record<string, unknown>) => Promise<unknown>;
  /** Handle a prompt request from the UI */
  onPrompt?: (prompt: string) => void;
  /** Handle a link click from the UI */
  onLink?: (url: string) => void;
  /** Handle an intent from the UI */
  onIntent?: (intent: string, params: Record<string, unknown>) => void;
  /** Handle a notification from the UI */
  onNotify?: (message: string) => void;
}

export interface MCPUIRendererProps {
  /** The MCP-UI resource to render */
  resource: MCPUIResource;
  /** Action handlers for UI interactions */
  actionHandlers?: MCPUIActionHandler;
  /** Additional CSS classes */
  className?: string;
  /** Custom styles for the iframe container */
  style?: React.CSSProperties;
  /** Whether to auto-resize iframe to content */
  autoResize?: boolean;
}

/**
 * MCPUIRenderer - Renders MCP-UI resources with action handling
 *
 * Integrates @mcp-ui/client with the Hadrian chat UI, routing
 * UI actions to appropriate handlers.
 */
export function MCPUIRenderer({
  resource,
  actionHandlers,
  className,
  style,
  autoResize = true,
}: MCPUIRendererProps) {
  // Handle UI actions from the rendered content
  const handleUIAction = useCallback(
    async (result: UIActionResult): Promise<unknown> => {
      switch (result.type) {
        case "tool":
          if (actionHandlers?.onToolCall) {
            return actionHandlers.onToolCall(result.payload.toolName, result.payload.params);
          }
          console.warn("MCP-UI tool call received but no handler provided:", result.payload);
          return { status: "unhandled", reason: "No tool call handler" };

        case "prompt":
          if (actionHandlers?.onPrompt) {
            actionHandlers.onPrompt(result.payload.prompt);
            return { status: "handled" };
          }
          console.warn("MCP-UI prompt received but no handler provided:", result.payload);
          return { status: "unhandled", reason: "No prompt handler" };

        case "link":
          if (actionHandlers?.onLink) {
            actionHandlers.onLink(result.payload.url);
          } else {
            // Default: open link in new tab
            window.open(result.payload.url, "_blank", "noopener,noreferrer");
          }
          return { status: "handled" };

        case "intent":
          if (actionHandlers?.onIntent) {
            actionHandlers.onIntent(result.payload.intent, result.payload.params);
            return { status: "handled" };
          }
          console.warn("MCP-UI intent received but no handler provided:", result.payload);
          return { status: "unhandled", reason: "No intent handler" };

        case "notify":
          if (actionHandlers?.onNotify) {
            actionHandlers.onNotify(result.payload.message);
          } else {
            // Default: log notification
            console.info("MCP-UI notification:", result.payload.message);
          }
          return { status: "handled" };

        default:
          console.warn("Unknown MCP-UI action type:", result);
          return { status: "unhandled", reason: "Unknown action type" };
      }
    },
    [actionHandlers]
  );

  return (
    <div className={cn("mcp-ui-renderer", className)} style={style}>
      <UIResourceRenderer
        resource={resource}
        onUIAction={handleUIAction}
        htmlProps={{
          autoResizeIframe: autoResize,
          style: {
            border: "none",
            borderRadius: "0.375rem",
            width: "100%",
            minHeight: "100px",
          },
        }}
      />
    </div>
  );
}

/**
 * Check if a tool result contains MCP-UI resources
 */
export function hasMCPUIResources(content: unknown[]): boolean {
  if (!Array.isArray(content)) return false;

  return content.some((item) => {
    if (typeof item !== "object" || item === null) return false;
    const obj = item as Record<string, unknown>;

    // Check for resource content type
    if (obj.type === "resource" && typeof obj.resource === "object") {
      const resource = obj.resource as Record<string, unknown>;
      const mimeType = resource.mimeType as string | undefined;

      // Check for MCP-UI supported MIME types
      return (
        mimeType === "text/html" ||
        mimeType === "text/uri-list" ||
        mimeType?.startsWith("application/vnd.mcp-ui")
      );
    }

    return false;
  });
}

/**
 * Extract MCP-UI resources from tool content
 */
export function extractMCPUIResources(content: unknown[]): MCPUIResource[] {
  if (!Array.isArray(content)) return [];

  const resources: MCPUIResource[] = [];

  for (const item of content) {
    if (typeof item !== "object" || item === null) continue;
    const obj = item as Record<string, unknown>;

    if (obj.type === "resource" && typeof obj.resource === "object") {
      const resource = obj.resource as Record<string, unknown>;
      const mimeType = resource.mimeType as string | undefined;

      // Check for MCP-UI supported MIME types
      if (
        mimeType === "text/html" ||
        mimeType === "text/uri-list" ||
        mimeType?.startsWith("application/vnd.mcp-ui")
      ) {
        resources.push({
          uri: (resource.uri as string) || "",
          mimeType: mimeType || "text/html",
          text: resource.text as string | undefined,
          blob: resource.blob as string | undefined,
        });
      }
    }
  }

  return resources;
}
