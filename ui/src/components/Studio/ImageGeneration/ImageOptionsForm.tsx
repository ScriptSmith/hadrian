import { useState, useId, useMemo } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { cn } from "@/utils/cn";
import type { ImageSize, ImageQuality } from "@/api/generated/types.gen";
import type { ModelInstance } from "@/components/chat-types";
import type { ModelInfo } from "@/components/ModelPicker/model-utils";

export interface ImageOptions {
  n: number;
  size: ImageSize;
  quality: ImageQuality;
}

interface ImageOptionsFormProps {
  options: ImageOptions;
  onChange: (options: ImageOptions) => void;
  disabled?: boolean;
  /** Currently selected model instances */
  instances?: ModelInstance[];
  /** Available model metadata (for looking up capabilities) */
  availableModels?: ModelInfo[];
}

const ALL_SIZES: { value: ImageSize; label: string }[] = [
  { value: "256x256", label: "Small" },
  { value: "512x512", label: "Medium" },
  { value: "1024x1024", label: "Square" },
  { value: "1536x1024", label: "Landscape" },
  { value: "1024x1536", label: "Portrait" },
  { value: "1792x1024", label: "Wide" },
  { value: "1024x1792", label: "Tall" },
  { value: "auto", label: "Auto" },
];

const ALL_QUALITIES: ImageQuality[] = ["auto", "low", "medium", "high", "standard", "hd"];

/** Compute the union of capabilities from selected models. */
function useModelCapabilities(instances?: ModelInstance[], availableModels?: ModelInfo[]) {
  return useMemo(() => {
    if (!instances?.length || !availableModels?.length) {
      return { sizes: null, qualities: null, maxN: 4 };
    }

    const modelInfos = instances
      .map((inst) => availableModels.find((m) => m.id === inst.modelId))
      .filter((m): m is ModelInfo => !!m);

    // Only constrain if at least one model has metadata
    const modelsWithSizes = modelInfos.filter((m) => m.image_sizes?.length);
    const modelsWithQualities = modelInfos.filter((m) => m.image_qualities?.length);
    const modelsWithMaxImages = modelInfos.filter((m) => m.max_images != null);

    // Union of sizes across all models that have size data
    const sizes =
      modelsWithSizes.length > 0
        ? [...new Set(modelsWithSizes.flatMap((m) => m.image_sizes!))]
        : null;

    // Union of qualities across all models that have quality data
    const qualities =
      modelsWithQualities.length > 0
        ? [...new Set(modelsWithQualities.flatMap((m) => m.image_qualities!))]
        : null;

    // Min of max_images across all models that specify it
    const maxN =
      modelsWithMaxImages.length > 0
        ? Math.min(...modelsWithMaxImages.map((m) => m.max_images!))
        : 4;

    return { sizes, qualities, maxN };
  }, [instances, availableModels]);
}

function PillGroup<T extends string>({
  options,
  value,
  onChange,
  disabled,
  label,
}: {
  options: T[];
  value: T;
  onChange: (v: T) => void;
  disabled?: boolean;
  label: string;
}) {
  const id = useId();
  return (
    <fieldset disabled={disabled}>
      <legend className="mb-1.5 text-xs font-medium text-muted-foreground">{label}</legend>
      <div id={id} className="flex gap-1 rounded-lg bg-muted/50 p-1" role="radiogroup">
        {options.map((opt) => (
          <button
            key={opt}
            type="button"
            role="radio"
            aria-checked={value === opt}
            className={cn(
              "flex-1 rounded-md px-2.5 py-1 text-xs font-medium capitalize",
              "motion-safe:transition-colors motion-safe:duration-150",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              value === opt
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground"
            )}
            onClick={() => onChange(opt)}
          >
            {opt}
          </button>
        ))}
      </div>
    </fieldset>
  );
}

