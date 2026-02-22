import { useState, useCallback, useRef, useMemo, useEffect } from "react";
import { Upload, X, Wand2 } from "lucide-react";
import { useMutation } from "@tanstack/react-query";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";
import { useToast } from "@/components/Toast/Toast";
import { PromptInput } from "@/components/Studio/PromptInput/PromptInput";
import { ImageGallery } from "@/components/Studio/ImageGeneration/ImageGallery";
import { useImageHistory } from "@/pages/studio/useStudioHistory";
import { apiV1ImagesEdits, apiV1ImagesVariations } from "@/api/generated/sdk.gen";
import { extractCostFromResponse } from "@/pages/studio/useMultiModelExecution";
import type { ImageHistoryEntry } from "@/pages/studio/types";

type SubMode = "edit" | "variations";

function FileUploadZone({
  file,
  onFileChange,
  label,
  accept,
  disabled,
}: {
  file: File | null;
  onFileChange: (f: File | null) => void;
  label: string;
  accept: string;
  disabled?: boolean;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragOver, setDragOver] = useState(false);

  const previewUrl = useMemo(() => (file ? URL.createObjectURL(file) : null), [file]);
  useEffect(() => {
    return () => {
      if (previewUrl) URL.revokeObjectURL(previewUrl);
    };
  }, [previewUrl]);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const f = e.dataTransfer.files[0];
      if (f) onFileChange(f);
    },
    [onFileChange]
  );

  return (
    <div>
      <span className="mb-1.5 block text-xs font-medium text-muted-foreground">{label}</span>
      {file ? (
        <div className="relative overflow-hidden rounded-xl border border-border">
          <img
            src={previewUrl ?? undefined}
            alt={`${label} preview`}
            className="h-40 w-full object-cover"
          />
          <Button
            variant="ghost"
            size="icon"
            className="absolute right-2 top-2 h-7 w-7 bg-black/40 text-white hover:bg-black/60"
            onClick={() => onFileChange(null)}
            aria-label={`Remove ${label.toLowerCase()}`}
          >
            <X className="h-4 w-4" />
          </Button>
        </div>
      ) : (
        <button
          type="button"
          disabled={disabled}
          className={cn(
            "flex w-full flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed p-6",
            "text-sm text-muted-foreground",
            "motion-safe:transition-colors motion-safe:duration-200",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
            dragOver
              ? "border-primary bg-primary/5"
              : "border-border hover:border-primary/50 hover:bg-muted/30",
            disabled && "cursor-not-allowed opacity-50"
          )}
          onClick={() => inputRef.current?.click()}
          onDragOver={(e) => {
            e.preventDefault();
            setDragOver(true);
          }}
          onDragLeave={() => setDragOver(false)}
          onDrop={handleDrop}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              inputRef.current?.click();
            }
          }}
        >
          <Upload className="h-6 w-6" />
          <span>Drop image or click to browse</span>
        </button>
      )}
      <input
        ref={inputRef}
        type="file"
        accept={accept}
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) onFileChange(f);
          e.target.value = "";
        }}
        aria-label={label}
      />
    </div>
  );
}

