import { Loader2, Database } from "lucide-react";

import { cn } from "@/utils/cn";
import { PythonIcon, JavaScriptIcon, SqlIcon, getToolIcon } from "@/components/ToolIcons";

/** Types of tool calls that can be displayed */
export type ToolCallType =
  | "file_search"
  | "web_search"
  | "code_interpreter"
  | "js_code_interpreter"
  | "sql_query"
  | "chart_render"
  | "function";

/** Status of a tool call execution */
export type ToolCallStatus = "pending" | "executing" | "completed" | "failed";

/** Represents a single tool call being executed */
export interface ToolCall {
  /** Unique identifier for this tool call */
  id: string;
  /** Type of tool being called */
  type: ToolCallType;
  /** Tool function name (for function type) */
  name?: string;
  /** Current execution status */
  status: ToolCallStatus;
  /** Error message if failed */
  error?: string;
}

interface ToolCallIndicatorProps {
  /** List of tool calls being executed */
  toolCalls: ToolCall[];
  /** Optional class name for the container */
  className?: string;
}

/**
 * Get the display configuration for a tool type
 * Uses shared icons from ToolIcons for consistency across UI
 */
function getToolConfig(type: ToolCallType, name?: string) {
  switch (type) {
    case "file_search":
      return {
        icon: getToolIcon("file_search"),
        label: "Searching documents",
        color: "text-blue-500",
        bgColor: "bg-blue-500/10",
      };
    case "web_search":
      return {
        icon: getToolIcon("web_search"),
        label: "Searching web",
        color: "text-green-500",
        bgColor: "bg-green-500/10",
      };
    case "code_interpreter":
      return {
        icon: PythonIcon,
        label: "Running Python",
        color: "text-orange-500",
        bgColor: "bg-orange-500/10",
      };
    case "js_code_interpreter":
      return {
        icon: JavaScriptIcon,
        label: "Running JavaScript",
        color: "text-yellow-500",
        bgColor: "bg-yellow-500/10",
      };
    case "sql_query":
      return {
        icon: SqlIcon,
        label: "Running SQL",
        color: "text-cyan-500",
        bgColor: "bg-cyan-500/10",
      };
    case "chart_render":
      return {
        icon: getToolIcon("chart_render"),
        label: "Rendering chart",
        color: "text-emerald-500",
        bgColor: "bg-emerald-500/10",
      };
    case "function":
      return {
        icon: Database,
        label: name ? `Calling ${name}` : "Calling function",
        color: "text-purple-500",
        bgColor: "bg-purple-500/10",
      };
  }
}

/** Get the status icon/animation for a tool call */
function ToolCallStatusIcon({ status, type }: { status: ToolCallStatus; type: ToolCallType }) {
  const config = getToolConfig(type);
  const Icon = config.icon;

  if (status === "executing") {
    return <Loader2 className={cn("h-4 w-4 animate-spin", config.color)} />;
  }

  return (
    <Icon
      className={cn("h-4 w-4", status === "completed" ? "text-muted-foreground" : config.color)}
    />
  );
}

/**
 * ToolCallIndicator - Visual indicator for tool call execution
 *
 * Displays the status of tool calls being executed during chat interactions.
 * Used in client-side RAG mode where the UI orchestrates tool execution.
 *
 * ## When to Use
 *
 * Show this component when:
 * - The model has requested a tool call (file_search, web_search, etc.)
 * - The UI is executing the tool call
 * - Waiting for tool results before continuing the response
 *
 * ## Usage Example
 *
 * ```tsx
 * <ToolCallIndicator
 *   toolCalls={[
 *     { id: "1", type: "file_search", status: "executing" },
 *     { id: "2", type: "web_search", status: "pending" },
 *   ]}
 * />
 * ```
 */
export function ToolCallIndicator({ toolCalls, className }: ToolCallIndicatorProps) {
  if (toolCalls.length === 0) {
    return null;
  }

  // Group by status for display
  const executing = toolCalls.filter((tc) => tc.status === "executing");
  const pending = toolCalls.filter((tc) => tc.status === "pending");
  const failed = toolCalls.filter((tc) => tc.status === "failed");

  const activeCount = executing.length + pending.length;
  const hasFailures = failed.length > 0;

  return (
    <div
      className={cn(
        "flex flex-wrap items-center gap-2 px-3 py-2 bg-muted/50 rounded-lg border border-border/50",
        className
      )}
      role="status"
      aria-live="polite"
      aria-label={`${activeCount} tool call${activeCount !== 1 ? "s" : ""} in progress`}
    >
      {/* Show each tool call */}
      {toolCalls.map((toolCall) => {
        const config = getToolConfig(toolCall.type, toolCall.name);

        return (
          <div
            key={toolCall.id}
            className={cn(
              "flex items-center gap-1.5 px-2 py-1 rounded-md text-xs font-medium transition-colors",
              toolCall.status === "executing" && config.bgColor,
              toolCall.status === "executing" && "animate-pulse",
              toolCall.status === "pending" && "bg-muted text-muted-foreground",
              toolCall.status === "completed" && "bg-muted/50 text-muted-foreground",
              toolCall.status === "failed" && "bg-destructive/10 text-destructive"
            )}
          >
            <ToolCallStatusIcon status={toolCall.status} type={toolCall.type} />
            <span>{config.label}</span>
            {toolCall.status === "executing" && <span className="sr-only">in progress</span>}
            {toolCall.status === "failed" && toolCall.error && (
              <span className="text-[10px] ml-1">({toolCall.error})</span>
            )}
          </div>
        );
      })}

      {/* Summary for multiple calls */}
      {toolCalls.length > 1 && (
        <span className="text-xs text-muted-foreground ml-1">
          {executing.length > 0 && `${executing.length} running`}
          {executing.length > 0 && pending.length > 0 && ", "}
          {pending.length > 0 && `${pending.length} queued`}
          {hasFailures && ` (${failed.length} failed)`}
        </span>
      )}
    </div>
  );
}
