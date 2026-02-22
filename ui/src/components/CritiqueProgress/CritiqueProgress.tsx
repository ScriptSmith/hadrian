import { MessageSquareWarning } from "lucide-react";

import type { CritiqueData } from "@/stores/streamingStore";
import { useActiveCritiquedState } from "@/stores/streamingStore";
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
  type ProgressPhase,
} from "@/components/ModeProgress";

interface CritiqueProgressProps {
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    primaryModel: string;
    initialResponse?: string;
    initialUsage?: MessageUsage;
    critiques: Array<{
      model: string;
      content: string;
      usage?: MessageUsage;
    }>;
  };
}

/**
 * CritiqueProgress - Visual indicator for critiqued mode
 *
 * Shows the critique process:
 * 1. During "initial" phase: Primary model is generating initial response
 * 2. During "critiquing" phase: Shows progress of critic models providing feedback
 * 3. During "revising" phase: Shows the primary model revising based on critiques
 * 4. During "done" phase: Shows completion with expandable initial response and critiques
 *
 * Uses the new `useActiveCritiquedState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function CritiqueProgress({ persistedMetadata }: CritiqueProgressProps) {
  // Use new discriminated union selector for live streaming state
  const liveState = useActiveCritiquedState();

  // Determine which state to use: persisted metadata or live streaming state
  const isPersisted = !!persistedMetadata;
  const phase = isPersisted ? "done" : (liveState?.phase ?? "done");
  const primaryModel = persistedMetadata?.primaryModel ?? liveState?.primaryModel ?? "";
  const critiqueModels = liveState?.critiqueModels ?? [];
  const initialResponse = persistedMetadata?.initialResponse ?? liveState?.initialResponse;
  const initialUsage = persistedMetadata?.initialUsage ?? liveState?.initialUsage;
  const critiques: CritiqueData[] = persistedMetadata?.critiques ?? liveState?.critiques ?? [];
  const completedCritiques = liveState?.completedCritiques ?? critiques.length;

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isInitial = phase === "initial";
  const isCritiquing = phase === "critiquing";
  const isRevising = phase === "revising";
  const isDone = phase === "done";

  // Map internal phases to ProgressPhase
  // CritiqueProgress has 4 phases: initial (blue), critiquing (orange), revising (amber), done (green)
  // ProgressContainer only supports: initial (blue), active (amber), complete (green), warning (orange)
  // So we map: initial -> initial, critiquing -> warning (orange), revising -> active (amber), done -> complete
  const progressPhase: ProgressPhase = isInitial
    ? "initial"
    : isCritiquing
      ? "warning"
      : isRevising
        ? "active"
        : "complete";

  // Use persisted data if provided, otherwise use live state
  const displayInitialResponse = initialResponse;
  const displayInitialUsage = initialUsage;
  const displayCritiques = critiques;
  const hasContent = !!displayInitialResponse || displayCritiques.length > 0;

  // Calculate aggregate usage
  const initialTokens = displayInitialUsage?.totalTokens ?? 0;
  const critiqueTokens = displayCritiques.reduce((sum, c) => sum + (c.usage?.totalTokens ?? 0), 0);
  const totalTokens = initialTokens + critiqueTokens;
  const initialCost = displayInitialUsage?.cost ?? 0;
  const critiqueCost = displayCritiques.reduce((sum, c) => sum + (c.usage?.cost ?? 0), 0);
  const totalCost = initialCost + critiqueCost;

  // Build status badge text and variant
  const statusText = isInitial
    ? "INITIAL"
    : isCritiquing
      ? "CRITIQUING"
      : isRevising
        ? "REVISING"
        : "COMPLETE";

  return (
    <div className="mb-3">
      <ProgressContainer
        phase={progressPhase}
        isLoading={isInitial || isCritiquing || isRevising}
        icon={MessageSquareWarning}
        header={
          <ModeHeader
            name="Critiqued"
            badge={<StatusBadge text={statusText} variant={progressPhase} />}
          />
        }
        expandableSection={
          hasContent ? (
            <>
              {/* Initial response */}
              {displayInitialResponse && (
                <ResponseCard
                  title={`Initial: ${getShortModelName(primaryModel)}`}
                  content={displayInitialResponse}
                  usage={displayInitialUsage}
                  variant="blue"
                />
              )}
              {/* Critiques */}
              {displayCritiques.map((critique, index) => (
                <ResponseCard
                  key={critique.model + index}
                  title={`Critique: ${getShortModelName(critique.model)}`}
                  content={critique.content}
                  usage={critique.usage}
                  variant="orange"
                />
              ))}
            </>
          ) : undefined
        }
        expandLabel={{ collapsed: "Show details", expanded: "Hide details" }}
        showExpandable={hasContent}
        renderFooter={
          isDone
            ? ({ isExpanded, toggleExpand, hasExpandable }) => (
                <div className="mt-1 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <p className="text-[10px] text-muted-foreground">
                      Revised using {displayCritiques.length} critique
                      {displayCritiques.length !== 1 ? "s" : ""}
                    </p>
                    {hasContent && <UsageSummary totalTokens={totalTokens} totalCost={totalCost} />}
                  </div>
                  {hasExpandable && (
                    <ExpandButton
                      isExpanded={isExpanded}
                      onToggle={toggleExpand}
                      expandedLabel="Hide details"
                      collapsedLabel="Show details"
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
              <span className="text-xs text-muted-foreground">Primary model:</span>
              <ModelBadge model={primaryModel} variant="blue" />
            </div>
            <p className="text-[10px] text-muted-foreground animate-pulse">
              Generating initial response...
            </p>
          </div>
        )}

        {/* Progress during critiquing phase */}
        {isCritiquing && (
          <div className="mt-2 space-y-1">
            <div className="flex items-center gap-1.5 flex-wrap">
              {critiqueModels.map((model) => {
                const isComplete = critiques.some((c) => c.model === model);
                return (
                  <ModelBadge
                    key={model}
                    model={model}
                    variant={isComplete ? "primary" : "default"}
                    showCheck={isComplete}
                    showLoading={!isComplete}
                  />
                );
              })}
            </div>
            <p className="text-[10px] text-muted-foreground">
              {completedCritiques}/{critiqueModels.length} critiques received
            </p>
          </div>
        )}

        {/* Progress during revising phase */}
        {isRevising && (
          <div className="mt-2 space-y-1">
            <div className="flex items-center gap-1.5">
              <span className="text-xs text-muted-foreground">Revising with:</span>
              <ModelBadge model={primaryModel} variant="amber" />
            </div>
            <p className="text-[10px] text-muted-foreground animate-pulse">
              Incorporating {displayCritiques.length} critique
              {displayCritiques.length !== 1 ? "s" : ""}...
            </p>
          </div>
        )}
      </ProgressContainer>
    </div>
  );
}
