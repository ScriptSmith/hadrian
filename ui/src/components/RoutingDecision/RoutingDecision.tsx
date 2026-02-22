import { GitBranch, Loader2, ArrowRight, CheckCircle2, AlertTriangle } from "lucide-react";

import { useActiveRoutedState } from "@/stores/streamingStore";
import type { MessageUsage } from "@/components/chat-types";
import { getShortModelName, UsageSummary } from "@/components/ModeProgress";
import { cn } from "@/utils/cn";

interface RoutingDecisionProps {
  /**
   * Persisted metadata for displaying historical messages.
   * When provided, this takes precedence over live streaming state.
   */
  persistedMetadata?: {
    routerModel: string;
    selectedModel: string;
    reasoning?: string;
    isFallback?: boolean;
    routerUsage?: MessageUsage;
  };
}

/**
 * RoutingDecision - Visual indicator for routed mode
 *
 * Shows the routing process:
 * 1. During "routing" phase: Shows the router model analyzing the prompt
 * 2. During "selected" phase: Shows which model was selected and why
 * 3. Shows fallback indicator if router failed to select a valid model
 *
 * Uses the new `useActiveRoutedState()` selector for live streaming state.
 * For persisted messages, accepts `persistedMetadata` prop to display historical data.
 */
export function RoutingDecision({ persistedMetadata }: RoutingDecisionProps) {
  // Use new discriminated union selector for live streaming state
  const liveState = useActiveRoutedState();

  // Determine which state to use: persisted metadata or live streaming state
  const isPersisted = !!persistedMetadata;
  const phase = isPersisted ? "selected" : (liveState?.phase ?? "selected");
  const routerModel = persistedMetadata?.routerModel ?? liveState?.routerModel ?? "";
  const selectedModel = persistedMetadata?.selectedModel ?? liveState?.selectedModel ?? "";
  const reasoning = persistedMetadata?.reasoning ?? liveState?.reasoning;
  const isFallback = persistedMetadata?.isFallback ?? liveState?.isFallback ?? false;
  const routerUsage = persistedMetadata?.routerUsage;

  // Don't render if there's no state at all (neither live nor persisted)
  if (!liveState && !persistedMetadata) {
    return null;
  }

  const isRouting = phase === "routing";

  // Determine styling based on state
  const containerStyle = isRouting
    ? "bg-amber-500/10 border-amber-500/30"
    : isFallback
      ? "bg-orange-500/10 border-orange-500/30"
      : "bg-primary/5 border-primary/30";

  const iconColor = isRouting ? "text-amber-500" : isFallback ? "text-orange-500" : "text-primary";

  return (
    <div className={cn("flex items-start gap-2 px-3 py-2 rounded-lg border", containerStyle)}>
      <div className="shrink-0 mt-0.5">
        {isRouting ? (
          <Loader2 className="h-4 w-4 text-amber-500 animate-spin" />
        ) : isFallback ? (
          <AlertTriangle className="h-4 w-4 text-orange-500" />
        ) : (
          <CheckCircle2 className={cn("h-4 w-4", iconColor)} />
        )}
      </div>

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 text-xs font-medium">
          <GitBranch className="h-3.5 w-3.5 text-muted-foreground" />
          <span className="text-muted-foreground">Routed</span>
          {isFallback && !isRouting && (
            <span className="px-1.5 py-0.5 rounded text-[9px] font-semibold bg-orange-500/20 text-orange-800 dark:text-orange-400">
              FALLBACK
            </span>
          )}
        </div>

        <div className="mt-1 flex items-center gap-1.5 text-sm">
          <span
            className={cn(
              "px-1.5 py-0.5 rounded text-[10px] font-medium",
              "bg-muted text-muted-foreground"
            )}
          >
            {getShortModelName(routerModel)}
          </span>

          <ArrowRight className="h-3 w-3 text-muted-foreground" />

          {isRouting ? (
            <span className="text-xs text-amber-800 dark:text-amber-400 animate-pulse">
              Selecting model...
            </span>
          ) : (
            <span
              className={cn(
                "px-1.5 py-0.5 rounded text-[10px] font-medium",
                isFallback
                  ? "bg-orange-500/20 text-orange-800 dark:text-orange-400"
                  : "bg-primary/10 text-primary"
              )}
            >
              {getShortModelName(selectedModel || "")}
            </span>
          )}
        </div>

        {/* eslint-disable jsx-a11y/no-noninteractive-tabindex -- scrollable region needs keyboard access (axe: scrollable-region-focusable) */}
        {!isRouting && reasoning && (
          <p
            className="mt-1 text-[11px] text-muted-foreground leading-tight max-h-24 overflow-y-auto scrollbar-thin"
            tabIndex={0}
          >
            {reasoning}
          </p>
        )}
        {/* eslint-enable jsx-a11y/no-noninteractive-tabindex */}

        {/* Router usage display */}
        {!isRouting && routerUsage && routerUsage.totalTokens > 0 && (
          <div className="mt-1">
            <UsageSummary
              label="Router"
              totalTokens={routerUsage.totalTokens}
              totalCost={routerUsage.cost}
            />
          </div>
        )}
      </div>
    </div>
  );
}
