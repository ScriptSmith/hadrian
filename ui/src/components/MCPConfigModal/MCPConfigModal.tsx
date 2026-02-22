/**
 * MCPConfigModal - Configuration modal for MCP (Model Context Protocol) servers
 *
 * Allows users to:
 * - Add/edit/remove MCP server configurations
 * - Connect/disconnect from servers
 * - Enable/disable servers and individual tools
 * - View connection status and discovered tools
 */

import { useState, useCallback } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import {
  AlertCircle,
  Check,
  ChevronDown,
  ChevronRight,
  Loader2,
  Pencil,
  Plug,
  Plus,
  Power,
  PowerOff,
  Trash2,
  Wrench,
} from "lucide-react";

import { Button } from "@/components/Button/Button";
import { FormField } from "@/components/FormField/FormField";
import { Input } from "@/components/Input/Input";
import {
  Modal,
  ModalHeader,
  ModalTitle,
  ModalContent,
  ModalFooter,
  ModalClose,
} from "@/components/Modal/Modal";
import { Switch } from "@/components/Switch/Switch";
import { cn } from "@/utils/cn";
import { useMCPStore, useMCPServers } from "@/stores/mcpStore";
import type { MCPServerState, MCPConnectionStatus } from "@/services/mcp";

// =============================================================================
// Types
// =============================================================================

export interface MCPConfigModalProps {
  open: boolean;
  onClose: () => void;
}

// =============================================================================
// Validation Schema
// =============================================================================

const serverFormSchema = z.object({
  name: z.string().min(1, "Name is required"),
  url: z.string().url("Must be a valid URL"),
  headers: z.string(),
});

type ServerFormValues = z.infer<typeof serverFormSchema>;

// =============================================================================
// Sub-components
// =============================================================================

/** Status badge for connection state */
function StatusBadge({ status }: { status: MCPConnectionStatus }) {
  const config = {
    disconnected: { color: "bg-muted text-muted-foreground", label: "Disconnected" },
    connecting: { color: "bg-primary/10 text-primary", label: "Connecting" },
    connected: { color: "bg-success/10 text-success", label: "Connected" },
    error: { color: "bg-destructive/10 text-destructive", label: "Error" },
  }[status];

  return (
    <span className={cn("px-2 py-0.5 rounded-full text-xs font-medium", config.color)}>
      {status === "connecting" && <Loader2 className="inline h-3 w-3 mr-1 animate-spin" />}
      {config.label}
    </span>
  );
}

/** Individual server card with controls */
interface ServerCardProps {
  server: MCPServerState;
  onEdit: (server: MCPServerState) => void;
  onDelete: (serverId: string) => void;
}

