import { GraduationCap, Loader2, BookOpen, User, Baby } from "lucide-react";

import type { ExplanationLevel } from "@/stores/streamingStore";
import { useActiveExplainerState } from "@/stores/streamingStore";
import type { MessageUsage, ExplanationData } from "@/components/chat-types";
import {
  ProgressContainer,
  ModeHeader,
  StatusBadge,
  ExpandButton,
  getShortModelName,
  ResponseCard,
  UsageSummary,
  aggregateUsage,
  type ProgressPhase,
} from "@/components/ModeProgress";
import { cn } from "@/utils/cn";

interface ExplainerProgressProps {
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    explanations: ExplanationData[];
    levels: string[];
    aggregateUsage?: MessageUsage;
  };
}

/**
 * Get an icon for an audience level
 */
function getLevelIcon(level: string) {
  const lowerLevel = level.toLowerCase();
  if (lowerLevel === "expert" || lowerLevel === "advanced") return BookOpen;
  if (lowerLevel === "child" || lowerLevel === "kids") return Baby;
  return User;
}

/**
 * Get color variant for an audience level based on its position in the list
 */
function getLevelColor(_level: string, index: number, total: number) {
  // Create a gradient from complex (blue) to simple (green)
  const position = total > 1 ? index / (total - 1) : 0;

  if (position <= 0.33) {
    return {
      bg: "bg-blue-500/20",
      text: "text-blue-700 dark:text-blue-400",
      border: "border-blue-500/30",
    };
  } else if (position <= 0.66) {
    return {
      bg: "bg-amber-500/20",
      text: "text-amber-800 dark:text-amber-400",
      border: "border-amber-500/30",
    };
  } else {
    return {
      bg: "bg-emerald-500/20",
      text: "text-emerald-800 dark:text-emerald-400",
      border: "border-emerald-500/30",
    };
  }
}

