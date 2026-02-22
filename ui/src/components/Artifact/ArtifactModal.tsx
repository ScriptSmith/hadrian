/**
 * ArtifactModal - Full-size artifact display in a modal dialog
 *
 * Opens when clicking on an ArtifactThumbnail to show the full artifact content.
 * Supports all artifact types: code, table, chart, image, html.
 */

import { memo } from "react";
import { X, Code2, Table2, Image, BarChart3, Globe } from "lucide-react";

import type {
  Artifact as ArtifactType,
  ArtifactType as ArtifactKind,
} from "@/components/chat-types";
import { Modal, ModalTitle } from "@/components/Modal/Modal";
import { Button } from "@/components/Button/Button";
import { cn } from "@/utils/cn";

import { CodeArtifact } from "./CodeArtifact";
import { TableArtifact } from "./TableArtifact";
import { ImageArtifact } from "./ImageArtifact";
import { ChartArtifact } from "./ChartArtifact";
import { HtmlArtifact } from "./HtmlArtifact";

export interface ArtifactModalProps {
  artifact: ArtifactType | null;
  open: boolean;
  onClose: () => void;
}

/** Get icon for artifact type */
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
    default:
      return "Output";
  }
}

/** Render artifact content based on type */
function ArtifactContent({ artifact }: { artifact: ArtifactType }) {
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
    default:
      return (
        <div className="flex items-center gap-2 text-sm text-muted-foreground p-4">
          <span>Unknown artifact type</span>
        </div>
      );
  }
}

function ArtifactModalComponent({ artifact, open, onClose }: ArtifactModalProps) {
  if (!artifact) return null;

  const label = artifact.title || getArtifactLabel(artifact.type);

  return (
    <Modal
      open={open}
      onClose={onClose}
      className={cn(
        "max-w-4xl w-[90vw] max-h-[85vh] p-0 flex flex-col",
        // Larger size for charts and tables
        (artifact.type === "chart" || artifact.type === "table") && "max-w-5xl"
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between border-b px-4 py-3 shrink-0">
        <div className="flex items-center gap-2">
          <ArtifactIcon type={artifact.type} className="h-5 w-5 text-muted-foreground" />
          <ModalTitle className="text-base font-semibold">{label}</ModalTitle>
          {artifact.role === "input" && (
            <span className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
              Input
            </span>
          )}
        </div>
        <Button variant="ghost" size="icon" onClick={onClose} className="h-8 w-8">
          <X className="h-4 w-4" />
          <span className="sr-only">Close</span>
        </Button>
      </div>

      {/* Content - scrollable */}
      <div className="flex-1 overflow-auto p-4">
        <ArtifactContent artifact={artifact} />
      </div>
    </Modal>
  );
}

export const ArtifactModal = memo(ArtifactModalComponent);
