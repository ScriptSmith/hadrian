import { Combine } from "lucide-react";

import type { SourceResponse } from "@/stores/streamingStore";
import { useActiveSynthesizedState } from "@/stores/streamingStore";
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

interface SynthesisProgressProps {
  /** All models participating (for display during gathering) */
  allModels?: string[];
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    synthesizerModel: string;
    completedModels: string[];
    sourceResponses?: Array<{
      model: string;
      content: string;
      usage?: MessageUsage;
    }>;
  };
}

/**
 * SynthesisProgress - Visual indicator for synthesized mode
 *
 * Shows the synthesis process:
 * 1. During "gathering" phase: Shows progress of parallel model responses
 * 2. During "synthesizing" phase: Shows the synthesizer combining responses
 * 3. During "done" phase: Shows completion status with expandable source responses
 *
 * Uses the new `useActiveSynthesizedState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function SynthesisProgress({ allModels, persistedMetadata }: SynthesisProgressProps) {
  // Use new discriminated union selector for live streaming state
  const liveState = useActiveSynthesizedState();

  // Determine which state to use: persisted metadata or live streaming state
  // Persisted metadata indicates this is a historical message display
  const isPersisted = !!persistedMetadata;
  const phase = isPersisted ? "done" : (liveState?.phase ?? "done");
  const synthesizerModel = persistedMetadata?.synthesizerModel ?? liveState?.synthesizerModel ?? "";
  const completedModels = persistedMetadata?.completedModels ?? liveState?.completedModels ?? [];
  const totalModels = liveState?.totalModels ?? completedModels.length;
  const sourceResponses: SourceResponse[] =
    persistedMetadata?.sourceResponses ?? liveState?.sourceResponses ?? [];

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isGathering = phase === "gathering";
  const isSynthesizing = phase === "synthesizing";
  const isDone = phase === "done";

  // Map internal phase to ProgressPhase
  const progressPhase: ProgressPhase = isGathering
    ? "initial"
    : isSynthesizing
      ? "active"
      : "complete";

  // Get display models - during gathering, show all models; after, show completed
  const displayModels = allModels || completedModels;

  // Calculate usage for footer
  const hasResponses = sourceResponses.length > 0;
  const { totalTokens, cost: totalCost } = aggregateUsage(sourceResponses);

  return (
    <div className="mb-3">
      <ProgressContainer
        phase={progressPhase}
        isLoading={isGathering || isSynthesizing}
        icon={Combine}
        header={
          <ModeHeader
            name="Synthesized"
            badge={
              <StatusBadge
                text={isGathering ? "GATHERING" : isSynthesizing ? "SYNTHESIZING" : "COMPLETE"}
                variant={progressPhase}
              />
            }
          />
        }
        expandableSection={
          hasResponses ? (
            <>
              {sourceResponses.map((response, index) => (
                <ResponseCard
                  key={response.model + index}
                  title={getShortModelName(response.model)}
                  content={response.content}
                  usage={response.usage}
                />
              ))}
            </>
          ) : undefined
        }
        expandLabel={{ collapsed: "Show sources", expanded: "Hide sources" }}
        showExpandable={hasResponses}
        renderFooter={
          isDone
            ? ({ isExpanded, toggleExpand, hasExpandable }) => (
                <div className="mt-1 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <p className="text-[10px] text-muted-foreground">
                      Synthesized {completedModels.length} responses using{" "}
                      {getShortModelName(synthesizerModel)}
                    </p>
                    {hasResponses && (
                      <UsageSummary totalTokens={totalTokens} totalCost={totalCost} />
                    )}
                  </div>
                  {hasExpandable && (
                    <ExpandButton
                      isExpanded={isExpanded}
                      onToggle={toggleExpand}
                      expandedLabel="Hide sources"
                      collapsedLabel="Show sources"
                    />
                  )}
                </div>
              )
            : undefined
        }
      >
        {/* Model progress during gathering phase */}
        {isGathering && (
          <div className="mt-2 space-y-1">
            <div className="flex items-center gap-1.5 flex-wrap">
              {displayModels.map((model) => {
                const isComplete = completedModels.includes(model);
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
              {completedModels.length}/{totalModels} responses received
            </p>
          </div>
        )}

        {/* Synthesizer info during synthesizing phase */}
        {isSynthesizing && (
          <div className="mt-2 space-y-1">
            <div className="flex items-center gap-1.5">
              <span className="text-xs text-muted-foreground">Synthesizer:</span>
              <ModelBadge model={synthesizerModel} variant="amber" />
            </div>
            <p className="text-[10px] text-muted-foreground animate-pulse">
              Combining {completedModels.length} responses...
            </p>
          </div>
        )}
      </ProgressContainer>
    </div>
  );
}
