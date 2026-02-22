import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FileAudio } from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";
import { useToast } from "@/components/Toast/Toast";
import { formatCost } from "@/utils/formatters";
import { AudioDropZone } from "./AudioDropZone";
import { TranscriptionResult } from "./TranscriptionResult";
import { ModelSelector } from "@/components/ModelSelector/ModelSelector";
import { MultiModelResultGrid } from "@/components/Studio/MultiModelResultGrid/MultiModelResultGrid";
import { AudioModeToggle } from "@/components/Studio/AudioPanel/AudioModeToggle";
import { useTranscriptionHistory } from "@/pages/studio/useStudioHistory";
import {
  useMultiModelExecution,
  extractCostFromResponse,
} from "@/pages/studio/useMultiModelExecution";
import {
  apiV1AudioTranscriptions,
  apiV1AudioTranslations,
  apiV1ChatCompletions,
} from "@/api/generated/sdk.gen";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { createDefaultInstance } from "@/components/chat-types";
import type { ModelInstance } from "@/components/chat-types";
import type { AudioResponseFormat } from "@/api/generated/types.gen";
import type { AudioMode } from "@/components/Studio/AudioPanel/AudioModeToggle";
import type { ModelInfo } from "@/components/ModelPicker/model-utils";
import type { TranscriptionHistoryEntry, InstanceTranscriptionResult } from "@/pages/studio/types";

type TranscriptionMode = "transcribe" | "translate";

const FORMATS: AudioResponseFormat[] = ["text", "json", "verbose_json", "srt", "vtt"];

const LANGUAGES = [
  { code: "auto", label: "Auto-detect" },
  { code: "en", label: "English" },
  { code: "es", label: "Spanish" },
  { code: "fr", label: "French" },
  { code: "de", label: "German" },
  { code: "it", label: "Italian" },
  { code: "pt", label: "Portuguese" },
  { code: "nl", label: "Dutch" },
  { code: "ru", label: "Russian" },
  { code: "zh", label: "Chinese" },
  { code: "ja", label: "Japanese" },
  { code: "ko", label: "Korean" },
  { code: "ar", label: "Arabic" },
  { code: "hi", label: "Hindi" },
  { code: "tr", label: "Turkish" },
  { code: "pl", label: "Polish" },
  { code: "sv", label: "Swedish" },
  { code: "da", label: "Danish" },
  { code: "fi", label: "Finnish" },
  { code: "uk", label: "Ukrainian" },
  { code: "th", label: "Thai" },
  { code: "vi", label: "Vietnamese" },
  { code: "id", label: "Indonesian" },
] as const;

interface TranscriptionPanelProps {
  mode: TranscriptionMode;
  availableModels?: ModelInfo[];
  /** Chat-capable models for the text translation step (non-English targets) */
  chatModels?: ModelInfo[];
  audioMode: AudioMode;
  onAudioModeChange: (mode: AudioMode) => void;
}

