import { useState, useCallback, useEffect, useRef } from "react";
import { Wand2 } from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";
import { useToast } from "@/components/Toast/Toast";
import { PromptInput } from "@/components/Studio/PromptInput/PromptInput";
import { ModelSelector } from "@/components/ModelSelector/ModelSelector";
import { MultiModelResultGrid } from "@/components/Studio/MultiModelResultGrid/MultiModelResultGrid";
import { ImageOptionsForm, type ImageOptions } from "./ImageOptionsForm";
import { ImageGallery } from "./ImageGallery";
import { useImageHistory } from "@/pages/studio/useStudioHistory";
import {
  useMultiModelExecution,
  extractCostFromResponse,
} from "@/pages/studio/useMultiModelExecution";
import { apiV1ImagesGenerations } from "@/api/generated/sdk.gen";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { createDefaultInstance } from "@/components/chat-types";
import type { ModelInstance } from "@/components/chat-types";
import type { ModelInfo } from "@/components/ModelPicker/model-utils";
import { ExpandableCaption } from "./ExpandableCaption";
import type { ImageHistoryEntry, InstanceImageResult } from "@/pages/studio/types";

interface ImageResult {
  images: Array<{ imageData: string; revisedPrompt?: string }>;
}

const DEFAULT_OPTIONS: ImageOptions = {
  n: 1,
  size: "1024x1024",
  quality: "auto",
};

interface ImageGenerationPanelProps {
  availableModels?: ModelInfo[];
}

