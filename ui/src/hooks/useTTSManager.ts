import { useCallback, useEffect, useRef } from "react";

import { apiV1AudioSpeech } from "@/api/generated/sdk.gen";
import type { Voice } from "@/api/generated/types.gen";
import {
  useChatUIStore,
  useTTSStateForResponse,
  useTTSVoice,
  useTTSSpeed,
} from "@/stores/chatUIStore";
import type { PlaybackState } from "@/hooks/useAudioPlayback";
import { DEFAULT_TTS_MODEL, MIN_TTS_SPEED, MAX_TTS_SPEED } from "@/hooks/useAudioPlayback";
import { isBrowserTTSAvailable } from "@/hooks/useBrowserTTS";

/** Options for TTS speech generation */
export interface TTSManagerOptions {
  voice?: Voice;
  speed?: number;
  model?: string;
}

/** Return type for useTTSManager hook at the global level */
export interface TTSManagerReturn {
  /** Speak text for a specific response, auto-stopping any other playing audio */
  speak: (
    text: string,
    groupId: string,
    instanceId: string,
    options?: TTSManagerOptions
  ) => Promise<void>;
  /** Stop any currently playing audio */
  stop: () => void;
  /** Set playback speed */
  setSpeed: (speed: number) => void;
  /** Current playback speed */
  speed: number;
}

/** Return type for useTTSForResponse hook at the response level */
export interface TTSResponseReturn {
  /** Current playback state for this response */
  state: PlaybackState;
  /** Speak this response's content */
  speak: () => Promise<void>;
  /** Stop playback (only works if this response is active) */
  stop: () => void;
}

// Global audio element and state (module-level singleton)
let globalAudio: HTMLAudioElement | null = null;
let globalBlobUrl: string | null = null;
let usingBrowserTTS = false;

/** Cleanup the global audio resources */
function cleanup() {
  // Cleanup Gateway TTS audio
  if (globalAudio) {
    globalAudio.pause();
    globalAudio.src = "";
  }
  if (globalBlobUrl) {
    URL.revokeObjectURL(globalBlobUrl);
    globalBlobUrl = null;
  }
  // Cleanup browser TTS
  if (usingBrowserTTS && isBrowserTTSAvailable()) {
    window.speechSynthesis.cancel();
    usingBrowserTTS = false;
  }
}

/** Get or create the global audio element */
function getAudio(): HTMLAudioElement {
  if (!globalAudio) {
    globalAudio = new Audio();
  }
  return globalAudio;
}

/** Map OpenAI speed (0.25-4.0) to browser rate (0.1-10) */
function mapSpeedToBrowser(openAISpeed: number): number {
  const clampedSpeed = Math.max(MIN_TTS_SPEED, Math.min(MAX_TTS_SPEED, openAISpeed));
  if (clampedSpeed <= 1) {
    // Map 0.25-1.0 to 0.5-1.0
    return 0.5 + (clampedSpeed - 0.25) * (0.5 / 0.75);
  } else {
    // Map 1.0-4.0 to 1.0-3.0
    return 1 + (clampedSpeed - 1) * (2 / 3);
  }
}

/** Find best matching browser voice */
function findBrowserVoice(): SpeechSynthesisVoice | null {
  if (!isBrowserTTSAvailable()) return null;
  const voices = window.speechSynthesis.getVoices();
  if (voices.length === 0) return null;

  // Prefer English voices
  const englishDefault = voices.find((v) => v.lang.startsWith("en") && v.default);
  if (englishDefault) return englishDefault;

  const anyEnglish = voices.find((v) => v.lang.startsWith("en"));
  if (anyEnglish) return anyEnglish;

  return voices.find((v) => v.default) || voices[0] || null;
}

/**
 * Global TTS manager hook.
 *
 * Provides a single audio instance that can be used across all responses.
 * When a new response starts speaking, the previous one is automatically stopped.
 *
 * Use this at the conversation/chat level to provide TTS callbacks to child components.
 *
 * @example
 * ```tsx
 * const { speak, stop } = useTTSManager();
 *
 * // In a child component
 * const handleSpeak = () => speak(content, groupId, instanceId);
 * ```
 */
