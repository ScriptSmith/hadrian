import { memo } from "react";
import { MessageSquare } from "lucide-react";
import type { ToolExecutionRound, Artifact } from "@/components/chat-types";
import { ToolExecutionStep } from "./ToolExecutionStep";

interface ExecutionTimelineProps {
  rounds: ToolExecutionRound[];
  /** Callback when an artifact is clicked for expansion */
  onArtifactClick?: (artifact: Artifact) => void;
}

/**
 * Expanded timeline view of tool executions
 *
 * Shows all rounds of execution with their tool calls and
 * any model reasoning between rounds.
 */
function ExecutionTimelineComponent({ rounds, onArtifactClick }: ExecutionTimelineProps) {
  return (
    <div className="mt-2 space-y-3">
      {rounds.map((round, roundIndex) => (
        <div key={round.round}>
          {/* Round header (only show if multiple rounds) */}
          {rounds.length > 1 && (
            <div className="mb-1.5 flex items-center gap-1.5">
              <span className="text-[10px] font-medium uppercase tracking-wider text-zinc-600 dark:text-zinc-400">
                Round {round.round}
              </span>
              {round.hasError && (
                <span className="rounded px-1 py-0.5 text-[9px] font-medium text-red-700 bg-red-50 dark:bg-red-900/30 dark:text-red-400">
                  failed
                </span>
              )}
            </div>
          )}

          {/* Tool executions in this round */}
          <div className="space-y-0.5">
            {round.executions.map((execution) => (
              <ToolExecutionStep
                key={execution.id}
                execution={execution}
                onArtifactClick={onArtifactClick}
              />
            ))}
          </div>

          {/* Model reasoning between rounds */}
          {round.modelReasoning && roundIndex < rounds.length - 1 && (
            <div className="my-2 flex items-start gap-1.5 rounded-md bg-zinc-50 dark:bg-zinc-800/50 px-2 py-1.5 border-l-2 border-zinc-300 dark:border-zinc-600">
              <MessageSquare className="mt-0.5 h-3 w-3 flex-shrink-0 text-zinc-600 dark:text-zinc-400" />
              <p className="text-[11px] italic text-zinc-600 dark:text-zinc-400">
                {round.modelReasoning}
              </p>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

export const ExecutionTimeline = memo(ExecutionTimelineComponent);