export function ImageGenerationPanel({ availableModels }: ImageGenerationPanelProps) {
  const [prompt, setPrompt] = useState("");
  const [options, setOptions] = useState<ImageOptions>(DEFAULT_OPTIONS);
  const [instances, setInstances] = useState<ModelInstance[]>([]);
  const { entries, addEntry, removeEntry } = useImageHistory();
  const { toast } = useToast();
  const { isExecuting, results, execute, clearResults } = useMultiModelExecution<ImageResult>();
  const { preferences } = usePreferences();

  // Initialize instances from task-specific defaults (once, when models load)
  const hasInitRef = useRef(false);
  useEffect(() => {
    if (hasInitRef.current || !availableModels?.length) return;
    hasInitRef.current = true;
    const defaults = preferences.defaultModels?.images || [];
    const valid = defaults.filter((m) => availableModels.some((am) => am.id === m));
    if (valid.length > 0) {
      setInstances(valid.map((m) => createDefaultInstance(m)));
    }
  }, [availableModels, preferences.defaultModels]);

  // Auto-correct options when selected models change (e.g. clamp N, fix invalid size)
  useEffect(() => {
    if (!instances.length || !availableModels?.length) return;
    const modelInfos = instances
      .map((i) => availableModels.find((m) => m.id === i.modelId))
      .filter(Boolean);
    const withMax = modelInfos.filter((m) => m?.max_images != null);
    const maxN = withMax.length > 0 ? Math.min(...withMax.map((m) => m!.max_images!)) : 4;
    const withSizes = modelInfos.filter((m) => m?.image_sizes?.length);
    const allSizes =
      withSizes.length > 0 ? [...new Set(withSizes.flatMap((m) => m!.image_sizes!))] : null;

    let corrected = false;
    const next = { ...options };
    if (options.n > maxN) {
      next.n = maxN;
      corrected = true;
    }
    if (allSizes && !allSizes.includes(options.size)) {
      next.size = allSizes[0] as typeof options.size;
      corrected = true;
    }
    if (corrected) setOptions(next);
  }, [instances, availableModels]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleSubmit = useCallback(async () => {
    if (!prompt.trim() || isExecuting || instances.length === 0) return;

    const settled = await execute(instances, async (instance) => {
      // Per-model parameter adjustment
      const modelInfo = availableModels?.find((m) => m.id === instance.modelId);
      const modelSizes = modelInfo?.image_sizes;
      const modelQualities = modelInfo?.image_qualities;
      const modelMaxN = modelInfo?.max_images ?? 4;

      const n = Math.min(options.n, modelMaxN);
      const size =
        modelSizes?.length && !modelSizes.includes(options.size)
          ? (modelSizes[0] as typeof options.size)
          : options.size;
      const quality = modelQualities?.length
        ? modelQualities.includes(options.quality)
          ? options.quality
          : (modelQualities[0] as typeof options.quality)
        : undefined;

      const response = await apiV1ImagesGenerations({
        body: {
          prompt,
          model: instance.modelId,
          n,
          size,
          ...(quality != null && { quality }),
          response_format: "b64_json",
        },
      });
      if (response.error) throw new Error("Failed to generate image");
      const images = (response.data?.data ?? []).map((img) => ({
        imageData: img.b64_json ? `data:image/png;base64,${img.b64_json}` : (img.url ?? ""),
        revisedPrompt: img.revised_prompt ?? undefined,
      }));
      return {
        data: { images },
        costMicrocents: extractCostFromResponse(response.response),
      };
    });

    // Build grouped history entry from settled results
    const instanceResults: InstanceImageResult[] = settled
      .filter((r) => r.status === "complete" || r.status === "error")
      .map((r) => ({
        instanceId: r.instanceId,
        modelId: r.modelId,
        label: r.label,
        images: r.data?.images ?? [],
        error: r.error,
        costMicrocents: r.costMicrocents,
      }));

    if (instanceResults.length > 0) {
      const entry: ImageHistoryEntry = {
        id: crypto.randomUUID(),
        prompt,
        options: {
          size: options.size,
          quality: options.quality,
          n: options.n,
        },
        results: instanceResults,
        createdAt: Date.now(),
      };
      addEntry(entry);
      clearResults();
    }

    // Notify on any errors
    const errors = settled.filter((r) => r.status === "error");
    if (errors.length > 0) {
      toast({
        title: "Some models failed",
        description: errors.map((e) => `${e.modelId}: ${e.error}`).join("; "),
        type: "error",
      });
    }
  }, [
    prompt,
    isExecuting,
    instances,
    options,
    execute,
    addEntry,
    clearResults,
    toast,
    availableModels,
  ]);

  return (
    <div className="flex h-full flex-col lg:flex-row">
      {/* Left panel: Controls */}
      <div className="flex w-full flex-col gap-4 border-b p-5 lg:w-[380px] lg:border-b-0 lg:border-r lg:overflow-y-auto">
        {/* Model selector */}
        <div>
          <span className="mb-1.5 block text-xs font-medium text-muted-foreground">Models</span>
          <ModelSelector
            selectedInstances={instances}
            onInstancesChange={setInstances}
            availableModels={(availableModels ?? []) as ModelInfo[]}
            task="images"
          />
        </div>

        <PromptInput
          value={prompt}
          onChange={setPrompt}
          onSubmit={handleSubmit}
          placeholder="Describe the image you want to create..."
          disabled={isExecuting}
          maxLength={32000}
        />

        <Button
          variant="primary"
          className={cn("w-full gap-2", isExecuting && "motion-safe:animate-pulse")}
          onClick={handleSubmit}
          disabled={!prompt.trim() || isExecuting || instances.length === 0}
          isLoading={isExecuting}
        >
          <Wand2 className="h-4 w-4" aria-hidden="true" />
          Generate
        </Button>

        <ImageOptionsForm
          options={options}
          onChange={setOptions}
          disabled={isExecuting}
          instances={instances}
          availableModels={availableModels}
        />
      </div>

      {/* Right panel: Results + Gallery */}
      <div className="flex-1 overflow-y-auto p-5 space-y-6">
        {/* Live results from current execution */}
        <MultiModelResultGrid
          results={results}
          renderResult={(r) => (
            <div className="grid gap-2 grid-cols-1 sm:grid-cols-[repeat(auto-fill,minmax(180px,1fr))]">
              {r.data?.images.map((img, i) => (
                <div key={i} className="overflow-hidden rounded-xl border border-border bg-card">
                  <img
                    src={img.imageData}
                    alt={img.revisedPrompt ?? prompt}
                    className="w-full object-cover"
                    loading="lazy"
                  />
                  {img.revisedPrompt && (
                    <div className="border-t px-3 py-2">
                      <ExpandableCaption text={img.revisedPrompt} />
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        />

        {/* History */}
        <ImageGallery entries={entries} onDelete={removeEntry} />
      </div>
    </div>
  );
}
