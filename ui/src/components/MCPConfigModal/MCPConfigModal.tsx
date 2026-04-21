/**
 * MCPConfigModal - Configuration modal for MCP (Model Context Protocol) servers
 *
 * Allows users to:
 * - Add/edit/remove MCP server configurations
 * - Test connections before saving
 * - Connect/disconnect from servers via a single toggle
 * - Enable/disable individual tools with expandable descriptions
 */

import { useState, useCallback, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import {
  AlertCircle,
  AlertTriangle,
  ArrowLeft,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Copy,
  Eye,
  EyeOff,
  KeyRound,
  Loader2,
  Pencil,
  Plug,
  Plus,
  ShieldCheck,
  Terminal,
  Trash2,
  Wifi,
  Wrench,
  XCircle,
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
import { useDebouncedValue } from "@/hooks/useDebouncedValue";
import { useConfig } from "@/config/ConfigProvider";
import { useMCPStore, useMCPServers } from "@/stores/mcpStore";
import {
  MCPClient,
  type MCPServerState,
  type MCPConnectionStatus,
  type MCPAuthType,
  type MCPOAuthConfig,
  startOAuthFlow,
  getValidAccessToken,
  hasValidTokens,
  clearOAuthData,
  detectServerAuth,
} from "@/services/mcp";
import type { MCPToolDefinition, JSONSchema } from "@/services/mcp";
import { MCPCatalog, type CatalogPrefill } from "./MCPCatalog";

// =============================================================================
// Types
// =============================================================================

/** Pre-fill data for adding a new server (e.g., from URL query params or catalog). */
export interface MCPServerPrefill {
  url: string;
  name?: string;
  authType?: MCPAuthType;
  bearerToken?: string;
  /** Additional headers pre-filled into the form's JSON textarea. */
  headers?: Record<string, string>;
  /** If present, show an install banner for a locally-run stdio server. */
  localInstall?: {
    command: string;
    envVars: Array<{
      name: string;
      description?: string;
      isSecret?: boolean;
      isRequired?: boolean;
    }>;
  };
}

export interface MCPConfigModalProps {
  open: boolean;
  onClose: () => void;
  /** Pre-fill a new server (e.g., from ?mcp_server_url= query param) */
  prefill?: MCPServerPrefill | null;
}

// =============================================================================
// Validation Schema
// =============================================================================

const serverFormSchema = z.object({
  name: z.string().min(1, "Name is required"),
  url: z.string().url("Must be a valid URL"),
  authType: z.enum(["none", "bearer", "oauth"]),
  bearerToken: z.string(),
  oauthClientId: z.string(),
  oauthScopes: z.string(),
  headers: z.string(),
  timeout: z.number().int().min(1, "Must be at least 1 second"),
});

type ServerFormValues = z.infer<typeof serverFormSchema>;

// =============================================================================
// Sub-components
// =============================================================================

/** Status badge for connection state */
function StatusBadge({ status }: { status: MCPConnectionStatus }) {
  const config = {
    disconnected: { color: "bg-muted text-muted-foreground", label: "Off" },
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

// =============================================================================
// Tool Item (expandable description + schema)
// =============================================================================

interface ToolItemProps {
  tool: MCPToolDefinition;
  isEnabled: boolean;
  onToggle: (toolName: string, enabled: boolean) => void;
}

/** Render a JSON Schema property list */
function SchemaProperties({ schema }: { schema: JSONSchema }) {
  const properties = schema.properties;
  if (!properties || Object.keys(properties).length === 0) {
    return <span className="text-xs text-muted-foreground italic">No parameters</span>;
  }

  return (
    <div className="bg-muted/50 rounded p-2 space-y-1.5">
      {Object.entries(properties).map(([name, prop]) => {
        const propSchema = prop as JSONSchema;
        const isRequired = schema.required?.includes(name);
        return (
          <div key={name} className="text-xs">
            <div className="flex items-baseline gap-1.5 flex-wrap">
              <code className="font-mono font-medium text-foreground">{name}</code>
              {propSchema.type && (
                <span className="text-muted-foreground font-mono text-[11px]">
                  {String(propSchema.type)}
                </span>
              )}
              {isRequired && (
                <span className="text-destructive text-[10px] font-semibold">required</span>
              )}
            </div>
            {propSchema.description && (
              <p className="text-muted-foreground mt-0.5 pl-0.5">{propSchema.description}</p>
            )}
            {propSchema.enum && (
              <p className="text-muted-foreground mt-0.5 pl-0.5 font-mono text-[11px]">
                enum: {propSchema.enum.map(String).join(" | ")}
              </p>
            )}
          </div>
        );
      })}
    </div>
  );
}

function ToolItem({ tool, isEnabled, onToggle }: ToolItemProps) {
  const [expanded, setExpanded] = useState(false);
  const hasSchema =
    tool.inputSchema?.properties && Object.keys(tool.inputSchema.properties).length > 0;
  const hasDetails = !!tool.description || hasSchema;

  return (
    <div className="rounded hover:bg-muted/50">
      <div className="flex items-center gap-3 p-2">
        <Switch
          checked={isEnabled}
          onChange={() => onToggle(tool.name, !isEnabled)}
          aria-label={`Toggle ${tool.name} tool`}
          label=""
          className="shrink-0"
        />
        <button
          type="button"
          className="flex-1 min-w-0 text-left cursor-pointer"
          onClick={() => hasDetails && setExpanded(!expanded)}
          aria-expanded={expanded}
        >
          <div className="flex items-center gap-1.5">
            {hasDetails && (
              <span className="shrink-0 text-muted-foreground">
                {expanded ? (
                  <ChevronDown className="h-3 w-3" />
                ) : (
                  <ChevronRight className="h-3 w-3" />
                )}
              </span>
            )}
            <span className="text-sm font-medium">{tool.name}</span>
          </div>
          {tool.description && !expanded && (
            <div className="text-xs text-muted-foreground truncate mt-0.5">{tool.description}</div>
          )}
        </button>
      </div>

      {expanded && hasDetails && (
        <div className="px-2 pb-2 pl-12 space-y-2">
          {tool.description && (
            <p className="text-xs text-muted-foreground leading-relaxed">{tool.description}</p>
          )}
          {hasSchema && (
            <div className="space-y-1">
              <div className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
                Parameters
              </div>
              <SchemaProperties schema={tool.inputSchema} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// =============================================================================
// Server Card
// =============================================================================

interface ServerCardProps {
  server: MCPServerState;
  onEdit: (server: MCPServerState) => void;
  onDelete: (serverId: string) => void;
}

function ServerCard({ server, onEdit, onDelete }: ServerCardProps) {
  const [expanded, setExpanded] = useState(false);
  const { connectServer, disconnectServer, setToolEnabled } = useMCPStore();
  const [isToggling, setIsToggling] = useState(false);
  const [isAuthorizing, setIsAuthorizing] = useState(false);
  const [authError, setAuthError] = useState<string>();
  const oauthAuthorized = server.authType === "oauth" && hasValidTokens(server.url);

  const handleAuthorize = useCallback(async () => {
    setIsAuthorizing(true);
    setAuthError(undefined);
    try {
      await startOAuthFlow(server.url, server.oauth);
      // Tokens obtained — now connect
      try {
        await connectServer(server.id);
      } catch {
        // Connection error stored in server state
      }
    } catch (err) {
      setAuthError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsAuthorizing(false);
    }
  }, [server.url, server.oauth, server.id, connectServer]);

  // Unified toggle: switch ON = enable + connect, switch OFF = disconnect + disable
  const handleToggle = useCallback(async () => {
    if (isToggling) return;
    setIsToggling(true);
    try {
      if (server.enabled) {
        // Turning OFF
        disconnectServer(server.id);
        useMCPStore.getState().toggleServerEnabled(server.id);
      } else {
        // Turning ON: enable first, then connect
        useMCPStore.getState().toggleServerEnabled(server.id);
        try {
          await connectServer(server.id);
        } catch {
          // Error is stored in server state and shown inline
        }
      }
    } finally {
      setIsToggling(false);
    }
  }, [server.enabled, server.id, isToggling, connectServer, disconnectServer]);

  const handleToolToggle = useCallback(
    (toolName: string, enabled: boolean) => {
      setToolEnabled(server.id, toolName, enabled);
    },
    [setToolEnabled, server.id]
  );

  const isConnectingStatus = server.status === "connecting" || isToggling;
  const hasTools = server.tools.length > 0;
  const needsAuthorization = server.authType === "oauth" && !oauthAuthorized;

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

          {/* Server icon — color reflects connection status */}
          <Plug
            className={cn(
              "h-4 w-4 shrink-0",
              server.status === "connected"
                ? "text-primary"
                : server.status === "connecting"
                  ? "text-primary/50"
                  : "text-muted-foreground"
            )}
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

          {/* Single toggle: enable+connect / disconnect+disable */}
          <Switch
            checked={server.enabled}
            onChange={handleToggle}
            disabled={isConnectingStatus || needsAuthorization}
            aria-label={server.enabled ? `Disconnect ${server.name}` : `Connect ${server.name}`}
            label=""
            className="shrink-0"
          />

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

        {/* OAuth status & authorize button */}
        {server.authType === "oauth" && (
          <div
            className={cn(
              "mt-2 flex items-center gap-2 rounded-md p-2",
              needsAuthorization
                ? "bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-800"
                : ""
            )}
          >
            {oauthAuthorized ? (
              <span className="flex items-center gap-1.5 text-xs text-green-600 dark:text-green-400">
                <ShieldCheck className="h-3.5 w-3.5" />
                Authorized
              </span>
            ) : (
              <span className="flex items-center gap-1.5 text-xs text-amber-700 dark:text-amber-400 flex-1">
                <KeyRound className="h-3.5 w-3.5 shrink-0" />
                Authorization required to connect
              </span>
            )}
            <Button
              type="button"
              variant={needsAuthorization ? "primary" : "outline"}
              size="sm"
              onClick={handleAuthorize}
              disabled={isAuthorizing}
              isLoading={isAuthorizing}
            >
              {oauthAuthorized ? "Re-authorize" : "Authorize"}
            </Button>
            {authError && (
              <div className="flex items-start gap-1.5 text-xs text-destructive mt-1.5">
                <AlertCircle className="h-3 w-3 shrink-0 mt-0.5" />
                <span>{authError}</span>
              </div>
            )}
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
            <div className="space-y-0.5">
              {server.tools.map((tool) => (
                <ToolItem
                  key={tool.name}
                  tool={tool}
                  isEnabled={server.toolsEnabled[tool.name] !== false}
                  onToggle={handleToolToggle}
                />
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// =============================================================================
// Server Form (with test connection)
// =============================================================================

interface ServerFormProps {
  editingServer?: MCPServerState | null;
  onSubmit: (values: ServerFormValues) => void;
  onCancel: () => void;
  /** Pre-fill data (e.g., from URL query params) */
  prefill?: MCPServerPrefill | null;
}

type TestStatus = "idle" | "testing" | "success" | "error";

type OAuthStatus = "idle" | "authorizing" | "authorized" | "error";

function ServerForm({ editingServer, onSubmit, onCancel, prefill }: ServerFormProps) {
  const [showToken, setShowToken] = useState(false);
  const isNewServer = !editingServer;

  // Extract bearer token from existing headers, pass the rest as extra headers
  const existingHeaders = editingServer?.headers ?? {};
  const existingBearer = (() => {
    const auth = existingHeaders["Authorization"] ?? existingHeaders["authorization"] ?? "";
    return auth.startsWith("Bearer ") ? auth.slice(7) : "";
  })();
  const extraHeaders = Object.fromEntries(
    Object.entries(existingHeaders).filter(([k]) => k.toLowerCase() !== "authorization")
  );

  // Infer initial auth type from existing config or prefill
  const initialAuthType: MCPAuthType =
    editingServer?.authType ?? prefill?.authType ?? (existingBearer ? "bearer" : "none");

  // Merge extra headers from editingServer with prefill headers — prefill wins
  // when keys collide (prefill is either catalog-supplied or user-confirmed
  // via a query-param flow).
  const prefillExtraHeaders = prefill?.headers ?? {};
  const mergedExtraHeaders =
    Object.keys(prefillExtraHeaders).length > 0
      ? { ...extraHeaders, ...prefillExtraHeaders }
      : extraHeaders;

  const form = useForm<ServerFormValues>({
    resolver: zodResolver(serverFormSchema),
    defaultValues: {
      name: editingServer?.name ?? prefill?.name ?? "",
      url: editingServer?.url ?? prefill?.url ?? "",
      authType: initialAuthType,
      bearerToken: prefill?.bearerToken ?? existingBearer,
      oauthClientId: editingServer?.oauth?.clientId ?? "",
      oauthScopes: editingServer?.oauth?.scopes ?? "",
      headers:
        Object.keys(mergedExtraHeaders).length > 0
          ? JSON.stringify(mergedExtraHeaders, null, 2)
          : "",
      timeout: Math.round((editingServer?.timeout ?? 300000) / 1000),
    },
  });

  const [testStatus, setTestStatus] = useState<TestStatus>("idle");
  const [testMessage, setTestMessage] = useState<string>();
  const [testLatency, setTestLatency] = useState<number>();

  // OAuth state
  const [oauthStatus, setOauthStatus] = useState<OAuthStatus>(() =>
    initialAuthType === "oauth" && editingServer?.url && hasValidTokens(editingServer.url)
      ? "authorized"
      : "idle"
  );
  const [oauthError, setOauthError] = useState<string>();

  // Auth detection state
  type DetectionStatus = "idle" | "detecting" | "detected";
  const [detectionStatus, setDetectionStatus] = useState<DetectionStatus>("idle");
  const [detectionMessage, setDetectionMessage] = useState("");
  // Track whether user manually changed auth type (disables auto-select)
  const [userOverrodeAuth, setUserOverrodeAuth] = useState(false);

  // Watched form values
  const watchedUrl = form.watch("url");
  const watchedHeaders = form.watch("headers");
  const watchedAuthType = form.watch("authType") as MCPAuthType;
  // Debounce URL so network-touching effects (auth probe, template checks) run
  // only after the user stops typing.
  const debouncedUrl = useDebouncedValue(watchedUrl, 500);

  // Reset test results when URL or headers change
  useEffect(() => {
    if (testStatus !== "idle" && testStatus !== "testing") {
      setTestStatus("idle");
      setTestMessage(undefined);
      setTestLatency(undefined);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only reset on field changes
  }, [watchedUrl, watchedHeaders]);

  // Reset OAuth status when auth type or URL changes
  useEffect(() => {
    if (watchedAuthType === "oauth" && watchedUrl) {
      setOauthStatus(hasValidTokens(watchedUrl) ? "authorized" : "idle");
      setOauthError(undefined);
    } else {
      setOauthStatus("idle");
      setOauthError(undefined);
    }
  }, [watchedAuthType, watchedUrl]);

  // Auto-detect auth requirements when the debounced URL changes (new servers only)
  useEffect(() => {
    if (!isNewServer || !debouncedUrl || userOverrodeAuth) {
      setDetectionStatus("idle");
      setDetectionMessage("");
      return;
    }

    // Validate URL before probing
    if (!z.string().url().safeParse(debouncedUrl).success) {
      setDetectionStatus("idle");
      setDetectionMessage("");
      return;
    }

    setDetectionStatus("detecting");
    setDetectionMessage("");

    let cancelled = false;
    detectServerAuth(debouncedUrl).then((result) => {
      if (cancelled) return;
      setDetectionStatus("detected");
      setDetectionMessage(result.message);
      if (result.authType !== watchedAuthType) {
        form.setValue("authType", result.authType);
      }
      // Pre-fill server name from resource metadata if the field is still empty
      if (result.serverName && !form.getValues("name")) {
        form.setValue("name", result.serverName);
      }
    });

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only re-run on URL change
  }, [debouncedUrl, isNewServer, userOverrodeAuth]);

  const handleAuthorize = useCallback(async () => {
    const valid = await form.trigger("url");
    if (!valid) return;

    const url = form.getValues("url");
    const clientId = form.getValues("oauthClientId") || undefined;
    const scopes = form.getValues("oauthScopes") || undefined;

    setOauthStatus("authorizing");
    setOauthError(undefined);

    try {
      await startOAuthFlow(url, { clientId, scopes });
      setOauthStatus("authorized");
    } catch (err) {
      setOauthStatus("error");
      setOauthError(err instanceof Error ? err.message : String(err));
    }
  }, [form]);

  const handleRevoke = useCallback(() => {
    const url = form.getValues("url");
    if (url) clearOAuthData(url);
    setOauthStatus("idle");
  }, [form]);

  const handleTestConnection = useCallback(async () => {
    // Validate URL field first
    const valid = await form.trigger("url");
    if (!valid) return;

    const values = form.getValues();

    // Parse extra headers
    const extra: Record<string, string> = {};
    if (values.headers) {
      try {
        Object.assign(extra, JSON.parse(values.headers));
      } catch {
        setTestStatus("error");
        setTestMessage("Invalid JSON in headers field");
        return;
      }
    }

    // Build client config based on auth type
    const headers: Record<string, string> = { ...extra };
    let getAccessTokenFn: (() => Promise<string | null>) | undefined;

    if (values.authType === "bearer" && values.bearerToken) {
      headers["Authorization"] = `Bearer ${values.bearerToken}`;
    } else if (values.authType === "oauth") {
      const oauthCfg: MCPOAuthConfig = {
        clientId: values.oauthClientId || undefined,
        scopes: values.oauthScopes || undefined,
      };
      getAccessTokenFn = () => getValidAccessToken(values.url, oauthCfg);
    }

    setTestStatus("testing");
    setTestMessage(undefined);
    setTestLatency(undefined);

    const client = new MCPClient({
      url: values.url,
      headers: Object.keys(headers).length > 0 ? headers : undefined,
      timeout: 10000,
      getAccessToken: getAccessTokenFn,
    });
    const start = performance.now();

    try {
      await client.connect();
      const elapsed = Math.round(performance.now() - start);
      const info = client.getServerInfo();
      setTestStatus("success");
      setTestLatency(elapsed);
      setTestMessage(info ? `${info.name} v${info.version}` : "Connection successful");
    } catch (err) {
      setTestStatus("error");
      setTestMessage(err instanceof Error ? err.message : String(err));
    } finally {
      try {
        await client.disconnect();
      } catch {
        // ignore
      }
    }
  }, [form]);

  const handleSubmit = form.handleSubmit((values) => {
    onSubmit(values);
  });

  const authTypeOptions = [
    { value: "none" as const, label: "None" },
    { value: "bearer" as const, label: "Bearer Token" },
    { value: "oauth" as const, label: "OAuth (PKCE)" },
  ];

  // Flag `{placeholder}` tokens in the bearer token or headers JSON — these
  // come from catalog prefills with templated values the user must replace.
  const watchedBearer = form.watch("bearerToken");
  const hasTemplateTokens =
    /\{[^}]+\}/.test(watchedHeaders ?? "") ||
    (watchedAuthType === "bearer" && /\{[^}]+\}/.test(watchedBearer ?? ""));

  const [copiedInstall, setCopiedInstall] = useState(false);
  const handleCopyInstall = useCallback(async () => {
    if (!prefill?.localInstall?.command) return;
    try {
      await navigator.clipboard.writeText(prefill.localInstall.command);
      setCopiedInstall(true);
      setTimeout(() => setCopiedInstall(false), 1500);
    } catch {
      // Clipboard may be unavailable (insecure context); ignore silently.
    }
  }, [prefill?.localInstall?.command]);

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      {/* Local-install banner: shown when the catalog picked a stdio server */}
      {prefill?.localInstall && (
        <div className="rounded-md border border-primary/30 bg-primary/5 p-3 space-y-2">
          <div className="flex items-start gap-2">
            <Terminal className="h-4 w-4 shrink-0 mt-0.5 text-primary" />
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium">Local setup required</div>
              <p className="text-xs text-muted-foreground mt-0.5">
                Run the command below on your machine. Once it&apos;s up, the server is reachable at{" "}
                <code className="font-mono">{prefill.url}</code>.
              </p>
            </div>
          </div>
          <div className="flex items-start gap-2">
            <pre className="flex-1 min-w-0 text-xs font-mono bg-muted rounded px-2 py-1.5 whitespace-pre-wrap break-all">
              {prefill.localInstall.command}
            </pre>
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={handleCopyInstall}
              className="shrink-0"
            >
              <Copy className="h-3.5 w-3.5" />
              {copiedInstall ? "Copied" : "Copy"}
            </Button>
          </div>
          {prefill.localInstall.envVars.length > 0 && (
            <div className="text-xs space-y-1">
              <div className="font-medium text-muted-foreground">Required environment:</div>
              <ul className="space-y-0.5 pl-3">
                {prefill.localInstall.envVars.map((v) => (
                  <li key={v.name} className="flex items-baseline gap-2">
                    <code className="font-mono text-foreground">{v.name}</code>
                    {v.isRequired && (
                      <span className="text-[10px] text-destructive font-semibold">required</span>
                    )}
                    {v.description && (
                      <span className="text-muted-foreground">— {v.description}</span>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {/* Warning banner when pre-filled from a URL param */}
      {prefill && !prefill.localInstall && (
        <div className="flex items-start gap-2 text-xs text-amber-700 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-800 p-2.5 rounded-md">
          <AlertTriangle className="h-3.5 w-3.5 shrink-0 mt-0.5" />
          <span>Server URL provided via link. Only add servers you trust.</span>
        </div>
      )}

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
        label="Server Name"
        htmlFor="server-name"
        required
        error={form.formState.errors.name?.message}
      >
        <Input id="server-name" {...form.register("name")} placeholder="My MCP Server" />
      </FormField>

      {/* Auth detection indicator */}
      {detectionStatus === "detecting" && (
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <Loader2 className="h-3 w-3 animate-spin" />
          Checking authentication requirements...
        </div>
      )}
      {detectionStatus === "detected" && detectionMessage && (
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <CheckCircle2 className="h-3 w-3" />
          {detectionMessage}
        </div>
      )}

      {/* Auth type selector */}
      <FormField label="Authentication" htmlFor="server-auth-type">
        <div className="flex gap-1.5" role="radiogroup" aria-label="Authentication type">
          {authTypeOptions.map((opt) => (
            <button
              key={opt.value}
              type="button"
              role="radio"
              aria-checked={watchedAuthType === opt.value}
              onClick={() => {
                form.setValue("authType", opt.value);
                setUserOverrodeAuth(true);
              }}
              className={cn(
                "px-3 py-1.5 rounded-md text-sm border transition-colors",
                watchedAuthType === opt.value
                  ? "border-primary bg-primary/10 text-primary font-medium"
                  : "border-input text-muted-foreground hover:bg-muted"
              )}
            >
              {opt.label}
            </button>
          ))}
        </div>
      </FormField>

      {/* Bearer Token fields */}
      {watchedAuthType === "bearer" && (
        <FormField
          label="Bearer Token"
          htmlFor="server-bearer-token"
          helpText="Token for authenticating with the MCP server"
          error={form.formState.errors.bearerToken?.message}
        >
          <div className="relative">
            <Input
              id="server-bearer-token"
              type={showToken ? "text" : "password"}
              {...form.register("bearerToken")}
              placeholder="your-api-key"
              className="pr-10 font-mono"
            />
            <button
              type="button"
              onClick={() => setShowToken(!showToken)}
              className="absolute right-2 top-1/2 -translate-y-1/2 p-1 rounded hover:bg-muted text-muted-foreground"
              aria-label={showToken ? "Hide token" : "Show token"}
            >
              {showToken ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </button>
          </div>
        </FormField>
      )}

      {/* OAuth fields */}
      {watchedAuthType === "oauth" && (
        <div
          className={cn(
            "space-y-3 rounded-md p-3",
            oauthStatus === "authorized"
              ? "border border-input"
              : "border border-amber-300 dark:border-amber-800 bg-amber-50/60 dark:bg-amber-950/20"
          )}
        >
          {/* Prominent authorize CTA */}
          {oauthStatus === "authorized" ? (
            <div className="flex items-center gap-3">
              <span className="flex items-center gap-1.5 text-sm font-medium text-green-600 dark:text-green-400">
                <ShieldCheck className="h-4 w-4" />
                Authorized
              </span>
              <Button type="button" variant="ghost" size="sm" onClick={handleRevoke}>
                Revoke
              </Button>
            </div>
          ) : (
            <div className="space-y-2">
              <div className="flex items-start gap-2 text-sm text-amber-800 dark:text-amber-300">
                <KeyRound className="h-4 w-4 shrink-0 mt-0.5" />
                <div className="flex-1">
                  <div className="font-medium">Authorization required</div>
                  <div className="text-xs text-amber-700 dark:text-amber-400 mt-0.5">
                    Click Authorize to sign in and grant access. You won&apos;t be able to test or
                    add the server until this step is completed.
                  </div>
                </div>
              </div>
              <div className="flex items-center gap-3">
                <Button
                  type="button"
                  variant="primary"
                  size="sm"
                  onClick={handleAuthorize}
                  isLoading={oauthStatus === "authorizing"}
                  disabled={oauthStatus === "authorizing"}
                >
                  <KeyRound className="h-4 w-4 mr-1.5" />
                  {oauthStatus === "authorizing" ? "Waiting for authorization..." : "Authorize"}
                </Button>
                {oauthStatus === "error" && oauthError && (
                  <div className="flex items-start gap-1.5 text-xs text-destructive flex-1 min-w-0">
                    <XCircle className="h-3 w-3 shrink-0 mt-0.5" />
                    <span className="break-words">{oauthError}</span>
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Advanced OAuth fields */}
          <details className="pt-1">
            <summary className="text-xs text-muted-foreground cursor-pointer select-none hover:text-foreground">
              Advanced options
            </summary>
            <div className="space-y-3 mt-3">
              <FormField
                label="Client ID"
                htmlFor="server-oauth-client-id"
                helpText="Leave empty to use dynamic client registration"
              >
                <Input
                  id="server-oauth-client-id"
                  {...form.register("oauthClientId")}
                  placeholder="Optional — for pre-registered apps"
                  className="font-mono"
                />
              </FormField>

              <FormField
                label="Scopes"
                htmlFor="server-oauth-scopes"
                helpText="Space-separated OAuth scopes (auto-detected if empty)"
              >
                <Input
                  id="server-oauth-scopes"
                  {...form.register("oauthScopes")}
                  placeholder="Optional — e.g. read write"
                />
              </FormField>
            </div>
          </details>
        </div>
      )}

      <details
        className="group rounded-md border border-input bg-muted/20"
        open={Object.keys(mergedExtraHeaders).length > 0}
      >
        <summary className="cursor-pointer select-none list-none px-3 py-2 text-sm font-medium text-muted-foreground hover:text-foreground flex items-center gap-1.5">
          <ChevronRight className="h-3.5 w-3.5 transition-transform group-open:rotate-90" />
          Advanced
        </summary>
        <div className="p-3 pt-1 space-y-3 border-t">
          <FormField
            label="Additional Headers (JSON)"
            htmlFor="server-headers"
            helpText="Optional extra HTTP headers"
            error={form.formState.errors.headers?.message}
          >
            <textarea
              id="server-headers"
              {...form.register("headers")}
              placeholder='{"X-Custom-Header": "value"}'
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono min-h-[80px]"
            />
          </FormField>

          <FormField
            label="Request Timeout (seconds)"
            htmlFor="server-timeout"
            helpText="Maximum time to wait for MCP tool responses"
            error={form.formState.errors.timeout?.message}
          >
            <Input
              id="server-timeout"
              type="number"
              min={1}
              {...form.register("timeout", { valueAsNumber: true })}
            />
          </FormField>
        </div>
      </details>

      {/* Test connection result */}
      {testStatus !== "idle" && (
        <div role="status" aria-live="polite">
          {testStatus === "testing" && (
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <Loader2 className="h-3 w-3 animate-spin" />
              Testing connection...
            </div>
          )}
          {testStatus === "success" && (
            <div className="flex items-center gap-1.5 text-xs text-green-600 dark:text-green-400">
              <CheckCircle2 className="h-3 w-3" />
              {testMessage}
              {testLatency != null && (
                <span className="text-muted-foreground">({testLatency}ms)</span>
              )}
            </div>
          )}
          {testStatus === "error" && (
            <div className="flex items-start gap-1.5 text-xs text-destructive">
              <XCircle className="h-3 w-3 shrink-0 mt-0.5" />
              <span>{testMessage ?? "Connection failed"}</span>
            </div>
          )}
        </div>
      )}

      {/* Gating hint when OAuth is required but not authorized */}
      {watchedAuthType === "oauth" && oauthStatus !== "authorized" && (
        <div className="flex items-start gap-1.5 text-xs text-muted-foreground">
          <AlertCircle className="h-3 w-3 shrink-0 mt-0.5" />
          <span>Authorize above to enable testing and saving this server.</span>
        </div>
      )}

      {/* Gating hint when templated placeholders like {api_key} haven't been replaced */}
      {hasTemplateTokens && (
        <div className="flex items-start gap-1.5 text-xs text-amber-700 dark:text-amber-400">
          <AlertTriangle className="h-3 w-3 shrink-0 mt-0.5" />
          <span>
            Replace placeholder values like <code className="font-mono">{"{api_key}"}</code> with
            your real credentials before saving.
          </span>
        </div>
      )}

      <div className="flex justify-between pt-2">
        <Button type="button" variant="ghost" onClick={onCancel}>
          <ArrowLeft className="h-4 w-4 mr-1.5" />
          Back
        </Button>
        <div className="flex gap-2">
          <Button
            type="button"
            variant="outline"
            onClick={handleTestConnection}
            isLoading={testStatus === "testing"}
            disabled={
              testStatus === "testing" ||
              hasTemplateTokens ||
              (watchedAuthType === "oauth" && oauthStatus !== "authorized")
            }
          >
            <Wifi className="h-4 w-4 mr-1.5" />
            Test
          </Button>
          <Button
            type="submit"
            disabled={
              hasTemplateTokens || (watchedAuthType === "oauth" && oauthStatus !== "authorized")
            }
          >
            {editingServer ? "Save" : "Add Server"}
          </Button>
        </div>
      </div>
    </form>
  );
}

// =============================================================================
// Main Component
// =============================================================================

export function MCPConfigModal({ open, onClose, prefill }: MCPConfigModalProps) {
  const servers = useMCPServers();
  const { addServer, updateServer, removeServer } = useMCPStore();
  const { config } = useConfig();
  const favorites = config.mcp.favorites;
  const [view, setView] = useState<"list" | "catalog" | "form">("list");
  const [editingServer, setEditingServer] = useState<MCPServerState | null>(null);
  // Catalog-selected prefill. Preserved separately from the incoming
  // `prefill` prop so that cancelling the form returns the user to the list,
  // not the original deep-linked state.
  const [catalogPrefill, setCatalogPrefill] = useState<MCPServerPrefill | null>(null);
  // Which view should the form's Back button return to?
  const [formOrigin, setFormOrigin] = useState<"list" | "catalog">("list");

  // Auto-show form when opened with an external prefill (e.g. ?mcp_server_url=)
  useEffect(() => {
    if (open && prefill) {
      setEditingServer(null);
      setCatalogPrefill(null);
      setFormOrigin("list");
      setView("form");
    }
  }, [open, prefill]);

  const handleAddClick = useCallback(() => {
    setEditingServer(null);
    setCatalogPrefill(null);
    setView("catalog");
  }, []);

  const handleEditClick = useCallback((server: MCPServerState) => {
    setEditingServer(server);
    setCatalogPrefill(null);
    setFormOrigin("list");
    setView("form");
  }, []);

  const handleCatalogPick = useCallback((p: CatalogPrefill) => {
    setEditingServer(null);
    setCatalogPrefill(p);
    setFormOrigin("catalog");
    setView("form");
  }, []);

  const handleAddManual = useCallback(() => {
    setEditingServer(null);
    setCatalogPrefill(null);
    setFormOrigin("catalog");
    setView("form");
  }, []);

  const handleCatalogCancel = useCallback(() => {
    setView("list");
  }, []);

  const handleFormCancel = useCallback(() => {
    setView(formOrigin);
    setEditingServer(null);
    setCatalogPrefill(null);
  }, [formOrigin]);

  const handleFormSubmit = useCallback(
    (values: ServerFormValues) => {
      // Parse extra headers
      const extra: Record<string, string> = {};
      if (values.headers) {
        try {
          Object.assign(extra, JSON.parse(values.headers));
        } catch {
          // Invalid JSON - ignore extra headers
        }
      }

      // Build headers — only add Authorization for bearer auth
      let headers: Record<string, string> | undefined;
      if (values.authType === "bearer") {
        if (values.bearerToken || Object.keys(extra).length > 0) {
          headers = { ...extra };
          if (values.bearerToken) {
            headers["Authorization"] = `Bearer ${values.bearerToken}`;
          }
        }
      } else if (Object.keys(extra).length > 0) {
        headers = extra;
      }

      // Build OAuth config
      const oauth: MCPOAuthConfig | undefined =
        values.authType === "oauth"
          ? {
              clientId: values.oauthClientId || undefined,
              scopes: values.oauthScopes || undefined,
            }
          : undefined;

      const timeout = values.timeout * 1000;

      if (editingServer) {
        updateServer(editingServer.id, {
          name: values.name,
          url: values.url,
          authType: values.authType as MCPAuthType,
          headers,
          timeout,
          oauth,
        });
      } else {
        addServer({
          name: values.name,
          url: values.url,
          enabled: true,
          authType: values.authType as MCPAuthType,
          headers,
          timeout,
          oauth,
        });
      }

      setView("list");
      setEditingServer(null);
      setCatalogPrefill(null);
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
    <Modal open={open} onClose={onClose} className={view === "catalog" ? "max-w-4xl" : "max-w-2xl"}>
      <ModalClose onClose={onClose} />
      <ModalHeader>
        <ModalTitle className="flex items-center gap-2">
          <Plug className="h-5 w-5 text-primary" />
          MCP Server Configuration
        </ModalTitle>
      </ModalHeader>

      <ModalContent className="max-h-[60vh] overflow-y-auto">
        {view === "form" && (
          <ServerForm
            editingServer={editingServer}
            onSubmit={handleFormSubmit}
            onCancel={handleFormCancel}
            prefill={editingServer ? null : (catalogPrefill ?? prefill)}
          />
        )}
        {view === "catalog" && (
          <MCPCatalog
            onPick={handleCatalogPick}
            onAddManual={handleAddManual}
            onCancel={handleCatalogCancel}
            favorites={favorites}
          />
        )}
        {view === "list" && (
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

      {view === "list" && (
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
