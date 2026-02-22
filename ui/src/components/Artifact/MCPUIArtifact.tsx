/**
 * MCPUIArtifact - Renders MCP-UI artifacts from external MCP servers
 *
 * Displays rich UI content returned by MCP tools, including:
 * - HTML content (sandboxed iframe)
 * - External URLs (embedded websites)
 * - Remote DOM (server-generated components)
 */

import { Plug } from "lucide-react";
import { MCPUIRenderer, type MCPUIActionHandler } from "@/components/MCPUIRenderer";

/** Data structure for MCP-UI artifacts */
export interface MCPUIArtifactData {
  uri: string;
  mimeType: string;
  text?: string;
  blob?: string;
  serverName?: string;
  toolName?: string;
}

interface MCPUIArtifactProps {
  data: MCPUIArtifactData;
  /** Callback when a tool call is triggered from the UI */
  onToolCall?: (toolName: string, params: Record<string, unknown>) => Promise<unknown>;
  /** Callback when a prompt is requested from the UI */
  onPrompt?: (prompt: string) => void;
}

export function MCPUIArtifact({ data, onToolCall, onPrompt }: MCPUIArtifactProps) {
  // Create action handlers for UI interactions
  const actionHandlers: MCPUIActionHandler = {
    onToolCall,
    onPrompt,
    // Links open in new tab by default (handled by MCPUIRenderer)
  };

  // Convert artifact data to MCPUIResource format
  const resource = {
    uri: data.uri,
    mimeType: data.mimeType,
    text: data.text,
    blob: data.blob,
  };

  return (
    <div className="rounded-lg border bg-card overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b bg-muted/30">
        <Plug className="h-4 w-4 text-muted-foreground" />
        <span className="text-sm font-medium truncate">
          {data.serverName && <span className="text-muted-foreground">{data.serverName}: </span>}
          {data.toolName || "MCP UI"}
        </span>
        <span className="ml-auto text-xs text-muted-foreground">
          {getMimeTypeLabel(data.mimeType)}
        </span>
      </div>

      {/* Content */}
      <div className="p-3">
        <MCPUIRenderer resource={resource} actionHandlers={actionHandlers} autoResize={true} />
      </div>
    </div>
  );
}

/** Get human-readable label for MIME type */
function getMimeTypeLabel(mimeType: string): string {
  if (mimeType === "text/html") return "HTML";
  if (mimeType === "text/uri-list") return "External URL";
  if (mimeType.includes("remote-dom")) return "Remote DOM";
  return mimeType;
}
