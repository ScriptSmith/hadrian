/**
 * AgentArtifact - Sub-Agent Task, Internal Reasoning, and Output Display
 *
 * Renders the results of a sub-agent tool execution showing:
 * - Task: The original request from the parent model (collapsible)
 * - Internal: The agent's investigation/reasoning (collapsible, not sent to parent)
 * - Output: The curated response sent back to the parent model (always visible)
 */

import { memo, useState } from "react";
import { Bot, ChevronDown, ChevronRight, Copy, Check, Brain, MessageSquare } from "lucide-react";

import type { Artifact, AgentArtifactData } from "@/components/chat-types";
import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { StreamingMarkdown } from "@/components/StreamingMarkdown/StreamingMarkdown";
import { cn } from "@/utils/cn";
import { formatTokens, formatCost } from "@/utils/formatters";

export interface AgentArtifactProps {
  artifact: Artifact;
  className?: string;
}

function isAgentArtifactData(data: unknown): data is AgentArtifactData {
  return (
    typeof data === "object" &&
    data !== null &&
    "task" in data &&
    "model" in data &&
    "internal" in data &&
    "output" in data &&
    typeof (data as AgentArtifactData).task === "string" &&
    typeof (data as AgentArtifactData).model === "string" &&
    typeof (data as AgentArtifactData).internal === "string" &&
    typeof (data as AgentArtifactData).output === "string"
  );
}

/** Format model ID to display name */
function formatModelName(model: string): string {
  const parts = model.split("/");
  return parts[parts.length - 1];
}

/** Collapsible section component */
function CollapsibleSection({
  label,
  icon: Icon,
  content,
  preview,
  expanded,
  onToggle,
  className,
}: {
  label: string;
  icon: typeof Brain;
  content: string;
  preview?: string;
  expanded: boolean;
  onToggle: () => void;
  className?: string;
}) {
  return (
    <>
      <button
        onClick={onToggle}
        className={cn(
          "w-full flex items-center gap-2 px-3 py-2 text-left",
          "hover:bg-muted/30 transition-colors",
          "border-b border-dashed",
          className
        )}
      >
        {expanded ? (
          <ChevronDown className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
        ) : (
          <ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
        )}
        <Icon className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
        <span className="text-xs font-medium text-muted-foreground">{label}</span>
        {!expanded && preview && (
          <span className="text-xs text-muted-foreground truncate flex-1">{preview}</span>
        )}
      </button>
      {expanded && (
        // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- scrollable region needs keyboard access (axe: scrollable-region-focusable)
        <div className="px-4 py-3 bg-muted/20 border-b max-h-[300px] overflow-y-auto" tabIndex={0}>
          <StreamingMarkdown content={content} isStreaming={false} />
        </div>
      )}
    </>
  );
}

function AgentArtifactComponent({ artifact, className }: AgentArtifactProps) {
  const [taskExpanded, setTaskExpanded] = useState(false);
  const [internalExpanded, setInternalExpanded] = useState(false);
  const [copied, setCopied] = useState(false);

  // Validate and extract data
  if (!isAgentArtifactData(artifact.data)) {
    return <div className="p-4 text-sm text-muted-foreground">Invalid agent artifact data</div>;
  }

  const { task, model, internal, output, usage } = artifact.data;

  const handleCopy = async () => {
    await navigator.clipboard.writeText(output);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className={cn("relative group", className)}>
      {/* Header with model info */}
      <div className="flex items-center gap-2 px-3 py-2 bg-muted/30 border-b">
        <Bot className="h-4 w-4 text-primary shrink-0" />
        <span className="text-sm font-medium text-muted-foreground">{formatModelName(model)}</span>

        {/* Usage stats */}
        {usage && (
          <div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
            <span>{formatTokens(usage.totalTokens)} tokens</span>
            {usage.cost !== undefined && usage.cost > 0 && (
              <span className="text-muted-foreground">{formatCost(usage.cost)}</span>
            )}
          </div>
        )}
      </div>

      {/* Task section (collapsible) */}
      <CollapsibleSection
        label="Task"
        icon={MessageSquare}
        content={task}
        preview={task}
        expanded={taskExpanded}
        onToggle={() => setTaskExpanded(!taskExpanded)}
      />

      {/* Internal reasoning section (collapsible) */}
      <CollapsibleSection
        label="Internal"
        icon={Brain}
        content={internal}
        preview={internal.slice(0, 100) + (internal.length > 100 ? "..." : "")}
        expanded={internalExpanded}
        onToggle={() => setInternalExpanded(!internalExpanded)}
      />

      {/* Output section (always visible) */}
      <div className="relative">
        {/* Section label */}
        <div className="flex items-center gap-2 px-3 py-2 border-b bg-primary/5">
          <MessageSquare className="h-3.5 w-3.5 text-primary shrink-0" />
          <span className="text-xs font-medium text-primary">Output</span>
          <span className="text-xs text-muted-foreground">(sent to parent model)</span>
        </div>

        {/* Copy button */}
        <div className="absolute right-2 top-10 opacity-0 group-hover:opacity-100 transition-opacity z-10">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="secondary"
                size="sm"
                className="h-7 w-7 p-0"
                onClick={handleCopy}
                aria-label={copied ? "Copied" : "Copy output"}
              >
                {copied ? <Check className="h-3.5 w-3.5" /> : <Copy className="h-3.5 w-3.5" />}
              </Button>
            </TooltipTrigger>
            <TooltipContent>{copied ? "Copied!" : "Copy output"}</TooltipContent>
          </Tooltip>
        </div>

        {/* Output content */}
        {/* eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- scrollable region needs keyboard access (axe: scrollable-region-focusable) */}
        <div className="p-4 max-h-[400px] overflow-y-auto" tabIndex={0}>
          <StreamingMarkdown content={output} isStreaming={false} />
        </div>
      </div>
    </div>
  );
}

export const AgentArtifact = memo(AgentArtifactComponent);
