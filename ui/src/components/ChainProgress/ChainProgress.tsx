import { Link, Check, Circle } from "lucide-react";

import { useActiveChainedState } from "@/stores/streamingStore";
import { getShortModelName } from "@/components/ModeProgress";
import { cn } from "@/utils/cn";

interface ChainProgressProps {
  /** List of model names in order */
  models: string[];
  /**
   * Preview position for Storybook/testing.
   * When provided, this takes precedence over live streaming state.
   * Format: [currentIndex, totalCount]
   */
  previewPosition?: [number, number];
}

/**
 * ChainProgress - Visual indicator for chained mode progress
 *
 * Shows the sequential progress through models in chained mode.
 * Each model is shown as a step in a horizontal chain with visual
 * indicators for completed, active, and pending states.
 *
 * Uses the new `useActiveChainedState()` selector for live streaming state.
 * This component only renders during live streaming (no persisted state).
 */
export function ChainProgress({ models, previewPosition }: ChainProgressProps) {
  // Use new discriminated union selector for live streaming state
  const liveState = useActiveChainedState();

  // Determine which state to use: preview (for Storybook) or live streaming state
  const position = previewPosition ?? liveState?.position;

  // Don't render if there's no state at all (neither preview nor live)
  if (!position) {
    return null;
  }

  const [currentIndex, totalCount] = position;

  return (
    <div className="flex items-center gap-1.5 px-3 py-2 bg-muted/50 rounded-lg border border-border/50">
      <Link className="h-4 w-4 text-primary shrink-0" />
      <span className="text-xs font-medium text-muted-foreground mr-1">Chain</span>
      <div className="flex items-center gap-1">
        {models.map((model, index) => {
          const isComplete = index < currentIndex;
          const isActive = index === currentIndex;
          const isPending = index > currentIndex;
          const isLast = index === totalCount - 1;

          const displayName = getShortModelName(model);

          return (
            <div key={model} className="flex items-center">
              <div
                className={cn(
                  "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors",
                  isComplete && "bg-primary/10 text-primary",
                  isActive && "bg-primary text-primary-foreground animate-pulse",
                  isPending && "bg-muted text-muted-foreground"
                )}
              >
                {isComplete && <Check className="h-3 w-3" />}
                {isActive && <Circle className="h-3 w-3 fill-current" />}
                {isPending && <Circle className="h-3 w-3" />}
                <span className="max-w-[60px] truncate">{displayName}</span>
              </div>
              {!isLast && (
                <svg
                  className={cn(
                    "h-3 w-3 mx-0.5",
                    isComplete ? "text-primary" : "text-muted-foreground/40"
                  )}
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <path d="M9 6l6 6-6 6" />
                </svg>
              )}
            </div>
          );
        })}
      </div>
      <span className="text-xs text-muted-foreground ml-1">
        {currentIndex + 1}/{totalCount}
      </span>
    </div>
  );
}