export function ImageEditingPanel() {
  const [subMode, setSubMode] = useState<SubMode>("edit");
  const [prompt, setPrompt] = useState("");
  const [sourceFile, setSourceFile] = useState<File | null>(null);
  const [maskFile, setMaskFile] = useState<File | null>(null);
  const [count, setCount] = useState(1);
  const { entries, addEntry, removeEntry } = useImageHistory();
  const { toast } = useToast();

  const editMutation = useMutation({
    mutationFn: async () => {
      if (!sourceFile) throw new Error("Source image is required");

      if (subMode === "edit") {
        const body: Record<string, unknown> = {
          prompt,
          image: sourceFile,
          n: count,
          response_format: "b64_json",
        };
        if (maskFile) body.mask = maskFile;

        const response = await apiV1ImagesEdits({
          body: body as never,
        });
        if (response.error) throw new Error("Image edit failed");
        return {
          data: response.data,
          costMicrocents: extractCostFromResponse(response.response),
        };
      } else {
        const response = await apiV1ImagesVariations({
          body: {
            image: sourceFile,
            n: count,
            response_format: "b64_json",
          } as never,
        });
        if (response.error) throw new Error("Image variation failed");
        return {
          data: response.data,
          costMicrocents: extractCostFromResponse(response.response),
        };
      }
    },
    onSuccess: ({ data, costMicrocents }) => {
      const imgs = data?.data ?? [];
      const images = imgs.map((img) => ({
        imageData: img.b64_json ? `data:image/png;base64,${img.b64_json}` : (img.url ?? ""),
        revisedPrompt: img.revised_prompt ?? undefined,
      }));

      const entry: ImageHistoryEntry = {
        id: crypto.randomUUID(),
        prompt: subMode === "edit" ? prompt : "(variation)",
        options: {},
        results: [
          {
            instanceId: "default",
            modelId: "default",
            images,
            costMicrocents,
          },
        ],
        createdAt: Date.now(),
      };
      addEntry(entry);
    },
    onError: (err) => {
      toast({
        title: `Image ${subMode} failed`,
        description: err instanceof Error ? err.message : "An error occurred",
        type: "error",
      });
    },
  });

  const handleSubmit = useCallback(() => {
    if (!sourceFile || editMutation.isPending) return;
    if (subMode === "edit" && !prompt.trim()) return;
    editMutation.mutate();
  }, [sourceFile, prompt, subMode, editMutation]);

  return (
    <div className="flex h-full flex-col lg:flex-row">
      {/* Left panel */}
      <div className="flex w-full flex-col gap-4 border-b p-5 lg:w-[380px] lg:border-b-0 lg:border-r lg:overflow-y-auto">
        {/* Sub-mode toggle */}
        <div
          className="flex gap-1 rounded-lg bg-muted/50 p-1"
          role="radiogroup"
          aria-label="Edit mode"
        >
          {(["edit", "variations"] as const).map((mode) => (
            <button
              key={mode}
              type="button"
              role="radio"
              aria-checked={subMode === mode}
              className={cn(
                "flex-1 rounded-md px-3 py-1.5 text-sm font-medium capitalize",
                "motion-safe:transition-colors motion-safe:duration-150",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                subMode === mode
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              )}
              onClick={() => setSubMode(mode)}
            >
              {mode === "edit" ? "Edit" : "Variations"}
            </button>
          ))}
        </div>

        <FileUploadZone
          file={sourceFile}
          onFileChange={setSourceFile}
          label="Source Image"
          accept="image/png,image/jpeg,image/webp"
          disabled={editMutation.isPending}
        />

        {subMode === "edit" && (
          <>
            <FileUploadZone
              file={maskFile}
              onFileChange={setMaskFile}
              label="Mask (optional)"
              accept="image/png"
              disabled={editMutation.isPending}
            />
            <PromptInput
              value={prompt}
              onChange={setPrompt}
              onSubmit={handleSubmit}
              placeholder="Describe the edit..."
              disabled={editMutation.isPending}
              maxLength={1000}
              minHeight={60}
            />
          </>
        )}

        {/* Count */}
        <div>
          <label
            htmlFor="edit-count"
            className="mb-1.5 block text-xs font-medium text-muted-foreground"
          >
            Count
          </label>
          <div className="flex items-center gap-2">
            <button
              type="button"
              disabled={editMutation.isPending || count <= 1}
              className="flex h-8 w-8 items-center justify-center rounded-lg border border-input text-sm font-medium hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onClick={() => setCount(Math.max(1, count - 1))}
              aria-label="Decrease count"
            >
              -
            </button>
            <span id="edit-count" className="w-8 text-center text-sm font-medium tabular-nums">
              {count}
            </span>
            <button
              type="button"
              disabled={editMutation.isPending || count >= 4}
              className="flex h-8 w-8 items-center justify-center rounded-lg border border-input text-sm font-medium hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onClick={() => setCount(Math.min(4, count + 1))}
              aria-label="Increase count"
            >
              +
            </button>
          </div>
        </div>

        <Button
          variant="primary"
          className={cn("w-full gap-2", editMutation.isPending && "motion-safe:animate-pulse")}
          onClick={handleSubmit}
          disabled={!sourceFile || (subMode === "edit" && !prompt.trim()) || editMutation.isPending}
          isLoading={editMutation.isPending}
        >
          <Wand2 className="h-4 w-4" aria-hidden="true" />
          {subMode === "edit" ? "Edit" : "Generate Variations"}
        </Button>
      </div>

      {/* Right panel: Gallery */}
      <div className="flex-1 overflow-y-auto p-5">
        <ImageGallery entries={entries} onDelete={removeEntry} />
      </div>
    </div>
  );
}