/**
 * ExplainerProgress - Visual indicator for explainer mode
 *
 * Shows the progressive explanation process:
 * 1. During "initial" phase: Shows first explanation being generated
 * 2. During "simplifying" phase: Shows adaptations for other audience levels
 * 3. During "done" phase: Shows all explanations with expandable details
 *
 * Uses the new `useActiveExplainerState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function ExplainerProgress({ persistedMetadata }: ExplainerProgressProps) {
  // Use new discriminated union selector for live streaming state
  const liveState = useActiveExplainerState();

  // Determine which state to use: persisted metadata or live streaming state
  const phase = persistedMetadata ? "done" : (liveState?.phase ?? "done");
  const audienceLevels = liveState?.audienceLevels ?? [];
  const currentLevelIndex = liveState?.currentLevelIndex ?? 0;
  const explanations = liveState?.explanations ?? [];
  const currentModel = liveState?.currentModel;

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isInitial = phase === "initial";
  const isSimplifying = phase === "simplifying";
  const isDone = phase === "done";

  // Use persisted data if available
  const displayExplanations: Array<ExplanationLevel | ExplanationData> =
    persistedMetadata?.explanations ?? explanations ?? [];
  const displayLevels = persistedMetadata?.levels ?? audienceLevels ?? [];

  // Calculate usage
  const usageItems = displayExplanations.map((e) => ({ usage: e.usage }));
  const computedUsage = aggregateUsage(usageItems);
  const totalUsage = persistedMetadata?.aggregateUsage
    ? {
        totalTokens: persistedMetadata.aggregateUsage.totalTokens,
        totalCost: persistedMetadata.aggregateUsage.cost ?? 0,
      }
    : { totalTokens: computedUsage.totalTokens, totalCost: computedUsage.cost ?? 0 };

  // Map internal phase to ProgressPhase
  const progressPhase: ProgressPhase = isInitial
    ? "initial"
    : isSimplifying
      ? "active"
      : "complete";

  // Get status badge text
  const statusText = isInitial
    ? "EXPLAINING"
    : isSimplifying
      ? `ADAPTING ${currentLevelIndex + 1}/${displayLevels.length}`
      : `${displayLevels.length} LEVELS`;

  const hasExplanations = displayExplanations.length > 0;

  // Build expandable section content
  const expandableSection = hasExplanations ? (
    <>
      <div className="text-[10px] font-medium text-muted-foreground mb-2">
        Explanations by Audience Level ({displayExplanations.length})
      </div>
      {displayExplanations.map((explanation, index) => {
        const colors = getLevelColor(
          explanation.level,
          displayLevels.indexOf(explanation.level),
          displayLevels.length
        );
        return (
          <ResponseCard
            key={`${explanation.level}-${index}`}
            title={`${explanation.level.charAt(0).toUpperCase() + explanation.level.slice(1)} (${getShortModelName(explanation.model)})`}
            content={explanation.content}
            usage={explanation.usage}
            variant={
              colors.bg.includes("blue")
                ? "blue"
                : colors.bg.includes("orange")
                  ? "orange"
                  : "default"
            }
            previewLength={300}
          />
        );
      })}
    </>
  ) : undefined;

  return (
    <ProgressContainer
      phase={progressPhase}
      isLoading={isInitial || isSimplifying}
      icon={GraduationCap}
      header={
        <ModeHeader
          name="Explainer"
          badge={<StatusBadge text={statusText} variant={progressPhase} />}
        />
      }
      expandableSection={expandableSection}
      expandLabel={{ collapsed: "Show explanations", expanded: "Hide explanations" }}
      showExpandable={hasExplanations}
      renderFooter={({ isExpanded, toggleExpand, hasExpandable }) => (
        <div className="mt-1 flex items-center justify-between">
          <div className="flex items-center gap-2">
            {isDone && totalUsage.totalTokens > 0 && (
              <UsageSummary
                totalTokens={totalUsage.totalTokens}
                totalCost={totalUsage.totalCost}
                label="Total"
              />
            )}
          </div>
          {hasExpandable && (
            <ExpandButton
              isExpanded={isExpanded}
              onToggle={toggleExpand}
              collapsedLabel="Show explanations"
              expandedLabel="Hide explanations"
            />
          )}
        </div>
      )}
    >
      {/* Current model during active phases */}
      {(isInitial || isSimplifying) && currentModel && (
        <div className="mt-2 flex items-center gap-1.5 text-[10px] text-muted-foreground">
          <span>Model:</span>
          <span className="px-1.5 py-0.5 rounded bg-primary/10 text-primary font-medium">
            {getShortModelName(currentModel)}
          </span>
          {isSimplifying && displayLevels[currentLevelIndex] && (
            <>
              <span className="mx-1">for</span>
              <span
                className={cn(
                  "px-1.5 py-0.5 rounded font-medium",
                  getLevelColor(
                    displayLevels[currentLevelIndex],
                    currentLevelIndex,
                    displayLevels.length
                  ).bg,
                  getLevelColor(
                    displayLevels[currentLevelIndex],
                    currentLevelIndex,
                    displayLevels.length
                  ).text
                )}
              >
                {displayLevels[currentLevelIndex]}
              </span>
            </>
          )}
        </div>
      )}

      {/* Audience levels preview */}
      <div className="mt-2 flex flex-wrap items-center gap-1.5">
        <span className="text-[10px] text-muted-foreground">Audience levels:</span>
        {displayLevels.map((level, index) => {
          const completed = displayExplanations.some((e) => e.level === level);
          const isCurrent = !isDone && index === currentLevelIndex;
          const colors = getLevelColor(level, index, displayLevels.length);
          const LevelIcon = getLevelIcon(level);

          return (
            <span
              key={level}
              className={cn(
                "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors",
                completed || isCurrent ? colors.bg : "bg-muted",
                completed || isCurrent ? colors.text : "text-muted-foreground",
                isCurrent && "ring-1 ring-offset-1 ring-offset-background",
                isCurrent && colors.border.replace("border-", "ring-")
              )}
            >
              <LevelIcon className="h-3 w-3" />
              {level}
              {isCurrent && <Loader2 className="h-2.5 w-2.5 animate-spin ml-0.5" />}
            </span>
          );
        })}
      </div>
    </ProgressContainer>
  );
}
