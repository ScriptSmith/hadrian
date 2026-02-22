import type { ReactNode } from "react";
import { AlertCircle } from "lucide-react";
import { cn } from "@/utils/cn";
import { getModelDisplayName } from "@/utils/modelNames";
import { formatCost } from "@/utils/formatters";
import type { InstanceResult } from "@/pages/studio/useMultiModelExecution";

interface MultiModelResultGridProps<T> {
  results: Map<string, InstanceResult<T>>;
  renderResult: (result: InstanceResult<T>) => ReactNode;
  renderLoading?: () => ReactNode;
  renderError?: (error: string) => ReactNode;
  className?: string;
}

function DefaultLoading() {
  return (
    <div className="flex items-center justify-center py-12">
      <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
    </div>
  );
}

function DefaultError({ error }: { error: string }) {
  return (
    <div className="flex flex-col items-center gap-2 py-8 text-sm text-destructive">
      <AlertCircle className="h-5 w-5" />
      <p>{error}</p>
    </div>
  );
}

function CostBadge({ microcents }: { microcents: number }) {
  const dollars = microcents / 1_000_000;
  return <span className="shrink-0 text-[10px] text-muted-foreground">{formatCost(dollars)}</span>;
}

export function MultiModelResultGrid<T>({
  results,
  renderResult,
  renderLoading,
  renderError,
  className,
}: MultiModelResultGridProps<T>) {
  const entries = Array.from(results.values());
  if (entries.length === 0) return null;

  const isSingle = entries.length === 1;

  return (
    <div
      className={cn(
        "grid gap-4",
        isSingle ? "grid-cols-1" : "grid-cols-1 md:grid-cols-2 xl:grid-cols-3",
        className
      )}
    >
      {entries.map((result) => {
        const hasStats =
          (result.durationMs != null || result.costMicrocents != null) &&
          result.status === "complete";

        return (
          <div key={result.instanceId} className="min-w-0">
            {/* Model header (multi-model) */}
            {!isSingle && (
              <div className="mb-2 flex items-center gap-2">
                <span className="truncate text-xs font-medium text-muted-foreground">
                  {result.label || getModelDisplayName(result.modelId)}
                </span>
                {hasStats && (
                  <div className="flex shrink-0 items-center gap-1.5">
                    {result.durationMs != null && (
                      <span className="text-[10px] text-muted-foreground">
                        {(result.durationMs / 1000).toFixed(1)}s
                      </span>
                    )}
                    {result.costMicrocents != null && result.costMicrocents > 0 && (
                      <>
                        <span className="text-muted-foreground/50">&middot;</span>
                        <CostBadge microcents={result.costMicrocents} />
                      </>
                    )}
                  </div>
                )}
              </div>
            )}

            {/* Stats bar for single-model results */}
            {isSingle && hasStats && (
              <div className="mb-2 flex items-center gap-1.5 text-[10px] text-muted-foreground">
                {result.durationMs != null && <span>{(result.durationMs / 1000).toFixed(1)}s</span>}
                {result.costMicrocents != null && result.costMicrocents > 0 && (
                  <>
                    {result.durationMs != null && (
                      <span className="text-muted-foreground/50">&middot;</span>
                    )}
                    <CostBadge microcents={result.costMicrocents} />
                  </>
                )}
              </div>
            )}

            {/* Content */}
            {result.status === "loading" && (renderLoading ? renderLoading() : <DefaultLoading />)}
            {result.status === "error" &&
              (renderError ? (
                renderError(result.error ?? "Unknown error")
              ) : (
                <DefaultError error={result.error ?? "Unknown error"} />
              ))}
            {result.status === "complete" && renderResult(result)}
          </div>
        );
      })}
    </div>
  );
}
