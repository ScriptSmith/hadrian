import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  Search,
  Loader2,
  FileText,
  AlertCircle,
  ChevronDown,
  ChevronUp,
  SlidersHorizontal,
} from "lucide-react";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import type { SearchResultItem } from "@/api/generated/types.gen";
import { Badge } from "@/components/Badge/Badge";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { Input } from "@/components/Input/Input";
import { Label } from "@/components/Label/Label";
import { cn } from "@/utils/cn";

// Mock data for stories
const mockSearchResults: SearchResultItem[] = [
  {
    chunk_id: "chunk-1",
    object: "vector_store.search_result",
    collection_id: "vs_123",
    file_id: "file_456",
    chunk_index: 2,
    content: `# Revenue Analysis

Total revenue for Q3 2024 was $4.2 billion, representing a 15% increase from Q3 2023. This growth was driven primarily by our cloud services segment, which saw a 32% increase in revenue.

The breakdown by segment:
- Cloud Services: $2.1B (+32%)
- Enterprise Software: $1.4B (+8%)
- Professional Services: $0.7B (+2%)`,
    score: 0.92,
    filename: "q3_2024_report.pdf",
    metadata: null,
  },
  {
    chunk_id: "chunk-2",
    object: "vector_store.search_result",
    collection_id: "vs_123",
    file_id: "file_789",
    chunk_index: 0,
    content: `## Q2 2024 Financial Highlights

Revenue reached $3.9 billion in Q2, up 12% year-over-year. Operating margin improved to 26% driven by operational efficiencies and strong cloud growth.

Key metrics:
- Total Revenue: $3.9B
- Operating Income: $1.0B
- Net Income: $0.8B`,
    score: 0.78,
    filename: "q2_2024_report.pdf",
    metadata: null,
  },
  {
    chunk_id: "chunk-3",
    object: "vector_store.search_result",
    collection_id: "vs_123",
    file_id: "file_abc",
    chunk_index: 5,
    content: `Annual revenue projections for 2025 show continued growth momentum. Based on current trajectory and market conditions, we forecast:

- FY2025 Revenue: $18-19B (15-18% YoY growth)
- Cloud Services: Expected to exceed 50% of total revenue
- Operating Margin: Target 28-30%`,
    score: 0.65,
    filename: "annual_forecast.xlsx",
    metadata: null,
  },
];

// Static components for stories (no API calls)
interface SearchResultProps {
  result: SearchResultItem;
  isExpanded: boolean;
  onToggle: () => void;
  onFileClick?: (fileId: string) => void;
}

