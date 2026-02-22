import { memo } from "react";
import { Zap, ChevronDown, ChevronUp, Loader2, AlertCircle } from "lucide-react";
import type { ToolExecutionRound } from "@/components/chat-types";
import { getToolIcon, getToolShortName } from "@/components/ToolIcons";

interface ExecutionSummaryBarProps {
  rounds: ToolExecutionRound[];
  /** Whether the timeline is expanded */
  isExpanded: boolean;
  /** Toggle expand/collapse */
  onToggle: () => void;
  /** Whether execution is still in progress */
  isStreaming?: boolean;
}

/** Format duration in human-readable form */
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`;
}

/**
 * Subtle summary bar for tool execution timeline
 *
 * Shows tool count, retry count, total duration, and tool type icons.
 * Clicking toggles between collapsed and expanded views.
 */
function ExecutionSummaryBarComponent({
  rounds,
  isExpanded,
  onToggle,
  isStreaming = false,
}: ExecutionSummaryBarProps) {
  // Calculate summary stats
  const totalExecutions = rounds.reduce((sum, round) => sum + round.executions.length, 0);
  const failedExecutions = rounds.reduce(
    (sum, round) => sum + round.executions.filter((e) => e.status === "error").length,
    0
  );
  const retryCount = rounds.length > 1 ? rounds.length - 1 : 0;
  const totalDuration = rounds.reduce((sum, round) => sum + (round.totalDuration || 0), 0);

  // Get unique tool names used
  const toolNames = [...new Set(rounds.flatMap((r) => r.executions.map((e) => e.toolName)))];

  // Check if any execution is currently running and get its status message
  const runningExecution = rounds.flatMap((r) => r.executions).find((e) => e.status === "running");
  const hasRunning = !!runningExecution;

  return (
    <button
      type="button"
      onClick={onToggle}
      className="group inline-flex items-center gap-1.5 rounded-md border border-zinc-200/80 bg-zinc-50/50 px-2 py-1 text-left transition-colors hover:bg-zinc-100/80 dark:border-zinc-700/60 dark:bg-zinc-800/30 dark:hover:bg-zinc-800/60"
    >
      {/* Leading icon */}
      {isStreaming || hasRunning ? (
        <Loader2 className="h-3 w-3 animate-spin text-blue-500" />
      ) : failedExecutions > 0 ? (
        <AlertCircle className="h-3 w-3 text-amber-500" />
      ) : (
        <Zap className="h-3 w-3 text-zinc-600 dark:text-zinc-400" />
      )}

      {/* Summary text - shows status message during execution */}
      <span className="text-[11px] text-zinc-600 dark:text-zinc-400">
        {isStreaming || hasRunning ? (
          runningExecution?.statusMessage || "running"
        ) : (
          <>
            {totalExecutions} tool{totalExecutions !== 1 ? "s" : ""}
            {retryCount > 0 && (
              <span className="text-amber-800 dark:text-amber-400"> · {retryCount} retry</span>
            )}
          </>
        )}
      </span>

      {/* Tool type icons */}
      <div className="flex items-center gap-0.5">
        {toolNames.slice(0, 3).map((toolName) => {
          const Icon = getToolIcon(toolName);
          const isRunning = isStreaming || hasRunning;
          return (
            <span
              key={toolName}
              className={`flex items-center rounded px-1 py-0.5 text-zinc-600 dark:text-zinc-400 ${isRunning ? "animate-pulse" : ""}`}
              title={getToolShortName(toolName)}
            >
              <Icon className="h-2.5 w-2.5" />
            </span>
          );
        })}
        {toolNames.length > 3 && (
          <span className="text-[10px] text-zinc-600 dark:text-zinc-400">
            +{toolNames.length - 3}
          </span>
        )}
      </div>

      {/* Duration */}
      {totalDuration > 0 && !isStreaming && !hasRunning && (
        <span className="text-[10px] text-zinc-600 dark:text-zinc-400">
          {formatDuration(totalDuration)}
        </span>
      )}

      {/* Expand/collapse indicator */}
      {isExpanded ? (
        <ChevronUp className="h-3 w-3 text-zinc-600 dark:text-zinc-400" />
      ) : (
        <ChevronDown className="h-3 w-3 text-zinc-600 dark:text-zinc-400" />
      )}
    </button>
  );
}

export const ExecutionSummaryBar = memo(ExecutionSummaryBarComponent);
