import { cn } from "@/utils/cn";
import { getModelDisplayName } from "@/utils/modelNames";
import type { ModelInstance } from "@/components/chat-types";
import type { ModelInfo } from "@/components/ModelPicker/model-utils";

const FALLBACK_VOICES = ["alloy", "echo", "fable", "nova", "onyx", "shimmer"];

interface VoiceSelectorProps {
  instances: ModelInstance[];
  availableModels: ModelInfo[];
  voiceMap: Record<string, string[]>;
  onChange: (voiceMap: Record<string, string[]>) => void;
  disabled?: boolean;
}

function getVoicesForInstance(instance: ModelInstance, availableModels: ModelInfo[]): string[] {
  const model = availableModels.find((m) => m.id === instance.modelId);
  return model?.voices?.length ? model.voices : FALLBACK_VOICES;
}

export function VoiceSelector({
  instances,
  availableModels,
  voiceMap,
  onChange,
  disabled,
}: VoiceSelectorProps) {
  if (instances.length === 0) {
    return (
      <div>
        <span className="mb-1.5 block text-xs font-medium text-muted-foreground">Voices</span>
        <p className="text-sm text-muted-foreground">Select a model to see available voices</p>
      </div>
    );
  }

  const toggleVoice = (instanceId: string, voice: string, voices: string[]) => {
    const current = voiceMap[instanceId] || [];
    const isSelected = current.includes(voice);
    let next: string[];
    if (isSelected) {
      // Don't allow deselecting the last voice
      if (current.length <= 1) return;
      next = current.filter((v) => v !== voice);
    } else {
      // Maintain original order from available voices
      next = voices.filter((v) => current.includes(v) || v === voice);
    }
    onChange({ ...voiceMap, [instanceId]: next });
  };

  const showModelLabels = instances.length > 1;

  return (
    <div className="space-y-2.5">
      <span className="block text-xs font-medium text-muted-foreground">Voices</span>
      {instances.map((inst) => {
        const voices = getVoicesForInstance(inst, availableModels);
        const selected = voiceMap[inst.id] || [];

        return (
          <div key={inst.id}>
            {showModelLabels && (
              <span className="mb-1 block text-[10px] font-medium text-muted-foreground">
                {inst.label || getModelDisplayName(inst.modelId)}
              </span>
            )}
            <div
              className="flex flex-wrap gap-1.5"
              role="group"
              aria-label={`Voice selection for ${inst.label || getModelDisplayName(inst.modelId)}`}
            >
              {voices.map((v) => {
                const isSelected = selected.includes(v);
                return (
                  <button
                    key={v}
                    type="button"
                    role="checkbox"
                    aria-checked={isSelected}
                    disabled={disabled}
                    className={cn(
                      "shrink-0 rounded-full px-3 py-1 text-xs font-medium capitalize",
                      "motion-safe:transition-all motion-safe:duration-200",
                      "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
                      "disabled:cursor-not-allowed disabled:opacity-50",
                      isSelected
                        ? "bg-primary text-primary-foreground shadow-sm"
                        : "bg-muted/60 text-muted-foreground hover:bg-muted hover:text-foreground"
                    )}
                    onClick={() => toggleVoice(inst.id, v, voices)}
                  >
                    {v}
                  </button>
                );
              })}
            </div>
          </div>
        );
      })}
    </div>
  );
}