function SearchResult({ result, isExpanded, onToggle, onFileClick }: SearchResultProps) {
  const preview =
    result.content.length > 200 ? result.content.slice(0, 200) + "..." : result.content;

  const scoreColor =
    result.score >= 0.85
      ? "bg-green-500/10 text-green-800 dark:text-green-400"
      : result.score >= 0.7
        ? "bg-yellow-500/10 text-yellow-800 dark:text-yellow-400"
        : "bg-muted text-muted-foreground";

  return (
    <div
      className={cn(
        "border rounded-lg overflow-hidden transition-colors",
        "hover:border-primary/50"
      )}
    >
      <div className="flex items-center gap-2 px-3 py-2 bg-muted/30">
        <span className={cn("text-xs px-2 py-0.5 rounded font-medium shrink-0", scoreColor)}>
          {Math.round(result.score * 100)}%
        </span>
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
        <span className="text-xs text-muted-foreground shrink-0">chunk {result.chunk_index}</span>
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
}

interface StaticSearchPreviewProps {
  results: SearchResultItem[];
  query?: string;
  isLoading?: boolean;
  error?: string;
  onFileClick?: (fileId: string) => void;
}

function StaticSearchPreview({
  results,
  query,
  isLoading,
  error,
  onFileClick,
}: StaticSearchPreviewProps) {
  const [searchQuery, setSearchQuery] = useState(query || "");
  const [showOptions, setShowOptions] = useState(false);
  const [maxResults, setMaxResults] = useState(10);
  const [threshold, setThreshold] = useState(0.7);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

  const hasResults = results.length > 0;
  const hasSearched = query !== undefined && query !== "";

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

  return (
    <Card>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-base">
          <Search className="h-4 w-4" />
          Search Preview
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Search form */}
        <div className="space-y-3">
          <div className="flex gap-2">
            <Input
              type="text"
              placeholder="Enter search query..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="flex-1"
              disabled={isLoading}
            />
            <Button type="button" disabled={!searchQuery.trim() || isLoading} aria-label="Search">
              {isLoading ? (
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
        </div>

        {/* Error state */}
        {error && (
          <div className="flex items-center gap-2 p-3 bg-destructive/10 text-destructive rounded-lg">
            <AlertCircle className="h-4 w-4 shrink-0" />
            <span className="text-sm">{error}</span>
          </div>
        )}

        {/* Results header */}
        {hasSearched && !isLoading && (
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              {hasResults ? (
                <>
                  {results.length} result{results.length !== 1 ? "s" : ""} for &ldquo;{query}&rdquo;
                </>
              ) : (
                <>No results for &ldquo;{query}&rdquo;</>
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
        {hasSearched && !hasResults && !isLoading && !error && (
          <div className="text-center py-6 text-muted-foreground">
            <Search className="h-8 w-8 mx-auto mb-2 opacity-50" />
            <p className="text-sm">No matching results found.</p>
            <p className="text-xs mt-1">
              Try adjusting your query or lowering the minimum score threshold.
            </p>
          </div>
        )}

        {/* Initial state */}
        {!hasSearched && !isLoading && (
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

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const meta: Meta<typeof StaticSearchPreview> = {
  title: "VectorStores/SearchPreview",
  component: StaticSearchPreview,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div className="max-w-3xl">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof StaticSearchPreview>;

/** Default state - ready to search */
export const Default: Story = {
  args: {
    results: [],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify initial state message
    await expect(
      canvas.getByText("Enter a search query to test semantic search.")
    ).toBeInTheDocument();

    // Verify search input is present
    const searchInput = canvas.getByPlaceholderText("Enter search query...");
    await expect(searchInput).toBeInTheDocument();

    // Verify search button is disabled when input is empty
    const searchButton = canvas.getAllByRole("button")[0];
    await expect(searchButton).toBeDisabled();

    // Type in search input
    await userEvent.type(searchInput, "test query");

    // Verify search button is now enabled
    await expect(searchButton).toBeEnabled();
  },
};

/** With search results displayed */
export const WithResults: Story = {
  args: {
    results: mockSearchResults,
    query: "revenue growth",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify results count is displayed
    await expect(canvas.getByText(/3 results for/)).toBeInTheDocument();

    // Verify score legend is shown
    await expect(canvas.getByText("85%+ High")).toBeInTheDocument();

    // Verify first result shows score percentage
    await expect(canvas.getByText("92%")).toBeInTheDocument();

    // Find the first expand button for a result
    const firstExpandButton = canvas.getAllByRole("button", { name: /^expand result$/i })[0];
    await expect(firstExpandButton).toBeInTheDocument();

    // Click first result's expand button
    await userEvent.click(firstExpandButton);

    // Verify result is expanded (full content visible - use specific text from first result only)
    await expect(canvas.getByText(/Cloud Services: \$2\.1B/)).toBeInTheDocument();

    // Click "Expand all" button
    const expandAllButton = canvas.getByRole("button", { name: /expand all/i });
    await userEvent.click(expandAllButton);

    // Verify button changes to "Collapse all"
    const collapseAllButton = canvas.getByRole("button", { name: /collapse all/i });
    await expect(collapseAllButton).toBeInTheDocument();

    // Click "Collapse all"
    await userEvent.click(collapseAllButton);

    // Button should change back to "Expand all"
    await expect(canvas.getByRole("button", { name: /expand all/i })).toBeInTheDocument();
  },
};

/** Empty search results */
export const NoResults: Story = {
  args: {
    results: [],
    query: "nonexistent topic",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify "no results" message
    await expect(canvas.getByText(/No results for/)).toBeInTheDocument();
    await expect(canvas.getByText("No matching results found.")).toBeInTheDocument();

    // Verify suggestion to adjust query
    await expect(
      canvas.getByText(/Try adjusting your query or lowering the minimum score threshold/)
    ).toBeInTheDocument();
  },
};

/** Search error */
export const Error: Story = {
  args: {
    results: [],
    query: "test query",
    error: "File search is not configured. Enable [features.file_search] in configuration.",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify error message is displayed
    await expect(canvas.getByText(/File search is not configured/)).toBeInTheDocument();
  },
};

/** Loading state */
export const Loading: Story = {
  args: {
    results: [],
    query: "loading query",
    isLoading: true,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify search input is disabled during loading
    const searchInput = canvas.getByPlaceholderText("Enter search query...");
    await expect(searchInput).toBeDisabled();

    // Verify loading spinner is present
    const spinner = canvasElement.querySelector(".animate-spin");
    await expect(spinner).toBeInTheDocument();
  },
};

/** Single high-scoring result */
export const SingleResult: Story = {
  args: {
    results: [mockSearchResults[0]],
    query: "Q3 revenue",
  },
};

/** Results without filenames (uses file_id fallback) */
export const WithoutFilenames: Story = {
  args: {
    results: mockSearchResults.map((r) => ({ ...r, filename: null })),
    query: "revenue",
  },
};

/** Many results with varied scores */
export const ManyResults: Story = {
  args: {
    results: Array.from({ length: 10 }, (_, i) => ({
      chunk_id: `chunk-${i}`,
      object: "vector_store.search_result",
      collection_id: "vs_123",
      file_id: `file_${i}`,
      chunk_index: i,
      content: `This is result ${i + 1} content. It contains relevant information about the search query. The content varies in length and relevance to demonstrate the search preview component.`,
      score: 0.95 - i * 0.05,
      filename: `document_${i + 1}.pdf`,
      metadata: null,
    })),
    query: "search query",
  },
};

/** With advanced options visible */
export const WithOptions: Story = {
  args: {
    results: mockSearchResults,
    query: "revenue growth",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify options panel is initially hidden
    await expect(canvas.queryByLabelText("Max Results")).not.toBeInTheDocument();

    // Find and click the options toggle button
    const optionsButton = canvas.getByRole("button", { name: /search options/i });
    await userEvent.click(optionsButton);

    // Verify options panel is now visible
    await expect(canvas.getByLabelText("Max Results")).toBeInTheDocument();
    await expect(canvas.getByLabelText(/Min Score/)).toBeInTheDocument();

    // Click again to hide
    await userEvent.click(optionsButton);

    // Verify options panel is hidden again
    await expect(canvas.queryByLabelText("Max Results")).not.toBeInTheDocument();
  },
};
