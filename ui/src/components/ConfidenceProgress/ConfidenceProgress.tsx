import { useState } from "react";
import { Scale, Check, Circle } from "lucide-react";

import type { ConfidenceResponse } from "@/stores/streamingStore";
import { useActiveConfidenceWeightedState } from "@/stores/streamingStore";
import type { MessageUsage, ConfidenceResponseData } from "@/components/chat-types";
import {
  ProgressContainer,
  ModeHeader,
  StatusBadge,
  ExpandButton,
  getShortModelName,
  UsageSummary,
  aggregateUsage,
  type ProgressPhase,
} from "@/components/ModeProgress";
import { StreamingMarkdown } from "@/components/StreamingMarkdown/StreamingMarkdown";
import { cn } from "@/utils/cn";

interface ConfidenceProgressProps {
  /** All models participating (for display during responding) */
  allModels?: string[];
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    responses: ConfidenceResponseData[];
    synthesizerModel?: string;
  };
}

/**
 * Get color class based on confidence level
 */
function getConfidenceColor(confidence: number): string {
  if (confidence >= 0.8) return "text-emerald-800 dark:text-emerald-400";
  if (confidence >= 0.6) return "text-blue-700 dark:text-blue-400";
  if (confidence >= 0.4) return "text-amber-800 dark:text-amber-400";
  return "text-rose-800 dark:text-rose-400";
}

/**
 * Get background color class based on confidence level
 */
function getConfidenceBgColor(confidence: number): string {
  if (confidence >= 0.8) return "bg-emerald-500/20";
  if (confidence >= 0.6) return "bg-blue-500/20";
  if (confidence >= 0.4) return "bg-amber-500/20";
  return "bg-rose-500/20";
}

/**
 * ConfidenceProgress - Visual indicator for confidence-weighted mode
 *
 * Shows the confidence-weighted synthesis process:
 * 1. During "responding" phase: Shows progress of model responses with confidence scores
 * 2. During "synthesizing" phase: Shows the synthesizer combining weighted responses
 * 3. During "done" phase: Shows completion status with expandable source responses
 *
 * Uses the new `useActiveConfidenceWeightedState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function ConfidenceProgress({ allModels, persistedMetadata }: ConfidenceProgressProps) {
  // Use new discriminated union selector for live streaming state
  const liveState = useActiveConfidenceWeightedState();

  // Determine which state to use: persisted metadata or live streaming state
  const phase = persistedMetadata ? "done" : (liveState?.phase ?? "done");
  const synthesizerModel = persistedMetadata?.synthesizerModel ?? liveState?.synthesizerModel ?? "";
  const responses = liveState?.responses ?? [];
  const completedResponses = liveState?.completedResponses ?? 0;
  const totalModels = liveState?.totalModels ?? 0;

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isResponding = phase === "responding";
  const isSynthesizing = phase === "synthesizing";

  // Use persisted data if provided, otherwise use live state
  const displaySynthesizerModel = synthesizerModel;
  const responsesToDisplay: Array<ConfidenceResponse | ConfidenceResponseData> =
    persistedMetadata?.responses ?? responses ?? [];
  const hasResponses = responsesToDisplay.length > 0;

  // Calculate aggregate usage
  const { totalTokens, cost: totalCost } = aggregateUsage(responsesToDisplay);

  // Calculate average confidence
  const avgConfidence =
    responsesToDisplay.length > 0
      ? responsesToDisplay.reduce((sum, r) => sum + r.confidence, 0) / responsesToDisplay.length
      : 0;

  // Map phase to ProgressPhase
  const progressPhase: ProgressPhase = isResponding
    ? "initial"
    : isSynthesizing
      ? "active"
      : "complete";

  // Map phase to status badge variant
  const statusVariant = isResponding ? "initial" : isSynthesizing ? "active" : "complete";
  const statusText = isResponding ? "RESPONDING" : isSynthesizing ? "SYNTHESIZING" : "COMPLETE";

  // Get display models - during responding, show all models
  const displayModels = allModels || responses.map((r) => r.model);

  // Build header
  const header = (
    <ModeHeader
      name="Confidence-Weighted"
      badge={<StatusBadge text={statusText} variant={statusVariant} />}
    />
  );

  // Build expandable section content
  const expandableSection = hasResponses ? (
    <>
      {/* Sort by confidence (highest first) */}
      {[...responsesToDisplay]
        .sort((a, b) => b.confidence - a.confidence)
        .map((response, index) => (
          <ConfidenceResponseCard
            key={response.model + index}
            model={response.model}
            content={response.content}
            confidence={response.confidence}
            usage={response.usage}
          />
        ))}
    </>
  ) : undefined;

  return (
    <ProgressContainer
      phase={progressPhase}
      isLoading={isResponding || isSynthesizing}
      icon={Scale}
      header={header}
      expandableSection={expandableSection}
      expandLabel={{ collapsed: "Show sources", expanded: "Hide sources" }}
      showExpandable={hasResponses}
      renderFooter={({ isExpanded: expanded, toggleExpand, hasExpandable }) => (
        <div className="mt-1 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <p className="text-[10px] text-muted-foreground">
              Synthesized {responsesToDisplay.length} responses
              {responsesToDisplay.length > 0 && (
                <span className={cn("font-medium ml-1", getConfidenceColor(avgConfidence))}>
                  (avg: {(avgConfidence * 100).toFixed(0)}%)
                </span>
              )}
            </p>
            {hasResponses && <UsageSummary totalTokens={totalTokens} totalCost={totalCost} />}
          </div>
          {hasExpandable && (
            <ExpandButton
              isExpanded={expanded}
              onToggle={toggleExpand}
              collapsedLabel="Show sources"
              expandedLabel="Hide sources"
            />
          )}
        </div>
      )}
    >
      {/* Model progress during responding phase */}
      {isResponding && (
        <div className="mt-2 space-y-1">
          <div className="flex items-center gap-1.5 flex-wrap">
            {displayModels.map((model) => {
              const response = responses.find((r) => r.model === model);
              const isComplete = !!response;
              return (
                <div
                  key={model}
                  className={cn(
                    "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors",
                    isComplete
                      ? cn(
                          getConfidenceBgColor(response.confidence),
                          getConfidenceColor(response.confidence)
                        )
                      : "bg-muted text-muted-foreground"
                  )}
                >
                  {isComplete ? (
                    <>
                      <Check className="h-3 w-3" />
                      <span className="max-w-[60px] truncate">{getShortModelName(model)}</span>
                      <span className="text-[9px]">{(response.confidence * 100).toFixed(0)}%</span>
                    </>
                  ) : (
                    <>
                      <Circle className="h-3 w-3 animate-pulse" />
                      <span className="max-w-[80px] truncate">{getShortModelName(model)}</span>
                    </>
                  )}
                </div>
              );
            })}
          </div>
          <p className="text-[10px] text-muted-foreground">
            {completedResponses}/{totalModels} responses with confidence scores
          </p>
        </div>
      )}

      {/* Synthesizer info during synthesizing phase */}
      {isSynthesizing && (
        <div className="mt-2 space-y-1">
          <div className="flex items-center gap-1.5">
            <span className="text-xs text-muted-foreground">Synthesizer:</span>
            <span className="px-1.5 py-0.5 rounded text-[10px] font-medium bg-amber-500/20 text-amber-800 dark:text-amber-400">
              {getShortModelName(displaySynthesizerModel)}
            </span>
          </div>
          <div className="flex items-center gap-2 text-[10px] text-muted-foreground animate-pulse">
            <span>Weighting {responses.length} responses</span>
            {responses.length > 0 && (
              <span className={cn("font-medium", getConfidenceColor(avgConfidence))}>
                (avg: {(avgConfidence * 100).toFixed(0)}%)
              </span>
            )}
          </div>
        </div>
      )}
    </ProgressContainer>
  );
}

