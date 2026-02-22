export type StudioTab = "images" | "audio" | "video";

/** Per-instance result within a grouped history entry */
export interface InstanceImageResult {
  instanceId: string;
  modelId: string;
  label?: string;
  images: Array<{ imageData: string; revisedPrompt?: string }>;
  error?: string;
  costMicrocents?: number;
}

export interface ImageHistoryEntry {
  id: string;
  prompt: string;
  options: {
    size?: string;
    quality?: string;
    style?: string;
    outputFormat?: string;
    n?: number;
  };
  results: InstanceImageResult[];
  createdAt: number;
}

/** Per-instance result within a grouped audio history entry */
export interface InstanceAudioResult {
  instanceId: string;
  modelId: string;
  label?: string;
  voice: string;
  /** OPFS filename (e.g. `{entryId}_{instanceId}.mp3`), or `""` if unavailable */
  audioData: string;
  error?: string;
  costMicrocents?: number;
}

export interface AudioHistoryEntry {
  id: string;
  text: string;
  options: {
    speed: number;
    format: string;
  };
  results: InstanceAudioResult[];
  createdAt: number;
}

/** Per-instance result within a grouped transcription history entry */
export interface InstanceTranscriptionResult {
  instanceId: string;
  modelId: string;
  label?: string;
  resultText: string;
  error?: string;
  costMicrocents?: number;
}

export interface TranscriptionHistoryEntry {
  id: string;
  fileName: string;
  fileSize: number;
  mode: "transcribe" | "translate";
  options: {
    language?: string;
    targetLanguage?: string;
    responseFormat: string;
    temperature: number;
  };
  results: InstanceTranscriptionResult[];
  createdAt: number;
}
