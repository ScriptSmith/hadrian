import { useState } from "react";
import {
  LayoutGrid,
  Link,
  GitBranch,
  Combine,
  Sparkles,
  MessageSquareWarning,
  Vote,
  Trophy,
  Handshake,
  Swords,
  Users,
  Network,
  Shuffle,
  Target,
  Scale,
  Dna,
  GraduationCap,
  Layers,
  Blend,
  Zap,
  FlaskConical,
  type LucideIcon,
  type LucideProps,
} from "lucide-react";

import { Dropdown, DropdownContent, DropdownTrigger } from "@/components/Dropdown/Dropdown";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import type { ConversationMode, ModeMetadata } from "@/components/chat-types";
import { CONVERSATION_MODES, getModeMetadata } from "@/components/chat-types";
import { cn } from "@/utils/cn";

/** Map mode icon names to Lucide components */
const ICON_MAP: Record<string, LucideIcon> = {
  LayoutGrid,
  Link,
  GitBranch,
  Combine,
  Sparkles,
  MessageSquareWarning,
  Vote,
  Trophy,
  Handshake,
  Swords,
  Users,
  Network,
  Shuffle,
  Target,
  Scale,
  Dna,
  GraduationCap,
};

/** Render a mode icon by name - declared as a stable component */
function ModeIcon({ iconName, ...props }: { iconName: string } & LucideProps) {
  const IconComponent = ICON_MAP[iconName] || LayoutGrid;
  return <IconComponent {...props} />;
}

interface ModeSelectorProps {
  /** Current conversation mode */
  mode: ConversationMode;
  /** Callback when mode changes */
  onModeChange: (mode: ConversationMode) => void;
  /** Number of currently selected models (affects which modes are available) */
  selectedModelCount: number;
  /** Whether the chat is currently streaming (disables mode changes) */
  isStreaming?: boolean;
}

/** Phase metadata for tabs */
type Phase = 1 | 2 | 3 | 4 | 5;

const PHASES: { id: Phase; label: string; icon: LucideIcon; color: string }[] = [
  { id: 1, label: "Core", icon: Layers, color: "text-blue-500" },
  { id: 2, label: "Synthesis", icon: Blend, color: "text-violet-500" },
  { id: 3, label: "Competitive", icon: Trophy, color: "text-amber-500" },
  { id: 4, label: "Advanced", icon: Zap, color: "text-emerald-500" },
  { id: 5, label: "Experimental", icon: FlaskConical, color: "text-rose-500" },
];

/** Group modes by phase */
function groupModesByPhase(modes: ModeMetadata[]): Map<Phase, ModeMetadata[]> {
  const grouped = new Map<Phase, ModeMetadata[]>();
  for (const mode of modes) {
    const existing = grouped.get(mode.phase) || [];
    existing.push(mode);
    grouped.set(mode.phase, existing);
  }
  return grouped;
}

export function ModeSelector({
  mode,
  onModeChange,
  selectedModelCount,
  isStreaming = false,
}: ModeSelectorProps) {
  const currentMode = getModeMetadata(mode);
  const groupedModes = groupModesByPhase(CONVERSATION_MODES);
  const [selectedPhase, setSelectedPhase] = useState<Phase>(currentMode.phase);

  const phaseModes = groupedModes.get(selectedPhase) || [];

  return (
    <Dropdown>
      <Tooltip>
        <TooltipTrigger asChild>
          <DropdownTrigger
            disabled={isStreaming}
            className={cn(
              "h-8 gap-1.5 rounded-lg border border-input bg-background px-2.5 text-sm font-medium",
              "transition-all duration-150",
              "hover:bg-accent hover:text-accent-foreground hover:border-accent",
              "focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
              "disabled:opacity-50 disabled:cursor-not-allowed",
              !currentMode.implemented && "border-dashed"
            )}
          >
            <span className="text-muted-foreground hidden sm:inline">Mode:</span>
            <ModeIcon iconName={currentMode.icon} className="h-4 w-4" />
            <span className="hidden sm:inline">{currentMode.name}</span>
          </DropdownTrigger>
        </TooltipTrigger>
        <TooltipContent side="bottom">
          <p className="font-medium">{currentMode.name} Mode</p>
          <p className="text-muted-foreground text-xs max-w-[200px]">{currentMode.description}</p>
        </TooltipContent>
      </Tooltip>
      <DropdownContent align="start" className="w-72 p-0">
        {/* Phase tabs */}
        <div className="flex border-b border-border">
          {PHASES.map((phase) => {
            const isActive = phase.id === selectedPhase;
            const hasCurrentMode = groupedModes.get(phase.id)?.some((m) => m.id === mode);
            const PhaseIcon = phase.icon;
            return (
              <Tooltip key={phase.id}>
                <TooltipTrigger asChild>
                  <button
                    onClick={() => setSelectedPhase(phase.id)}
                    className={cn(
                      "flex-1 flex items-center justify-center py-2.5 transition-colors relative",
                      "hover:bg-accent/50",
                      isActive ? "text-foreground bg-accent/30" : "text-muted-foreground"
                    )}
                  >
                    <PhaseIcon className={cn("h-4 w-4", isActive && phase.color)} />
                    {hasCurrentMode && !isActive && (
                      <span className="absolute top-1 right-1 w-1.5 h-1.5 rounded-full bg-primary" />
                    )}
                  </button>
                </TooltipTrigger>
                <TooltipContent side="bottom" className="text-xs">
                  {phase.label}
                </TooltipContent>
              </Tooltip>
            );
          })}
        </div>

        {/* Modes grid for selected phase */}
        <div className="p-2 grid gap-1">
          {phaseModes.map((modeItem) => {
            const isSelected = modeItem.id === mode;
            const hasEnoughModels = selectedModelCount >= modeItem.minModels;
            const isAvailable = modeItem.implemented && hasEnoughModels;
            const isDisabled = !isAvailable || isStreaming;

            return (
              <button
                key={modeItem.id}
                disabled={isDisabled}
                onClick={() => onModeChange(modeItem.id)}
                className={cn(
                  "flex items-start gap-2.5 p-2 rounded-md text-left w-full",
                  "transition-colors",
                  isSelected ? "bg-primary/10 ring-1 ring-primary/30" : "hover:bg-accent",
                  isDisabled && "opacity-50 cursor-not-allowed",
                  !modeItem.implemented && "opacity-60"
                )}
              >
                <ModeIcon
                  iconName={modeItem.icon}
                  className={cn(
                    "h-4 w-4 mt-0.5 shrink-0",
                    isSelected
                      ? "text-primary"
                      : isAvailable
                        ? "text-muted-foreground"
                        : "text-muted-foreground/50"
                  )}
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-1.5">
                    <span className={cn("text-sm", isSelected && "font-medium text-primary")}>
                      {modeItem.name}
                    </span>
                    {!modeItem.implemented && (
                      <span className="text-[9px] px-1 py-px rounded-sm bg-muted/80 text-muted-foreground uppercase tracking-wider">
                        soon
                      </span>
                    )}
                  </div>
                  <p className="text-[11px] text-muted-foreground leading-tight mt-0.5">
                    {modeItem.description}
                  </p>
                  {!hasEnoughModels && modeItem.minModels > 1 && (
                    <p className="text-[10px] text-amber-800 dark:text-amber-500 mt-0.5">
                      Needs {modeItem.minModels}+ models
                    </p>
                  )}
                </div>
              </button>
            );
          })}
        </div>
      </DropdownContent>
    </Dropdown>
  );
}