export function ImageOptionsForm({
  options,
  onChange,
  disabled,
  instances,
  availableModels,
}: ImageOptionsFormProps) {
  const [expanded, setExpanded] = useState(false);
  const contentId = useId();
  const { sizes, qualities, maxN } = useModelCapabilities(instances, availableModels);

  // Filter sizes/qualities to only those supported by selected models
  const filteredSizes = sizes ? ALL_SIZES.filter((s) => sizes.includes(s.value)) : ALL_SIZES;

  const filteredQualities = qualities
    ? ALL_QUALITIES.filter((q) => qualities.includes(q))
    : ALL_QUALITIES;

  const hasQualities = qualities === null || qualities.length > 0;

  const update = <K extends keyof ImageOptions>(key: K, value: ImageOptions[K]) => {
    onChange({ ...options, [key]: value });
  };

  return (
    <div className="space-y-3">
      {/* Count stepper */}
      <div>
        <label
          htmlFor="img-count"
          className="mb-1.5 block text-xs font-medium text-muted-foreground"
        >
          Count
        </label>
        <div className="flex items-center gap-2">
          <button
            type="button"
            disabled={disabled || options.n <= 1}
            className={cn(
              "flex h-8 w-8 items-center justify-center rounded-lg border border-input text-sm font-medium",
              "hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            )}
            onClick={() => update("n", Math.max(1, options.n - 1))}
            aria-label="Decrease count"
          >
            -
          </button>
          <span className="w-8 text-center text-sm font-medium tabular-nums">{options.n}</span>
          <button
            type="button"
            disabled={disabled || options.n >= maxN}
            className={cn(
              "flex h-8 w-8 items-center justify-center rounded-lg border border-input text-sm font-medium",
              "hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            )}
            onClick={() => update("n", Math.min(maxN, options.n + 1))}
            aria-label="Increase count"
          >
            +
          </button>
          {maxN < 4 && <span className="text-[10px] text-muted-foreground">max {maxN}</span>}
        </div>
      </div>

      {/* Collapsible advanced options */}
      <button
        type="button"
        className={cn(
          "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-xs font-medium text-muted-foreground",
          "hover:bg-accent/50 motion-safe:transition-colors motion-safe:duration-150",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        )}
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        aria-controls={contentId}
      >
        {expanded ? (
          <ChevronDown className="h-3.5 w-3.5" aria-hidden="true" />
        ) : (
          <ChevronRight className="h-3.5 w-3.5" aria-hidden="true" />
        )}
        Advanced Options
      </button>

      <div
        id={contentId}
        className={cn(
          "space-y-3 overflow-hidden motion-safe:transition-all motion-safe:duration-300",
          expanded ? "max-h-[500px] opacity-100" : "max-h-0 opacity-0"
        )}
        aria-hidden={!expanded}
        {...(!expanded && { inert: true })}
      >
        {/* Size selector */}
        {filteredSizes.length > 0 && (
          <div>
            <span className="mb-1.5 block text-xs font-medium text-muted-foreground">Size</span>
            <div className="grid grid-cols-3 gap-1.5">
              {filteredSizes.map(({ value, label }) => (
                <button
                  key={value}
                  type="button"
                  disabled={disabled}
                  className={cn(
                    "flex flex-col items-center gap-1 rounded-lg border p-2",
                    "text-xs motion-safe:transition-colors motion-safe:duration-150",
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    options.size === value
                      ? "border-primary bg-primary/5 text-foreground"
                      : "border-input text-muted-foreground hover:border-primary/50"
                  )}
                  onClick={() => update("size", value)}
                  aria-pressed={options.size === value}
                >
                  <span className="font-medium">{label}</span>
                  <span className="text-[10px] text-muted-foreground">{value}</span>
                </button>
              ))}
            </div>
          </div>
        )}

        {hasQualities && (
          <PillGroup
            label="Quality"
            options={filteredQualities}
            value={options.quality}
            onChange={(v) => update("quality", v)}
            disabled={disabled}
          />
        )}
      </div>
    </div>
  );
}
