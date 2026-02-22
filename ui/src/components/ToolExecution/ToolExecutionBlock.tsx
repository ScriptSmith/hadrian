import { memo, useState, useCallback, useMemo, useEffect } from "react";
import type { ToolExecutionRound, Artifact, DisplaySelectionData } from "@/components/chat-types";
import { Artifact as ArtifactComponent } from "@/components/Artifact";
import { ExecutionSummaryBar } from "./ExecutionSummaryBar";
import { ExecutionTimeline } from "./ExecutionTimeline";

interface ToolExecutionBlockProps {
  rounds: ToolExecutionRound[];
  /** Whether execution is still in progress */
  isStreaming?: boolean;
  /** Callback when an artifact is clicked for expansion */
  onArtifactClick?: (artifact: Artifact) => void;
  /** Start expanded (useful for single-tool, no-error cases) */
  defaultExpanded?: boolean;
  /**
   * Display selection from the model's display_artifacts call.
   * When provided, selected artifacts render inline at full size.
   */
  displaySelection?: DisplaySelectionData | null;
}

/**
 * Container for tool execution timeline with progressive disclosure.
 *
 * Default view shows:
 * - Compact summary bar (tool count, duration, retries)
 * - Model-selected artifacts inline at full size
 *
 * Expanded view shows:
 * - Full timeline with all rounds
 * - Each tool call with input/output
 * - Model reasoning between rounds
 *
 * During streaming:
 * - Auto-expands to show real-time progress
 * - Collapses automatically when streaming completes
 */
function ToolExecutionBlockComponent({
  rounds,
  isStreaming = false,
  onArtifactClick,
  defaultExpanded = false,
  displaySelection,
}: ToolExecutionBlockProps) {
  const [isManuallyExpanded, setIsManuallyExpanded] = useState(defaultExpanded);
  // Track if user has manually toggled during this streaming session
  const [userOverride, setUserOverride] = useState(false);

  // Auto-expand during streaming, collapse when done (unless user overrode)
  const isExpanded = useMemo(() => {
    if (userOverride) {
      return isManuallyExpanded;
    }
    // Auto-expand during streaming to show real-time progress
    if (isStreaming) {
      return true;
    }
    // After streaming completes, collapse to summary view
    return isManuallyExpanded;
  }, [isStreaming, isManuallyExpanded, userOverride]);

  // Reset user override when streaming starts (new execution session)
  useEffect(() => {
    if (isStreaming) {
      setUserOverride(false);
    }
  }, [isStreaming]);

  const handleToggle = useCallback(() => {
    setIsManuallyExpanded((prev) => !prev);
    setUserOverride(true);
  }, []);

  // Extract all output artifacts for prominent display
  const outputArtifacts = useMemo(() => {
    const artifacts: Artifact[] = [];
    for (const round of rounds) {
      for (const execution of round.executions) {
        artifacts.push(...execution.outputArtifacts);
      }
    }
    return artifacts;
  }, [rounds]);

  // Get artifacts to display inline (from model's display_artifacts call)
  const displayedArtifacts = useMemo(() => {
    if (!displaySelection || !displaySelection.artifactIds.length) {
      return [];
    }

    // Maintain order from selection
    const displayed: Artifact[] = [];
    for (const id of displaySelection.artifactIds) {
      const artifact = outputArtifacts.find((a) => a.id === id);
      if (artifact) {
        displayed.push(artifact);
      }
    }
    return displayed;
  }, [displaySelection, outputArtifacts]);

  // Don't render if no rounds
  if (rounds.length === 0) {
    return null;
  }

  // Get layout class based on display selection layout mode
  const layoutClass =
    displaySelection?.layout === "gallery"
      ? "grid grid-cols-2 gap-3"
      : displaySelection?.layout === "stacked"
        ? "space-y-3"
        : "space-y-3"; // inline is default, same as stacked

  return (
    <div className="space-y-3">
      {/* Model-selected artifacts rendered inline at full size */}
      {!isExpanded && displayedArtifacts.length > 0 && (
        <div className={layoutClass}>
          {displayedArtifacts.map((artifact) => (
            <ArtifactComponent key={artifact.id} artifact={artifact} />
          ))}
        </div>
      )}

      {/* Summary bar (clickable to expand/collapse timeline) */}
      <ExecutionSummaryBar
        rounds={rounds}
        isExpanded={isExpanded}
        onToggle={handleToggle}
        isStreaming={isStreaming}
      />

      {/* Expanded timeline */}
      {isExpanded && <ExecutionTimeline rounds={rounds} onArtifactClick={onArtifactClick} />}
    </div>
  );
}

export const ToolExecutionBlock = memo(ToolExecutionBlockComponent);
