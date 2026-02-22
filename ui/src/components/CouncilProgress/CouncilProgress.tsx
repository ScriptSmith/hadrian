import { useState } from "react";
import {
  Users,
  Loader2,
  Check,
  Circle,
  ChevronDown,
  ChevronRight,
  FileText,
  Wand2,
} from "lucide-react";

import { useActiveCouncilState } from "@/stores/streamingStore";
import type { MessageUsage, CouncilStatementData } from "@/components/chat-types";
import {
  getShortModelName,
  ResponseCard,
  UsageSummary,
  aggregateUsage,
} from "@/components/ModeProgress";
import { cn } from "@/utils/cn";

interface CouncilProgressProps {
  /** All models participating (for display) */
  allModels?: string[];
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    statements: CouncilStatementData[];
    roles: Record<string, string>;
    councilRounds?: number;
    synthesizerModel?: string;
    aggregateUsage?: MessageUsage;
  };
}

/**
 * CouncilProgress - Visual indicator for council mode
 *
 * Shows the council discussion process:
 * 1. During "assigning" phase: Shows role assignment in progress
 * 2. During "opening" phase: Shows progress of opening perspectives
 * 3. During "discussing" phase: Shows progress of discussion rounds
 * 4. During "synthesizing" phase: Shows synthesis in progress
 * 5. During "done" phase: Shows synthesis with expandable discussion transcript
 *
 * Uses the new `useActiveCouncilState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function CouncilProgress({ allModels, persistedMetadata }: CouncilProgressProps) {
  // All hooks must be called before any early returns
  const [isExpanded, setIsExpanded] = useState(false);

  // Use new discriminated union selector for live streaming state
  const liveState = useActiveCouncilState();

  // Determine which state to use: persisted metadata or live streaming state
  const isPersisted = !!persistedMetadata;
  const phase = isPersisted ? "done" : (liveState?.phase ?? "done");
  const currentRound = liveState?.currentRound ?? 0;
  const totalRounds = liveState?.totalRounds ?? 0;
  const roles = liveState?.roles ?? {};
  const statements = liveState?.statements ?? [];
  const currentRoundStatements = liveState?.currentRoundStatements ?? [];
  const synthesizerModel = persistedMetadata?.synthesizerModel ?? liveState?.synthesizerModel;

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isAssigning = phase === "assigning";
  const isOpening = phase === "opening";
  const isDiscussing = phase === "discussing";
  const isSynthesizing = phase === "synthesizing";
  const isDone = phase === "done";

  // Use persisted data if available
  const displayStatements = persistedMetadata?.statements ?? statements ?? [];
  const displayRoles = persistedMetadata?.roles ?? roles ?? {};
  const displaySynthesizerModel = synthesizerModel;

  // Get council members (all models except synthesizer) from roles or props
  const displayModels = allModels
    ? allModels.filter((m) => m !== displaySynthesizerModel)
    : Object.keys(displayRoles);
  const completedModels = currentRoundStatements.map((s) => s.model);

  // Determine container styling based on phase
  const containerStyle = isAssigning
    ? "bg-cyan-500/10 border-cyan-500/30"
    : isOpening
      ? "bg-blue-500/10 border-blue-500/30"
      : isDiscussing
        ? "bg-amber-500/10 border-amber-500/30"
        : isSynthesizing
          ? "bg-purple-500/10 border-purple-500/30"
          : "bg-primary/5 border-primary/30";

  const iconColor = isAssigning
    ? "text-cyan-500"
    : isOpening
      ? "text-blue-500"
      : isDiscussing
        ? "text-amber-500"
        : isSynthesizing
          ? "text-purple-500"
          : "text-primary";

  // Calculate aggregate usage
  const computedUsage = aggregateUsage(
    displayStatements.map((s) => ({
      model: s.model,
      content: s.content,
      usage: s.usage,
    }))
  );
  const totalUsage = persistedMetadata?.aggregateUsage
    ? {
        totalTokens: persistedMetadata.aggregateUsage.totalTokens,
        totalCost: persistedMetadata.aggregateUsage.cost ?? 0,
      }
    : { totalTokens: computedUsage.totalTokens, totalCost: computedUsage.cost ?? 0 };

  const hasStatements = displayStatements.length > 0;

  // Group statements by round for display
  const statementsByRound: Record<number, CouncilStatementData[]> = {};
  displayStatements.forEach((statement) => {
    if (!statementsByRound[statement.round]) {
      statementsByRound[statement.round] = [];
    }
    statementsByRound[statement.round].push(statement);
  });
  const roundNumbers = Object.keys(statementsByRound)
    .map(Number)
    .sort((a, b) => a - b);

  // Role colors for visual distinction
  const getRoleColor = (index: number): string => {
    const colors = [
      "bg-blue-500/20 text-blue-700 dark:text-blue-400",
      "bg-emerald-500/20 text-emerald-800 dark:text-emerald-400",
      "bg-purple-500/20 text-purple-800 dark:text-purple-400",
      "bg-orange-500/20 text-orange-800 dark:text-orange-400",
      "bg-rose-500/20 text-rose-800 dark:text-rose-400",
      "bg-cyan-500/20 text-cyan-600 dark:text-cyan-400",
      "bg-amber-500/20 text-amber-800 dark:text-amber-400",
      "bg-indigo-500/20 text-indigo-600 dark:text-indigo-400",
    ];
    return colors[index % colors.length];
  };

  // Create a consistent model-to-color mapping
  const modelColorMap = new Map<string, string>();
  displayModels.forEach((model, index) => {
    modelColorMap.set(model, getRoleColor(index));
  });

  return (
    <div className={cn("rounded-lg border", containerStyle)}>
      {/* Header section */}
      <div className="flex items-start gap-2 px-3 py-2">
        <div className="shrink-0 mt-0.5">
          {isAssigning || isOpening || isDiscussing || isSynthesizing ? (
            <Loader2 className={cn("h-4 w-4 animate-spin", iconColor)} />
          ) : (
            <Users className={cn("h-4 w-4", iconColor)} />
          )}
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 text-xs font-medium">
            <Users className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-muted-foreground">Council</span>
            <span
              className={cn(
                "px-1.5 py-0.5 rounded text-[9px] font-semibold",
                isAssigning && "bg-cyan-500/20 text-cyan-600 dark:text-cyan-400",
                isOpening && "bg-blue-500/20 text-blue-700 dark:text-blue-400",
                isDiscussing && "bg-amber-500/20 text-amber-800 dark:text-amber-400",
                isSynthesizing && "bg-purple-500/20 text-purple-800 dark:text-purple-400",
                isDone && "bg-primary/10 text-primary"
              )}
            >
              {isAssigning
                ? "ASSIGNING ROLES"
                : isOpening
                  ? "OPENING"
                  : isDiscussing
                    ? `ROUND ${currentRound}/${totalRounds}`
                    : isSynthesizing
                      ? "SYNTHESIZING"
                      : "COMPLETE"}
            </span>
          </div>

          {/* Role assignment in progress */}
          {isAssigning && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center gap-1.5">
                <div className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-cyan-500/20 text-cyan-600 dark:text-cyan-400">
                  <Wand2 className="h-3 w-3" />
                  <span className="max-w-[80px] truncate">
                    {displaySynthesizerModel ? getShortModelName(displaySynthesizerModel) : ""}
                  </span>
                </div>
              </div>
              <p className="text-[10px] text-muted-foreground animate-pulse">
                Analyzing question and assigning roles to council members...
              </p>
            </div>
          )}

          {/* Model roles and progress during opening phase */}
          {isOpening && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center gap-1.5 flex-wrap">
                {displayModels.map((model) => {
                  const isComplete = completedModels.includes(model);
                  const role = displayRoles[model] || "Member";
                  const colorClass = modelColorMap.get(model) || getRoleColor(0);
                  return (
                    <div
                      key={model}
                      className={cn(
                        "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors",
                        isComplete ? colorClass : "bg-muted text-muted-foreground"
                      )}
                    >
                      {isComplete ? (
                        <Check className="h-3 w-3" />
                      ) : (
                        <Circle className="h-3 w-3 animate-pulse" />
                      )}
                      <span className="max-w-[60px] truncate">{getShortModelName(model)}</span>
                      <span className="text-[9px] max-w-[50px] truncate">({role})</span>
                    </div>
                  );
                })}
              </div>
              <p className="text-[10px] text-muted-foreground">
                {completedModels.length}/{displayModels.length} opening perspectives
              </p>
            </div>
          )}

          {/* Discussion progress during discussing phase */}
          {isDiscussing && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center gap-1.5 flex-wrap">
                {displayModels.map((model) => {
                  const isComplete = completedModels.includes(model);
                  const role = displayRoles[model] || "Member";
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
                      <span className="max-w-[60px] truncate">{getShortModelName(model)}</span>
                      <span className="text-[9px] max-w-[50px] truncate">({role})</span>
                    </div>
                  );
                })}
              </div>
              <p className="text-[10px] text-muted-foreground animate-pulse">
                {completedModels.length}/{displayModels.length} responses for round {currentRound}
                ...
              </p>
            </div>
          )}

          {/* Synthesizing phase */}
          {isSynthesizing && (
            <div className="mt-2 space-y-1">
              <div className="flex items-center gap-1.5">
                <div className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-purple-500/20 text-purple-800 dark:text-purple-400">
                  <FileText className="h-3 w-3" />
                  <span className="max-w-[80px] truncate">
                    {displaySynthesizerModel ? getShortModelName(displaySynthesizerModel) : ""}
                  </span>
                </div>
              </div>
              <p className="text-[10px] text-muted-foreground animate-pulse">
                Synthesizing council discussion into comprehensive response...
              </p>
            </div>
          )}

          {/* Council complete */}
          {isDone && (
            <div className="mt-2 space-y-2">
              {/* Role summary */}
              <div className="flex items-center gap-1.5 flex-wrap">
                {displayModels.map((model) => {
                  const role = displayRoles[model] || "Member";
                  const colorClass = modelColorMap.get(model) || getRoleColor(0);
                  return (
                    <div
                      key={model}
                      className={cn(
                        "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium",
                        colorClass
                      )}
                    >
                      <span className="max-w-[60px] truncate">{getShortModelName(model)}</span>
                      <span className="text-[9px] max-w-[50px] truncate">({role})</span>
                    </div>
                  );
                })}
              </div>

              {/* Round progression */}
              <div className="flex items-center gap-1.5 flex-wrap">
                {roundNumbers.map((round) => {
                  const roundStatements = statementsByRound[round] || [];
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
                      <span className="text-[9px]">{roundStatements.length} views</span>
                    </div>
                  );
                })}
                <div className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-primary/10 text-primary">
                  <FileText className="h-3 w-3" />
                  <span>Synthesis</span>
                </div>
              </div>

              {/* Usage and expand button */}
              <div className="flex items-center justify-between">
                <UsageSummary
                  totalTokens={totalUsage.totalTokens}
                  totalCost={totalUsage.totalCost}
                />
                {hasStatements && (
                  <button
                    onClick={() => setIsExpanded(!isExpanded)}
                    className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground transition-colors"
                  >
                    {isExpanded ? (
                      <>
                        <ChevronDown className="h-3 w-3" />
                        Hide discussion
                      </>
                    ) : (
                      <>
                        <ChevronRight className="h-3 w-3" />
                        Show discussion
                      </>
                    )}
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Expandable discussion transcript section */}
      {isDone && isExpanded && hasStatements && (
        <div className="border-t border-border/50 px-3 py-2 space-y-4">
          {roundNumbers.map((round) => {
            const roundStatements = statementsByRound[round] || [];
            const isOpeningRound = round === 0;
            return (
              <div key={round}>
                <div className="flex items-center gap-2 mb-2">
                  <p className="text-[10px] font-medium text-muted-foreground">
                    {isOpeningRound ? "Opening Perspectives" : `Discussion Round ${round}`}
                  </p>
                </div>
                <div className="space-y-2">
                  {roundStatements.map((statement, statementIndex) => {
                    const role = displayRoles[statement.model] || statement.role;
                    const modelIndex = displayModels.indexOf(statement.model);
                    const variant = modelIndex % 2 === 0 ? "blue" : "orange";
                    return (
                      <ResponseCard
                        key={`${round}-${statementIndex}`}
                        title={`${getShortModelName(statement.model)} (${role})`}
                        content={statement.content}
                        usage={statement.usage}
                        variant={variant}
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
