import type { LucideIcon } from "lucide-react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

/** Color variants for capability badges */
const colorMap = {
  purple: "bg-purple-500/10 text-purple-700 dark:text-purple-400",
  green: "bg-green-500/10 text-green-800 dark:text-green-400",
  cyan: "bg-cyan-500/10 text-cyan-800 dark:text-cyan-400",
  amber: "bg-amber-500/10 text-amber-800 dark:text-amber-400",
  indigo: "bg-indigo-500/10 text-indigo-700 dark:text-indigo-400",
  blue: "bg-blue-500/10 text-blue-700 dark:text-blue-400",
} as const;

export type CapabilityColor = keyof typeof colorMap;

interface CapabilityBadgeProps {
  icon: LucideIcon;
  label: string;
  color: CapabilityColor;
  className?: string;
}

/**
 * A compact badge displaying a capability icon with tooltip.
 * Used in ModelCard to show model capabilities like reasoning, vision, etc.
 */
export function CapabilityBadge({ icon: Icon, label, color, className }: CapabilityBadgeProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          className={cn(
            "inline-flex items-center justify-center rounded-md p-1",
            colorMap[color],
            className
          )}
        >
          <Icon className="h-3.5 w-3.5" />
        </span>
      </TooltipTrigger>
      <TooltipContent side="top">{label}</TooltipContent>
    </Tooltip>
  );
}
