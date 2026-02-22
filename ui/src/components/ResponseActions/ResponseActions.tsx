import {
  Copy,
  Check,
  RefreshCw,
  Maximize2,
  Minimize2,
  Trophy,
  EyeOff,
  Volume2,
  Square,
  Loader2,
  Pencil,
  Bug,
} from "lucide-react";
import { useState } from "react";

import type { PlaybackState } from "@/hooks/useAudioPlayback";

import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

/** Configuration for which action buttons to show */
export interface ResponseActionConfig {
  showSelectBest?: boolean;
  showRegenerate?: boolean;
  showCopy?: boolean;
  showExpand?: boolean;
  showHide?: boolean;
  showSpeak?: boolean;
}

export const DEFAULT_ACTION_CONFIG: ResponseActionConfig = {
  showSelectBest: true,
  showRegenerate: true,
  showCopy: true,
  showExpand: true,
  showHide: true,
  showSpeak: true,
};

export interface ResponseActionsProps {
  /** Content to copy */
  content: string;
  /** Whether this response is selected as best */
  isSelectedBest?: boolean;
  /** Whether to show the select best button */
  canSelectBest?: boolean;
  /** Whether the response is expanded */
  isExpanded?: boolean;
  /** Whether the expand button should be shown */
  canExpand?: boolean;
  /** Callback when selected as best */
  onSelectBest?: () => void;
  /** Callback to regenerate */
  onRegenerate?: () => void;
  /** Callback to toggle expand */
  onExpand?: () => void;
  /** Callback to hide this response */
  onHide?: () => void;
  /** Callback to speak the response content */
  onSpeak?: () => void;
  /** Callback to stop speaking */
  onStopSpeaking?: () => void;
  /** Current TTS playback state */
  speakingState?: PlaybackState;
  /** Callback to edit the response */
  onEdit?: () => void;
  /** Callback to open debug info */
  onOpenDebug?: () => void;
  /** Configuration for which buttons to show */
  config?: ResponseActionConfig;
  /** Additional CSS classes */
  className?: string;
}

/**
 * ResponseActions - Action buttons for model responses
 *
 * Uses a hover-reveal pattern:
 * - Primary actions (Copy, Regenerate) are always visible
 * - Secondary actions (Select Best, Hide, Speak, Expand) slide in from the right on hover
 */
