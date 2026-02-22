import { useState, useCallback, useEffect, useMemo, useRef } from "react";
import { Volume2 } from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";
import { useToast } from "@/components/Toast/Toast";
import { PromptInput } from "@/components/Studio/PromptInput/PromptInput";
import { VoiceSelector } from "./VoiceSelector";
import { AudioOutputCard } from "./AudioOutputCard";
import { ModelSelector } from "@/components/ModelSelector/ModelSelector";
import { MultiModelResultGrid } from "@/components/Studio/MultiModelResultGrid/MultiModelResultGrid";
import { AudioPlayer } from "@/components/AudioPlayer/AudioPlayer";
import { AudioModeToggle } from "@/components/Studio/AudioPanel/AudioModeToggle";
import { useAudioHistory } from "@/pages/studio/useStudioHistory";
import {
  useMultiModelExecution,
  extractCostFromResponse,
} from "@/pages/studio/useMultiModelExecution";
import { apiV1AudioSpeech } from "@/api/generated/sdk.gen";
import { usePreferences } from "@/preferences/PreferencesProvider";
import { createDefaultInstance } from "@/components/chat-types";
import { getModelDisplayName } from "@/utils/modelNames";
import { writeAudioFile } from "@/services/opfs/opfsService";
import type { ModelInstance } from "@/components/chat-types";
import type { SpeechResponseFormat } from "@/api/generated/types.gen";
import type { AudioMode } from "@/components/Studio/AudioPanel/AudioModeToggle";
import type { ModelInfo } from "@/components/ModelPicker/model-utils";
import type { AudioHistoryEntry, InstanceAudioResult } from "@/pages/studio/types";

const FORMATS: SpeechResponseFormat[] = ["mp3", "opus", "aac", "flac", "wav"];
const FALLBACK_VOICES = ["alloy", "echo", "fable", "nova", "onyx", "shimmer"];

interface AudioSpeechPanelProps {
  availableModels?: ModelInfo[];
  audioMode: AudioMode;
  onAudioModeChange: (mode: AudioMode) => void;
}

/** Inline audio player that manages its own Audio element */
function InlinePlayer({
  blob,
  format,
  autoPlay = false,
}: {
  blob: Blob;
  format: string;
  autoPlay?: boolean;
}) {
  const audioRef = useRef<{ element: HTMLAudioElement; url: string } | null>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [speed, setSpeed] = useState(1);

  useEffect(() => {
    const url = URL.createObjectURL(blob);
    const el = new Audio(url);
    audioRef.current = { element: el, url };

    el.addEventListener("loadedmetadata", () => setDuration(el.duration));
    el.addEventListener("timeupdate", () => setCurrentTime(el.currentTime));
    el.addEventListener("play", () => setPlaying(true));
    el.addEventListener("pause", () => setPlaying(false));
    el.addEventListener("ended", () => {
      setPlaying(false);
      setCurrentTime(0);
    });
    if (autoPlay) el.play().catch(() => {});

    return () => {
      el.pause();
      el.src = "";
      URL.revokeObjectURL(url);
    };
  }, [blob, autoPlay]);

  const progress = duration > 0 ? (currentTime / duration) * 100 : 0;
  const state = playing ? "playing" : duration > 0 ? "paused" : "idle";

  return (
    <div className="rounded-xl border border-border bg-card p-3">
      <div className="mb-1 flex items-center gap-2 text-xs text-muted-foreground">
        <span className="uppercase">{format}</span>
      </div>
      <AudioPlayer
        state={state}
        currentTime={currentTime}
        duration={duration}
        progress={progress}
        speed={speed}
        onTogglePlayPause={() => {
          if (playing) audioRef.current?.element.pause();
          else audioRef.current?.element.play().catch(() => {});
        }}
        onStop={() => {
          const el = audioRef.current?.element;
          if (el) {
            el.pause();
            el.currentTime = 0;
          }
          setPlaying(false);
          setCurrentTime(0);
        }}
        onSeek={(pos) => {
          const el = audioRef.current?.element;
          if (duration && el) el.currentTime = pos * duration;
        }}
        onSetSpeed={(s) => {
          setSpeed(s);
          const el = audioRef.current?.element;
          if (el) el.playbackRate = s;
        }}
        showStop
      />
    </div>
  );
}

function getVoicesForModel(modelId: string, availableModels: ModelInfo[]): string[] {
  const model = availableModels.find((m) => m.id === modelId);
  return model?.voices?.length ? model.voices : FALLBACK_VOICES;
}

