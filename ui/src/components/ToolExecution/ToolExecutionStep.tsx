import { memo, useState, useMemo } from "react";
import {
  ChevronDown,
  CheckCircle2,
  XCircle,
  Loader2,
  Clock,
  Terminal,
  FileSearch,
  Globe,
  Braces,
  Database,
  BarChart3,
  Eye,
  Maximize2,
} from "lucide-react";
import type {
  ToolExecution,
  Artifact,
  CodeArtifactData,
  DisplaySelectionData,
} from "@/components/chat-types";
import { cn } from "@/utils/cn";
import { Artifact as ArtifactComponent } from "@/components/Artifact";
import { HighlightedCode } from "@/components/HighlightedCode/HighlightedCode";
import { ArtifactThumbnail } from "./ArtifactThumbnail";

interface ToolExecutionStepProps {
  execution: ToolExecution;
  /** Callback when an artifact is clicked for expansion */
  onArtifactClick?: (artifact: Artifact) => void;
  /** Whether to show input artifacts expanded by default */
  defaultInputExpanded?: boolean;
}

/** Tool name to icon mapping */
const TOOL_ICONS: Record<string, typeof Terminal> = {
  code_interpreter: Terminal,
  file_search: FileSearch,
  web_search: Globe,
  js_code_interpreter: Braces,
  sql_query: Database,
  chart_render: BarChart3,
  display_artifacts: Eye,
};

/** Tool name to display name mapping */
const TOOL_NAMES: Record<string, string> = {
  code_interpreter: "Python",
  file_search: "File Search",
  web_search: "Web Search",
  js_code_interpreter: "JavaScript",
  sql_query: "SQL Query",
  chart_render: "Chart",
  display_artifacts: "Display",
};

/** Layout mode labels */
const LAYOUT_LABELS: Record<string, string> = {
  inline: "inline",
  gallery: "gallery",
  stacked: "stacked",
};

/** Tool name to language mapping for syntax highlighting */
const TOOL_LANGUAGES: Record<string, string> = {
  code_interpreter: "python",
  js_code_interpreter: "javascript",
  sql_query: "sql",
  chart_render: "json",
};