export function ResponseActions({
  content,
  isSelectedBest,
  canSelectBest,
  isExpanded,
  canExpand,
  onSelectBest,
  onRegenerate,
  onExpand,
  onHide,
  onSpeak,
  onStopSpeaking,
  speakingState = "idle",
  onEdit,
  onOpenDebug,
  config = DEFAULT_ACTION_CONFIG,
  className,
}: ResponseActionsProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Primary actions - always visible
  const showRegenerate = config.showRegenerate && onRegenerate;
  const showCopy = config.showCopy;

  // Secondary actions - shown on hover
  const showEdit = !!onEdit;
  const showDebug = !!onOpenDebug;
  const showSelectBest = config.showSelectBest && canSelectBest && onSelectBest && !isSelectedBest;
  const showHide = config.showHide && onHide;
  const showSpeak = config.showSpeak && onSpeak;
  const showExpand = config.showExpand && canExpand && onExpand;

  const isSpeaking = speakingState === "playing";
  const isSpeakLoading = speakingState === "loading";
  const canStopSpeaking =
    (isSpeaking || isSpeakLoading || speakingState === "paused") && onStopSpeaking;

  const hasPrimaryActions = showRegenerate || showCopy;
  const hasSecondaryActions =
    showEdit || showDebug || showSelectBest || showHide || showSpeak || showExpand;
  const hasAnyAction = hasPrimaryActions || hasSecondaryActions;

  if (!hasAnyAction) return null;

  return (
    <div className={cn("flex items-center", className)}>
      {/* Secondary actions - always visible on mobile, slide in on hover for desktop */}
      {hasSecondaryActions && (
        <div
          className={cn(
            "flex items-center gap-0.5",
            // Mobile: always visible
            "max-w-[250px] opacity-100",
            // Desktop: hidden by default, revealed on card hover
            "sm:max-w-0 sm:opacity-0 sm:overflow-hidden",
            "sm:group-hover/card:max-w-[250px] sm:group-hover/card:opacity-100",
            "transition-all duration-200 ease-out"
          )}
        >
          {/* Debug */}
          {showDebug && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 p-0 shrink-0 text-muted-foreground hover:text-foreground"
                  onClick={onOpenDebug}
                  aria-label="View debug info"
                >
                  <Bug className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>View debug info</TooltipContent>
            </Tooltip>
          )}

          {/* Edit */}
          {showEdit && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 p-0 shrink-0 text-muted-foreground hover:text-foreground"
                  onClick={onEdit}
                  aria-label="Edit response"
                >
                  <Pencil className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Edit response</TooltipContent>
            </Tooltip>
          )}

          {/* Select as best */}
          {showSelectBest && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 p-0 shrink-0 text-muted-foreground hover:text-success"
                  onClick={onSelectBest}
                  aria-label="Select as best response"
                >
                  <Trophy className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Select as best response</TooltipContent>
            </Tooltip>
          )}

          {/* Hide */}
          {showHide && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 p-0 shrink-0"
                  onClick={onHide}
                  aria-label="Hide response"
                >
                  <EyeOff className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Hide response</TooltipContent>
            </Tooltip>
          )}

          {/* Speak */}
          {showSpeak && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className={cn(
                    "h-7 w-7 p-0 shrink-0",
                    isSpeaking &&
                      "text-primary bg-primary/10 hover:bg-primary/20 hover:text-primary"
                  )}
                  onClick={canStopSpeaking ? onStopSpeaking : onSpeak}
                  disabled={isSpeakLoading}
                  aria-label={
                    isSpeakLoading
                      ? "Generating audio"
                      : canStopSpeaking
                        ? "Stop speaking"
                        : "Read aloud"
                  }
                >
                  {isSpeakLoading ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : canStopSpeaking ? (
                    <Square className="h-3.5 w-3.5" />
                  ) : (
                    <Volume2 className="h-3.5 w-3.5" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {isSpeakLoading
                  ? "Generating audio..."
                  : canStopSpeaking
                    ? "Stop speaking"
                    : "Read aloud"}
              </TooltipContent>
            </Tooltip>
          )}

          {/* Expand */}
          {showExpand && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 p-0 shrink-0"
                  onClick={onExpand}
                  aria-label={isExpanded ? "Collapse response" : "Expand response"}
                >
                  {isExpanded ? (
                    <Minimize2 className="h-3.5 w-3.5" />
                  ) : (
                    <Maximize2 className="h-3.5 w-3.5" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>{isExpanded ? "Collapse" : "Expand"}</TooltipContent>
            </Tooltip>
          )}
        </div>
      )}

      {/* Primary actions - always visible */}
      {hasPrimaryActions && (
        <div className="flex items-center gap-0.5">
          {/* Regenerate */}
          {showRegenerate && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 w-7 p-0 shrink-0"
                  onClick={onRegenerate}
                  aria-label="Regenerate response"
                >
                  <RefreshCw className="h-3.5 w-3.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Regenerate</TooltipContent>
            </Tooltip>
          )}

          {/* Copy */}
          {showCopy && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className={cn("h-7 w-7 p-0 shrink-0", copied && "text-success")}
                  onClick={handleCopy}
                  aria-label={copied ? "Copied" : "Copy response"}
                >
                  {copied ? (
                    <Check className="h-3.5 w-3.5 animate-bounce-in" />
                  ) : (
                    <Copy className="h-3.5 w-3.5" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>{copied ? "Copied!" : "Copy"}</TooltipContent>
            </Tooltip>
          )}
        </div>
      )}
    </div>
  );
}
