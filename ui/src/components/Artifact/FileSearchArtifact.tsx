/**
 * FileSearchArtifact - Display file search query and results
 *
 * Renders the search query, vector stores searched, and results with
 * relevance scores and content snippets.
 */

import { memo, useState } from "react";
import { Search, FileText, ChevronDown, ChevronUp, Copy, Check } from "lucide-react";

import type {
  Artifact,
  FileSearchArtifactData,
  FileSearchResultItem,
} from "@/components/chat-types";
import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

export interface FileSearchArtifactProps {
  artifact: Artifact;
  className?: string;
}

function isFileSearchArtifactData(data: unknown): data is FileSearchArtifactData {
  return (
    typeof data === "object" &&
    data !== null &&
    "query" in data &&
    typeof (data as FileSearchArtifactData).query === "string" &&
    "results" in data &&
    Array.isArray((data as FileSearchArtifactData).results)
  );
}

/** Format relevance score as percentage */
function formatScore(score: number): string {
  return `${Math.round(score * 100)}%`;
}

/** Single search result item */
function SearchResultItem({
  result,
  index,
  isExpanded,
  onToggle,
}: {
  result: FileSearchResultItem;
  index: number;
  isExpanded: boolean;
  onToggle: () => void;
}) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await navigator.clipboard.writeText(result.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div
      className={cn(
        "border rounded-lg overflow-hidden transition-colors",
        isExpanded ? "border-primary/30 bg-muted/20" : "border-border hover:border-primary/20"
      )}
    >
      {/* Result header - clickable to expand */}
      <button
        onClick={onToggle}
        className="w-full flex items-center gap-3 px-3 py-2 text-left hover:bg-muted/30 transition-colors"
      >
        <div className="flex items-center justify-center w-6 h-6 rounded bg-primary/10 text-primary text-xs font-medium shrink-0">
          {index + 1}
        </div>

        <FileText className="h-4 w-4 text-muted-foreground shrink-0" />

        <div className="flex-1 min-w-0">
          <div className="font-medium text-sm truncate">{result.filename}</div>
          <div className="text-xs text-muted-foreground truncate">{result.fileId}</div>
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <span
            className={cn(
              "px-2 py-0.5 rounded text-xs font-medium",
              result.score >= 0.7
                ? "bg-success/10 text-success"
                : result.score >= 0.4
                  ? "bg-amber-500/10 text-amber-800 dark:text-amber-400"
                  : "bg-muted text-muted-foreground"
            )}
          >
            {formatScore(result.score)}
          </span>

          {isExpanded ? (
            <ChevronUp className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          )}
        </div>
      </button>

      {/* Expanded content */}
      {isExpanded && (
        <div className="border-t bg-muted/10">
          <div className="relative">
            {/* Copy button */}
            <div className="absolute right-2 top-2 z-10">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="secondary"
                    size="sm"
                    className="h-7 px-2 text-xs"
                    onClick={handleCopy}
                  >
                    {copied ? (
                      <>
                        <Check className="h-3 w-3 mr-1" />
                        Copied
                      </>
                    ) : (
                      <>
                        <Copy className="h-3 w-3 mr-1" />
                        Copy
                      </>
                    )}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Copy content</TooltipContent>
              </Tooltip>
            </div>

            {/* Content */}
            <pre className="p-3 pr-20 text-xs font-mono whitespace-pre-wrap break-words max-h-[300px] overflow-auto">
              {result.content}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}

function FileSearchArtifactComponent({ artifact, className }: FileSearchArtifactProps) {
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);

  // Validate data
  if (!isFileSearchArtifactData(artifact.data)) {
    return (
      <div className="p-4 text-sm text-muted-foreground">Invalid file search artifact data</div>
    );
  }

  const { query, vectorStoreIds, results, totalResults } = artifact.data;

  const toggleExpanded = (index: number) => {
    setExpandedIndex(expandedIndex === index ? null : index);
  };

  return (
    <div className={cn("space-y-3", className)}>
      {/* Search query header */}
      <div className="flex items-start gap-3 px-3 py-2 bg-muted/30 rounded-lg">
        <Search className="h-4 w-4 text-primary mt-0.5 shrink-0" />
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium mb-1">Search Query</div>
          <div className="text-sm text-muted-foreground break-words">{query}</div>
          {vectorStoreIds.length > 0 && (
            <div className="text-xs text-muted-foreground mt-1">
              Searched {vectorStoreIds.length} knowledge base
              {vectorStoreIds.length !== 1 ? "s" : ""}
            </div>
          )}
        </div>
      </div>

      {/* Results count */}
      <div className="px-3 text-xs text-muted-foreground">
        {totalResults} result{totalResults !== 1 ? "s" : ""} found
      </div>

      {/* Results list */}
      {results.length > 0 ? (
        <div className="space-y-2 px-1">
          {results.map((result, index) => (
            <SearchResultItem
              key={`${result.fileId}-${index}`}
              result={result}
              index={index}
              isExpanded={expandedIndex === index}
              onToggle={() => toggleExpanded(index)}
            />
          ))}
        </div>
      ) : (
        <div className="px-3 py-4 text-center text-sm text-muted-foreground">
          No results found for this query
        </div>
      )}
    </div>
  );
}

export const FileSearchArtifact = memo(FileSearchArtifactComponent);
