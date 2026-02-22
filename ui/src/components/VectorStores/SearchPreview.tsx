import { useMutation } from "@tanstack/react-query";
import {
  Search,
  Loader2,
  FileText,
  AlertCircle,
  ChevronDown,
  ChevronUp,
  SlidersHorizontal,
} from "lucide-react";
import { useState, memo, useCallback } from "react";

import { vectorStoreSearchMutation } from "@/api/generated/@tanstack/react-query.gen";
import type { SearchResultItem, VectorStoreSearchRequest } from "@/api/generated/types.gen";
import { Badge } from "@/components/Badge/Badge";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { Input } from "@/components/Input/Input";
import { Label } from "@/components/Label/Label";
import { cn } from "@/utils/cn";

interface SearchResultProps {
  result: SearchResultItem;
  isExpanded: boolean;
  onToggle: () => void;
  onFileClick?: (fileId: string) => void;
}

/** Single search result item */
const SearchResult = memo(function SearchResult({
  result,
  isExpanded,
  onToggle,
  onFileClick,
}: SearchResultProps) {
  // Preview: first 200 chars
  const preview =
    result.content.length > 200 ? result.content.slice(0, 200) + "..." : result.content;

  const scoreColor =
    result.score >= 0.85
      ? "bg-success/10 text-success"
      : result.score >= 0.7
        ? "bg-amber-500/10 text-amber-800 dark:text-amber-400"
        : "bg-muted text-muted-foreground";

  return (
    <div
      className={cn(
        "border rounded-lg overflow-hidden transition-colors",
        "hover:border-primary/50"
      )}
    >
      <div className="flex items-center gap-2 px-3 py-2 bg-muted/30">
        {/* Score badge */}
        <span className={cn("text-xs px-2 py-0.5 rounded font-medium shrink-0", scoreColor)}>
          {Math.round(result.score * 100)}%
        </span>

        {/* Filename */}
        {result.filename ? (
          <button
            type="button"
            className="flex items-center gap-1 text-sm text-foreground hover:text-primary truncate"
            onClick={() => onFileClick?.(result.file_id)}
          >
            <FileText className="h-3 w-3 shrink-0" />
            <span className="truncate">{result.filename}</span>
          </button>
        ) : (
          <span className="text-sm text-muted-foreground truncate">
            <FileText className="h-3 w-3 inline mr-1" />
            {result.file_id.slice(0, 12)}...
          </span>
        )}

        {/* Chunk info */}
        <span className="text-xs text-muted-foreground shrink-0">chunk {result.chunk_index}</span>

        {/* Expand/collapse */}
        <div className="flex-1" />
        <Button
          variant="ghost"
          size="sm"
          className="h-6 w-6 p-0 shrink-0"
          onClick={onToggle}
          aria-label={isExpanded ? "Collapse result" : "Expand result"}
          aria-expanded={isExpanded}
        >
          {isExpanded ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
        </Button>
      </div>

      {/* Content */}
      <div className="px-3 py-2 border-t">
        {isExpanded ? (
          <pre className="text-sm text-foreground whitespace-pre-wrap font-mono max-h-[300px] overflow-y-auto">
            {result.content}
          </pre>
        ) : (
          <p className="text-sm text-muted-foreground line-clamp-2">{preview}</p>
        )}
      </div>
    </div>
  );
});

interface SearchPreviewProps {
  /** Vector store ID to search */
  vectorStoreId: string;
  /** Called when a file result is clicked */
  onFileClick?: (fileId: string) => void;
  /** Optional class name */
  className?: string;
}

/**
 * SearchPreview - Test semantic search against a vector store
 *
 * Provides an interface to test search queries against a vector store,
 * useful for debugging search relevance and verifying embeddings.
 *
 * ## Usage Example
 *
 * ```tsx
 * <SearchPreview
 *   vectorStoreId="vs_abc123"
 *   onFileClick={(fileId) => navigate(`/files/${fileId}`)}
 * />
 * ```
 */
export function SearchPreview({ vectorStoreId, onFileClick, className }: SearchPreviewProps) {
  const [query, setQuery] = useState("");
  const [maxResults, setMaxResults] = useState(10);
  const [threshold, setThreshold] = useState(0.5);
  const [showOptions, setShowOptions] = useState(false);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [results, setResults] = useState<SearchResultItem[]>([]);
  const [searchedQuery, setSearchedQuery] = useState("");

  const searchMutation = useMutation({
    ...vectorStoreSearchMutation(),
    onSuccess: (data) => {
      setResults(data.data ?? []);
      setSearchedQuery(data.query ?? query);
      setExpandedIds(new Set());
    },
  });

  const handleSearch = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!query.trim()) return;

      const request: VectorStoreSearchRequest = {
        query: query.trim(),
        max_num_results: maxResults,
        ranking_options: {
          score_threshold: threshold,
        },
      };

      searchMutation.mutate({
        path: { vector_store_id: vectorStoreId },
        body: request,
      });
    },
    [query, maxResults, threshold, vectorStoreId, searchMutation]
  );

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

  const hasResults = results.length > 0;
  const hasSearched = searchedQuery !== "";

  return (
    <Card className={className}>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <Search className="h-4 w-4" />
          Search Preview
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Search form */}
        <form onSubmit={handleSearch} className="space-y-3">
          <div className="flex gap-2">
            <Input
              type="text"
              placeholder="Enter search query..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="flex-1"
              disabled={searchMutation.isPending}
            />
            <Button
              type="submit"
              disabled={!query.trim() || searchMutation.isPending}
              aria-label="Search"
            >
              {searchMutation.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Search className="h-4 w-4" />
              )}
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon"
              onClick={() => setShowOptions(!showOptions)}
              aria-label="Search options"
              aria-expanded={showOptions}
            >
              <SlidersHorizontal className="h-4 w-4" />
            </Button>
          </div>

          {/* Advanced options */}
          {showOptions && (
            <div className="grid grid-cols-2 gap-4 p-3 bg-muted/30 rounded-lg">
              <div className="space-y-1">
                <Label htmlFor="max-results" className="text-xs">
                  Max Results
                </Label>
                <Input
                  id="max-results"
                  type="number"
                  min={1}
                  max={50}
                  value={maxResults}
                  onChange={(e) => setMaxResults(parseInt(e.target.value) || 10)}
                  className="h-8"
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="threshold" className="text-xs">
                  Min Score ({Math.round(threshold * 100)}%)
                </Label>
                <Input
                  id="threshold"
                  type="range"
                  min={0}
                  max={1}
                  step={0.05}
                  value={threshold}
                  onChange={(e) => setThreshold(parseFloat(e.target.value))}
                  className="h-8"
                />
              </div>
            </div>
          )}
        </form>

        {/* Error state */}
        {searchMutation.error && (
          <div className="flex items-center gap-2 p-3 bg-destructive/10 text-destructive rounded-lg">
            <AlertCircle className="h-4 w-4 shrink-0" />
            <span className="text-sm">
              {searchMutation.error instanceof Error
                ? searchMutation.error.message
                : "Search failed"}
            </span>
          </div>
        )}

        {/* Results header */}
        {hasSearched && !searchMutation.isPending && (
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              {hasResults ? (
                <>
                  {results.length} result{results.length !== 1 ? "s" : ""} for &ldquo;
                  {searchedQuery}&rdquo;
                </>
              ) : (
                <>No results for &ldquo;{searchedQuery}&rdquo;</>
              )}
            </span>
            {hasResults && results.length > 1 && (
              <Button
                variant="ghost"
                size="sm"
                className="h-7 text-xs"
                onClick={() => {
                  if (expandedIds.size === results.length) {
                    setExpandedIds(new Set());
                  } else {
                    setExpandedIds(new Set(results.map((r) => r.chunk_id)));
                  }
                }}
              >
                {expandedIds.size === results.length ? "Collapse all" : "Expand all"}
              </Button>
            )}
          </div>
        )}

        {/* Results list */}
        {hasResults && (
          <div className="space-y-2">
            {results.map((result, index) => (
              <SearchResult
                key={result.chunk_id || `result-${index}`}
                result={result}
                isExpanded={expandedIds.has(result.chunk_id)}
                onToggle={() => toggleExpanded(result.chunk_id)}
                onFileClick={onFileClick}
              />
            ))}
          </div>
        )}

        {/* Empty state when searched but no results */}
        {hasSearched && !hasResults && !searchMutation.isPending && !searchMutation.error && (
          <div className="text-center py-6 text-muted-foreground">
            <Search className="h-8 w-8 mx-auto mb-2 opacity-50" />
            <p className="text-sm">No matching results found.</p>
            <p className="text-xs mt-1">
              Try adjusting your query or lowering the minimum score threshold.
            </p>
          </div>
        )}

        {/* Initial state */}
        {!hasSearched && !searchMutation.isPending && (
          <div className="text-center py-6 text-muted-foreground">
            <Search className="h-8 w-8 mx-auto mb-2 opacity-50" />
            <p className="text-sm">Enter a search query to test semantic search.</p>
            <p className="text-xs mt-1">Results are ranked by embedding similarity score.</p>
          </div>
        )}

        {/* Score legend */}
        {hasResults && (
          <div className="flex items-center gap-3 text-xs text-muted-foreground pt-2 border-t">
            <span>Score:</span>
            <Badge variant="outline" className="bg-green-500/10 text-green-800 dark:text-green-400">
              85%+ High
            </Badge>
            <Badge
              variant="outline"
              className="bg-yellow-500/10 text-yellow-800 dark:text-yellow-400"
            >
              70-84% Medium
            </Badge>
            <Badge variant="outline" className="bg-muted text-muted-foreground">
              &lt;70% Low
            </Badge>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
