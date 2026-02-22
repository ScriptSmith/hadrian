import { useState, type ReactNode } from "react";
import { Loader2, Check, Circle, ChevronDown, ChevronRight, type LucideIcon } from "lucide-react";

import type { MessageUsage } from "@/components/chat-types";
import { StreamingMarkdown } from "@/components/StreamingMarkdown/StreamingMarkdown";
import { cn } from "@/utils/cn";
import { getShortModelName } from "@/utils/modelName";

export { getShortModelName };

/** Phase/status configuration for progress containers */
export type ProgressPhase = "initial" | "active" | "complete" | "warning";

/** Container styling based on phase */
const PHASE_STYLES: Record<ProgressPhase, { container: string; icon: string }> = {
  initial: {
    container: "bg-blue-500/10 border-blue-500/30",
    icon: "text-blue-500",
  },
  active: {
    container: "bg-amber-500/10 border-amber-500/30",
    icon: "text-amber-500",
  },
  complete: {
    container: "bg-primary/5 border-primary/30",
    icon: "text-primary",
  },
  warning: {
    container: "bg-orange-500/10 border-orange-500/30",
    icon: "text-orange-500",
  },
};

/** Props passed to custom footer render function */
export interface FooterRenderProps {
  isExpanded: boolean;
  toggleExpand: () => void;
  hasExpandable: boolean;
}

/** Props for the expand button component */
interface ExpandButtonProps {
  isExpanded: boolean;
  onToggle: () => void;
  expandedLabel?: string;
  collapsedLabel?: string;
}

/**
 * ExpandButton - Reusable expand/collapse button for use in custom footers
 */
export function ExpandButton({
  isExpanded,
  onToggle,
  expandedLabel = "Hide details",
  collapsedLabel = "Show details",
}: ExpandButtonProps) {
  return (
    <button
      onClick={onToggle}
      className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground transition-colors"
    >
      {isExpanded ? (
        <>
          <ChevronDown className="h-3 w-3" />
          {expandedLabel}
        </>
      ) : (
        <>
          <ChevronRight className="h-3 w-3" />
          {collapsedLabel}
        </>
      )}
    </button>
  );
}

/**
 * Props for the progress container component
 */
interface ProgressContainerProps {
  /** Current phase determines styling */
  phase: ProgressPhase;
  /** Whether currently loading/streaming */
  isLoading?: boolean;
  /** Icon to display when not loading */
  icon: LucideIcon;
  /** Header content (mode name + status badge) */
  header: ReactNode;
  /** Main content */
  children: ReactNode;
  /** Expandable footer section (for history/sources) */
  expandableSection?: ReactNode;
  /** Label for expand button */
  expandLabel?: { collapsed: string; expanded: string };
  /** Whether expandable section should be shown */
  showExpandable?: boolean;
  /**
   * Custom footer render function for complex layouts.
   * When provided, replaces the default expand button with custom content.
   * Use this when you need the expand button inline with other content (e.g., usage summary).
   */
  renderFooter?: (props: FooterRenderProps) => ReactNode;
}

/**
 * ProgressContainer - Shared wrapper for all mode progress indicators
 *
 * Provides consistent styling, loading states, and expandable sections
 * across ChainProgress, SynthesisProgress, RefinementProgress, and CritiqueProgress.
 */
export function ProgressContainer({
  phase,
  isLoading = false,
  icon: Icon,
  header,
  children,
  expandableSection,
  expandLabel = { collapsed: "Show details", expanded: "Hide details" },
  showExpandable = true,
  renderFooter,
}: ProgressContainerProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const styles = PHASE_STYLES[phase];
  const hasExpandable = !!(expandableSection && showExpandable);
  const toggleExpand = () => setIsExpanded(!isExpanded);

  return (
    <div className={cn("rounded-lg border", styles.container)}>
      {/* Header section */}
      <div className="flex items-start gap-2 px-3 py-2">
        <div className="shrink-0 mt-0.5">
          {isLoading ? (
            <Loader2 className={cn("h-4 w-4 animate-spin", styles.icon)} />
          ) : (
            <Icon className={cn("h-4 w-4", styles.icon)} />
          )}
        </div>

        <div className="flex-1 min-w-0">
          {header}
          {children}

          {/* Custom footer or default expand button */}
          {phase === "complete" &&
            (renderFooter
              ? renderFooter({ isExpanded, toggleExpand, hasExpandable })
              : hasExpandable && (
                  <div className="mt-1 flex items-center justify-end">
                    <button
                      onClick={toggleExpand}
                      className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground transition-colors"
                    >
                      {isExpanded ? (
                        <>
                          <ChevronDown className="h-3 w-3" />
                          {expandLabel.expanded}
                        </>
                      ) : (
                        <>
                          <ChevronRight className="h-3 w-3" />
                          {expandLabel.collapsed}
                        </>
                      )}
                    </button>
                  </div>
                ))}
        </div>
      </div>

      {/* Expandable section */}
      {hasExpandable && phase === "complete" && isExpanded && (
        <div className="border-t border-border/50 px-3 py-2 space-y-3">{expandableSection}</div>
      )}
    </div>
  );
}

/**
 * Props for status badge
 */
interface StatusBadgeProps {
  /** Text to display */
  text: string;
  /** Badge variant */
  variant: "initial" | "active" | "complete" | "warning";
}

const BADGE_STYLES: Record<StatusBadgeProps["variant"], string> = {
  initial: "bg-blue-500/20 text-blue-700 dark:text-blue-400",
  active: "bg-amber-500/20 text-amber-800 dark:text-amber-400",
  complete: "bg-primary/10 text-primary",
  warning: "bg-orange-500/20 text-orange-800 dark:text-orange-400",
};

