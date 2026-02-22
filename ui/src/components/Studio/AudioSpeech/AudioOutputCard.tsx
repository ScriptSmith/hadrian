import { useState, useEffect, useCallback, useRef } from "react";
import { Download, Trash2 } from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";
import { AudioPlayer } from "@/components/AudioPlayer/AudioPlayer";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { getModelDisplayName } from "@/utils/modelNames";
import { formatCost } from "@/utils/formatters";
import { readAudioFile } from "@/services/opfs/opfsService";
import type { AudioHistoryEntry, InstanceAudioResult } from "@/pages/studio/types";

interface AudioOutputCardProps {
  entry: AudioHistoryEntry;
  onDelete: (id: string) => void;
  className?: string;
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

function InstancePlayer({ result, format }: { result: InstanceAudioResult; format: string }) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const objectUrlRef = useRef<string | null>(null);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);
  const [loadFailed, setLoadFailed] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setLoading(true);
      setLoadFailed(false);

      if (!result.audioData) {
        setLoadFailed(true);
        setLoading(false);
        return;
      }

      const blob = await readAudioFile(result.audioData);

      if (cancelled) return;

      if (!blob) {
        setLoadFailed(true);
        setLoading(false);
        return;
      }

      const url = URL.createObjectURL(blob);
      objectUrlRef.current = url;
      const audio = new Audio(url);
      audioRef.current = audio;

      audio.addEventListener("loadedmetadata", () => {
        if (!cancelled) {
          setDuration(audio.duration);
          setLoading(false);
        }
      });
      audio.addEventListener("timeupdate", () => {
        if (!cancelled) setCurrentTime(audio.currentTime);
      });
      audio.addEventListener("play", () => {
        if (!cancelled) setPlaying(true);
      });
      audio.addEventListener("pause", () => {
        if (!cancelled) setPlaying(false);
      });
      audio.addEventListener("ended", () => {
        if (!cancelled) {
          setPlaying(false);
          setCurrentTime(0);
        }
      });
      audio.addEventListener("error", () => {
        if (!cancelled) {
          setLoadFailed(true);
          setLoading(false);
        }
      });

      // For very small/empty blobs loadedmetadata may not fire
      audio.load();
    }

    load();

    return () => {
      cancelled = true;
      audioRef.current?.pause();
      if (audioRef.current) audioRef.current.src = "";
      if (objectUrlRef.current) URL.revokeObjectURL(objectUrlRef.current);
      audioRef.current = null;
      objectUrlRef.current = null;
    };
  }, [result.audioData]);

  const togglePlayPause = useCallback(() => {
    const audio = audioRef.current;
    if (!audio) return;
    if (playing) audio.pause();
    else audio.play().catch(() => {});
  }, [playing]);

  const stop = useCallback(() => {
    const audio = audioRef.current;
    if (!audio) return;
    audio.pause();
    audio.currentTime = 0;
    setPlaying(false);
    setCurrentTime(0);
  }, []);

  const seek = useCallback(
    (pos: number) => {
      const audio = audioRef.current;
      if (audio && duration) {
        audio.currentTime = Math.max(0, Math.min(1, pos)) * duration;
      }
    },
    [duration]
  );

  const handleSetSpeed = useCallback((s: number) => {
    setSpeed(s);
    if (audioRef.current) audioRef.current.playbackRate = s;
  }, []);

  const handleDownload = useCallback(() => {
    if (!objectUrlRef.current) return;
    const a = document.createElement("a");
    a.href = objectUrlRef.current;
    a.download = `speech-${result.instanceId}.${format || "mp3"}`;
    a.click();
  }, [result.instanceId, format]);

  if (result.error) {
    return <div className="text-xs text-destructive">{result.error}</div>;
  }

  if (loadFailed) {
    return <div className="text-xs text-muted-foreground italic">Audio no longer available</div>;
  }

  if (loading) {
    return <div className="text-xs text-muted-foreground animate-pulse">Loading audio...</div>;
  }

  const progress = duration > 0 ? (currentTime / duration) * 100 : 0;
  const state = playing ? "playing" : duration > 0 ? "paused" : "idle";

  return (
    <div className="flex items-center gap-2">
      <div className="flex-1">
        <AudioPlayer
          state={state}
          currentTime={currentTime}
          duration={duration}
          progress={progress}
          speed={speed}
          onTogglePlayPause={togglePlayPause}
          onStop={stop}
          onSeek={seek}
          onSetSpeed={handleSetSpeed}
        />
      </div>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 shrink-0"
            onClick={handleDownload}
            aria-label="Download audio"
          >
            <Download className="h-3.5 w-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Download</TooltipContent>
      </Tooltip>
    </div>
  );
}

export function AudioOutputCard({ entry, onDelete, className }: AudioOutputCardProps) {
  const hasMultipleResults = entry.results.length > 1;
  // Collect unique voices across results for badge display
  const uniqueVoices = new Set(entry.results.map((r) => r.voice).filter(Boolean));
  const showPerResultVoice = hasMultipleResults || uniqueVoices.size > 1;
  const totalCostMicrocents = entry.results.reduce((sum, r) => sum + (r.costMicrocents ?? 0), 0);

  return (
    <div
      className={cn(
        "rounded-xl border border-border bg-card p-3 space-y-2",
        "motion-safe:transition-shadow motion-safe:duration-200 hover:shadow-sm",
        className
      )}
    >
      {/* Text preview + metadata */}
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <p className="text-sm text-foreground line-clamp-2">{entry.text}</p>
          <div className="mt-1 flex items-center gap-2">
            {/* Show single voice badge when all results share the same voice */}
            {!showPerResultVoice && uniqueVoices.size === 1 && (
              <span className="rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary capitalize">
                {[...uniqueVoices][0]}
              </span>
            )}
            <span className="text-[10px] text-muted-foreground">{formatTime(entry.createdAt)}</span>
            {totalCostMicrocents > 0 && (
              <span className="text-[10px] text-muted-foreground">
                {formatCost(totalCostMicrocents / 1_000_000)}
              </span>
            )}
          </div>
        </div>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7 shrink-0 text-muted-foreground hover:text-destructive"
              onClick={() => onDelete(entry.id)}
              aria-label="Delete audio"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Delete</TooltipContent>
        </Tooltip>
      </div>

      {/* Per-instance players */}
      {entry.results.map((result) => {
        const voice = result.voice;
        return (
          <div key={result.instanceId}>
            {hasMultipleResults && (
              <div className="mb-1 flex items-center gap-1.5">
                <span className="text-[10px] font-medium text-muted-foreground">
                  {result.label || getModelDisplayName(result.modelId)}
                </span>
                {showPerResultVoice && voice && (
                  <span className="rounded-full bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary capitalize">
                    {voice}
                  </span>
                )}
                {result.costMicrocents != null && result.costMicrocents > 0 && (
                  <span className="text-[10px] text-muted-foreground">
                    {formatCost(result.costMicrocents / 1_000_000)}
                  </span>
                )}
              </div>
            )}
            <InstancePlayer result={result} format={entry.options.format} />
          </div>
        );
      })}
    </div>
  );
}
