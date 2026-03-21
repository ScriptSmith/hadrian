import { memo, useState, useCallback } from "react";
import type { ToolExecutionRound, Artifact } from "@/components/chat-types";
import { ReasoningSection } from "@/components/ReasoningSection/ReasoningSection";
import { StreamingMarkdown } from "@/components/StreamingMarkdown/StreamingMarkdown";
import { ExecutionSummaryBar, ExecutionTimeline } from "@/components/ToolExecution";

interface ContentRoundProps {
  /** Round's reasoning content */
  reasoning?: string | null;
  /** Round's text content */
  content?: string | null;
  /** Whether text content is actively streaming */
  isStreaming?: boolean;
  /** Whether reasoning is actively streaming */
  isReasoningStreaming?: boolean;
  /** Token count for reasoning (shown in ReasoningSection) */
  reasoningTokenCount?: number;
  /** Tool execution round for this round (if tools were called) */
  toolExecutionRound?: ToolExecutionRound;
  /** Whether tool execution is still in progress for this round */
  isToolsStreaming?: boolean;
  /** Artifact click handler for tool execution timeline */
  onArtifactClick?: (artifact: Artifact) => void;
}

/**
 * A single round of model output: reasoning → content → tool execution summary.
 *
 * Used in multi-round tool calling to render each iteration as a distinct block
 * with consistent spacing, replacing raw `<hr>` separators.
 */
function ContentRoundComponent({
  reasoning,
  content,
  isStreaming = false,
  isReasoningStreaming = false,
  reasoningTokenCount,
  toolExecutionRound,
  isToolsStreaming = false,
  onArtifactClick,
}: ContentRoundProps) {
  const [toolsExpanded, setToolsExpanded] = useState(false);
  const handleToggleTools = useCallback(() => setToolsExpanded((p) => !p), []);

  const hasContent = !!content?.trim();
  const hasReasoning = !!reasoning;
  const hasTools = !!toolExecutionRound;

  if (!hasContent && !hasReasoning && !hasTools) return null;

  return (
    <div className="space-y-1 border-l-2 border-zinc-200 pl-3 dark:border-zinc-700">
      {hasReasoning && (
        <ReasoningSection
          content={reasoning!}
          isStreaming={isReasoningStreaming}
          tokenCount={reasoningTokenCount}
        />
      )}
      {hasContent && <StreamingMarkdown content={content!} isStreaming={isStreaming} />}
      {hasTools && (
        <div className="mt-1.5">
          <ExecutionSummaryBar
            rounds={[toolExecutionRound!]}
            isExpanded={toolsExpanded}
            onToggle={handleToggleTools}
            isStreaming={isToolsStreaming}
          />
          {toolsExpanded && (
            <ExecutionTimeline rounds={[toolExecutionRound!]} onArtifactClick={onArtifactClick} />
          )}
        </div>
      )}
    </div>
  );
}

export const ContentRound = memo(ContentRoundComponent);