export function AudioSpeechPanel({
  availableModels,
  audioMode,
  onAudioModeChange,
}: AudioSpeechPanelProps) {
  const [text, setText] = useState("");
  const [instructions, setInstructions] = useState("");
  const [showInstructions, setShowInstructions] = useState(false);
  const [voiceMap, setVoiceMap] = useState<Record<string, string[]>>({});
  const [instances, setInstances] = useState<ModelInstance[]>([]);
  const [format, setFormat] = useState<SpeechResponseFormat>("mp3");
  const [speed, setSpeed] = useState(1.0);
  const { entries, addEntry, removeEntry } = useAudioHistory();
  const { toast } = useToast();
  const { isExecuting, results, execute, clearResults } = useMultiModelExecution<Blob>();
  const { preferences } = usePreferences();
  const models = useMemo(() => availableModels ?? [], [availableModels]);

  // Initialize instances from task-specific defaults (once, when models load)
  const hasInitRef = useRef(false);
  useEffect(() => {
    if (hasInitRef.current || !availableModels?.length) return;
    hasInitRef.current = true;
    const defaults = preferences.defaultModels?.tts || [];
    const valid = defaults.filter((m) => availableModels.some((am) => am.id === m));
    if (valid.length > 0) {
      setInstances(valid.map((m) => createDefaultInstance(m)));
    }
  }, [availableModels, preferences.defaultModels]);

  // Keep voiceMap in sync with instances — auto-select first voice for new models, clean up removed
  useEffect(() => {
    setVoiceMap((prev) => {
      const next = { ...prev };
      for (const inst of instances) {
        if (!next[inst.id]?.length) {
          const modelVoices = getVoicesForModel(inst.modelId, models);
          next[inst.id] = [modelVoices[0]];
        }
      }
      // Clean up removed instances
      for (const key of Object.keys(next)) {
        if (!instances.some((i) => i.id === key)) delete next[key];
      }
      return next;
    });
  }, [instances, models]);

  // Count total virtual instances (instances × voices) for auto-play decision
  const totalVirtualInstances = instances.reduce(
    (sum, inst) => sum + (voiceMap[inst.id]?.length || 0),
    0
  );

  const handleSubmit = useCallback(async () => {
    if (!text.trim() || isExecuting || instances.length === 0) return;

    // Expand instances × voices into virtual instances
    const virtualInstances: (ModelInstance & { voice: string })[] = instances.flatMap((inst) => {
      const voices = voiceMap[inst.id] || [];
      return voices.map((v) => ({
        ...inst,
        id: `${inst.id}::${v}`,
        label: `${inst.label || getModelDisplayName(inst.modelId)} — ${v}`,
        voice: v,
      }));
    });

    if (virtualInstances.length === 0) return;

    // Store voice lookup before execute (virtual instances are stable for this call)
    const voiceLookup = new Map(virtualInstances.map((vi) => [vi.id, vi.voice]));

    const settled = await execute(virtualInstances, async (instance) => {
      const modelId = instance.modelId;
      const voice = voiceLookup.get(instance.id) || "alloy";
      const response = await apiV1AudioSpeech({
        body: {
          input: text,
          voice: voice as never,
          model: modelId,
          speed,
          response_format: format,
          instructions:
            models.find((m) => m.id === modelId)?.family === "gpt-4o-mini-tts" && instructions
              ? instructions
              : undefined,
        },
      });
      if (response.error) throw new Error("Speech generation failed");
      return {
        data: response.data as Blob,
        costMicrocents: extractCostFromResponse(response.response),
      };
    });

    // Build grouped history entry — persist audio blobs to OPFS
    const entryId = crypto.randomUUID();
    const instanceResults: InstanceAudioResult[] = [];
    for (const r of settled) {
      const voice = voiceLookup.get(r.instanceId) || "alloy";
      if (r.status === "complete" && r.data) {
        const filename = await writeAudioFile(entryId, r.instanceId, format, r.data);
        instanceResults.push({
          instanceId: r.instanceId,
          modelId: r.modelId,
          label: r.label,
          voice,
          audioData: filename ?? "",
          costMicrocents: r.costMicrocents,
        });
      } else if (r.status === "error") {
        instanceResults.push({
          instanceId: r.instanceId,
          modelId: r.modelId,
          label: r.label,
          voice,
          audioData: "",
          error: r.error,
        });
      }
    }

    if (instanceResults.length > 0) {
      const entry: AudioHistoryEntry = {
        id: entryId,
        text,
        options: { speed, format },
        results: instanceResults,
        createdAt: Date.now(),
      };
      addEntry(entry);
      clearResults();
    }

    const errors = settled.filter((r) => r.status === "error");
    if (errors.length > 0) {
      toast({
        title: "Some models failed",
        description: errors.map((e) => `${e.modelId}: ${e.error}`).join("; "),
        type: "error",
      });
    }
  }, [
    text,
    isExecuting,
    instances,
    voiceMap,
    models,
    speed,
    format,
    instructions,
    execute,
    addEntry,
    clearResults,
    toast,
  ]);

  // Check if any selected model supports instructions
  const supportsInstructions = instances.some((i) => {
    const info = models.find((m) => m.id === i.modelId);
    return info?.family === "gpt-4o-mini-tts";
  });

  return (
    <div className="flex h-full flex-col lg:flex-row">
      {/* Left panel: Controls */}
      <div className="flex w-full flex-col gap-4 border-b p-5 lg:w-[420px] lg:border-b-0 lg:border-r lg:overflow-y-auto">
        <AudioModeToggle value={audioMode} onChange={onAudioModeChange} />

        {/* Text input */}
        <PromptInput
          value={text}
          onChange={setText}
          onSubmit={handleSubmit}
          placeholder="Enter text to convert to speech..."
          disabled={isExecuting}
          maxLength={4096}
          minHeight={100}
          maxHeight={180}
        />

        {/* Options */}
        <div className="space-y-3">
          {/* Model selector */}
          <div>
            <span className="mb-1.5 block text-xs font-medium text-muted-foreground">Models</span>
            <ModelSelector
              selectedInstances={instances}
              onInstancesChange={setInstances}
              availableModels={(availableModels ?? []) as ModelInfo[]}
              task="tts"
            />
          </div>

          {/* Voice selector */}
          <VoiceSelector
            instances={instances}
            availableModels={models}
            voiceMap={voiceMap}
            onChange={setVoiceMap}
            disabled={isExecuting}
          />

          {/* Format */}
          <div>
            <label
              htmlFor="tts-format"
              className="mb-1.5 block text-xs font-medium text-muted-foreground"
            >
              Format
            </label>
            <select
              id="tts-format"
              value={format}
              onChange={(e) => setFormat(e.target.value as SpeechResponseFormat)}
              disabled={isExecuting}
              className="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
            >
              {FORMATS.map((f) => (
                <option key={f} value={f}>
                  {f.toUpperCase()}
                </option>
              ))}
            </select>
          </div>

          {/* Speed */}
          <div>
            <label
              htmlFor="tts-speed"
              className="mb-1.5 block text-xs font-medium text-muted-foreground"
            >
              Speed: {speed.toFixed(2)}x
            </label>
            <input
              id="tts-speed"
              type="range"
              min={0.25}
              max={4.0}
              step={0.25}
              value={speed}
              onChange={(e) => setSpeed(parseFloat(e.target.value))}
              disabled={isExecuting}
              className="w-full accent-primary"
              aria-label="Playback speed"
            />
          </div>

          {/* Voice instructions (collapsible) */}
          {supportsInstructions && (
            <div>
              <button
                type="button"
                className="text-xs font-medium text-muted-foreground hover:text-foreground motion-safe:transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded"
                onClick={() => setShowInstructions(!showInstructions)}
              >
                {showInstructions ? "Hide" : "Show"} voice instructions
              </button>
              <div
                className={cn(
                  "overflow-hidden motion-safe:transition-all motion-safe:duration-300",
                  showInstructions ? "mt-2 max-h-40 opacity-100" : "max-h-0 opacity-0"
                )}
              >
                <textarea
                  value={instructions}
                  onChange={(e) => setInstructions(e.target.value)}
                  placeholder="Instructions to control the voice style..."
                  className="w-full resize-none rounded-lg border border-input bg-muted/30 px-3 py-2 text-sm placeholder:text-muted-foreground/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  rows={2}
                  disabled={isExecuting}
                  aria-label="Voice instructions"
                />
              </div>
            </div>
          )}
        </div>

        {/* Generate button */}
        <Button
          variant="primary"
          className={cn("w-full gap-2", isExecuting && "motion-safe:animate-pulse")}
          onClick={handleSubmit}
          disabled={!text.trim() || isExecuting || instances.length === 0}
          isLoading={isExecuting}
        >
          <Volume2 className="h-4 w-4" aria-hidden="true" />
          Speak
        </Button>
      </div>

      {/* Right panel: Results */}
      <div className="flex-1 overflow-y-auto p-5">
        {/* Live results */}
        <MultiModelResultGrid
          results={results}
          renderResult={(r) =>
            r.data ? (
              <InlinePlayer blob={r.data} format={format} autoPlay={totalVirtualInstances <= 1} />
            ) : null
          }
        />

        {/* Previous results */}
        {entries.length > 0 && (
          <div className={cn("space-y-2", results.size > 0 && "mt-6")}>
            {entries.map((entry) => (
              <AudioOutputCard key={entry.id} entry={entry} onDelete={removeEntry} />
            ))}
          </div>
        )}

        {/* Empty state */}
        {entries.length === 0 && results.size === 0 && !isExecuting && (
          <div className="flex h-full flex-col items-center justify-center py-16">
            <div className="mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-muted/50">
              <Volume2 className="h-8 w-8 text-muted-foreground/50" />
            </div>
            <h3 className="text-base font-medium text-foreground">Text to Speech</h3>
            <p className="mt-1 max-w-xs text-center text-sm text-muted-foreground">
              Enter text and choose a voice to generate speech
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
