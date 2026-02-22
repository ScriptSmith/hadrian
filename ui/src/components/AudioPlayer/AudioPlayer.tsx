import { Play, Pause, Square, Loader2 } from "lucide-react";

import { Button } from "@/components/Button/Button";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import type { PlaybackState } from "@/hooks/useAudioPlayback";
import { cn } from "@/utils/cn";

/** Speed presets for TTS playback */
const SPEED_OPTIONS = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2] as const;

/** Format seconds as MM:SS */
function formatTime(seconds: number): string {
  if (!isFinite(seconds) || seconds < 0) return "0:00";
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

export interface AudioPlayerProps {
  /** Current playback state */
  state: PlaybackState;
  /** Current playback position in seconds */
  currentTime: number;
  /** Total duration in seconds */
  duration: number;
  /** Progress as a percentage (0-100) */
  progress: number;
  /** Current playback speed */
  speed: number;
  /** Toggle play/pause */
  onTogglePlayPause: () => void;
  /** Stop playback */
  onStop: () => void;
  /** Seek to position (0-1) */
  onSeek: (position: number) => void;
  /** Set playback speed */
  onSetSpeed: (speed: number) => void;
  /** Whether to show the stop button */
  showStop?: boolean;
  /** Whether to show the speed control */
  showSpeed?: boolean;
  /** Additional CSS classes */
  className?: string;
}

/**
 * Inline audio player component with play/pause, progress bar, and speed control.
 *
 * Designed to be compact and fit within chat message responses.
 */
export function AudioPlayer({
  state,
  currentTime,
  duration,
  progress,
  speed,
  onTogglePlayPause,
  onStop,
  onSeek,
  onSetSpeed,
  showStop = false,
  showSpeed = true,
  className,
}: AudioPlayerProps) {
  const isLoading = state === "loading";
  const isPlaying = state === "playing";
  const isPaused = state === "paused";
  const hasAudio = isPlaying || isPaused;

  const handleProgressClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!hasAudio) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const position = (e.clientX - rect.left) / rect.width;
    onSeek(Math.max(0, Math.min(1, position)));
  };

  const handleProgressKeyDown = (e: React.KeyboardEvent) => {
    if (!hasAudio) return;
    const step = 0.05; // 5% per keypress
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      onSeek(Math.max(0, progress / 100 - step));
    } else if (e.key === "ArrowRight") {
      e.preventDefault();
      onSeek(Math.min(1, progress / 100 + step));
    }
  };

  return (
    <div className={cn("flex items-center gap-2 rounded-lg bg-muted/50 px-2 py-1.5", className)}>
      {/* Play/Pause button */}
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 w-7 p-0 shrink-0"
            onClick={onTogglePlayPause}
            disabled={isLoading}
            aria-label={isLoading ? "Loading audio" : isPlaying ? "Pause" : "Play"}
          >
            {isLoading ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : isPlaying ? (
              <Pause className="h-4 w-4" />
            ) : (
              <Play className="h-4 w-4" />
            )}
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          {isLoading ? "Generating audio..." : isPlaying ? "Pause" : "Play"}
        </TooltipContent>
      </Tooltip>

      {/* Stop button (optional) */}
      {showStop && hasAudio && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 w-7 p-0 shrink-0"
              onClick={onStop}
              aria-label="Stop"
            >
              <Square className="h-3.5 w-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Stop</TooltipContent>
        </Tooltip>
      )}

      {/* Progress bar */}
      <div
        role="slider"
        aria-label="Audio progress"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(progress)}
        aria-valuetext={`${formatTime(currentTime)} of ${formatTime(duration)}`}
        tabIndex={hasAudio ? 0 : -1}
        className={cn(
          "relative flex-1 h-1.5 min-w-[60px] rounded-full bg-muted cursor-pointer",
          "focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
          !hasAudio && "opacity-50 cursor-default"
        )}
        onClick={handleProgressClick}
        onKeyDown={handleProgressKeyDown}
      >
        {/* Progress fill */}
        <div
          className="absolute inset-y-0 left-0 rounded-full bg-primary transition-all"
          style={{ width: `${progress}%` }}
        />
        {/* Thumb indicator (shown when has audio) */}
        {hasAudio && (
          <div
            className="absolute top-1/2 -translate-y-1/2 h-3 w-3 rounded-full bg-primary shadow-sm transition-all"
            style={{ left: `calc(${progress}% - 6px)` }}
          />
        )}
      </div>

      {/* Time display */}
      <span className="text-xs text-muted-foreground tabular-nums min-w-[70px] text-right">
        {formatTime(currentTime)} / {formatTime(duration)}
      </span>

      {/* Speed control */}
      {showSpeed && (
        <Dropdown>
          <Tooltip>
            <TooltipTrigger asChild>
              <DropdownTrigger
                showChevron={false}
                aria-label="Playback speed"
                className="h-7 px-1.5 text-xs tabular-nums min-w-[40px] justify-center"
              >
                {speed}x
              </DropdownTrigger>
            </TooltipTrigger>
            <TooltipContent>Playback speed</TooltipContent>
          </Tooltip>
          <DropdownContent align="end">
            {SPEED_OPTIONS.map((s) => (
              <DropdownItem key={s} selected={speed === s} onClick={() => onSetSpeed(s)}>
                {s}x
              </DropdownItem>
            ))}
          </DropdownContent>
        </Dropdown>
      )}
    </div>
  );
}