export function TranscriptionPanel({
  mode,
  availableModels,
  chatModels,
  audioMode,
  onAudioModeChange,
}: TranscriptionPanelProps) {
  const task = mode === "translate" ? ("translation" as const) : ("transcription" as const);
  const [file, setFile] = useState<File | null>(null);
  const [instances, setInstances] = useState<ModelInstance[]>([]);
  const [language, setLanguage] = useState("");
  const [sourceLanguage, setSourceLanguage] = useState("auto");
  const [targetLanguage, setTargetLanguage] = useState("en");
  const [responseFormat, setResponseFormat] = useState<AudioResponseFormat>("text");
  const [temperature, setTemperature] = useState(0);
  const [prompt, setPrompt] = useState("");
  const [showPrompt, setShowPrompt] = useState(false);
  const [translationInstances, setTranslationInstances] = useState<ModelInstance[]>([]);
  const { entries: allEntries, addEntry, removeEntry } = useTranscriptionHistory();
  const entries = useMemo(() => allEntries.filter((e) => e.mode === mode), [allEntries, mode]);
  const { toast } = useToast();
  const { isExecuting, results, execute, clearResults } = useMultiModelExecution<string>();
  const { preferences } = usePreferences();

  // Initialize instances from task-specific defaults (once, when models load)
  const hasInitRef = useRef(false);
  useEffect(() => {
    if (hasInitRef.current || !availableModels?.length) return;
    hasInitRef.current = true;
    const defaults = preferences.defaultModels?.[task] || [];
    const valid = defaults.filter((m) => availableModels.some((am) => am.id === m));
    if (valid.length > 0) {
      setInstances(valid.map((m) => createDefaultInstance(m)));
    }
  }, [availableModels, preferences.defaultModels, task]);

  const targetLanguageOptions = useMemo(() => LANGUAGES.filter((l) => l.code !== "auto"), []);

  const selectClass =
    "w-full rounded-lg border border-input bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50";

  /** Extract text from API response */
  const extractText = (data: unknown): string =>
    typeof data === "string"
      ? data
      : ((data as { text?: string })?.text ?? JSON.stringify(data, null, 2));

  // For non-English translation, determine which instances drive the results grid
  const isNonEnglishTranslation = mode === "translate" && targetLanguage !== "en";
  const effectiveInstances = isNonEnglishTranslation ? translationInstances : instances;

  const handleSubmit = useCallback(async () => {
    if (!file || isExecuting || instances.length === 0) return;
    if (isNonEnglishTranslation && translationInstances.length === 0) return;

    const settled = await execute(effectiveInstances, async (instance) => {
      const modelId = instance.modelId;

      if (mode === "transcribe") {
        const response = await apiV1AudioTranscriptions({
          body: {
            file,
            model: modelId,
            language: language || undefined,
            response_format: responseFormat,
            temperature: temperature || undefined,
            prompt: prompt || undefined,
          } as never,
        });
        if (response.error) throw new Error("Transcription failed");
        return {
          data: extractText(response.data),
          costMicrocents: extractCostFromResponse(response.response),
        };
      }

      // Translate mode — English target: audio translation API
      if (targetLanguage === "en") {
        const response = await apiV1AudioTranslations({
          body: {
            file,
            model: modelId,
            response_format: responseFormat,
            temperature: temperature || undefined,
            prompt: prompt || undefined,
          } as never,
        });
        if (response.error) throw new Error("Translation failed");
        return {
          data: extractText(response.data),
          costMicrocents: extractCostFromResponse(response.response),
        };
      }

      // Non-English target: transcribe with first audio model, translate with this instance
      const transcribeModel = instances[0].modelId;
      const transcribeResponse = await apiV1AudioTranscriptions({
        body: {
          file,
          model: transcribeModel,
          language: sourceLanguage !== "auto" ? sourceLanguage : undefined,
          response_format: "text",
          temperature: temperature || undefined,
          prompt: prompt || undefined,
        } as never,
      });
      if (transcribeResponse.error) throw new Error("Transcription failed");
      const transcribedText = extractText(transcribeResponse.data);

      const targetLabel = LANGUAGES.find((l) => l.code === targetLanguage)?.label ?? targetLanguage;

      const chatResponse = await apiV1ChatCompletions({
        body: {
          model: modelId,
          messages: [
            {
              role: "system",
              content: `Translate the following text to ${targetLabel}. Preserve formatting. Output only the translation.`,
            },
            { role: "user", content: transcribedText },
          ],
        },
      });
      if (chatResponse.error) throw new Error("Translation failed");
      const choices = (chatResponse.data as { choices?: { message?: { content?: string } }[] })
        ?.choices;
      // Sum cost from both transcription and chat translation steps
      const transcriptionCost = extractCostFromResponse(transcribeResponse.response);
      const chatCost = extractCostFromResponse(chatResponse.response);
      const totalCost =
        transcriptionCost != null || chatCost != null
          ? (transcriptionCost ?? 0) + (chatCost ?? 0)
          : undefined;
      return {
        data: choices?.[0]?.message?.content ?? transcribedText,
        costMicrocents: totalCost,
      };
    });

    // Build grouped history entry
    const instanceResults: InstanceTranscriptionResult[] = settled
      .filter((r) => r.status === "complete" || r.status === "error")
      .map((r) => ({
        instanceId: r.instanceId,
        modelId: r.modelId,
        label: r.label,
        resultText: r.data ?? "",
        error: r.error,
        costMicrocents: r.costMicrocents,
      }));

    if (instanceResults.length > 0) {
      const entry: TranscriptionHistoryEntry = {
        id: crypto.randomUUID(),
        fileName: file.name,
        fileSize: file.size,
        mode,
        options: {
          language:
            mode === "transcribe"
              ? language || undefined
              : sourceLanguage !== "auto"
                ? sourceLanguage
                : undefined,
          targetLanguage: mode === "translate" ? targetLanguage : undefined,
          responseFormat,
          temperature,
        },
        results: instanceResults,
        createdAt: Date.now(),
      };
      addEntry(entry);
      clearResults();
    }

    const errors = settled.filter((r) => r.status === "error");
    if (errors.length > 0) {
      toast({
        title: `Some models failed`,
        description: errors.map((e) => `${e.modelId}: ${e.error}`).join("; "),
        type: "error",
      });
    }
  }, [
    file,
    isExecuting,
    instances,
    translationInstances,
    effectiveInstances,
    isNonEnglishTranslation,
    mode,
    language,
    sourceLanguage,
    targetLanguage,
    responseFormat,
    temperature,
    prompt,
    execute,
    addEntry,
    clearResults,
    toast,
  ]);

  return (
    <div className="flex h-full flex-col lg:flex-row">
      {/* Left panel: Controls */}
      <div className="flex w-full flex-col gap-4 border-b p-5 lg:w-[420px] lg:border-b-0 lg:border-r lg:overflow-y-auto">
        <AudioModeToggle value={audioMode} onChange={onAudioModeChange} />

        {/* Audio upload */}
        <AudioDropZone file={file} onFileChange={setFile} disabled={isExecuting} />

        {/* Options */}
        <div className="space-y-3">
          {/* Model selector */}
          <div>
            <span className="mb-1.5 block text-xs font-medium text-muted-foreground">
              {isNonEnglishTranslation ? "Transcription Model" : "Models"}
            </span>
            <ModelSelector
              selectedInstances={instances}
              onInstancesChange={setInstances}
              availableModels={(availableModels ?? []) as ModelInfo[]}
              task={task}
              maxModels={isNonEnglishTranslation ? 1 : undefined}
            />
          </div>

          {/* Language controls */}
          {mode === "transcribe" ? (
            <div>
              <label
                htmlFor="trans-lang"
                className="mb-1.5 block text-xs font-medium text-muted-foreground"
              >
                Language (optional)
              </label>
              <input
                id="trans-lang"
                type="text"
                value={language}
                onChange={(e) => setLanguage(e.target.value)}
                placeholder="e.g. en, fr, de..."
                disabled={isExecuting}
                className="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm placeholder:text-muted-foreground/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
              />
            </div>
          ) : (
            <>
              <div>
                <label
                  htmlFor="trans-source-lang"
                  className="mb-1.5 block text-xs font-medium text-muted-foreground"
                >
                  Source Language
                </label>
                <select
                  id="trans-source-lang"
                  value={sourceLanguage}
                  onChange={(e) => setSourceLanguage(e.target.value)}
                  disabled={isExecuting}
                  className={selectClass}
                >
                  {LANGUAGES.map((l) => (
                    <option key={l.code} value={l.code}>
                      {l.label}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label
                  htmlFor="trans-target-lang"
                  className="mb-1.5 block text-xs font-medium text-muted-foreground"
                >
                  Target Language
                </label>
                <select
                  id="trans-target-lang"
                  value={targetLanguage}
                  onChange={(e) => setTargetLanguage(e.target.value)}
                  disabled={isExecuting}
                  className={selectClass}
                >
                  {targetLanguageOptions.map((l) => (
                    <option key={l.code} value={l.code}>
                      {l.label}
                    </option>
                  ))}
                </select>
              </div>
              {targetLanguage !== "en" && chatModels && chatModels.length > 0 && (
                <div>
                  <span className="mb-1.5 block text-xs font-medium text-muted-foreground">
                    Translation Models
                  </span>
                  <ModelSelector
                    selectedInstances={translationInstances}
                    onInstancesChange={setTranslationInstances}
                    availableModels={chatModels as ModelInfo[]}
                  />
                </div>
              )}
            </>
          )}

          <div>
            <label
              htmlFor="trans-format"
              className="mb-1.5 block text-xs font-medium text-muted-foreground"
            >
              Output Format
            </label>
            <select
              id="trans-format"
              value={responseFormat}
              onChange={(e) => setResponseFormat(e.target.value as AudioResponseFormat)}
              disabled={isExecuting}
              className={selectClass}
            >
              {FORMATS.map((f) => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label
              htmlFor="trans-temp"
              className="mb-1.5 block text-xs font-medium text-muted-foreground"
            >
              Temperature: {temperature.toFixed(1)}
            </label>
            <input
              id="trans-temp"
              type="range"
              min={0}
              max={1}
              step={0.1}
              value={temperature}
              onChange={(e) => setTemperature(parseFloat(e.target.value))}
              disabled={isExecuting}
              className="w-full accent-primary"
            />
          </div>

          {/* Optional prompt */}
          <div>
            <button
              type="button"
              className="text-xs font-medium text-muted-foreground hover:text-foreground motion-safe:transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded"
              onClick={() => setShowPrompt(!showPrompt)}
            >
              {showPrompt ? "Hide" : "Show"} prompt
            </button>
            <div
              className={cn(
                "overflow-hidden motion-safe:transition-all motion-safe:duration-300",
                showPrompt ? "mt-2 max-h-40 opacity-100" : "max-h-0 opacity-0"
              )}
            >
              <textarea
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder="Optional text to guide the model..."
                className="w-full resize-none rounded-lg border border-input bg-background px-3 py-2 text-sm placeholder:text-muted-foreground/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
                rows={2}
                disabled={isExecuting}
                aria-label="Guidance prompt"
              />
            </div>
          </div>
        </div>

        {/* Submit */}
        <Button
          variant="primary"
          className={cn("w-full gap-2", isExecuting && "motion-safe:animate-pulse")}
          onClick={handleSubmit}
          disabled={
            !file ||
            isExecuting ||
            instances.length === 0 ||
            (isNonEnglishTranslation && translationInstances.length === 0)
          }
          isLoading={isExecuting}
        >
          <FileAudio className="h-4 w-4" aria-hidden="true" />
          {mode === "transcribe" ? "Transcribe" : "Translate"}
        </Button>
      </div>

      {/* Right panel: Result */}
      <div className="flex-1 overflow-y-auto p-5">
        {/* Live multi-model results */}
        <MultiModelResultGrid
          results={results}
          renderResult={(r) => <TranscriptionResult text={r.data ?? ""} format={responseFormat} />}
        />

        {/* Previous results */}
        {entries.length > 0 && (
          <div className={cn("space-y-4", results.size > 0 && "mt-6")}>
            {entries.map((entry) => (
              <div
                key={entry.id}
                className="space-y-3 rounded-xl border border-border bg-card/50 p-4"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium text-foreground">{entry.fileName}</p>
                    <div className="flex items-center gap-2">
                      <span className="text-[10px] text-muted-foreground">
                        {new Date(entry.createdAt).toLocaleTimeString(undefined, {
                          hour: "2-digit",
                          minute: "2-digit",
                        })}
                      </span>
                      {(() => {
                        const totalCost = entry.results.reduce(
                          (sum, r) => sum + (r.costMicrocents ?? 0),
                          0
                        );
                        return totalCost > 0 ? (
                          <span className="text-[10px] text-muted-foreground">
                            {formatCost(totalCost / 1_000_000)}
                          </span>
                        ) : null;
                      })()}
                    </div>
                  </div>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7 shrink-0 text-muted-foreground hover:text-destructive"
                    onClick={() => removeEntry(entry.id)}
                    aria-label="Delete"
                  >
                    <span className="text-xs">&times;</span>
                  </Button>
                </div>
                {entry.results.map((r) => (
                  <div key={r.instanceId}>
                    {entry.results.length > 1 && (
                      <div className="mb-1 flex items-center gap-1.5 text-[10px] font-medium text-muted-foreground">
                        <span>{r.label || r.modelId}</span>
                        {r.costMicrocents != null && r.costMicrocents > 0 && (
                          <>
                            <span className="text-muted-foreground/50">&middot;</span>
                            <span className="font-normal">
                              {formatCost(r.costMicrocents / 1_000_000)}
                            </span>
                          </>
                        )}
                      </div>
                    )}
                    {r.error ? (
                      <p className="text-xs text-destructive">{r.error}</p>
                    ) : (
                      <TranscriptionResult
                        text={r.resultText}
                        format={entry.options.responseFormat}
                      />
                    )}
                  </div>
                ))}
              </div>
            ))}
          </div>
        )}

        {/* Empty state */}
        {results.size === 0 && entries.length === 0 && (
          <div className="flex h-full flex-col items-center justify-center py-16">
            <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-muted/50">
              <FileAudio className="h-8 w-8 text-muted-foreground/50" />
            </div>
            <h3 className="text-base font-medium text-foreground">
              {mode === "transcribe" ? "Transcribe Audio" : "Translate Audio"}
            </h3>
            <p className="mt-1 max-w-xs text-center text-sm text-muted-foreground">
              {mode === "transcribe"
                ? "Upload an audio file to convert speech to text"
                : "Upload an audio file and translate to any language"}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
