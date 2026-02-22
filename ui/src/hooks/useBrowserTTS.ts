import { useCallback, useEffect, useRef, useState } from "react";

import type { PlaybackState } from "@/hooks/useAudioPlayback";
import { DEFAULT_TTS_SPEED, MIN_TTS_SPEED, MAX_TTS_SPEED } from "@/hooks/useAudioPlayback";

/** Check if browser TTS is available */
export function isBrowserTTSAvailable(): boolean {
  return typeof window !== "undefined" && "speechSynthesis" in window;
}

/** Get available browser voices */
export function getBrowserVoices(): SpeechSynthesisVoice[] {
  if (!isBrowserTTSAvailable()) return [];
  return window.speechSynthesis.getVoices();
}

/** Options for browser TTS */
export interface BrowserTTSOptions {
  /** Voice to use (by name or language) */
  voice?: string;
  /** Playback speed (0.25 to 4.0, will be mapped to browser range) */
  speed?: number;
  /** Pitch (0.1 to 2, default 1) */
  pitch?: number;
}

/** Return type for useBrowserTTS hook */
export interface UseBrowserTTSReturn {
  /** Current playback state */
  state: PlaybackState;
  /** Whether browser TTS is available */
  isAvailable: boolean;
  /** Available voices */
  voices: SpeechSynthesisVoice[];
  /** Speak text using browser TTS */
  speak: (text: string, options?: BrowserTTSOptions) => void;
  /** Pause speech */
  pause: () => void;
  /** Resume speech */
  resume: () => void;
  /** Stop speech */
  stop: () => void;
  /** Set playback speed (applies to next speak call) */
  setSpeed: (speed: number) => void;
  /** Current speed setting */
  speed: number;
}

/**
 * Hook for browser-native text-to-speech using Web Speech API.
 *
 * Used as a fallback when Gateway TTS endpoint is unavailable.
 *
 * @example
 * ```tsx
 * const { speak, stop, state, isAvailable } = useBrowserTTS();
 *
 * if (isAvailable) {
 *   speak("Hello, world!", { speed: 1.2 });
 * }
 * ```
 */
export function useBrowserTTS(): UseBrowserTTSReturn {
  const [state, setState] = useState<PlaybackState>("idle");
  const [voices, setVoices] = useState<SpeechSynthesisVoice[]>([]);
  const [speed, setSpeedState] = useState(DEFAULT_TTS_SPEED);
  const utteranceRef = useRef<SpeechSynthesisUtterance | null>(null);
  const isAvailable = isBrowserTTSAvailable();

  // Load voices (they load asynchronously in some browsers)
  useEffect(() => {
    if (!isAvailable) return;

    const loadVoices = () => {
      const availableVoices = window.speechSynthesis.getVoices();
      setVoices(availableVoices);
    };

    // Load immediately if available
    loadVoices();

    // Also listen for voiceschanged event (Chrome loads voices async)
    window.speechSynthesis.addEventListener("voiceschanged", loadVoices);

    return () => {
      window.speechSynthesis.removeEventListener("voiceschanged", loadVoices);
    };
  }, [isAvailable]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (isAvailable) {
        window.speechSynthesis.cancel();
      }
    };
  }, [isAvailable]);

  /** Find best matching voice */
  const findVoice = useCallback(
    (voiceName?: string): SpeechSynthesisVoice | null => {
      if (voices.length === 0) return null;

      if (voiceName) {
        // Try exact name match
        const exactMatch = voices.find((v) => v.name.toLowerCase() === voiceName.toLowerCase());
        if (exactMatch) return exactMatch;

        // Try partial name match
        const partialMatch = voices.find((v) =>
          v.name.toLowerCase().includes(voiceName.toLowerCase())
        );
        if (partialMatch) return partialMatch;
      }

      // Prefer English voices
      const englishVoice = voices.find((v) => v.lang.startsWith("en") && v.default);
      if (englishVoice) return englishVoice;

      const anyEnglish = voices.find((v) => v.lang.startsWith("en"));
      if (anyEnglish) return anyEnglish;

      // Fall back to default or first voice
      return voices.find((v) => v.default) || voices[0] || null;
    },
    [voices]
  );

  /** Map OpenAI speed (0.25-4.0) to browser rate (0.1-10) */
  const mapSpeed = (openAISpeed: number): number => {
    // OpenAI: 0.25-4.0, Browser: 0.1-10
    // We'll use a more conservative mapping: 0.25->0.5, 1.0->1.0, 4.0->3.0
    // This prevents speech from becoming unintelligible at high speeds
    const clampedSpeed = Math.max(MIN_TTS_SPEED, Math.min(MAX_TTS_SPEED, openAISpeed));
    if (clampedSpeed <= 1) {
      // Map 0.25-1.0 to 0.5-1.0
      return 0.5 + (clampedSpeed - 0.25) * (0.5 / 0.75);
    } else {
      // Map 1.0-4.0 to 1.0-3.0
      return 1 + (clampedSpeed - 1) * (2 / 3);
    }
  };

  const speak = useCallback(
    (text: string, options: BrowserTTSOptions = {}) => {
      if (!isAvailable) {
        setState("error");
        return;
      }

      // Cancel any ongoing speech
      window.speechSynthesis.cancel();

      const { voice: voiceName, speed: requestSpeed = speed, pitch = 1 } = options;

      const utterance = new SpeechSynthesisUtterance(text);
      utteranceRef.current = utterance;

      // Set voice
      const selectedVoice = findVoice(voiceName);
      if (selectedVoice) {
        utterance.voice = selectedVoice;
      }

      // Set rate and pitch
      utterance.rate = mapSpeed(requestSpeed);
      utterance.pitch = Math.max(0.1, Math.min(2, pitch));

      // Set up event handlers
      utterance.onstart = () => {
        setState("playing");
      };

      utterance.onpause = () => {
        setState("paused");
      };

      utterance.onresume = () => {
        setState("playing");
      };

      utterance.onend = () => {
        setState("idle");
        utteranceRef.current = null;
      };

      utterance.onerror = (event) => {
        // "canceled" is not really an error, it happens when stop() is called
        if (event.error !== "canceled") {
          console.error("Browser TTS error:", event.error);
          setState("error");
        } else {
          setState("idle");
        }
        utteranceRef.current = null;
      };

      // Start speaking
      setState("loading");
      window.speechSynthesis.speak(utterance);
    },
    [isAvailable, findVoice, speed]
  );

  const pause = useCallback(() => {
    if (!isAvailable) return;
    window.speechSynthesis.pause();
  }, [isAvailable]);

  const resume = useCallback(() => {
    if (!isAvailable) return;
    window.speechSynthesis.resume();
  }, [isAvailable]);

  const stop = useCallback(() => {
    if (!isAvailable) return;
    window.speechSynthesis.cancel();
    setState("idle");
    utteranceRef.current = null;
  }, [isAvailable]);

  const setSpeed = useCallback((newSpeed: number) => {
    const clampedSpeed = Math.max(MIN_TTS_SPEED, Math.min(MAX_TTS_SPEED, newSpeed));
    setSpeedState(clampedSpeed);
  }, []);

  return {
    state,
    isAvailable,
    voices,
    speak,
    pause,
    resume,
    stop,
    setSpeed,
    speed,
  };
}
