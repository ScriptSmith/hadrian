import { memo, useState, useCallback } from "react";
import { ChevronDown, Brain } from "lucide-react";

import { Button } from "@/components/Button/Button";
import { StreamingMarkdown } from "@/components/StreamingMarkdown/StreamingMarkdown";
import { cn } from "@/utils/cn";
import { formatTokens } from "@/utils/formatters";

/**
 * ReasoningSection - Collapsible Display for Extended Thinking
 *
 * ## Architecture
 *
 * This component displays reasoning/thinking content from models that support
 * extended thinking (like Claude with reasoning enabled). It's designed to:
 *
 * 1. **Preserve Performance**: Uses memo and stable callbacks to prevent
 *    unnecessary re-renders during streaming.
 *
 * 2. **Collapse by Default**: Reasoning can be verbose, so it's collapsed
 *    by default to keep the main response visible.
 *
 * 3. **Stream-Aware**: Expands automatically when streaming reasoning content,
 *    then can be collapsed after streaming completes.
 *
 * ## Usage
 *
 * ```tsx
 * <ReasoningSection
 *   content={reasoningContent}
 *   isStreaming={isStreamingReasoning}
 *   tokenCount={usage?.reasoningTokens}
 * />
 * ```
 */

interface ReasoningSectionProps {
  /** The reasoning content to display */
  content: string;
  /** Whether the reasoning content is currently streaming */
  isStreaming?: boolean;
  /** Number of reasoning tokens (for display) */
  tokenCount?: number;
  /** Additional class names */
  className?: string;
}

function ReasoningSectionComponent({
  content,
  isStreaming = false,
  tokenCount,
  className,
}: ReasoningSectionProps) {
  // Auto-expand while streaming, but allow manual toggle
  const [isExpanded, setIsExpanded] = useState(false);
  const [userToggled, setUserToggled] = useState(false);

  // Show expanded when streaming (unless user explicitly collapsed it)
  const shouldExpand = isExpanded || (isStreaming && !userToggled);

  const handleToggle = useCallback(() => {
    setIsExpanded((prev) => !prev);
    setUserToggled(true);
  }, []);

  // Don't render if there's no content
  if (!content && !isStreaming) {
    return null;
  }

  return (
    <div className={cn("border rounded-lg overflow-hidden mb-3", className)}>
      {/* Header - always visible */}
      <Button
        variant="ghost"
        onClick={handleToggle}
        className={cn(
          "w-full flex items-center justify-between px-3 py-2 h-auto",
          "text-sm font-medium text-muted-foreground hover:text-foreground",
          "hover:bg-muted/50 rounded-none",
          shouldExpand && "bg-muted/30"
        )}
      >
        <div className="flex items-center gap-2">
          <Brain className="h-4 w-4" />
          <span>Reasoning</span>
          {tokenCount !== undefined && tokenCount > 0 && (
            <span className="text-xs text-muted-foreground">
              ({formatTokens(tokenCount)} tokens)
            </span>
          )}
          {isStreaming && (
            <span className="inline-flex items-center gap-1.5">
              <span className="h-1.5 w-1.5 rounded-full bg-primary animate-pulse" />
              <span className="text-xs text-primary">thinking...</span>
            </span>
          )}
        </div>
        <ChevronDown
          className={cn("h-4 w-4 transition-transform duration-200", shouldExpand && "rotate-180")}
        />
      </Button>

      {/* Content - collapsible */}
      <div
        className={cn(
          "overflow-hidden transition-all duration-200",
          shouldExpand ? "max-h-[500px] opacity-100" : "max-h-0 opacity-0"
        )}
      >
        {/* eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- scrollable region needs keyboard access (axe: scrollable-region-focusable) */}
        <div className="px-3 py-2 border-t bg-muted/20 overflow-y-auto max-h-[450px]" tabIndex={0}>
          {content ? (
            <StreamingMarkdown
              content={content}
              isStreaming={isStreaming}
              className="text-sm text-muted-foreground"
            />
          ) : isStreaming ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <span className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-typing-dot" />
              <span className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-typing-dot-delay-1" />
              <span className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-typing-dot-delay-2" />
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

export const ReasoningSection = memo(ReasoningSectionComponent);