/**
 * StatusBadge - Small pill showing current phase status
 */
export function StatusBadge({ text, variant }: StatusBadgeProps) {
  return (
    <span className={cn("px-1.5 py-0.5 rounded text-[9px] font-semibold", BADGE_STYLES[variant])}>
      {text}
    </span>
  );
}

/**
 * Props for mode header
 */
interface ModeHeaderProps {
  /** Mode name (e.g., "Synthesized", "Refined") */
  name: string;
  /** Status badge */
  badge?: ReactNode;
}

/**
 * ModeHeader - Consistent header with name and optional badge
 *
 * Note: The icon is rendered by ProgressContainer (with loading spinner support),
 * so ModeHeader only handles the name and badge.
 */
export function ModeHeader({ name, badge }: ModeHeaderProps) {
  return (
    <div className="flex items-center gap-2 text-xs font-medium">
      <span className="text-muted-foreground">{name}</span>
      {badge}
    </div>
  );
}

/**
 * Props for model badge
 */
interface ModelBadgeProps {
  /** Model identifier */
  model: string;
  /** Badge variant */
  variant?: "default" | "primary" | "blue" | "amber" | "orange";
  /** Show checkmark icon */
  showCheck?: boolean;
  /** Show loading circle */
  showLoading?: boolean;
}

const MODEL_BADGE_STYLES: Record<NonNullable<ModelBadgeProps["variant"]>, string> = {
  default: "bg-muted text-muted-foreground",
  primary: "bg-primary/5 text-primary",
  blue: "bg-blue-500/20 text-blue-700 dark:text-blue-400",
  amber: "bg-amber-500/20 text-amber-800 dark:text-amber-400",
  orange: "bg-orange-500/20 text-orange-800 dark:text-orange-400",
};

/**
 * ModelBadge - Small badge showing a model name with optional status indicator
 */
export function ModelBadge({
  model,
  variant = "default",
  showCheck = false,
  showLoading = false,
}: ModelBadgeProps) {
  return (
    <div
      className={cn(
        "flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors",
        MODEL_BADGE_STYLES[variant]
      )}
    >
      {showCheck && <Check className="h-3 w-3" />}
      {showLoading && <Circle className="h-3 w-3 animate-pulse" />}
      <span className="max-w-[80px] truncate">{getShortModelName(model)}</span>
    </div>
  );
}

/**
 * Props for usage summary
 */
interface UsageSummaryProps {
  /** Total tokens used */
  totalTokens: number;
  /** Total cost in dollars */
  totalCost?: number;
  /** Optional label prefix */
  label?: string;
}

/**
 * UsageSummary - Display token count and cost in a compact format
 */
export function UsageSummary({ totalTokens, totalCost, label }: UsageSummaryProps) {
  if (totalTokens === 0) return null;

  return (
    <span className="text-[9px] text-muted-foreground px-1.5 py-0.5 bg-muted/50 rounded">
      {label && `${label}: `}
      {totalTokens.toLocaleString()} tokens
      {totalCost !== undefined && totalCost > 0 && ` / $${totalCost.toFixed(4)}`}
    </span>
  );
}

/**
 * Props for expandable response card
 */
interface ResponseCardProps {
  /** Card title (model name or description) */
  title: string;
  /** Response content */
  content: string;
  /** Usage data */
  usage?: MessageUsage;
  /** Card variant for border color */
  variant?: "default" | "blue" | "orange";
  /** Preview length before truncation */
  previewLength?: number;
}

const CARD_STYLES: Record<
  NonNullable<ResponseCardProps["variant"]>,
  { border: string; header: string; bg: string }
> = {
  default: {
    border: "border-border/50",
    header: "border-border/30",
    bg: "bg-background/50",
  },
  blue: {
    border: "border-blue-500/30",
    header: "border-blue-500/20",
    bg: "bg-blue-500/5",
  },
  orange: {
    border: "border-orange-500/30",
    header: "border-orange-500/20",
    bg: "bg-orange-500/5",
  },
};

/**
 * ResponseCard - Expandable card showing a response with truncation support
 */
export function ResponseCard({
  title,
  content,
  usage,
  variant = "default",
  previewLength = 200,
}: ResponseCardProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const styles = CARD_STYLES[variant];

  const needsTruncation = content.length > previewLength;
  const previewContent = needsTruncation ? content.slice(0, previewLength) + "..." : content;

  return (
    <div className={cn("rounded border", styles.border, styles.bg)}>
      <div className={cn("flex items-center justify-between px-2 py-1.5 border-b", styles.header)}>
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "px-1.5 py-0.5 rounded text-[10px] font-medium",
              variant === "blue"
                ? "bg-blue-500/20 text-blue-700 dark:text-blue-400"
                : variant === "orange"
                  ? "bg-orange-500/20 text-orange-800 dark:text-orange-400"
                  : "bg-muted text-muted-foreground"
            )}
          >
            {title}
          </span>
          {usage && (
            <span className="text-[9px] text-muted-foreground">{usage.totalTokens} tokens</span>
          )}
        </div>
        {needsTruncation && (
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="text-[10px] text-muted-foreground hover:text-foreground transition-colors"
          >
            {isExpanded ? "Collapse" : "Expand"}
          </button>
        )}
      </div>
      <div className="px-2 py-1.5 text-xs">
        <StreamingMarkdown content={isExpanded ? content : previewContent} isStreaming={false} />
      </div>
    </div>
  );
}