export function useTTSManager(): TTSManagerReturn {
  // Get TTS preferences from the store
  const storeVoice = useTTSVoice();
  const storeSpeed = useTTSSpeed();
  const speedRef = useRef(storeSpeed);
  const {
    setTTSActive,
    setTTSPlaybackState,
    stopTTS,
    setTTSSpeed: setStoreSpeed,
  } = useChatUIStore();

  // Keep speedRef in sync with store and update audio playback rate
  useEffect(() => {
    speedRef.current = storeSpeed;
    if (globalAudio) {
      globalAudio.playbackRate = storeSpeed;
    }
  }, [storeSpeed]);

  // Set up global audio event listeners (once per mount)
  useEffect(() => {
    const audio = getAudio();

    const handlePlay = () => {
      setTTSPlaybackState("playing");
    };

    const handlePause = () => {
      if (!audio.ended) {
        setTTSPlaybackState("paused");
      }
    };

    const handleEnded = () => {
      stopTTS();
    };

    const handleError = () => {
      setTTSPlaybackState("error");
    };

    audio.addEventListener("play", handlePlay);
    audio.addEventListener("pause", handlePause);
    audio.addEventListener("ended", handleEnded);
    audio.addEventListener("error", handleError);

    return () => {
      audio.removeEventListener("play", handlePlay);
      audio.removeEventListener("pause", handlePause);
      audio.removeEventListener("ended", handleEnded);
      audio.removeEventListener("error", handleError);
    };
  }, [setTTSPlaybackState, stopTTS]);

  /** Speak using browser TTS as fallback */
  const speakWithBrowserTTS = useCallback(
    (text: string, speed: number) => {
      if (!isBrowserTTSAvailable()) {
        console.error("Browser TTS not available");
        setTTSPlaybackState("error");
        return;
      }

      usingBrowserTTS = true;
      const utterance = new SpeechSynthesisUtterance(text);

      // Set voice and rate
      const voice = findBrowserVoice();
      if (voice) {
        utterance.voice = voice;
      }
      utterance.rate = mapSpeedToBrowser(speed);

      // Set up event handlers
      utterance.onstart = () => {
        setTTSPlaybackState("playing");
      };

      utterance.onpause = () => {
        setTTSPlaybackState("paused");
      };

      utterance.onresume = () => {
        setTTSPlaybackState("playing");
      };

      utterance.onend = () => {
        stopTTS();
        usingBrowserTTS = false;
      };

      utterance.onerror = (event) => {
        if (event.error !== "canceled") {
          console.error("Browser TTS error:", event.error);
          setTTSPlaybackState("error");
        }
        usingBrowserTTS = false;
      };

      window.speechSynthesis.speak(utterance);
    },
    [setTTSPlaybackState, stopTTS]
  );

  const speak = useCallback(
    async (text: string, groupId: string, instanceId: string, options: TTSManagerOptions = {}) => {
      const { voice = storeVoice, speed = speedRef.current, model = DEFAULT_TTS_MODEL } = options;

      // Stop any current playback and cleanup
      cleanup();

      // Mark as loading for this response
      setTTSActive(groupId, instanceId, "loading");

      try {
        const response = await apiV1AudioSpeech({
          body: {
            input: text,
            voice,
            speed,
            model,
          },
        });

        if (response.error) {
          throw new Error("Failed to generate speech");
        }

        const blob = response.data as Blob;
        globalBlobUrl = URL.createObjectURL(blob);

        const audio = getAudio();
        audio.playbackRate = speedRef.current;
        audio.src = globalBlobUrl;
        await audio.play();
      } catch (err) {
        console.error("Gateway TTS failed, trying browser fallback:", err);
        // Try browser TTS as fallback
        if (isBrowserTTSAvailable()) {
          speakWithBrowserTTS(text, speed);
        } else {
          console.error("No TTS available (Gateway failed, browser TTS not supported)");
          setTTSPlaybackState("error");
          cleanup();
        }
      }
    },
    [setTTSActive, setTTSPlaybackState, storeVoice, speakWithBrowserTTS]
  );

  const stop = useCallback(() => {
    cleanup();
    stopTTS();
  }, [stopTTS]);

  const setSpeed = useCallback(
    (newSpeed: number) => {
      const clampedSpeed = Math.max(MIN_TTS_SPEED, Math.min(MAX_TTS_SPEED, newSpeed));
      speedRef.current = clampedSpeed;
      setStoreSpeed(clampedSpeed);

      if (globalAudio) {
        globalAudio.playbackRate = clampedSpeed;
      }
    },
    [setStoreSpeed]
  );

  return {
    speak,
    stop,
    setSpeed,
    speed: storeSpeed,
  };
}

/**
 * TTS hook for a specific response.
 *
 * Provides playback state and controls scoped to a single response.
 * Use this in individual response cards to wire up TTS functionality.
 *
 * @example
 * ```tsx
 * const { state, speak, stop } = useTTSForResponse(content, groupId, instanceId);
 *
 * <ResponseActions
 *   speakingState={state}
 *   onSpeak={speak}
 *   onStopSpeaking={stop}
 * />
 * ```
 */
export function useTTSForResponse(
  content: string,
  groupId: string,
  instanceId: string
): TTSResponseReturn {
  const state = useTTSStateForResponse(groupId, instanceId);
  const { speak: globalSpeak, stop: globalStop } = useTTSManager();

  const speak = useCallback(async () => {
    await globalSpeak(content, groupId, instanceId);
  }, [globalSpeak, content, groupId, instanceId]);

  const stop = useCallback(() => {
    // Only stop if this response is currently active
    const currentActiveId = useChatUIStore.getState().ttsActiveResponseId;
    if (currentActiveId === `${groupId}:${instanceId}`) {
      globalStop();
    }
  }, [globalStop, groupId, instanceId]);

  return {
    state,
    speak,
    stop,
  };
}
