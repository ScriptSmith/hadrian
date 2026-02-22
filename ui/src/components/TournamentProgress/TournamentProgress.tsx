import { useState } from "react";
import { Trophy, Loader2, Check, Circle, ChevronDown, ChevronRight, Swords } from "lucide-react";

import type { TournamentMatch, CandidateResponse } from "@/stores/streamingStore";
import { useActiveTournamentState } from "@/stores/streamingStore";
import type { TournamentMatchData, MessageUsage } from "@/components/chat-types";
import {
  getShortModelName,
  ResponseCard,
  UsageSummary,
  aggregateUsage,
} from "@/components/ModeProgress";
import { cn } from "@/utils/cn";

interface TournamentProgressProps {
  /** All models participating (for display during generating) */
  allModels?: string[];
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    bracket: string[][];
    matches: TournamentMatchData[];
    winner?: string;
    eliminatedPerRound?: string[][];
    initialResponses?: Array<{
      model: string;
      content: string;
      usage?: MessageUsage;
    }>;
  };
}

/**
 * TournamentProgress - Visual indicator for tournament mode
 *
 * Shows the tournament process:
 * 1. During "generating" phase: Shows progress of parallel model responses
 * 2. During "competing" phase: Shows bracket with match progress
 * 3. During "done" phase: Shows winner with bracket visualization and expandable details
 *
 * Uses the new `useActiveTournamentState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function TournamentProgress({ allModels, persistedMetadata }: TournamentProgressProps) {
  // All hooks must be called before any early returns
  const [isExpanded, setIsExpanded] = useState(false);

  // Use new discriminated union selector for live streaming state
  const liveState = useActiveTournamentState();

  // Determine which state to use: persisted metadata or live streaming state
  const isPersisted = !!persistedMetadata;
  const phase = isPersisted ? "done" : (liveState?.phase ?? "done");
  const bracket = persistedMetadata?.bracket ?? liveState?.bracket ?? [];
  const currentRound = liveState?.currentRound ?? 0;
  const totalRounds = liveState?.totalRounds ?? (bracket.length > 0 ? bracket.length - 1 : 0);
  const matches: TournamentMatch[] = liveState?.matches ?? [];
  const currentMatch = liveState?.currentMatch;
  const initialResponses: CandidateResponse[] =
    persistedMetadata?.initialResponses ?? liveState?.initialResponses ?? [];
  const eliminatedPerRound =
    persistedMetadata?.eliminatedPerRound ?? liveState?.eliminatedPerRound ?? [];
  const winner = persistedMetadata?.winner ?? liveState?.winner;

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isGenerating = phase === "generating";
  const isCompeting = phase === "competing";
  const isDone = phase === "done";

  // Use persisted data if available, falling back to live state
  const displayBracket = bracket;
  const displayMatches: Array<TournamentMatch | TournamentMatchData> =
    persistedMetadata?.matches ?? matches ?? [];
  const displayWinner = winner;
  const displayEliminated = eliminatedPerRound;

  // Determine container styling based on phase
  const containerStyle = isGenerating
    ? "bg-blue-500/10 border-blue-500/30"
    : isCompeting
      ? "bg-amber-500/10 border-amber-500/30"
      : "bg-primary/5 border-primary/30";

  const iconColor = isGenerating
    ? "text-blue-500"
    : isCompeting
      ? "text-amber-500"
      : "text-primary";

  // Get display models - during generating, show all models
  const displayModels = allModels || displayBracket[0] || [];
  const completedModels = initialResponses?.map((r) => r.model) || [];

  // Calculate aggregate usage
  const initialResponseUsage = aggregateUsage(initialResponses || []);
  const matchUsage = displayMatches.reduce(
    (acc, m) => {
      const usage1 = "usage1" in m ? m.usage1 : undefined;
      const usage2 = "usage2" in m ? m.usage2 : undefined;
      const judgeUsage = m.judgeUsage;
      return {
        totalTokens:
          acc.totalTokens +
          (usage1?.totalTokens ?? 0) +
          (usage2?.totalTokens ?? 0) +
          (judgeUsage?.totalTokens ?? 0),
        totalCost:
          acc.totalCost + (usage1?.cost ?? 0) + (usage2?.cost ?? 0) + (judgeUsage?.cost ?? 0),
      };
    },
    { totalTokens: 0, totalCost: 0 }
  );

  const totalUsage = {
    totalTokens: initialResponseUsage.totalTokens + matchUsage.totalTokens,
    totalCost: (initialResponseUsage.cost ?? 0) + matchUsage.totalCost,
  };

  const hasMatches = displayMatches.length > 0;

  // Get current match info for display
  const currentMatchInfo = currentMatch
    ? displayMatches.find((m) => m.id === currentMatch)
    : undefined;

  return (
    <div className={cn("rounded-lg border", containerStyle)}>
      {/* Header section */}
      <div className="flex items-start gap-2 px-3 py-2">
        <div className="shrink-0 mt-0.5">
          {isGenerating || isCompeting ? (
            <Loader2 className={cn("h-4 w-4 animate-spin", iconColor)} />
          ) : (
            <Trophy className={cn("h-4 w-4", iconColor)} />
          )}
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 text-xs font-medium">
            <Trophy className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-muted-foreground">Tournament</span>
            <span
              className={cn(
                "px-1.5 py-0.5 rounded text-[9px] font-semibold",
                isGenerating && "bg-blue-500/20 text-blue-700 dark:text-blue-400",
                isCompeting && "bg-amber-500/20 text-amber-800 dark:text-amber-400",
                isDone && "bg-primary/10 text-primary"
              )}
            >
              {isGenerating
                ? "GENERATING"
                : isCompeting
                  ? `ROUND ${currentRound + 1}/${totalRounds}`
                  : "COMPLETE"}
            </span>
          </div>

          {/* Model progress during generating phase */}
          {isGenerating && (
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
                {completedModels.length}/{displayModels.length} responses ready
              </p>
            </div>
          )}

          {/* Current match during competing phase */}
          {isCompeting && currentMatchInfo && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center gap-2">
                <Swords className="h-3.5 w-3.5 text-amber-500" />
                <span className="text-xs font-medium">Current Match:</span>
              </div>
              <div className="flex items-center gap-2">
                <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-blue-500/20 text-blue-700 dark:text-blue-400">
                  {getShortModelName(currentMatchInfo.competitor1)}
                </span>
                <span className="text-[10px] text-muted-foreground">vs</span>
                <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-orange-500/20 text-orange-800 dark:text-orange-400">
                  {getShortModelName(currentMatchInfo.competitor2)}
                </span>
                <span className="text-[10px] text-muted-foreground animate-pulse">
                  {"status" in currentMatchInfo && currentMatchInfo.status === "judging"
                    ? "Judging..."
                    : "In progress..."}
                </span>
              </div>
              {displayMatches.length > 0 && (
                <p className="text-[10px] text-muted-foreground">
                  Match{" "}
                  {displayMatches.filter(
                    (m) => m.round === currentRound && "status" in m && m.status === "complete"
                  ).length + 1}
                  {" / "}
                  {Math.ceil(displayBracket[currentRound]?.length / 2) || "?"}
                </p>
              )}
            </div>
          )}

          {/* Bracket overview during competing phase */}
          {isCompeting && !currentMatchInfo && (
            <div className="mt-2 space-y-1">
              <p className="text-[10px] text-muted-foreground animate-pulse">
                Processing round {currentRound + 1}...
              </p>
            </div>
          )}

          {/* Winner info with bracket visualization */}
          {isDone && (
            <div className="mt-2 space-y-2">
              {/* Winner announcement */}
              <div className="flex items-center gap-2">
                <Trophy className="h-4 w-4 text-yellow-500" />
                <span className="text-xs font-medium">Champion:</span>
                <span className="px-1.5 py-0.5 rounded text-[10px] font-semibold bg-primary/10 text-primary">
                  {displayWinner ? getShortModelName(displayWinner) : "N/A"}
                </span>
              </div>

              {/* Mini bracket visualization */}
              {displayBracket.length > 1 && (
                <div className="flex items-center gap-1 flex-wrap">
                  {displayBracket.map((round, roundIndex) => (
                    <div key={roundIndex} className="flex items-center gap-1">
                      {roundIndex > 0 && (
                        <span className="text-muted-foreground/40 text-xs">→</span>
                      )}
                      <div className="flex items-center gap-0.5">
                        {round.map((model) => (
                          <span
                            key={model + roundIndex}
                            className={cn(
                              "px-1 py-0.5 rounded text-[9px] font-medium",
                              model === displayWinner
                                ? "bg-primary/10 text-primary"
                                : roundIndex === displayBracket.length - 1
                                  ? "bg-muted text-muted-foreground"
                                  : "bg-muted/50 text-muted-foreground"
                            )}
                          >
                            {getShortModelName(model)}
                          </span>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              )}

              {/* Usage and expand button */}
              <div className="flex items-center justify-between">
                <UsageSummary
                  totalTokens={totalUsage.totalTokens}
                  totalCost={totalUsage.totalCost}
                />
                {hasMatches && (
                  <button
                    onClick={() => setIsExpanded(!isExpanded)}
                    className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground transition-colors"
                  >
                    {isExpanded ? (
                      <>
                        <ChevronDown className="h-3 w-3" />
                        Hide details
                      </>
                    ) : (
                      <>
                        <ChevronRight className="h-3 w-3" />
                        Show details
                      </>
                    )}
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Expandable match details section */}
      {isDone && isExpanded && hasMatches && (
        <div className="border-t border-border/50 px-3 py-2 space-y-4">
          {/* Match results by round */}
          {displayBracket.slice(0, -1).map((_, roundIndex) => {
            const roundMatches = displayMatches.filter((m) => m.round === roundIndex);
            if (roundMatches.length === 0) return null;

            return (
              <div key={roundIndex}>
                <p className="text-[10px] font-medium text-muted-foreground mb-2">
                  Round {roundIndex + 1} -{" "}
                  {roundIndex === 0
                    ? "First Round"
                    : roundIndex === totalRounds - 1
                      ? "Final"
                      : `Round ${roundIndex + 1}`}
                </p>
                <div className="space-y-2">
                  {roundMatches.map((match) => {
                    const matchWinner = match.winner;
                    const response1 = "response1" in match ? (match.response1 ?? "") : "";
                    const response2 = "response2" in match ? (match.response2 ?? "") : "";

                    return (
                      <div
                        key={match.id}
                        className="rounded border border-border/50 bg-background/50 overflow-hidden"
                      >
                        {/* Match header */}
                        <div className="flex items-center justify-between px-2 py-1.5 bg-muted/30 border-b border-border/30">
                          <div className="flex items-center gap-2">
                            <Swords className="h-3 w-3 text-muted-foreground" />
                            <span
                              className={cn(
                                "px-1.5 py-0.5 rounded text-[10px] font-medium",
                                match.competitor1 === matchWinner
                                  ? "bg-primary/10 text-primary"
                                  : "bg-muted text-muted-foreground"
                              )}
                            >
                              {getShortModelName(match.competitor1)}
                              {match.competitor1 === matchWinner && " (W)"}
                            </span>
                            <span className="text-[10px] text-muted-foreground">vs</span>
                            <span
                              className={cn(
                                "px-1.5 py-0.5 rounded text-[10px] font-medium",
                                match.competitor2 === matchWinner
                                  ? "bg-primary/10 text-primary"
                                  : "bg-muted text-muted-foreground"
                              )}
                            >
                              {getShortModelName(match.competitor2)}
                              {match.competitor2 === matchWinner && " (W)"}
                            </span>
                          </div>
                          {match.judge && (
                            <span className="text-[9px] text-muted-foreground">
                              Judge: {getShortModelName(match.judge)}
                            </span>
                          )}
                        </div>

                        {/* Response comparisons */}
                        <div className="grid grid-cols-2 gap-2 p-2">
                          <ResponseCard
                            title={getShortModelName(match.competitor1)}
                            content={response1}
                            usage={"usage1" in match ? match.usage1 : undefined}
                            variant={match.competitor1 === matchWinner ? "blue" : "default"}
                            previewLength={150}
                          />
                          <ResponseCard
                            title={getShortModelName(match.competitor2)}
                            content={response2}
                            usage={"usage2" in match ? match.usage2 : undefined}
                            variant={match.competitor2 === matchWinner ? "blue" : "default"}
                            previewLength={150}
                          />
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            );
          })}

          {/* Eliminated models summary */}
          {displayEliminated.length > 0 && (
            <div>
              <p className="text-[10px] font-medium text-muted-foreground mb-2">
                Elimination Summary
              </p>
              <div className="flex items-center gap-2 flex-wrap">
                {displayEliminated.map((eliminated, roundIndex) => (
                  <div key={roundIndex} className="flex items-center gap-1">
                    <span className="text-[9px] text-muted-foreground">R{roundIndex + 1}:</span>
                    {eliminated.map((model) => (
                      <span
                        key={model}
                        className="px-1 py-0.5 rounded text-[9px] bg-red-500/10 text-red-700 dark:text-red-400 line-through"
                      >
                        {getShortModelName(model)}
                      </span>
                    ))}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
