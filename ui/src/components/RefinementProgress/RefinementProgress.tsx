import { Sparkles } from "lucide-react";

import type { RefinementRound } from "@/stores/streamingStore";
import { useActiveRefinedState } from "@/stores/streamingStore";
import type { MessageUsage } from "@/components/chat-types";
import {
  ProgressContainer,
  ModeHeader,
  StatusBadge,
  ModelBadge,
  ExpandButton,
  UsageSummary,
  ResponseCard,
  getShortModelName,
  aggregateUsage,
  type ProgressPhase,
} from "@/components/ModeProgress";

interface RefinementProgressProps {
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    currentRound: number;
    totalRounds: number;
    rounds: Array<{
      model: string;
      content: string;
      usage?: MessageUsage;
    }>;
  };
}

/**
 * RefinementProgress - Visual indicator for refined mode
 *
 * Shows the refinement process:
 * 1. During "initial" phase: First model is generating initial response
 * 2. During "refining" phase: Shows current round and model refining
 * 3. During "done" phase: Shows completion with expandable refinement history
 *
 * Uses the new `useActiveRefinedState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function RefinementProgress({ persistedMetadata }: RefinementProgressProps) {
  // Use new discriminated union selector for live streaming state
  const liveState = useActiveRefinedState();

  // Determine which state to use: persisted metadata or live streaming state
  const isPersisted = !!persistedMetadata;
  const phase = isPersisted ? "done" : (liveState?.phase ?? "done");
  const currentRound = persistedMetadata?.currentRound ?? liveState?.currentRound ?? 0;
  const totalRounds = persistedMetadata?.totalRounds ?? liveState?.totalRounds ?? 1;
  const currentModel = liveState?.currentModel ?? "";
  const rounds: RefinementRound[] = persistedMetadata?.rounds ?? liveState?.rounds ?? [];

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isInitial = phase === "initial";
  const isRefining = phase === "refining";
  const isDone = phase === "done";

  // Map internal phase to ProgressPhase
  const progressPhase: ProgressPhase = isInitial ? "initial" : isRefining ? "active" : "complete";

  // Use persisted rounds if provided, otherwise use live state
  const displayRounds = rounds;
  const hasHistory = displayRounds.length > 1; // More than just initial response
  const { totalTokens, cost: totalCost } = aggregateUsage(displayRounds);

  // Build status badge text
  const statusText = isInitial
    ? "INITIAL"
    : isRefining
      ? `ROUND ${currentRound + 1}/${totalRounds}`
      : "COMPLETE";

  return (
    <div className="mb-3">
      <ProgressContainer
        phase={progressPhase}
        isLoading={isInitial || isRefining}
        icon={Sparkles}
        header={
          <ModeHeader
            name="Refined"
            badge={<StatusBadge text={statusText} variant={progressPhase} />}
          />
        }
        expandableSection={
          hasHistory ? (
            <>
              {displayRounds.map((round, index) => (
                <ResponseCard
                  key={index}
                  title={`Round ${index + 1}: ${getShortModelName(round.model)}`}
                  content={round.content}
                  usage={round.usage}
                />
              ))}
            </>
          ) : undefined
        }
        expandLabel={{ collapsed: "Show history", expanded: "Hide history" }}
        showExpandable={hasHistory}
        renderFooter={
          isDone
            ? ({ isExpanded, toggleExpand, hasExpandable }) => (
                <div className="mt-1 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <p className="text-[10px] text-muted-foreground">
                      Refined through {displayRounds.length} round
                      {displayRounds.length !== 1 ? "s" : ""}
                    </p>
                    {displayRounds.length > 0 && (
                      <UsageSummary totalTokens={totalTokens} totalCost={totalCost} />
                    )}
                  </div>
                  {hasExpandable && (
                    <ExpandButton
                      isExpanded={isExpanded}
                      onToggle={toggleExpand}
                      expandedLabel="Hide history"
                      collapsedLabel="Show history"
                    />
                  )}
                </div>
              )
            : undefined
        }
      >
        {/* Progress during initial phase */}
        {isInitial && (
          <div className="mt-2 space-y-1">
            <div className="flex items-center gap-1.5">
              <span className="text-xs text-muted-foreground">Generating initial response:</span>
              <ModelBadge model={currentModel} variant="blue" />
            </div>
            <p className="text-[10px] text-muted-foreground animate-pulse">
              Round 1 of {totalRounds}...
            </p>
          </div>
        )}

        {/* Progress during refining phase */}
        {isRefining && (
          <div className="mt-2 space-y-1">
            <div className="flex items-center gap-1.5">
              <span className="text-xs text-muted-foreground">Refining with:</span>
              <ModelBadge model={currentModel} variant="amber" />
            </div>
            {/* Show completed rounds */}
            {rounds.length > 0 && (
              <div className="flex items-center gap-1 flex-wrap">
                {rounds.map((round, idx) => (
                  <ModelBadge key={idx} model={round.model} variant="primary" showCheck />
                ))}
              </div>
            )}
            <p className="text-[10px] text-muted-foreground animate-pulse">Improving response...</p>
          </div>
        )}
      </ProgressContainer>
    </div>
  );
}