/** Format duration in human-readable form */
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`;
}

/** Extract code string from artifact data */
function getCodeFromArtifact(artifact: Artifact): string | null {
  if (artifact.type !== "code") return null;
  const data = artifact.data as CodeArtifactData;
  return data?.code || null;
}

/** Get a compact preview of code (first few lines) */
function getCodePreview(code: string, maxLines = 3): { preview: string; isTruncated: boolean } {
  const lines = code.split("\n");
  if (lines.length <= maxLines) {
    return { preview: code, isTruncated: false };
  }
  return {
    preview: lines.slice(0, maxLines).join("\n"),
    isTruncated: true,
  };
}

/**
 * Individual tool execution step in the timeline
 *
 * Shows the tool name, status indicator, inline code preview,
 * and output artifacts. Code is shown inline for visibility.
 */
function ToolExecutionStepComponent({
  execution,
  onArtifactClick,
  defaultInputExpanded = false,
}: ToolExecutionStepProps) {
  const [isFullCodeExpanded, setIsFullCodeExpanded] = useState(defaultInputExpanded);

  const Icon = TOOL_ICONS[execution.toolName] || Terminal;
  const displayName = TOOL_NAMES[execution.toolName] || execution.toolName;

  // Filter out display_selection artifacts - they're meta-artifacts for controlling display
  const visibleOutputArtifacts = useMemo(
    () => execution.outputArtifacts.filter((a) => a.type !== "display_selection"),
    [execution.outputArtifacts]
  );
  const hasOutputArtifacts = visibleOutputArtifacts.length > 0;

  // Extract inline code preview from first code input artifact
  const inlineCode = useMemo(() => {
    const codeArtifact = execution.inputArtifacts.find((a) => a.type === "code");
    if (!codeArtifact) return null;
    const code = getCodeFromArtifact(codeArtifact);
    if (!code) return null;
    const { preview, isTruncated } = getCodePreview(code, 4);
    const language =
      (codeArtifact.data as CodeArtifactData)?.language || TOOL_LANGUAGES[execution.toolName];
    return { code, preview, isTruncated, artifact: codeArtifact, language };
  }, [execution.inputArtifacts, execution.toolName]);

  // Other input artifacts (non-code)
  const otherInputArtifacts = useMemo(
    () => execution.inputArtifacts.filter((a) => a.type !== "code"),
    [execution.inputArtifacts]
  );

  // Extract display selection metadata for display_artifacts tool
  const displayMeta = useMemo(() => {
    if (execution.toolName !== "display_artifacts") return null;
    const sel = execution.outputArtifacts.find((a) => a.type === "display_selection");
    if (!sel) return null;
    const data = sel.data as DisplaySelectionData;
    return {
      count: data.artifactIds.length,
      layout: LAYOUT_LABELS[data.layout] || data.layout,
    };
  }, [execution.toolName, execution.outputArtifacts]);

  return (
    <div className="relative pl-5">
      {/* Timeline connector line */}
      <div className="absolute left-[7px] top-5 -bottom-1 w-px bg-zinc-200 dark:bg-zinc-700" />

      {/* Status indicator dot */}
      <div className="absolute left-0 top-1">
        {execution.status === "running" && (
          <Loader2 className="h-[14px] w-[14px] animate-spin text-blue-500" />
        )}
        {execution.status === "success" && (
          <CheckCircle2 className="h-[14px] w-[14px] text-emerald-500" />
        )}
        {execution.status === "error" && <XCircle className="h-[14px] w-[14px] text-red-500" />}
        {execution.status === "pending" && (
          <div className="h-[14px] w-[14px] rounded-full border-2 border-zinc-300 bg-white dark:border-zinc-600 dark:bg-zinc-800" />
        )}
      </div>

      {/* Step content */}
      <div className="pb-3">
        {/* Header row */}
        <div className="flex items-center gap-1.5 text-[13px]">
          <Icon className="h-3.5 w-3.5 text-zinc-600 dark:text-zinc-400" />
          <span className="font-medium text-zinc-700 dark:text-zinc-300">{displayName}</span>

          {/* Duration */}
          {execution.duration !== undefined && (
            <span className="flex items-center gap-0.5 text-[11px] text-zinc-600 dark:text-zinc-400">
              <Clock className="h-2.5 w-2.5" />
              {formatDuration(execution.duration)}
            </span>
          )}

          {/* Status badge for running - shows status message if available */}
          {execution.status === "running" && (
            <span className="rounded px-1.5 py-0.5 text-[10px] font-medium text-blue-700 bg-blue-50 dark:bg-blue-900/30 dark:text-blue-400">
              {execution.statusMessage || "running"}
            </span>
          )}
        </div>

        {/* Display artifacts metadata */}
        {displayMeta && (
          <div className="mt-1 text-[11px] text-zinc-500 dark:text-zinc-400">
            Showing {displayMeta.count} {displayMeta.count === 1 ? "artifact" : "artifacts"}
            {displayMeta.layout !== "inline" && <> &middot; {displayMeta.layout}</>}
          </div>
        )}

        {/* Inline code preview - always visible */}
        {inlineCode && (
          <div className="mt-1.5">
            <div
              className={cn(
                "relative group/code rounded-lg border overflow-hidden",
                "bg-muted/30",
                "border-zinc-200 dark:border-zinc-700",
                "hover:border-zinc-300 dark:hover:border-zinc-600 transition-colors"
              )}
            >
              {/* Header - matches Artifact component pattern */}
              <div className="flex items-center gap-2 px-3 py-1.5 border-b bg-muted/50">
                <Icon className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                <span className="text-xs font-medium text-muted-foreground truncate">
                  {inlineCode.language || displayName}
                </span>
                <div className="flex-1" />
                <button
                  type="button"
                  onClick={() => onArtifactClick?.(inlineCode.artifact)}
                  className={cn(
                    "p-0.5 rounded",
                    "text-zinc-400 hover:text-zinc-700 dark:text-zinc-500 dark:hover:text-zinc-200",
                    "opacity-0 group-hover/code:opacity-100 transition-opacity"
                  )}
                  aria-label="Expand"
                >
                  <Maximize2 className="h-3.5 w-3.5" />
                </button>
              </div>

              {/* Code content */}
              <HighlightedCode
                code={isFullCodeExpanded ? inlineCode.code : inlineCode.preview}
                language={inlineCode.language}
                compact
                showCopy
              />
              {inlineCode.isTruncated && (
                <button
                  type="button"
                  onClick={() => setIsFullCodeExpanded(!isFullCodeExpanded)}
                  className="flex items-center gap-1 w-full px-2.5 py-1 bg-zinc-100 dark:bg-zinc-800 border-t border-zinc-200 dark:border-zinc-700 text-[10px] text-zinc-600 dark:text-zinc-400 hover:text-zinc-800 dark:hover:text-zinc-200 transition-colors"
                >
                  <ChevronDown
                    className={cn(
                      "h-2.5 w-2.5 transition-transform",
                      isFullCodeExpanded && "rotate-180"
                    )}
                  />
                  {isFullCodeExpanded ? "collapse" : "expand"}
                </button>
              )}
            </div>
          </div>
        )}

        {/* Error message */}
        {execution.error && (
          <div className="mt-1.5 rounded-md bg-red-50 dark:bg-red-900/20 px-2.5 py-1.5 text-[11px] text-red-700 dark:text-red-400 font-mono">
            {execution.error}
          </div>
        )}

        {/* Other input artifacts */}
        {otherInputArtifacts.length > 0 && (
          <div className="mt-1.5 flex flex-wrap gap-1.5">
            {otherInputArtifacts.map((artifact) => (
              <ArtifactThumbnail
                key={artifact.id}
                artifact={artifact}
                onClick={() => onArtifactClick?.(artifact)}
              />
            ))}
          </div>
        )}

        {/* Output artifacts - shown inline with expand button */}
        {hasOutputArtifacts && (
          <div className="mt-2 space-y-2">
            {visibleOutputArtifacts.map((artifact) => (
              <div key={artifact.id} className="relative group">
                <ArtifactComponent artifact={artifact} className="text-sm" />
                {/* Expand to modal button */}
                <button
                  type="button"
                  onClick={() => onArtifactClick?.(artifact)}
                  className={cn(
                    "absolute top-2 right-2 p-1 rounded",
                    "bg-zinc-100/80 dark:bg-zinc-800/80 backdrop-blur-sm",
                    "text-zinc-500 hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200",
                    "opacity-0 group-hover:opacity-100 transition-opacity",
                    "border border-zinc-200 dark:border-zinc-700"
                  )}
                  aria-label="Expand"
                >
                  <Maximize2 className="h-3.5 w-3.5" />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export const ToolExecutionStep = memo(ToolExecutionStepComponent);
