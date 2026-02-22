import { useState } from "react";
import { Swords, Loader2, Check, Circle, ChevronDown, ChevronRight, FileText } from "lucide-react";

import { useActiveDebatedState } from "@/stores/streamingStore";
import type { MessageUsage, DebateTurnData } from "@/components/chat-types";
import {
  getShortModelName,
  ResponseCard,
  UsageSummary,
  aggregateUsage,
} from "@/components/ModeProgress";
import { cn } from "@/utils/cn";

interface DebateProgressProps {
  /** All models participating (for display) */
  allModels?: string[];
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    turns: DebateTurnData[];
    positions: Record<string, string>;
    debateRounds?: number;
    summarizerModel?: string;
    aggregateUsage?: MessageUsage;
  };
}

/**
 * DebateProgress - Visual indicator for debate mode
 *
 * Shows the debate process:
 * 1. During "opening" phase: Shows progress of opening statements
 * 2. During "debating" phase: Shows progress of rebuttals per round
 * 3. During "summarizing" phase: Shows summary in progress
 * 4. During "done" phase: Shows debate summary with expandable transcript
 *
 * Uses the new `useActiveDebatedState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function DebateProgress({ allModels, persistedMetadata }: DebateProgressProps) {
  // All hooks must be called before any early returns
  const [isExpanded, setIsExpanded] = useState(false);

  // Use new discriminated union selector for live streaming state
  const liveState = useActiveDebatedState();

  // Determine which state to use: persisted metadata or live streaming state
  const isPersisted = !!persistedMetadata;
  const phase = isPersisted ? "done" : (liveState?.phase ?? "done");
  const currentRound = liveState?.currentRound ?? 0;
  const totalRounds = liveState?.totalRounds ?? 0;
  const positions = liveState?.positions ?? {};
  const turns = liveState?.turns ?? [];
  const currentRoundTurns = liveState?.currentRoundTurns ?? [];
  const summarizerModel = persistedMetadata?.summarizerModel ?? liveState?.summarizerModel;

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isOpening = phase === "opening";
  const isDebating = phase === "debating";
  const isSummarizing = phase === "summarizing";
  const isDone = phase === "done";

  // Use persisted data if available
  const displayTurns = persistedMetadata?.turns ?? turns ?? [];
  const displayPositions = persistedMetadata?.positions ?? positions ?? {};
  const displaySummarizerModel = summarizerModel;

  // Get display models from positions or props
  const displayModels = allModels || Object.keys(displayPositions);
  const completedModels = currentRoundTurns.map((t) => t.model);

  // Determine container styling based on phase
  const containerStyle = isOpening
    ? "bg-blue-500/10 border-blue-500/30"
    : isDebating
      ? "bg-amber-500/10 border-amber-500/30"
      : isSummarizing
        ? "bg-purple-500/10 border-purple-500/30"
        : "bg-primary/5 border-primary/30";

  const iconColor = isOpening
    ? "text-blue-500"
    : isDebating
      ? "text-amber-500"
      : isSummarizing
        ? "text-purple-500"
        : "text-primary";

  // Calculate aggregate usage
  const computedUsage = aggregateUsage(
    displayTurns.map((t) => ({
      model: t.model,
      content: t.content,
      usage: t.usage,
    }))
  );
  const totalUsage = persistedMetadata?.aggregateUsage
    ? {
        totalTokens: persistedMetadata.aggregateUsage.totalTokens,
        totalCost: persistedMetadata.aggregateUsage.cost ?? 0,
      }
    : { totalTokens: computedUsage.totalTokens, totalCost: computedUsage.cost ?? 0 };

  const hasTurns = displayTurns.length > 0;

  // Group turns by round for display
  const turnsByRound: Record<number, DebateTurnData[]> = {};
  displayTurns.forEach((turn) => {
    if (!turnsByRound[turn.round]) {
      turnsByRound[turn.round] = [];
    }
    turnsByRound[turn.round].push(turn);
  });
  const roundNumbers = Object.keys(turnsByRound)
    .map(Number)
    .sort((a, b) => a - b);

  return (
    <div className={cn("rounded-lg border", containerStyle)}>
      {/* Header section */}
      <div className="flex items-start gap-2 px-3 py-2">
        <div className="shrink-0 mt-0.5">
          {isOpening || isDebating || isSummarizing ? (
            <Loader2 className={cn("h-4 w-4 animate-spin", iconColor)} />
          ) : (
            <Swords className={cn("h-4 w-4", iconColor)} />
          )}
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 text-xs font-medium">
            <Swords className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-muted-foreground">Debate</span>
            <span
              className={cn(
                "px-1.5 py-0.5 rounded text-[9px] font-semibold",
                isOpening && "bg-blue-500/20 text-blue-700 dark:text-blue-400",
                isDebating && "bg-amber-500/20 text-amber-800 dark:text-amber-400",
                isSummarizing && "bg-purple-500/20 text-purple-800 dark:text-purple-400",
                isDone && "bg-primary/10 text-primary"
              )}
            >
              {isOpening
                ? "OPENING"
                : isDebating
                  ? `ROUND ${currentRound}/${totalRounds}`
                  : isSummarizing
                    ? "SUMMARIZING"
                    : "COMPLETE"}
            </span>
          </div>

          {/* Model positions and progress during opening phase */}
          {isOpening && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center gap-1.5 flex-wrap">
                {displayModels.map((model) => {
                  const isComplete = completedModels.includes(model);
                  const position = displayPositions[model] || "unknown";
                  return (
                    <div
                      key={model}
                      className={cn(
                        "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors",
                        isComplete
                          ? position === "pro"
                            ? "bg-emerald-500/20 text-emerald-800 dark:text-emerald-400"
                            : "bg-rose-500/20 text-rose-800 dark:text-rose-400"
                          : "bg-muted text-muted-foreground"
                      )}
                    >
                      {isComplete ? (
                        <Check className="h-3 w-3" />
                      ) : (
                        <Circle className="h-3 w-3 animate-pulse" />
                      )}
                      <span className="max-w-[80px] truncate">{getShortModelName(model)}</span>
                      <span className="text-[9px]">({position})</span>
                    </div>
                  );
                })}
              </div>
              <p className="text-[10px] text-muted-foreground">
                {completedModels.length}/{displayModels.length} opening statements
              </p>
            </div>
          )}

          {/* Rebuttal progress during debating phase */}
          {isDebating && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center gap-1.5 flex-wrap">
                {displayModels.map((model) => {
                  const isComplete = completedModels.includes(model);
                  const position = displayPositions[model] || "unknown";
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
                      <span className="text-[9px]">({position})</span>
                    </div>
                  );
                })}
              </div>
              <p className="text-[10px] text-muted-foreground animate-pulse">
                {completedModels.length}/{displayModels.length} rebuttals for round {currentRound}
                ...
              </p>
            </div>
          )}

          {/* Summarizing phase */}
          {isSummarizing && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center gap-1.5">
                <div className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-purple-500/20 text-purple-800 dark:text-purple-400">
                  <FileText className="h-3 w-3" />
                  <span className="max-w-[80px] truncate">
                    {displaySummarizerModel ? getShortModelName(displaySummarizerModel) : ""}
                  </span>
                </div>
              </div>
              <p className="text-[10px] text-muted-foreground animate-pulse">
                Synthesizing debate into balanced summary...
              </p>
            </div>
          )}

          {/* Debate complete */}
          {isDone && (
            <div className="mt-2 space-y-2">
              {/* Position summary */}
              <div className="flex items-center gap-1.5 flex-wrap">
                {displayModels.map((model) => {
                  const position = displayPositions[model] || "unknown";
                  return (
                    <div
                      key={model}
                      className={cn(
                        "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium",
                        position === "pro"
                          ? "bg-emerald-500/20 text-emerald-800 dark:text-emerald-400"
                          : "bg-rose-500/20 text-rose-800 dark:text-rose-400"
                      )}
                    >
                      <span className="max-w-[80px] truncate">{getShortModelName(model)}</span>
                      <span className="text-[9px]">({position})</span>
                    </div>
                  );
                })}
              </div>

              {/* Round progression */}
              <div className="flex items-center gap-1.5 flex-wrap">
                {roundNumbers.map((round) => {
                  const roundTurns = turnsByRound[round] || [];
                  const isOpeningRound = round === 0;
                  return (
                    <div
                      key={round}
                      className={cn(
                        "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium",
                        isOpeningRound
                          ? "bg-blue-500/20 text-blue-700 dark:text-blue-400"
                          : "bg-amber-500/20 text-amber-800 dark:text-amber-400"
                      )}
                    >
                      <span>{isOpeningRound ? "Open" : `R${round}`}</span>
                      <span className="text-[9px]">{roundTurns.length} turns</span>
                    </div>
                  );
                })}
                <div className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-primary/10 text-primary">
                  <FileText className="h-3 w-3" />
                  <span>Summary</span>
                </div>
              </div>

              {/* Usage and expand button */}
              <div className="flex items-center justify-between">
                <UsageSummary
                  totalTokens={totalUsage.totalTokens}
                  totalCost={totalUsage.totalCost}
                />
                {hasTurns && (
                  <button
                    onClick={() => setIsExpanded(!isExpanded)}
                    className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground transition-colors"
                  >
                    {isExpanded ? (
                      <>
                        <ChevronDown className="h-3 w-3" />
                        Hide transcript
                      </>
                    ) : (
                      <>
                        <ChevronRight className="h-3 w-3" />
                        Show transcript
                      </>
                    )}
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Expandable transcript section */}
      {isDone && isExpanded && hasTurns && (
        <div className="border-t border-border/50 px-3 py-2 space-y-4">
          {roundNumbers.map((round) => {
            const roundTurns = turnsByRound[round] || [];
            const isOpeningRound = round === 0;
            return (
              <div key={round}>
                <div className="flex items-center gap-2 mb-2">
                  <p className="text-[10px] font-medium text-muted-foreground">
                    {isOpeningRound ? "Opening Statements" : `Round ${round} Rebuttals`}
                  </p>
                </div>
                <div className="space-y-2">
                  {roundTurns.map((turn, turnIndex) => {
                    const position = displayPositions[turn.model] || turn.position;
                    return (
                      <ResponseCard
                        key={`${round}-${turnIndex}`}
                        title={`${getShortModelName(turn.model)} (${position})`}
                        content={turn.content}
                        usage={turn.usage}
                        variant={position === "pro" ? "blue" : "orange"}
                      />
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