interface ConfidenceResponseCardProps {
  model: string;
  content: string;
  confidence: number;
  usage?: MessageUsage;
}

/**
 * ConfidenceResponseCard - Card showing a response with its confidence score
 */
function ConfidenceResponseCard({
  model,
  content,
  confidence,
  usage,
}: ConfidenceResponseCardProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const previewLength = 200;
  const needsTruncation = content.length > previewLength;
  const previewContent = needsTruncation ? content.slice(0, previewLength) + "..." : content;

  const bgColor = getConfidenceBgColor(confidence);
  const textColor = getConfidenceColor(confidence);

  return (
    <div className={cn("rounded border", bgColor.replace("/20", "/10"), "border-current/20")}>
      <div
        className={cn(
          "flex items-center justify-between px-2 py-1.5 border-b",
          bgColor.replace("/20", "/10"),
          "border-current/10"
        )}
      >
        <div className="flex items-center gap-2">
          <span className={cn("px-1.5 py-0.5 rounded text-[10px] font-medium", bgColor, textColor)}>
            {getShortModelName(model)}
          </span>
          <span className={cn("text-[10px] font-semibold", textColor)}>
            {(confidence * 100).toFixed(0)}% confidence
          </span>
          {usage && (
            <span className="text-[9px] text-muted-foreground">{usage.totalTokens} tokens</span>
          )}
        </div>
        {needsTruncation && (
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="text-[10px] text-muted-foreground hover:text-foreground transition-colors"
          >
            {isExpanded ? "Collapse" : "Expand"}
          </button>
        )}
      </div>
      <div className="px-2 py-1.5 text-xs">
        <StreamingMarkdown content={isExpanded ? content : previewContent} isStreaming={false} />
      </div>
    </div>
  );
}
