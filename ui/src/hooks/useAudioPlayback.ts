import { useCallback, useEffect, useRef, useState } from "react";

import { apiV1AudioSpeech } from "@/api/generated/sdk.gen";
import type { Voice } from "@/api/generated/types.gen";

/** Available TTS voices */
export const TTS_VOICES: Voice[] = [
  "alloy",
  "ash",
  "ballad",
  "coral",
  "echo",
  "fable",
  "nova",
  "onyx",
  "sage",
  "shimmer",
  "verse",
  "marin",
  "cedar",
];

/** Default TTS settings */
export const DEFAULT_TTS_VOICE: Voice = "alloy";
export const DEFAULT_TTS_SPEED = 1.0;
export const MIN_TTS_SPEED = 0.25;
export const MAX_TTS_SPEED = 4.0;
export const DEFAULT_TTS_MODEL = "tts-1";

/** Playback state */
export type PlaybackState = "idle" | "loading" | "playing" | "paused" | "error";

/** Options for generating and playing TTS audio */
export interface TTSOptions {
  /** The voice to use */
  voice?: Voice;
  /** Playback speed (0.25 to 4.0) */
  speed?: number;
  /** TTS model to use */
  model?: string;
}

/** Return type for useAudioPlayback hook */
export interface UseAudioPlaybackReturn {
  /** Current playback state */
  state: PlaybackState;
  /** Current playback position in seconds */
  currentTime: number;
  /** Total duration in seconds (0 if unknown) */
  duration: number;
  /** Progress as a percentage (0-100) */
  progress: number;
  /** Current playback speed */
  speed: number;
  /** Error message if state is 'error' */
  error: string | null;
  /** Generate TTS audio and start playback */
  speak: (text: string, options?: TTSOptions) => Promise<void>;
  /** Toggle play/pause */
  togglePlayPause: () => void;
  /** Play audio */
  play: () => void;
  /** Pause audio */
  pause: () => void;
  /** Stop playback and reset */
  stop: () => void;
  /** Seek to a position (0-1) */
  seek: (position: number) => void;
  /** Set playback speed */
  setSpeed: (speed: number) => void;
  /** Whether audio is currently loaded */
  hasAudio: boolean;
}

/**
 * Hook for managing TTS audio playback.
 *
 * Handles generating audio from text via the Gateway TTS endpoint,
 * and provides controls for playback (play, pause, stop, seek, speed).
 *
 * @example
 * ```tsx
 * const { speak, state, togglePlayPause, progress } = useAudioPlayback();
 *
 * // Generate and play TTS
 * await speak("Hello, world!", { voice: "nova", speed: 1.2 });
 *
 * // Control playback
 * togglePlayPause();
 * ```
 */
export function useAudioPlayback(): UseAudioPlaybackReturn {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const blobUrlRef = useRef<string | null>(null);

  const [state, setState] = useState<PlaybackState>("idle");
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [speed, setSpeedState] = useState(DEFAULT_TTS_SPEED);
  const [error, setError] = useState<string | null>(null);

  // Cleanup blob URL and audio element
  const cleanup = useCallback(() => {
    if (audioRef.current) {
      audioRef.current.pause();
      audioRef.current.src = "";
      audioRef.current = null;
    }
    if (blobUrlRef.current) {
      URL.revokeObjectURL(blobUrlRef.current);
      blobUrlRef.current = null;
    }
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return cleanup;
  }, [cleanup]);

  // Create or get the audio element
  const getAudio = useCallback(() => {
    if (!audioRef.current) {
      audioRef.current = new Audio();

      // Set up event listeners
      audioRef.current.addEventListener("timeupdate", () => {
        setCurrentTime(audioRef.current?.currentTime ?? 0);
      });

      audioRef.current.addEventListener("loadedmetadata", () => {
        setDuration(audioRef.current?.duration ?? 0);
      });

      audioRef.current.addEventListener("ended", () => {
        setState("idle");
        setCurrentTime(0);
      });

      audioRef.current.addEventListener("error", () => {
        setState("error");
        setError("Failed to play audio");
      });

      audioRef.current.addEventListener("play", () => {
        setState("playing");
      });

      audioRef.current.addEventListener("pause", () => {
        if (audioRef.current && !audioRef.current.ended) {
          setState("paused");
        }
      });
    }
    return audioRef.current;
  }, []);

  // Generate TTS audio and start playback
  const speak = useCallback(
    async (text: string, options: TTSOptions = {}) => {
      const {
        voice = DEFAULT_TTS_VOICE,
        speed: requestSpeed = speed,
        model = DEFAULT_TTS_MODEL,
      } = options;

      // Cleanup previous audio
      cleanup();

      setState("loading");
      setError(null);
      setCurrentTime(0);
      setDuration(0);

      try {
        // Call the TTS API
        const response = await apiV1AudioSpeech({
          body: {
            input: text,
            voice,
            speed: requestSpeed,
            model,
          },
        });

        if (response.error) {
          throw new Error("Failed to generate speech");
        }

        const blob = response.data as Blob;
        const url = URL.createObjectURL(blob);
        blobUrlRef.current = url;

        // Load and play the audio
        const audio = getAudio();
        audio.playbackRate = speed;
        audio.src = url;
        await audio.play();
      } catch (err) {
        setState("error");
        setError(err instanceof Error ? err.message : "Failed to generate speech");
        cleanup();
      }
    },
    [cleanup, getAudio, speed]
  );

  // Toggle play/pause
  const togglePlayPause = useCallback(() => {
    const audio = audioRef.current;
    if (!audio || !blobUrlRef.current) return;

    if (state === "playing") {
      audio.pause();
    } else if (state === "paused" || state === "idle") {
      audio.play().catch(() => {
        setState("error");
        setError("Failed to play audio");
      });
    }
  }, [state]);

  // Play
  const play = useCallback(() => {
    const audio = audioRef.current;
    if (!audio || !blobUrlRef.current) return;

    audio.play().catch(() => {
      setState("error");
      setError("Failed to play audio");
    });
  }, []);

  // Pause
  const pause = useCallback(() => {
    audioRef.current?.pause();
  }, []);

  // Stop and reset
  const stop = useCallback(() => {
    const audio = audioRef.current;
    if (audio) {
      audio.pause();
      audio.currentTime = 0;
    }
    setState("idle");
    setCurrentTime(0);
  }, []);

  // Seek to position (0-1)
  const seek = useCallback(
    (position: number) => {
      const audio = audioRef.current;
      if (!audio || !duration) return;

      const clampedPosition = Math.max(0, Math.min(1, position));
      audio.currentTime = clampedPosition * duration;
    },
    [duration]
  );

  // Set playback speed
  const setSpeed = useCallback((newSpeed: number) => {
    const clampedSpeed = Math.max(MIN_TTS_SPEED, Math.min(MAX_TTS_SPEED, newSpeed));
    setSpeedState(clampedSpeed);

    if (audioRef.current) {
      audioRef.current.playbackRate = clampedSpeed;
    }
  }, []);

  // Calculate progress percentage
  const progress = duration > 0 ? (currentTime / duration) * 100 : 0;

  return {
    state,
    currentTime,
    duration,
    progress,
    speed,
    error,
    speak,
    togglePlayPause,
    play,
    pause,
    stop,
    seek,
    setSpeed,
    hasAudio: blobUrlRef.current !== null,
  };
}
