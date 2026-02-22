import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronUp, FileText, Hash, Loader2 } from "lucide-react";
import { useState, memo } from "react";

import { vectorStoreFileChunksListOptions } from "@/api/generated/@tanstack/react-query.gen";
import type { ChunkResponse } from "@/api/generated/types.gen";
import { Badge } from "@/components/Badge/Badge";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { cn } from "@/utils/cn";

interface ChunkItemProps {
  chunk: ChunkResponse;
  isExpanded: boolean;
  onToggle: () => void;
}

/** Single chunk item with expand/collapse */
const ChunkItem = memo(function ChunkItem({ chunk, isExpanded, onToggle }: ChunkItemProps) {
  // Preview: first 150 chars
  const preview = chunk.content.length > 150 ? chunk.content.slice(0, 150) + "..." : chunk.content;

  return (
    <div
      className={cn(
        "border rounded-lg overflow-hidden transition-colors",
        "hover:border-primary/50"
      )}
    >
      <button
        type="button"
        className="w-full text-left"
        onClick={onToggle}
        aria-expanded={isExpanded}
      >
        <div className="flex items-center gap-2 px-3 py-2 bg-muted/30">
          {/* Chunk index */}
          <Badge variant="outline" className="font-mono text-xs shrink-0">
            <Hash className="h-3 w-3 mr-1" />
            {chunk.chunk_index}
          </Badge>

          {/* Token count */}
          <span className="text-xs text-muted-foreground shrink-0">{chunk.token_count} tokens</span>

          {/* Character range */}
          <span className="text-xs text-muted-foreground shrink-0">
            chars {chunk.char_start}-{chunk.char_end}
          </span>

          {/* Expand indicator */}
          <div className="flex-1" />
          {isExpanded ? (
            <ChevronUp className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          )}
        </div>
      </button>

      {/* Content area */}
      <div className="px-3 py-2 border-t">
        {isExpanded ? (
          <pre className="text-sm text-foreground whitespace-pre-wrap font-mono max-h-[400px] overflow-y-auto">
            {chunk.content}
          </pre>
        ) : (
          <p className="text-sm text-muted-foreground truncate">{preview}</p>
        )}
      </div>
    </div>
  );
});

interface ChunkViewerProps {
  /** Vector store ID */
  vectorStoreId: string;
  /** File ID to view chunks for */
  fileId: string;
  /** Optional filename to display in header */
  filename?: string;
  /** Optional class name */
  className?: string;
}

/**
 * ChunkViewer - View file chunks extracted from a document
 *
 * Displays all chunks that have been extracted and embedded from a file
 * in a vector store. Useful for debugging chunking behavior and verifying
 * that documents are being processed correctly.
 *
 * ## Usage Example
 *
 * ```tsx
 * <ChunkViewer
 *   vectorStoreId="vs_abc123"
 *   fileId="file_xyz789"
 *   filename="report.pdf"
 * />
 * ```
 */
export function ChunkViewer({ vectorStoreId, fileId, filename, className }: ChunkViewerProps) {
  const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set());

  const {
    data: chunksResponse,
    isLoading,
    error,
  } = useQuery({
    ...vectorStoreFileChunksListOptions({
      path: { vector_store_id: vectorStoreId, file_id: fileId },
    }),
  });

  const chunks = chunksResponse?.data ?? [];

  const toggleExpanded = (chunkIndex: number) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(chunkIndex)) {
        next.delete(chunkIndex);
      } else {
        next.add(chunkIndex);
      }
      return next;
    });
  };

  const expandAll = () => {
    setExpandedIds(new Set(chunks.map((c) => c.chunk_index)));
  };

  const collapseAll = () => {
    setExpandedIds(new Set());
  };

  if (error) {
    return (
      <Card className={className}>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <FileText className="h-4 w-4" />
            {filename || "File Chunks"}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-sm text-destructive">
            Failed to load chunks: {error instanceof Error ? error.message : "Unknown error"}
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className={className}>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <FileText className="h-4 w-4" />
          {filename || "File Chunks"}
        </CardTitle>
        {!isLoading && chunks.length > 0 && (
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">
              {chunks.length} chunk{chunks.length !== 1 ? "s" : ""}
            </span>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 text-xs"
              onClick={expandedIds.size === chunks.length ? collapseAll : expandAll}
            >
              {expandedIds.size === chunks.length ? "Collapse all" : "Expand all"}
            </Button>
          </div>
        )}
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        ) : chunks.length === 0 ? (
          <div className="text-center py-8 text-muted-foreground">
            <FileText className="h-8 w-8 mx-auto mb-2 opacity-50" />
            <p className="text-sm">No chunks found for this file.</p>
            <p className="text-xs mt-1">
              The file may still be processing, or embedding generation may have failed.
            </p>
          </div>
        ) : (
          <div className="space-y-2">
            {chunks.map((chunk) => (
              <ChunkItem
                key={chunk.id}
                chunk={chunk}
                isExpanded={expandedIds.has(chunk.chunk_index)}
                onToggle={() => toggleExpanded(chunk.chunk_index)}
              />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
