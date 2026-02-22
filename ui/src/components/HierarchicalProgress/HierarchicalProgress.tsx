import { useState } from "react";
import {
  Network,
  Loader2,
  Check,
  Circle,
  ChevronDown,
  ChevronRight,
  AlertCircle,
  GitBranch,
} from "lucide-react";

import type { HierarchicalSubtask, HierarchicalWorkerResult } from "@/stores/streamingStore";
import { useActiveHierarchicalState } from "@/stores/streamingStore";
import type {
  MessageUsage,
  HierarchicalSubtaskData,
  HierarchicalWorkerResultData,
} from "@/components/chat-types";
import {
  getShortModelName,
  ResponseCard,
  UsageSummary,
  aggregateUsage,
} from "@/components/ModeProgress";
import { cn } from "@/utils/cn";

interface HierarchicalProgressProps {
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    subtasks: HierarchicalSubtaskData[];
    workerResults: HierarchicalWorkerResultData[];
    coordinatorModel?: string;
    aggregateUsage?: MessageUsage;
  };
}

/**
 * HierarchicalProgress - Visual indicator for hierarchical mode
 *
 * Shows the hierarchical delegation process:
 * 1. During "decomposing" phase: Shows coordinator analyzing and breaking down task
 * 2. During "executing" phase: Shows workers completing their assigned subtasks
 * 3. During "synthesizing" phase: Shows coordinator combining results
 * 4. During "done" phase: Shows final result with expandable subtask details
 *
 * Uses the new `useActiveHierarchicalState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function HierarchicalProgress({ persistedMetadata }: HierarchicalProgressProps) {
  // All hooks must be called before any early returns
  const [isExpanded, setIsExpanded] = useState(false);

  // Use new discriminated union selector for live streaming state
  const liveState = useActiveHierarchicalState();

  // Determine which state to use: persisted metadata or live streaming state
  const isPersisted = !!persistedMetadata;
  const phase = isPersisted ? "done" : (liveState?.phase ?? "done");
  const coordinatorModel = persistedMetadata?.coordinatorModel ?? liveState?.coordinatorModel ?? "";
  const subtasks = liveState?.subtasks ?? [];
  const workerResults = liveState?.workerResults ?? [];
  const decompositionUsage = liveState?.decompositionUsage;
  const synthesisUsage = liveState?.synthesisUsage;

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isDecomposing = phase === "decomposing";
  const isExecuting = phase === "executing";
  const isSynthesizing = phase === "synthesizing";
  const isDone = phase === "done";

  // Use persisted data if available
  const displaySubtasks = persistedMetadata?.subtasks ?? subtasks ?? [];
  const displayWorkerResults = persistedMetadata?.workerResults ?? workerResults ?? [];
  const displayCoordinatorModel = coordinatorModel;

  // Calculate usage
  const usageItems = displayWorkerResults.map((r) => ({ usage: r.usage }));
  if (decompositionUsage) usageItems.push({ usage: decompositionUsage });
  if (synthesisUsage) usageItems.push({ usage: synthesisUsage });
  const computedUsage = aggregateUsage(usageItems);
  const totalUsage = persistedMetadata?.aggregateUsage
    ? {
        totalTokens: persistedMetadata.aggregateUsage.totalTokens,
        totalCost: persistedMetadata.aggregateUsage.cost ?? 0,
      }
    : { totalTokens: computedUsage.totalTokens, totalCost: computedUsage.cost ?? 0 };

  // Count completed and in-progress subtasks
  const completedCount = displaySubtasks.filter((s) => s.status === "complete").length;
  const failedCount = displaySubtasks.filter((s) => s.status === "failed").length;
  const inProgressCount = displaySubtasks.filter((s) => s.status === "in_progress").length;
  const totalSubtasks = displaySubtasks.length;

  // Determine container styling based on phase
  const containerStyle = isDecomposing
    ? "bg-cyan-500/10 border-cyan-500/30"
    : isExecuting
      ? "bg-blue-500/10 border-blue-500/30"
      : isSynthesizing
        ? "bg-amber-500/10 border-amber-500/30"
        : "bg-primary/5 border-primary/30";

  const iconStyle = isDecomposing
    ? "text-cyan-500"
    : isExecuting
      ? "text-blue-500"
      : isSynthesizing
        ? "text-amber-500"
        : "text-primary";

  // Group worker results by model for display
  const resultsByModel = new Map<
    string,
    HierarchicalWorkerResult[] | HierarchicalWorkerResultData[]
  >();
  for (const result of displayWorkerResults) {
    const existing = resultsByModel.get(result.model) || [];
    existing.push(result);
    resultsByModel.set(result.model, existing);
  }

  return (
    <div className={cn("rounded-lg border", containerStyle)}>
      <div className="flex items-start gap-2 px-3 py-2">
        {/* Icon */}
        <div className="shrink-0 mt-0.5">
          {isDecomposing || isExecuting || isSynthesizing ? (
            <Loader2 className={cn("h-4 w-4 animate-spin", iconStyle)} />
          ) : (
            <Network className={cn("h-4 w-4", iconStyle)} />
          )}
        </div>

        <div className="flex-1 min-w-0">
          {/* Header */}
          <div className="flex items-center gap-2 text-xs">
            <Network className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="font-medium text-muted-foreground">Hierarchical</span>
            {isDecomposing && (
              <span className="px-1.5 py-0.5 rounded text-[9px] font-semibold bg-cyan-500/20 text-cyan-600 dark:text-cyan-400">
                Decomposing
              </span>
            )}
            {isExecuting && (
              <span className="px-1.5 py-0.5 rounded text-[9px] font-semibold bg-blue-500/20 text-blue-700 dark:text-blue-400">
                Executing {completedCount}/{totalSubtasks}
              </span>
            )}
            {isSynthesizing && (
              <span className="px-1.5 py-0.5 rounded text-[9px] font-semibold bg-amber-500/20 text-amber-800 dark:text-amber-400">
                Synthesizing
              </span>
            )}
            {isDone && (
              <span className="px-1.5 py-0.5 rounded text-[9px] font-semibold bg-primary/10 text-primary">
                Done
              </span>
            )}
          </div>

          {/* Progress details */}
          <div className="mt-1.5 space-y-2">
            {/* Coordinator model */}
            <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
              <span>Coordinator:</span>
              <span className="px-1.5 py-0.5 rounded bg-primary/10 text-primary font-medium">
                {getShortModelName(displayCoordinatorModel)}
              </span>
            </div>

            {/* Subtasks progress (during decomposing or later) */}
            {totalSubtasks > 0 && (
              <div className="space-y-1.5">
                <div className="text-[10px] text-muted-foreground flex items-center gap-1">
                  <GitBranch className="h-3 w-3" />
                  <span>
                    Subtasks ({completedCount}/{totalSubtasks} complete)
                  </span>
                  {failedCount > 0 && <span className="text-red-500">({failedCount} failed)</span>}
                </div>

                {/* Subtask list */}
                <div className="flex flex-wrap gap-1">
                  {displaySubtasks.map((subtask) => (
                    <SubtaskBadge key={subtask.id} subtask={subtask} />
                  ))}
                </div>
              </div>
            )}

            {/* Workers in progress */}
            {isExecuting && inProgressCount > 0 && (
              <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
                <Loader2 className="h-3 w-3 animate-spin" />
                <span>Workers executing: {inProgressCount}</span>
              </div>
            )}

            {/* Usage summary */}
            {isDone && totalUsage.totalTokens > 0 && (
              <div className="mt-1">
                <UsageSummary
                  totalTokens={totalUsage.totalTokens}
                  totalCost={totalUsage.totalCost}
                  label="Total"
                />
              </div>
            )}

            {/* Expand/collapse button */}
            {isDone && displayWorkerResults.length > 0 && (
              <div className="flex items-center justify-end">
                <button
                  onClick={() => setIsExpanded(!isExpanded)}
                  className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground transition-colors"
                >
                  {isExpanded ? (
                    <>
                      <ChevronDown className="h-3 w-3" />
                      Hide subtasks
                    </>
                  ) : (
                    <>
                      <ChevronRight className="h-3 w-3" />
                      Show subtasks
                    </>
                  )}
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Expandable worker results section */}
      {isDone && isExpanded && displayWorkerResults.length > 0 && (
        <div className="border-t border-border/50 px-3 py-2 space-y-2">
          <div className="text-[10px] font-medium text-muted-foreground mb-2">
            Worker Results ({displayWorkerResults.length})
          </div>
          {displayWorkerResults.map((result, index) => (
            <ResponseCard
              key={`${result.subtaskId}-${index}`}
              title={`${getShortModelName(result.model)}: ${result.description}`}
              content={result.content}
              usage={result.usage}
              variant="blue"
              previewLength={150}
            />
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * SubtaskBadge - Shows individual subtask status
 */
function SubtaskBadge({ subtask }: { subtask: HierarchicalSubtask | HierarchicalSubtaskData }) {
  const statusStyles = {
    pending: "bg-muted text-muted-foreground",
    in_progress: "bg-blue-500/20 text-blue-700 dark:text-blue-400",
    complete: "bg-primary/10 text-primary",
    failed: "bg-red-500/20 text-red-700 dark:text-red-400",
  };

  const StatusIcon = {
    pending: Circle,
    in_progress: Loader2,
    complete: Check,
    failed: AlertCircle,
  }[subtask.status];

  return (
    <div
      className={cn(
        "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium",
        statusStyles[subtask.status]
      )}
      title={subtask.description}
    >
      <StatusIcon className={cn("h-3 w-3", subtask.status === "in_progress" && "animate-spin")} />
      <span className="max-w-[100px] truncate">{subtask.id}</span>
    </div>
  );
}
