import { useState, useMemo, useCallback } from "react";
import { Clock, Image as ImageIcon, Trash2 } from "lucide-react";
import { cn } from "@/utils/cn";
import { ImageCard } from "./ImageCard";
import { ImageLightbox, type LightboxImage } from "./ImageLightbox";
import { getModelDisplayName } from "@/utils/modelNames";
import { formatCost } from "@/utils/formatters";
import type { ImageHistoryEntry } from "@/pages/studio/types";

function formatTimestamp(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  const time = d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  if (d.toDateString() === now.toDateString()) return time;
  return `${d.toLocaleDateString(undefined, { month: "short", day: "numeric" })} ${time}`;
}

interface ImageGalleryProps {
  entries: ImageHistoryEntry[];
  onDelete: (id: string) => void;
  className?: string;
}

export function ImageGallery({ entries, onDelete, className }: ImageGalleryProps) {
  const [lightboxIndex, setLightboxIndex] = useState<number | null>(null);

  // Build flat array of all images for lightbox navigation
  const allImages = useMemo(() => {
    const images: LightboxImage[] = [];
    for (const entry of entries) {
      for (const result of entry.results) {
        if (result.error) continue;
        const label = result.label || getModelDisplayName(result.modelId);
        for (const img of result.images) {
          images.push({
            imageData: img.imageData,
            prompt: entry.prompt,
            revisedPrompt: img.revisedPrompt,
            modelLabel: label,
          });
        }
      }
    }
    return images;
  }, [entries]);

  // Build a map from (entryId, resultInstanceId, imageIndex) → flat index
  const flatIndexMap = useMemo(() => {
    const map = new Map<string, number>();
    let idx = 0;
    for (const entry of entries) {
      for (const result of entry.results) {
        if (result.error) continue;
        for (let i = 0; i < result.images.length; i++) {
          map.set(`${entry.id}:${result.instanceId}:${i}`, idx);
          idx++;
        }
      }
    }
    return map;
  }, [entries]);

  const openLightbox = useCallback(
    (entryId: string, instanceId: string, imageIdx: number) => {
      const flatIdx = flatIndexMap.get(`${entryId}:${instanceId}:${imageIdx}`);
      if (flatIdx != null) setLightboxIndex(flatIdx);
    },
    [flatIndexMap]
  );

  if (entries.length === 0) {
    return (
      <div className={cn("flex flex-1 flex-col items-center justify-center py-16", className)}>
        <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-muted/50">
          <ImageIcon className="h-8 w-8 text-muted-foreground/50" />
        </div>
        <h3 className="text-base font-medium text-foreground">Create something</h3>
        <p className="mt-1 max-w-xs text-center text-sm text-muted-foreground">
          Describe an image and bring your ideas to life
        </p>
      </div>
    );
  }

  return (
    <>
      <div className={cn("space-y-5", className)}>
        {entries.map((entry) => {
          const hasMultipleModels = entry.results.length > 1;
          const totalImages = entry.results.reduce(
            (sum, r) => sum + (r.error ? 0 : r.images.length),
            0
          );
          const totalCostMicrocents = entry.results.reduce(
            (sum, r) => sum + (r.costMicrocents ?? 0),
            0
          );
          return (
            <div
              key={entry.id}
              className="rounded-xl border border-border bg-card/50 overflow-hidden"
            >
              {/* Entry header */}
              <div className="flex items-start gap-3 border-b border-border/50 px-4 py-3">
                <div className="min-w-0 flex-1 space-y-1">
                  <p className="text-sm font-medium text-foreground line-clamp-2">{entry.prompt}</p>
                  <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
                    <span className="inline-flex items-center gap-1">
                      <Clock className="h-3 w-3" aria-hidden="true" />
                      {formatTimestamp(entry.createdAt)}
                    </span>
                    {entry.options.size && (
                      <span className="rounded bg-muted px-1.5 py-0.5">{entry.options.size}</span>
                    )}
                    {entry.options.quality && (
                      <span className="rounded bg-muted px-1.5 py-0.5">
                        {entry.options.quality}
                      </span>
                    )}
                    <span>
                      {totalImages} {totalImages === 1 ? "image" : "images"}
                      {hasMultipleModels && ` across ${entry.results.length} models`}
                    </span>
                    {totalCostMicrocents > 0 && (
                      <span className="rounded bg-muted px-1.5 py-0.5">
                        {formatCost(totalCostMicrocents / 1_000_000)}
                      </span>
                    )}
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => onDelete(entry.id)}
                  className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-destructive/10 hover:text-destructive motion-safe:transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  aria-label="Delete entry"
                >
                  <Trash2 className="h-4 w-4" />
                </button>
              </div>

              {/* Results grid per instance */}
              <div className="p-4">
                <div
                  className={cn(
                    "grid gap-4",
                    hasMultipleModels
                      ? "grid-cols-1 md:grid-cols-2"
                      : "grid-cols-1 sm:grid-cols-[repeat(auto-fill,minmax(200px,1fr))]"
                  )}
                >
                  {entry.results.map((result) => (
                    <div key={result.instanceId}>
                      {hasMultipleModels && (
                        <div className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                          <span>{result.label || getModelDisplayName(result.modelId)}</span>
                          {result.costMicrocents != null && result.costMicrocents > 0 && (
                            <>
                              <span className="text-muted-foreground/50">&middot;</span>
                              <span className="text-[10px] font-normal">
                                {formatCost(result.costMicrocents / 1_000_000)}
                              </span>
                            </>
                          )}
                        </div>
                      )}
                      {result.error ? (
                        <div className="flex items-center gap-2 rounded-xl border border-destructive/30 bg-destructive/5 px-3 py-4 text-sm text-destructive">
                          {result.error}
                        </div>
                      ) : (
                        <div className="grid gap-2 grid-cols-1 sm:grid-cols-[repeat(auto-fill,minmax(180px,1fr))]">
                          {result.images.map((img, i) => (
                            <ImageCard
                              key={i}
                              imageData={img.imageData}
                              prompt={entry.prompt}
                              revisedPrompt={img.revisedPrompt}
                              createdAt={entry.createdAt}
                              onFullscreen={() => openLightbox(entry.id, result.instanceId, i)}
                            />
                          ))}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Lightbox */}
      {lightboxIndex != null && (
        <ImageLightbox
          images={allImages}
          currentIndex={lightboxIndex}
          onClose={() => setLightboxIndex(null)}
          onNavigate={setLightboxIndex}
        />
      )}
    </>
  );
}
