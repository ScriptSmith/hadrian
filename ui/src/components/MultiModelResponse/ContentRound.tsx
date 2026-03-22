import { memo, useState, useCallback, useMemo } from "react";
import type { ToolExecutionRound, Artifact, DisplaySelectionData } from "@/components/chat-types";
import { Artifact as ArtifactComponent } from "@/components/Artifact";
import { ReasoningSection } from "@/components/ReasoningSection/ReasoningSection";
import { StreamingMarkdown } from "@/components/StreamingMarkdown/StreamingMarkdown";
import { ExecutionSummaryBar, ExecutionTimeline } from "@/components/ToolExecution";
import { useCompactMode } from "@/stores/chatUIStore";

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
  /** Display selection if display_artifacts was called in this round */
  displaySelection?: DisplaySelectionData | null;
  /** All output artifacts across all rounds (for resolving display selection IDs) */
  allOutputArtifacts?: Artifact[];
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
  displaySelection,
  allOutputArtifacts,
}: ContentRoundProps) {
  const [toolsExpanded, setToolsExpanded] = useState(false);
  const handleToggleTools = useCallback(() => setToolsExpanded((p) => !p), []);
  const compactMode = useCompactMode();

  // Resolve display selection to actual artifacts
  const displayedArtifacts = useMemo(() => {
    if (!displaySelection?.artifactIds.length || !allOutputArtifacts) return [];
    const displayed: Artifact[] = [];
    for (const id of displaySelection.artifactIds) {
      const artifact = allOutputArtifacts.find((a) => a.id === id);
      if (artifact) displayed.push(artifact);
    }
    return displayed;
  }, [displaySelection, allOutputArtifacts]);

  const hasContent = !!content?.trim();
  const hasReasoning = !!reasoning;
  const hasTools = !!toolExecutionRound;
  const hasDisplayedArtifacts = displayedArtifacts.length > 0;

  if (!hasContent && !hasReasoning && !hasTools && !hasDisplayedArtifacts) return null;

  if (compactMode) {
    // Compact: show content + artifacts only; collapse reasoning/tool-only rounds
    if (hasContent || hasDisplayedArtifacts) {
      const layoutClass =
        displaySelection?.layout === "gallery" ? "grid grid-cols-2 gap-3" : "space-y-3";
      return (
        <div className="space-y-1 border-l-2 border-transparent pl-3 transition-colors hover:border-zinc-200 dark:hover:border-zinc-700">
          {hasContent && <StreamingMarkdown content={content!} isStreaming={isStreaming} />}
          {hasDisplayedArtifacts && (
            <div className={layoutClass}>
              {displayedArtifacts.map((artifact) => (
                <ArtifactComponent key={artifact.id} artifact={artifact} />
              ))}
            </div>
          )}
        </div>
      );
    }
    // No content in compact mode — parent manages status indicators
    return null;
  }

  const layoutClass =
    displaySelection?.layout === "gallery" ? "grid grid-cols-2 gap-3" : "space-y-3";

  return (
    <div className="space-y-1 border-l-2 border-transparent pl-3 transition-colors hover:border-zinc-200 dark:hover:border-zinc-700">
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
      {hasDisplayedArtifacts && (
        <div className={layoutClass}>
          {displayedArtifacts.map((artifact) => (
            <ArtifactComponent key={artifact.id} artifact={artifact} />
          ))}
        </div>
      )}
    </div>
  );
}

export const ContentRound = memo(ContentRoundComponent);