function ServerCard({ server, onEdit, onDelete }: ServerCardProps) {
  const [expanded, setExpanded] = useState(false);
  const { connectServer, disconnectServer, toggleServerEnabled, setToolEnabled } = useMCPStore();
  const [isConnecting, setIsConnecting] = useState(false);

  const handleConnect = useCallback(async () => {
    setIsConnecting(true);
    try {
      await connectServer(server.id);
    } catch {
      // Error is stored in server state
    } finally {
      setIsConnecting(false);
    }
  }, [connectServer, server.id]);

  const handleDisconnect = useCallback(() => {
    disconnectServer(server.id);
  }, [disconnectServer, server.id]);

  const handleToggleEnabled = useCallback(() => {
    toggleServerEnabled(server.id);
  }, [toggleServerEnabled, server.id]);

  const handleToolToggle = useCallback(
    (toolName: string, enabled: boolean) => {
      setToolEnabled(server.id, toolName, enabled);
    },
    [setToolEnabled, server.id]
  );

  const isConnected = server.status === "connected";
  const isConnectingStatus = server.status === "connecting" || isConnecting;
  const hasTools = server.tools.length > 0;

  return (
    <div className="border rounded-lg overflow-hidden">
      {/* Server header */}
      <div className="p-3 bg-muted/30">
        <div className="flex items-center gap-3">
          {/* Expand toggle for tools */}
          <button
            type="button"
            onClick={() => setExpanded(!expanded)}
            className={cn("p-1 rounded hover:bg-muted transition-colors", !hasTools && "invisible")}
            disabled={!hasTools}
            aria-label={expanded ? "Collapse tools" : "Expand tools"}
          >
            {expanded ? (
              <ChevronDown className="h-4 w-4 text-muted-foreground" />
            ) : (
              <ChevronRight className="h-4 w-4 text-muted-foreground" />
            )}
          </button>

          {/* Server icon */}
          <Plug
            className={cn("h-4 w-4", server.enabled ? "text-primary" : "text-muted-foreground")}
          />

          {/* Name and URL */}
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="font-medium text-sm truncate">{server.name}</span>
              <StatusBadge status={server.status} />
            </div>
            <div className="text-xs text-muted-foreground truncate">{server.url}</div>
          </div>

          {/* Tool count badge */}
          {hasTools && (
            <span className="text-xs text-muted-foreground bg-muted px-2 py-0.5 rounded-full">
              {server.tools.length} tool{server.tools.length !== 1 ? "s" : ""}
            </span>
          )}

          {/* Enable/disable switch */}
          <Switch
            checked={server.enabled}
            onChange={handleToggleEnabled}
            aria-label={`Toggle ${server.name} server`}
            label=""
            className="shrink-0"
          />

          {/* Connect/Disconnect button */}
          {server.enabled && (
            <Button
              variant="ghost"
              size="sm"
              onClick={isConnected ? handleDisconnect : handleConnect}
              disabled={isConnectingStatus}
              className="shrink-0"
              aria-label={
                isConnectingStatus
                  ? "Connecting"
                  : isConnected
                    ? "Disconnect server"
                    : "Connect server"
              }
            >
              {isConnectingStatus ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : isConnected ? (
                <PowerOff className="h-4 w-4" />
              ) : (
                <Power className="h-4 w-4" />
              )}
            </Button>
          )}

          {/* Edit button */}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onEdit(server)}
            className="shrink-0"
            aria-label="Edit server configuration"
          >
            <Pencil className="h-4 w-4" />
          </Button>

          {/* Delete button */}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onDelete(server.id)}
            className="shrink-0 text-destructive hover:text-destructive"
            aria-label="Delete server"
          >
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>

        {/* Error message */}
        {server.status === "error" && server.error && (
          <div className="mt-2 flex items-start gap-2 text-xs text-destructive bg-destructive/10 p-2 rounded">
            <AlertCircle className="h-3.5 w-3.5 shrink-0 mt-0.5" />
            <span>{server.error}</span>
          </div>
        )}
      </div>

      {/* Tools list (expandable) */}
      {expanded && hasTools && (
        <div className="border-t">
          <div className="p-2 bg-muted/10">
            <div className="text-xs font-medium text-muted-foreground mb-2 flex items-center gap-1.5">
              <Wrench className="h-3 w-3" />
              Available Tools
            </div>
            <div className="space-y-1">
              {server.tools.map((tool) => {
                const isEnabled = server.toolsEnabled[tool.name] !== false;
                return (
                  <div
                    key={tool.name}
                    className="flex items-center gap-3 p-2 rounded hover:bg-muted/50"
                  >
                    <Switch
                      checked={isEnabled}
                      onChange={() => handleToolToggle(tool.name, !isEnabled)}
                      aria-label={`Toggle ${tool.name} tool`}
                      label=""
                      className="shrink-0"
                    />
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium">{tool.name}</div>
                      {tool.description && (
                        <div className="text-xs text-muted-foreground truncate">
                          {tool.description}
                        </div>
                      )}
                    </div>
                    {isEnabled && <Check className="h-4 w-4 text-green-500 shrink-0" />}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

/** Form for adding/editing a server */
interface ServerFormProps {
  editingServer?: MCPServerState | null;
  onSubmit: (values: ServerFormValues) => void;
  onCancel: () => void;
}

function ServerForm({ editingServer, onSubmit, onCancel }: ServerFormProps) {
  const form = useForm<ServerFormValues>({
    resolver: zodResolver(serverFormSchema),
    defaultValues: {
      name: editingServer?.name ?? "",
      url: editingServer?.url ?? "",
      headers: editingServer?.headers ? JSON.stringify(editingServer.headers, null, 2) : "",
    },
  });

  const handleSubmit = form.handleSubmit((values) => {
    onSubmit(values);
  });

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <FormField
        label="Server Name"
        htmlFor="server-name"
        required
        error={form.formState.errors.name?.message}
      >
        <Input id="server-name" {...form.register("name")} placeholder="My MCP Server" />
      </FormField>

      <FormField
        label="Server URL"
        htmlFor="server-url"
        required
        helpText="The HTTP endpoint for the MCP server"
        error={form.formState.errors.url?.message}
      >
        <Input id="server-url" {...form.register("url")} placeholder="https://mcp.example.com" />
      </FormField>

      <FormField
        label="Headers (JSON)"
        htmlFor="server-headers"
        helpText="Optional HTTP headers for authentication (e.g., API keys)"
        error={form.formState.errors.headers?.message}
      >
        <textarea
          id="server-headers"
          {...form.register("headers")}
          placeholder='{"Authorization": "Bearer your-api-key"}'
          className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono min-h-[80px]"
        />
      </FormField>

      <div className="flex justify-end gap-2 pt-2">
        <Button type="button" variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit">{editingServer ? "Save" : "Add Server"}</Button>
      </div>
    </form>
  );
}

// =============================================================================
// Main Component
// =============================================================================

export function MCPConfigModal({ open, onClose }: MCPConfigModalProps) {
  const servers = useMCPServers();
  const { addServer, updateServer, removeServer } = useMCPStore();
  const [showForm, setShowForm] = useState(false);
  const [editingServer, setEditingServer] = useState<MCPServerState | null>(null);

  const handleAddClick = useCallback(() => {
    setEditingServer(null);
    setShowForm(true);
  }, []);

  const handleEditClick = useCallback((server: MCPServerState) => {
    setEditingServer(server);
    setShowForm(true);
  }, []);

  const handleFormCancel = useCallback(() => {
    setShowForm(false);
    setEditingServer(null);
  }, []);

  const handleFormSubmit = useCallback(
    (values: ServerFormValues) => {
      // Parse headers JSON if provided
      let headers: Record<string, string> | undefined;
      if (values.headers) {
        try {
          headers = JSON.parse(values.headers);
        } catch {
          // Invalid JSON - ignore headers
        }
      }

      if (editingServer) {
        // Update existing server
        updateServer(editingServer.id, {
          name: values.name,
          url: values.url,
          headers,
        });
      } else {
        // Add new server
        addServer({
          name: values.name,
          url: values.url,
          enabled: true,
          headers,
        });
      }

      setShowForm(false);
      setEditingServer(null);
    },
    [editingServer, addServer, updateServer]
  );

  const handleDeleteServer = useCallback(
    (serverId: string) => {
      removeServer(serverId);
    },
    [removeServer]
  );

  const connectedCount = servers.filter((s) => s.status === "connected").length;
  const totalToolCount = servers
    .filter((s) => s.status === "connected")
    .reduce((sum, s) => sum + s.tools.length, 0);

  return (
    <Modal open={open} onClose={onClose} className="max-w-2xl">
      <ModalClose onClose={onClose} />
      <ModalHeader>
        <ModalTitle className="flex items-center gap-2">
          <Plug className="h-5 w-5 text-primary" />
          MCP Server Configuration
        </ModalTitle>
      </ModalHeader>

      <ModalContent className="max-h-[60vh] overflow-y-auto">
        {showForm ? (
          <ServerForm
            editingServer={editingServer}
            onSubmit={handleFormSubmit}
            onCancel={handleFormCancel}
          />
        ) : (
          <div className="space-y-4">
            {/* Summary stats */}
            <div className="flex items-center gap-4 text-sm text-muted-foreground">
              <span>
                {connectedCount} connected server{connectedCount !== 1 ? "s" : ""}
              </span>
              <span>|</span>
              <span>
                {totalToolCount} tool{totalToolCount !== 1 ? "s" : ""} available
              </span>
            </div>

            {/* Server list */}
            {servers.length === 0 ? (
              <div className="text-center py-8 text-muted-foreground">
                <Plug className="h-8 w-8 mx-auto mb-3 opacity-50" />
                <p className="text-sm">No MCP servers configured</p>
                <p className="text-xs mt-1">Add a server to connect to external tools</p>
              </div>
            ) : (
              <div className="space-y-3">
                {servers.map((server) => (
                  <ServerCard
                    key={server.id}
                    server={server}
                    onEdit={handleEditClick}
                    onDelete={handleDeleteServer}
                  />
                ))}
              </div>
            )}
          </div>
        )}
      </ModalContent>

      {!showForm && (
        <ModalFooter>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
          <Button onClick={handleAddClick}>
            <Plus className="h-4 w-4 mr-1.5" />
            Add Server
          </Button>
        </ModalFooter>
      )}
    </Modal>
  );
}
