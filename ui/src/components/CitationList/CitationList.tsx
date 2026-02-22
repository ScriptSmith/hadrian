import { FileText, ExternalLink, ChevronDown, ChevronUp } from "lucide-react";
import { useState, memo } from "react";

import { Button } from "@/components/Button/Button";
import { cn } from "@/utils/cn";

/** Types of citations that can be displayed */
export type CitationType = "file" | "url" | "chunk";

/** Base citation interface */
interface BaseCitation {
  /** Unique identifier */
  id: string;
  /** Type of citation */
  type: CitationType;
  /** Relevance score (0-1) if available */
  score?: number;
}

/** File citation from vector store search */
export interface FileCitation extends BaseCitation {
  type: "file";
  /** File ID in the vector store */
  fileId: string;
  /** Display filename */
  filename: string;
  /** Optional chunk ID within the file */
  chunkId?: string;
  /** Optional snippet of the content */
  snippet?: string;
  /** Character range in the original file */
  charRange?: { start: number; end: number };
}

/** URL citation from web search */
export interface UrlCitation extends BaseCitation {
  type: "url";
  /** The source URL */
  url: string;
  /** Page title */
  title: string;
  /** Optional snippet of the content */
  snippet?: string;
}

/** Chunk citation with full content preview */
export interface ChunkCitation extends BaseCitation {
  type: "chunk";
  /** File ID containing this chunk */
  fileId: string;
  /** Display filename */
  filename: string;
  /** Chunk index within the file */
  chunkIndex: number;
  /** Full chunk content */
  content: string;
  /** Token count of the chunk */
  tokenCount?: number;
}

/** Union type for all citation types */
export type Citation = FileCitation | UrlCitation | ChunkCitation;

interface CitationItemProps {
  citation: Citation;
  isExpanded: boolean;
  onToggle: () => void;
  onFileClick?: (fileId: string, chunkId?: string) => void;
  onUrlClick?: (url: string) => void;
}

