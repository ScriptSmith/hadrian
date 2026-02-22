/**
 * Artifact - Container Component for Tool Execution Outputs
 *
 * Routes artifacts to their type-specific renderers based on the artifact type.
 * Artifacts are rich output objects produced by tool execution (Python, SQL, charts, etc.)
 * that are displayed in the UI but not sent back to the model.
 *
 * ## Supported Types
 *
 * - `code`: Syntax-highlighted code output (from code_interpreter, etc.)
 * - `table`: Data table with optional sorting (from SQL queries, data analysis)
 * - `image`: Images with download option (from matplotlib, image generation)
 * - `chart`: Vega-Lite charts (data visualizations)
 * - `html`: Sandboxed HTML preview (rendered HTML snippets)
 * - `agent`: Sub-agent task and response in a collapsible card
 */

import { memo } from "react";
import { Code2, Table2, Image, BarChart3, Globe, Bot, Search, AlertCircle } from "lucide-react";

import type {
  Artifact as ArtifactType,
  ArtifactType as ArtifactKind,
} from "@/components/chat-types";
import { cn } from "@/utils/cn";

import { CodeArtifact } from "./CodeArtifact";
import { TableArtifact } from "./TableArtifact";
import { ImageArtifact } from "./ImageArtifact";
import { ChartArtifact } from "./ChartArtifact";
import { HtmlArtifact } from "./HtmlArtifact";
import { AgentArtifact } from "./AgentArtifact";
import { FileSearchArtifact } from "./FileSearchArtifact";

export interface ArtifactProps {
  artifact: ArtifactType;
  className?: string;
}

/** Render icon for artifact type */
function ArtifactIcon({ type, className }: { type: ArtifactKind; className?: string }) {
  switch (type) {
    case "code":
      return <Code2 className={className} />;
    case "table":
      return <Table2 className={className} />;
    case "image":
      return <Image className={className} />;
    case "chart":
      return <BarChart3 className={className} />;
    case "html":
      return <Globe className={className} />;
    case "agent":
      return <Bot className={className} />;
    case "file_search":
      return <Search className={className} />;
    default:
      return <Code2 className={className} />;
  }
}

/** Get label for artifact type */
function getArtifactLabel(type: ArtifactKind): string {
  switch (type) {
    case "code":
      return "Code Output";
    case "table":
      return "Data Table";
    case "image":
      return "Image";
    case "chart":
      return "Chart";
    case "html":
      return "HTML Preview";
    case "agent":
      return "Sub-Agent";
    case "file_search":
      return "Knowledge Base Search";
    default:
      return "Output";
  }
}

function ArtifactComponent({ artifact, className }: ArtifactProps) {
  const label = artifact.title || getArtifactLabel(artifact.type);

  const renderContent = () => {
    switch (artifact.type) {
      case "code":
        return <CodeArtifact artifact={artifact} />;
      case "table":
        return <TableArtifact artifact={artifact} />;
      case "image":
        return <ImageArtifact artifact={artifact} />;
      case "chart":
        return <ChartArtifact artifact={artifact} />;
      case "html":
        return <HtmlArtifact artifact={artifact} />;
      case "agent":
        return <AgentArtifact artifact={artifact} />;
      case "file_search":
        return <FileSearchArtifact artifact={artifact} />;
      default:
        return (
          <div className="flex items-center gap-2 text-sm text-muted-foreground p-4">
            <AlertCircle className="h-4 w-4" />
            <span>Unknown artifact type: {artifact.type}</span>
          </div>
        );
    }
  };

  return (
    <div
      className={cn(
        "rounded-lg border bg-muted/30 overflow-hidden",
        "transition-colors hover:border-primary/30",
        className
      )}
    >
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b bg-muted/50">
        <ArtifactIcon type={artifact.type} className="h-4 w-4 text-muted-foreground shrink-0" />
        <span className="text-sm font-medium truncate">{label}</span>
      </div>

      {/* Content */}
      <div className="overflow-auto">{renderContent()}</div>
    </div>
  );
}

export const Artifact = memo(ArtifactComponent);

/**
 * ArtifactList - Render multiple artifacts
 */
export interface ArtifactListProps {
  artifacts: ArtifactType[];
  className?: string;
}

function ArtifactListComponent({ artifacts, className }: ArtifactListProps) {
  if (artifacts.length === 0) return null;

  return (
    <div className={cn("space-y-3", className)}>
      {artifacts.map((artifact) => (
        <Artifact key={artifact.id} artifact={artifact} />
      ))}
    </div>
  );
}

export const ArtifactList = memo(ArtifactListComponent);
