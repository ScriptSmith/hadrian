/**
 * MCPApprovalRequest — approve/deny prompt for a gateway MCP tool call that
 * paused on `require_approval`. Approving resumes the response server-side via
 * an `mcp_approval_response`; denying refuses the call.
 */

import { useState } from "react";
import { Check, ChevronDown, ChevronRight, ShieldAlert, X } from "lucide-react";

import { Button } from "@/components/Button/Button";
import { Badge } from "@/components/Badge/Badge";
import { cn } from "@/utils/cn";
import type { McpApprovalRequest } from "@/components/chat-types";

export interface MCPApprovalRequestProps {
  approval: McpApprovalRequest;
  /** Called with the user's decision. */
  onRespond: (approve: boolean) => void;
  /** Disable the buttons (e.g. while a resume is streaming). */
  disabled?: boolean;
}

function prettyArgs(approval: McpApprovalRequest): string {
  if (approval.parsedArguments) {
    try {
      return JSON.stringify(approval.parsedArguments, null, 2);
    } catch {
      // fall through to the raw string
    }
  }
  return approval.argumentsJson || "{}";
}

export function MCPApprovalRequest({
  approval,
  onRespond,
  disabled = false,
}: MCPApprovalRequestProps) {
  const [expanded, setExpanded] = useState(false);
  const resolved = approval.resolved;

  return (
    <div
      className={cn(
        "my-2 rounded-lg border px-3 py-2 text-sm",
        resolved ? "border-border bg-muted/40" : "border-amber-500/40 bg-amber-500/5"
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <ShieldAlert
            className={cn(
              "h-4 w-4 shrink-0",
              resolved ? "text-muted-foreground" : "text-amber-600 dark:text-amber-500"
            )}
          />
          <div className="min-w-0">
            <p className="font-medium">{resolved ? "Tool call reviewed" : "Approval required"}</p>
            <p className="text-xs text-muted-foreground truncate">
              <span className="font-mono">{approval.serverLabel}</span>
              {" · "}
              <span className="font-mono">{approval.toolName}</span>
            </p>
          </div>
        </div>

        {resolved ? (
          <Badge variant={resolved === "approved" ? "default" : "destructive"}>
            {resolved === "approved" ? "Approved" : "Denied"}
          </Badge>
        ) : (
          <div className="flex items-center gap-1 shrink-0">
            <Button size="sm" variant="ghost" onClick={() => onRespond(false)} disabled={disabled}>
              <X className="h-4 w-4 mr-1" />
              Deny
            </Button>
            <Button size="sm" variant="primary" onClick={() => onRespond(true)} disabled={disabled}>
              <Check className="h-4 w-4 mr-1" />
              Approve
            </Button>
          </div>
        )}
      </div>

      <button
        type="button"
        className="mt-2 flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
      >
        {expanded ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
        {expanded ? "Hide arguments" : "Show arguments"}
      </button>
      {expanded && (
        <pre className="mt-1 max-h-48 overflow-auto rounded bg-muted p-2 text-xs">
          <code>{prettyArgs(approval)}</code>
        </pre>
      )}
    </div>
  );
}