/** Single citation item with expand/collapse */
const CitationItem = memo(function CitationItem({
  citation,
  isExpanded,
  onToggle,
  onFileClick,
  onUrlClick,
}: CitationItemProps) {
  const hasExpandableContent =
    (citation.type === "file" && citation.snippet) ||
    citation.type === "chunk" ||
    (citation.type === "url" && citation.snippet);

  const handleClick = () => {
    if (citation.type === "file" && onFileClick) {
      onFileClick(citation.fileId, citation.chunkId);
    } else if (citation.type === "url" && onUrlClick) {
      onUrlClick(citation.url);
    } else if (citation.type === "chunk" && onFileClick) {
      onFileClick(citation.fileId);
    }
  };

  return (
    <div
      className={cn(
        "border rounded-lg overflow-hidden transition-colors",
        "hover:border-primary/50 hover:bg-muted/30"
      )}
    >
      <div className="flex items-center gap-2 px-3 py-2">
        {/* Icon */}
        {citation.type === "url" ? (
          <ExternalLink className="h-4 w-4 text-green-500 shrink-0" />
        ) : (
          <FileText className="h-4 w-4 text-blue-500 shrink-0" />
        )}

        {/* Title/filename */}
        <button
          type="button"
          className="text-sm font-medium text-foreground hover:text-primary truncate text-left flex-1 min-w-0"
          onClick={handleClick}
        >
          {citation.type === "url" ? citation.title : citation.filename}
        </button>

        {/* Score badge */}
        {citation.score !== undefined && (
          <span
            className={cn(
              "text-[10px] px-1.5 py-0.5 rounded font-medium shrink-0",
              citation.score >= 0.8
                ? "bg-success/10 text-success"
                : citation.score >= 0.6
                  ? "bg-amber-500/10 text-amber-800 dark:text-amber-400"
                  : "bg-muted text-muted-foreground"
            )}
          >
            {Math.round(citation.score * 100)}%
          </span>
        )}

        {/* Chunk info */}
        {citation.type === "chunk" && (
          <span className="text-[10px] text-muted-foreground shrink-0">
            chunk {citation.chunkIndex + 1}
            {citation.tokenCount && ` (${citation.tokenCount} tokens)`}
          </span>
        )}

        {/* Expand button */}
        {hasExpandableContent && (
          <Button
            variant="ghost"
            size="sm"
            className="h-6 w-6 p-0 shrink-0"
            onClick={onToggle}
            aria-label={isExpanded ? "Collapse citation" : "Expand citation"}
            aria-expanded={isExpanded}
          >
            {isExpanded ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
          </Button>
        )}
      </div>

      {/* Expanded content */}
      {isExpanded && hasExpandableContent && (
        <div className="px-3 py-2 border-t bg-muted/20">
          {citation.type === "chunk" ? (
            <pre className="text-xs text-muted-foreground whitespace-pre-wrap font-mono max-h-[200px] overflow-y-auto">
              {citation.content}
            </pre>
          ) : (
            <p className="text-xs text-muted-foreground line-clamp-4">
              {citation.type === "file" ? citation.snippet : citation.snippet}
            </p>
          )}
          {citation.type === "url" && (
            <a
              href={citation.url}
              target="_blank"
              rel="noopener noreferrer"
              className="text-xs text-primary hover:underline mt-1 inline-flex items-center gap-1"
            >
              Open source <ExternalLink className="h-3 w-3" />
            </a>
          )}
        </div>
      )}
    </div>
  );
});

interface CitationListProps {
  /** List of citations to display */
  citations: Citation[];
  /** Maximum citations to show before collapsing */
  maxVisible?: number;
  /** Called when a file citation is clicked */
  onFileClick?: (fileId: string, chunkId?: string) => void;
  /** Called when a URL citation is clicked */
  onUrlClick?: (url: string) => void;
  /** Optional class name */
  className?: string;
  /** Whether to show in compact mode */
  compact?: boolean;
}

/**
 * CitationList - Display source citations for RAG responses
 *
 * Shows file and URL citations with expandable snippets/content.
 * Used to provide transparency about which sources informed a response.
 *
 * ## Usage Example
 *
 * ```tsx
 * <CitationList
 *   citations={[
 *     {
 *       id: "1",
 *       type: "file",
 *       fileId: "file_abc",
 *       filename: "q3_report.pdf",
 *       snippet: "Revenue increased by 15%...",
 *       score: 0.92,
 *     },
 *     {
 *       id: "2",
 *       type: "url",
 *       url: "https://example.com/article",
 *       title: "Market Analysis 2024",
 *       snippet: "Industry trends show...",
 *       score: 0.78,
 *     },
 *   ]}
 *   onFileClick={(fileId) => navigate(`/files/${fileId}`)}
 * />
 * ```
 */
export function CitationList({
  citations,
  maxVisible = 5,
  onFileClick,
  onUrlClick,
  className,
  compact = false,
}: CitationListProps) {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [showAll, setShowAll] = useState(false);

  if (citations.length === 0) {
    return null;
  }

  const visibleCitations = showAll ? citations : citations.slice(0, maxVisible);
  const hiddenCount = citations.length - maxVisible;

  const toggleExpanded = (id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  if (compact) {
    // Compact mode: just show count and types
    const fileCount = citations.filter((c) => c.type === "file" || c.type === "chunk").length;
    const urlCount = citations.filter((c) => c.type === "url").length;

    return (
      <div className={cn("flex items-center gap-2 text-xs text-muted-foreground", className)}>
        <span className="font-medium">Sources:</span>
        {fileCount > 0 && (
          <span className="flex items-center gap-1">
            <FileText className="h-3 w-3" />
            {fileCount} file{fileCount !== 1 ? "s" : ""}
          </span>
        )}
        {urlCount > 0 && (
          <span className="flex items-center gap-1">
            <ExternalLink className="h-3 w-3" />
            {urlCount} link{urlCount !== 1 ? "s" : ""}
          </span>
        )}
      </div>
    );
  }

  return (
    <div className={cn("space-y-2", className)}>
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-medium text-muted-foreground">Sources ({citations.length})</h4>
        {citations.length > 1 && (
          <Button
            variant="ghost"
            size="sm"
            className="h-6 text-xs"
            onClick={() => {
              if (expandedIds.size === citations.length) {
                setExpandedIds(new Set());
              } else {
                setExpandedIds(new Set(citations.map((c) => c.id)));
              }
            }}
          >
            {expandedIds.size === citations.length ? "Collapse all" : "Expand all"}
          </Button>
        )}
      </div>

      <div className="space-y-1.5">
        {visibleCitations.map((citation) => (
          <CitationItem
            key={citation.id}
            citation={citation}
            isExpanded={expandedIds.has(citation.id)}
            onToggle={() => toggleExpanded(citation.id)}
            onFileClick={onFileClick}
            onUrlClick={onUrlClick}
          />
        ))}
      </div>

      {hiddenCount > 0 && !showAll && (
        <Button
          variant="ghost"
          size="sm"
          className="w-full text-xs text-muted-foreground"
          onClick={() => setShowAll(true)}
        >
          Show {hiddenCount} more source{hiddenCount !== 1 ? "s" : ""}
        </Button>
      )}

      {showAll && hiddenCount > 0 && (
        <Button
          variant="ghost"
          size="sm"
          className="w-full text-xs text-muted-foreground"
          onClick={() => setShowAll(false)}
        >
          Show less
        </Button>
      )}
    </div>
  );
}
