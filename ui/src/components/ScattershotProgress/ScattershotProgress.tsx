import { Target, Loader2, Check, AlertCircle } from "lucide-react";

import type { ScattershotVariation } from "@/stores/streamingStore";
import { useActiveScattershotState } from "@/stores/streamingStore";
import type { MessageUsage, ScattershotVariationData } from "@/components/chat-types";
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

interface ScattershotProgressProps {
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    variations: ScattershotVariationData[];
    targetModel?: string;
    aggregateUsage?: MessageUsage;
  };
}

/**
 * ScattershotProgress - Visual indicator for scattershot mode
 *
 * Shows the scattershot generation process:
 * 1. During "generating" phase: Shows variations being generated with their parameter labels
 * 2. During "done" phase: Shows all variations with expandable details
 *
 * Uses the new `useActiveScattershotState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function ScattershotProgress({ persistedMetadata }: ScattershotProgressProps) {
  // Use new discriminated union selector for live streaming state
  const liveState = useActiveScattershotState();

  // Determine which state to use: persisted metadata or live streaming state
  const phase = persistedMetadata ? "done" : (liveState?.phase ?? "done");
  const targetModel = persistedMetadata?.targetModel ?? liveState?.targetModel ?? "";
  const variations = liveState?.variations ?? [];

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isGenerating = phase === "generating";

  // Use persisted data if available
  const displayVariations = (persistedMetadata?.variations ??
    variations ??
    []) as ScattershotVariation[];
  const displayTargetModel = targetModel;

  // Calculate usage
  const usageItems = displayVariations.map((v) => ({ usage: v.usage }));
  const computedUsage = aggregateUsage(usageItems);
  const totalUsage = persistedMetadata?.aggregateUsage
    ? {
        totalTokens: persistedMetadata.aggregateUsage.totalTokens,
        totalCost: persistedMetadata.aggregateUsage.cost ?? 0,
      }
    : { totalTokens: computedUsage.totalTokens, totalCost: computedUsage.cost ?? 0 };

  // Count completed and failed variations
  const completedCount = displayVariations.filter((v) => v.status === "complete").length;
  const failedCount = displayVariations.filter((v) => v.status === "failed").length;
  const totalVariations = displayVariations.length;

  // Map phase to ProgressPhase
  const progressPhase: ProgressPhase = isGenerating ? "active" : "complete";

  // Map phase to status badge variant
  const statusVariant = isGenerating ? "active" : "complete";
  const statusText = isGenerating ? `GENERATING ${completedCount}/${totalVariations}` : "COMPLETE";

  // Build header
  const header = (
    <ModeHeader
      name="Scattershot"
      badge={<StatusBadge text={statusText} variant={statusVariant} />}
    />
  );

  // Build expandable section content
  const expandableSection =
    displayVariations.length > 0 ? (
      <>
        <div className="text-[10px] font-medium text-muted-foreground mb-2">
          All Variations ({displayVariations.length})
        </div>
        {displayVariations.map((variation) => (
          <ResponseCard
            key={variation.id}
            title={variation.label}
            content={variation.content || (variation.status === "failed" ? "(Failed)" : "")}
            usage={variation.usage}
            variant="default"
            previewLength={200}
          />
        ))}
      </>
    ) : undefined;

  return (
    <ProgressContainer
      phase={progressPhase}
      isLoading={isGenerating}
      icon={Target}
      header={header}
      expandableSection={expandableSection}
      expandLabel={{ collapsed: "Show variations", expanded: "Hide variations" }}
      showExpandable={displayVariations.length > 0}
      renderFooter={({ isExpanded, toggleExpand, hasExpandable }) => (
        <div className="mt-1 flex items-center justify-between">
          <div className="flex items-center gap-2">
            {totalUsage.totalTokens > 0 && (
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
              collapsedLabel="Show variations"
              expandedLabel="Hide variations"
            />
          )}
        </div>
      )}
    >
      {/* Target model */}
      <div className="mt-1.5 space-y-2">
        <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
          <span>Model:</span>
          <span className="px-1.5 py-0.5 rounded bg-primary/10 text-primary font-medium">
            {getShortModelName(displayTargetModel)}
          </span>
        </div>

        {/* Variation badges */}
        {totalVariations > 0 && (
          <div className="space-y-1.5">
            <div className="text-[10px] text-muted-foreground flex items-center gap-1">
              <span>
                Variations ({completedCount}/{totalVariations} complete)
              </span>
              {failedCount > 0 && <span className="text-red-500">({failedCount} failed)</span>}
            </div>

            {/* Variation badges */}
            <div className="flex flex-wrap gap-1">
              {displayVariations.map((variation) => (
                <VariationBadge key={variation.id} variation={variation} />
              ))}
            </div>
          </div>
        )}
      </div>
    </ProgressContainer>
  );
}

/**
 * VariationBadge - Shows individual variation status
 */
function VariationBadge({
  variation,
}: {
  variation: ScattershotVariation | ScattershotVariationData;
}) {
  const status = "status" in variation ? variation.status : "complete";

  const statusStyles: Record<string, string> = {
    pending: "bg-muted text-muted-foreground",
    generating: "bg-orange-500/20 text-orange-800 dark:text-orange-400",
    complete: "bg-primary/10 text-primary",
    failed: "bg-red-500/20 text-red-700 dark:text-red-400",
  };

  const StatusIcon =
    {
      pending: Target,
      generating: Loader2,
      complete: Check,
      failed: AlertCircle,
    }[status] || Check;

  return (
    <div
      className={cn(
        "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium",
        statusStyles[status]
      )}
      title={variation.label}
    >
      <StatusIcon className={cn("h-3 w-3", status === "generating" && "animate-spin")} />
      <span className="max-w-[120px] truncate">{variation.label}</span>
    </div>
  );
}
