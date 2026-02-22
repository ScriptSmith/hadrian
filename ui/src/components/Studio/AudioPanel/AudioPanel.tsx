import { useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import { AudioSpeechPanel } from "@/components/Studio/AudioSpeech/AudioSpeechPanel";
import { TranscriptionPanel } from "@/components/Studio/Transcription/TranscriptionPanel";
import type { AudioMode } from "./AudioModeToggle";
import type { ModelInfo } from "@/components/ModelPicker/model-utils";

const VALID_MODES = new Set<AudioMode>(["speak", "transcribe", "translate"]);

interface AudioPanelProps {
  audioModels?: ModelInfo[];
  transcriptionModels?: ModelInfo[];
  translationModels?: ModelInfo[];
  chatModels?: ModelInfo[];
}

export function AudioPanel({
  audioModels,
  transcriptionModels,
  translationModels,
  chatModels,
}: AudioPanelProps) {
  const [searchParams, setSearchParams] = useSearchParams();
  const rawMode = searchParams.get("mode") as AudioMode | null;
  const activeMode: AudioMode = rawMode && VALID_MODES.has(rawMode) ? rawMode : "speak";

  const handleModeChange = useCallback(
    (mode: AudioMode) => {
      setSearchParams({ tab: "audio", mode }, { replace: true });
    },
    [setSearchParams]
  );

  return (
    <div className="h-full">
      {activeMode === "speak" && (
        <AudioSpeechPanel
          availableModels={audioModels}
          audioMode={activeMode}
          onAudioModeChange={handleModeChange}
        />
      )}
      {activeMode === "transcribe" && (
        <TranscriptionPanel
          mode="transcribe"
          availableModels={transcriptionModels}
          audioMode={activeMode}
          onAudioModeChange={handleModeChange}
        />
      )}
      {activeMode === "translate" && (
        <TranscriptionPanel
          mode="translate"
          availableModels={
            translationModels && translationModels.length > 0
              ? translationModels
              : transcriptionModels
          }
          chatModels={chatModels}
          audioMode={activeMode}
          onAudioModeChange={handleModeChange}
        />
      )}
    </div>
  );
}
