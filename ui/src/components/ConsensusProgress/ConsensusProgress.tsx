import { useState } from "react";
import { Handshake, Loader2, Check, Circle, ChevronDown, ChevronRight, Target } from "lucide-react";

import type { CandidateResponse } from "@/stores/streamingStore";
import { useActiveConsensusState } from "@/stores/streamingStore";
import type { MessageUsage, ConsensusRoundData } from "@/components/chat-types";
import {
  getShortModelName,
  ResponseCard,
  UsageSummary,
  aggregateUsage,
} from "@/components/ModeProgress";
import { cn } from "@/utils/cn";

interface ConsensusProgressProps {
  /** All models participating (for display during responding) */
  allModels?: string[];
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    rounds: ConsensusRoundData[];
    finalScore?: number;
    consensusReached?: boolean;
    aggregateUsage?: MessageUsage;
    threshold?: number;
  };
}

/**
 * ConsensusProgress - Visual indicator for consensus mode
 *
 * Shows the consensus building process:
 * 1. During "responding" phase: Shows progress of initial parallel model responses
 * 2. During "revising" phase: Shows revision progress for current round
 * 3. During "done" phase: Shows consensus result with score and expandable round history
 *
 * Uses the new `useActiveConsensusState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function ConsensusProgress({ allModels, persistedMetadata }: ConsensusProgressProps) {
  // All hooks must be called before any early returns
  const [isExpanded, setIsExpanded] = useState(false);

  // Use new discriminated union selector for live streaming state
  const liveState = useActiveConsensusState();

  // Determine which state to use: persisted metadata or live streaming state
  const isPersisted = !!persistedMetadata;
  const phase = isPersisted ? "done" : (liveState?.phase ?? "done");
  const currentRound = liveState?.currentRound ?? 0;
  const maxRounds = liveState?.maxRounds ?? 5;
  const threshold = persistedMetadata?.threshold ?? liveState?.threshold ?? 0.8;
  const rounds = liveState?.rounds ?? [];
  const finalScore = persistedMetadata?.finalScore ?? liveState?.finalScore;
  const currentRoundResponses: CandidateResponse[] = liveState?.currentRoundResponses ?? [];

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isResponding = phase === "responding";
  const isRevising = phase === "revising";
  const isDone = phase === "done";

  // Use persisted data if available
  const displayRounds = persistedMetadata?.rounds ?? rounds ?? [];
  const displayFinalScore = finalScore;
  const consensusReached =
    persistedMetadata?.consensusReached ?? (displayFinalScore ?? 0) >= threshold;

  // Get display models from the first round or from props
  const displayModels =
    allModels ||
    displayRounds[0]?.responses.map((r) => r.model) ||
    currentRoundResponses.map((r) => r.model);
  const completedModels = currentRoundResponses.map((r) => r.model);

  // Determine container styling based on phase
  const containerStyle = isResponding
    ? "bg-blue-500/10 border-blue-500/30"
    : isRevising
      ? "bg-amber-500/10 border-amber-500/30"
      : consensusReached
        ? "bg-primary/5 border-primary/30"
        : "bg-orange-500/10 border-orange-500/30";

  const iconColor = isResponding
    ? "text-blue-500"
    : isRevising
      ? "text-amber-500"
      : consensusReached
        ? "text-primary"
        : "text-orange-500";

  // Calculate aggregate usage
  const computedUsage = displayRounds.reduce(
    (acc, round) => {
      const roundUsage = aggregateUsage(round.responses);
      return {
        totalTokens: acc.totalTokens + roundUsage.totalTokens,
        totalCost: acc.totalCost + (roundUsage.cost ?? 0),
      };
    },
    { totalTokens: 0, totalCost: 0 }
  );
  const totalUsage = persistedMetadata?.aggregateUsage
    ? {
        totalTokens: persistedMetadata.aggregateUsage.totalTokens,
        totalCost: persistedMetadata.aggregateUsage.cost ?? 0,
      }
    : computedUsage;

  const hasRounds = displayRounds.length > 0;

  return (
    <div className={cn("rounded-lg border", containerStyle)}>
      {/* Header section */}
      <div className="flex items-start gap-2 px-3 py-2">
        <div className="shrink-0 mt-0.5">
          {isResponding || isRevising ? (
            <Loader2 className={cn("h-4 w-4 animate-spin", iconColor)} />
          ) : (
            <Handshake className={cn("h-4 w-4", iconColor)} />
          )}
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 text-xs font-medium">
            <Handshake className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-muted-foreground">Consensus</span>
            <span
              className={cn(
                "px-1.5 py-0.5 rounded text-[9px] font-semibold",
                isResponding && "bg-blue-500/20 text-blue-700 dark:text-blue-400",
                isRevising && "bg-amber-500/20 text-amber-800 dark:text-amber-400",
                isDone && consensusReached && "bg-primary/10 text-primary",
                isDone &&
                  !consensusReached &&
                  "bg-orange-500/20 text-orange-800 dark:text-orange-400"
              )}
            >
              {isResponding
                ? "INITIAL"
                : isRevising
                  ? `ROUND ${currentRound + 1}/${maxRounds}`
                  : consensusReached
                    ? "CONSENSUS"
                    : "MAX ROUNDS"}
            </span>
          </div>

          {/* Model progress during responding phase */}
          {isResponding && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center gap-1.5 flex-wrap">
                {displayModels.map((model) => {
                  const isComplete = completedModels.includes(model);
                  return (
                    <div
                      key={model}
                      className={cn(
                        "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors",
                        isComplete ? "bg-primary/10 text-primary" : "bg-muted text-muted-foreground"
                      )}
                    >
                      {isComplete ? (
                        <Check className="h-3 w-3" />
                      ) : (
                        <Circle className="h-3 w-3 animate-pulse" />
                      )}
                      <span className="max-w-[80px] truncate">{getShortModelName(model)}</span>
                    </div>
                  );
                })}
              </div>
              <p className="text-[10px] text-muted-foreground">
                {completedModels.length}/{displayModels.length} initial responses
              </p>
            </div>
          )}

          {/* Revision progress during revising phase */}
          {isRevising && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center gap-1.5 flex-wrap">
                {displayModels.map((model) => {
                  const isComplete = completedModels.includes(model);
                  return (
                    <div
                      key={model}
                      className={cn(
                        "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors",
                        isComplete
                          ? "bg-amber-500/20 text-amber-800 dark:text-amber-400"
                          : "bg-muted text-muted-foreground"
                      )}
                    >
                      {isComplete ? (
                        <Check className="h-3 w-3" />
                      ) : (
                        <Circle className="h-3 w-3 animate-pulse" />
                      )}
                      <span className="max-w-[80px] truncate">{getShortModelName(model)}</span>
                    </div>
                  );
                })}
              </div>
              <p className="text-[10px] text-muted-foreground animate-pulse">
                {completedModels.length}/{displayModels.length} revisions for round{" "}
                {currentRound + 1}
                ...
              </p>
            </div>
          )}

          {/* Consensus result */}
          {isDone && (
            <div className="mt-2 space-y-2">
              {/* Consensus status */}
              <div className="flex items-center gap-2">
                <Target
                  className={cn("h-4 w-4", consensusReached ? "text-primary" : "text-orange-500")}
                />
                <span className="text-xs font-medium">
                  {consensusReached ? "Consensus reached" : "Max rounds reached"}
                </span>
                {displayFinalScore !== undefined && (
                  <span className="text-[10px] text-muted-foreground">
                    (Score: {(displayFinalScore * 100).toFixed(1)}% / Threshold:{" "}
                    {(threshold * 100).toFixed(0)}%)
                  </span>
                )}
              </div>

              {/* Round progression */}
              {hasRounds && (
                <div className="flex items-center gap-1.5 flex-wrap">
                  {displayRounds.map((round, index) => {
                    const isLastRound = index === displayRounds.length - 1;
                    const roundScore = round.consensusScore ?? 0;
                    return (
                      <div
                        key={index}
                        className={cn(
                          "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium",
                          round.consensusReached
                            ? "bg-primary/10 text-primary"
                            : isLastRound
                              ? "bg-orange-500/20 text-orange-800 dark:text-orange-400"
                              : "bg-muted text-muted-foreground"
                        )}
                      >
                        <span>R{index}</span>
                        {roundScore > 0 && (
                          <span className="text-[9px]">{(roundScore * 100).toFixed(0)}%</span>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}

              {/* Usage and expand button */}
              <div className="flex items-center justify-between">
                <UsageSummary
                  totalTokens={totalUsage.totalTokens}
                  totalCost={totalUsage.totalCost}
                />
                {hasRounds && (
                  <button
                    onClick={() => setIsExpanded(!isExpanded)}
                    className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground transition-colors"
                  >
                    {isExpanded ? (
                      <>
                        <ChevronDown className="h-3 w-3" />
                        Hide rounds
                      </>
                    ) : (
                      <>
                        <ChevronRight className="h-3 w-3" />
                        Show rounds
                      </>
                    )}
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Expandable rounds section */}
      {isDone && isExpanded && hasRounds && (
        <div className="border-t border-border/50 px-3 py-2 space-y-4">
          {displayRounds.map((round, roundIndex) => (
            <div key={roundIndex}>
              <div className="flex items-center gap-2 mb-2">
                <p className="text-[10px] font-medium text-muted-foreground">
                  {roundIndex === 0 ? "Initial Responses" : `Round ${roundIndex} Revisions`}
                </p>
                {round.consensusScore !== undefined && (
                  <span
                    className={cn(
                      "px-1.5 py-0.5 rounded text-[9px] font-medium",
                      round.consensusReached
                        ? "bg-primary/10 text-primary"
                        : "bg-muted text-muted-foreground"
                    )}
                  >
                    {(round.consensusScore * 100).toFixed(1)}% agreement
                  </span>
                )}
              </div>
              <div className="space-y-2">
                {round.responses.map((response, respIndex) => (
                  <ResponseCard
                    key={`${roundIndex}-${respIndex}`}
                    title={getShortModelName(response.model)}
                    content={response.content}
                    usage={response.usage}
                    variant={
                      round.consensusReached && roundIndex === displayRounds.length - 1
                        ? "blue"
                        : "default"
                    }
                  />
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
