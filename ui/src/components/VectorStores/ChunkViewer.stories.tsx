import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { FileText, Hash, Loader2, ChevronDown, ChevronUp } from "lucide-react";
import { expect, userEvent, within } from "storybook/test";
import { useState } from "react";

import type { ChunkResponse } from "@/api/generated/types.gen";
import { Badge } from "@/components/Badge/Badge";
import { Button } from "@/components/Button/Button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { cn } from "@/utils/cn";

// Mock data for stories
const mockChunks: ChunkResponse[] = [
  {
    id: "chunk-1",
    object: "vector_store.file.chunk",
    collection_id: "vs_123",
    file_id: "file_456",
    chunk_index: 0,
    content: `# Introduction

This document provides an overview of the Q3 2024 financial results. The quarter was marked by strong revenue growth across all segments.

Key highlights include:
- Revenue increased 15% year-over-year
- Operating margin expanded to 28%
- Customer acquisition reached record levels`,
    token_count: 85,
    char_start: 0,
    char_end: 287,
    metadata: null,
    created_at: Date.now() / 1000,
  },
  {
    id: "chunk-2",
    object: "vector_store.file.chunk",
    collection_id: "vs_123",
    file_id: "file_456",
    chunk_index: 1,
    content: `## Revenue Analysis

Total revenue for Q3 2024 was $4.2 billion, representing a 15% increase from Q3 2023. This growth was driven primarily by our cloud services segment, which saw a 32% increase in revenue.

The breakdown by segment:
- Cloud Services: $2.1B (+32%)
- Enterprise Software: $1.4B (+8%)
- Professional Services: $0.7B (+2%)`,
    token_count: 112,
    char_start: 288,
    char_end: 587,
    metadata: null,
    created_at: Date.now() / 1000,
  },
  {
    id: "chunk-3",
    object: "vector_store.file.chunk",
    collection_id: "vs_123",
    file_id: "file_456",
    chunk_index: 2,
    content: `## Operating Expenses

Operating expenses were $3.0 billion, an increase of 10% from the prior year period. The increase was primarily due to:

1. Increased R&D investment in AI capabilities
2. Expansion of our global workforce
3. Marketing spend for new product launches

Despite the increase in expenses, operating margin improved to 28% from 25% in Q3 2023.`,
    token_count: 98,
    char_start: 588,
    char_end: 912,
    metadata: null,
    created_at: Date.now() / 1000,
  },
];

// Static component for stories (no API calls)
interface ChunkItemProps {
  chunk: ChunkResponse;
  isExpanded: boolean;
  onToggle: () => void;
}

function ChunkItem({ chunk, isExpanded, onToggle }: ChunkItemProps) {
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
          <Badge variant="outline" className="font-mono text-xs shrink-0">
            <Hash className="h-3 w-3 mr-1" />
            {chunk.chunk_index}
          </Badge>
          <span className="text-xs text-muted-foreground shrink-0">{chunk.token_count} tokens</span>
          <span className="text-xs text-muted-foreground shrink-0">
            chars {chunk.char_start}-{chunk.char_end}
          </span>
          <div className="flex-1" />
          {isExpanded ? (
            <ChevronUp className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          )}
        </div>
      </button>
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
}

interface StaticChunkViewerProps {
  chunks: ChunkResponse[];
  filename?: string;
  isLoading?: boolean;
  error?: string;
}

function StaticChunkViewer({ chunks, filename, isLoading, error }: StaticChunkViewerProps) {
  const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set());

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

  if (error) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <FileText className="h-4 w-4" />
            {filename || "File Chunks"}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-sm text-destructive">Failed to load chunks: {error}</div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
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
              onClick={() =>
                setExpandedIds(
                  expandedIds.size === chunks.length
                    ? new Set()
                    : new Set(chunks.map((c) => c.chunk_index))
                )
              }
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

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const meta: Meta<typeof StaticChunkViewer> = {
  title: "VectorStores/ChunkViewer",
  component: StaticChunkViewer,
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
type Story = StoryObj<typeof StaticChunkViewer>;

/** Default view with multiple chunks */
export const Default: Story = {
  args: {
    chunks: mockChunks,
    filename: "q3_2024_report.pdf",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify chunk count is displayed
    await expect(canvas.getByText("3 chunks")).toBeInTheDocument();

    // Verify all chunks are initially collapsed (showing truncated preview)
    const chunkButtons = canvas.getAllByRole("button", { expanded: false });
    const chunkToggleButtons = chunkButtons.filter(
      (btn) => btn.getAttribute("aria-expanded") !== null
    );
    await expect(chunkToggleButtons.length).toBe(3);

    // Click first chunk to expand it
    await userEvent.click(chunkToggleButtons[0]);

    // Verify first chunk is now expanded
    const expandedButton = canvas.getByRole("button", { expanded: true });
    await expect(expandedButton).toBeInTheDocument();

    // Click again to collapse
    await userEvent.click(expandedButton);

    // Verify it's collapsed again
    const collapsedButtons = canvas.getAllByRole("button", { expanded: false });
    const collapsedChunks = collapsedButtons.filter(
      (btn) => btn.getAttribute("aria-expanded") !== null
    );
    await expect(collapsedChunks.length).toBe(3);

    // Test "Expand all" button
    const expandAllButton = canvas.getByRole("button", { name: /expand all/i });
    await userEvent.click(expandAllButton);

    // Verify all chunks are expanded
    const allExpandedButtons = canvas.getAllByRole("button", { expanded: true });
    await expect(allExpandedButtons.length).toBe(3);

    // Verify button now says "Collapse all"
    const collapseAllButton = canvas.getByRole("button", { name: /collapse all/i });
    await expect(collapseAllButton).toBeInTheDocument();

    // Click "Collapse all"
    await userEvent.click(collapseAllButton);

    // Verify all chunks are collapsed
    const allCollapsedButtons = canvas.getAllByRole("button", { expanded: false });
    const allCollapsedChunks = allCollapsedButtons.filter(
      (btn) => btn.getAttribute("aria-expanded") !== null
    );
    await expect(allCollapsedChunks.length).toBe(3);
  },
};

/** Loading state */
export const Loading: Story = {
  args: {
    chunks: [],
    filename: "loading_file.txt",
    isLoading: true,
  },
};

/** Empty state - no chunks */
export const Empty: Story = {
  args: {
    chunks: [],
    filename: "empty_file.txt",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify empty state message is displayed
    await expect(canvas.getByText("No chunks found for this file.")).toBeInTheDocument();
    await expect(canvas.getByText(/The file may still be processing/)).toBeInTheDocument();

    // Verify no expand/collapse buttons exist
    const buttons = canvas.queryAllByRole("button");
    const expandButtons = buttons.filter((btn) => btn.getAttribute("aria-expanded") !== null);
    await expect(expandButtons.length).toBe(0);
  },
};

/** Error state */
export const Error: Story = {
  args: {
    chunks: [],
    filename: "error_file.txt",
    error: "File search is not configured. Enable [features.file_search] in configuration.",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify error message is displayed
    await expect(canvas.getByText(/Failed to load chunks:/)).toBeInTheDocument();
    await expect(canvas.getByText(/File search is not configured/)).toBeInTheDocument();
  },
};

/** Single chunk */
export const SingleChunk: Story = {
  args: {
    chunks: [mockChunks[0]],
    filename: "small_file.txt",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);

    // Verify single chunk count displayed
    await expect(canvas.getByText("1 chunk")).toBeInTheDocument();

    // Verify chunk is initially collapsed
    const chunkButton = canvas.getByRole("button", { expanded: false });
    await expect(chunkButton).toBeInTheDocument();

    // Click to expand
    await userEvent.click(chunkButton);

    // Verify chunk is now expanded and shows full content
    const expandedButton = canvas.getByRole("button", { expanded: true });
    await expect(expandedButton).toBeInTheDocument();

    // Verify full content is visible (the pre element with full text)
    const fullContent = canvas.getByText(/Key highlights include:/);
    await expect(fullContent).toBeInTheDocument();

    // Click to collapse
    await userEvent.click(expandedButton);

    // Verify chunk is collapsed again
    const collapsedButton = canvas.getByRole("button", { expanded: false });
    await expect(collapsedButton).toBeInTheDocument();
  },
};

/** Long chunk content */
export const LongContent: Story = {
  args: {
    chunks: [
      {
        id: "chunk-long",
        object: "vector_store.file.chunk",
        collection_id: "vs_123",
        file_id: "file_long",
        chunk_index: 0,
        content: `# Very Long Document Section

This is a very long chunk of text that demonstrates how the ChunkViewer handles larger content. In real-world scenarios, chunks can be quite substantial, containing hundreds of tokens and spanning multiple paragraphs.

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.

## Code Example

Here's some example code that might appear in a chunk:

\`\`\`python
def calculate_embedding(text: str) -> list[float]:
    """Generate embedding for input text."""
    tokens = tokenize(text)
    return model.encode(tokens)
\`\`\`

## Additional Context

Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.

This chunk continues with more content to demonstrate scrolling behavior when expanded.`,
        token_count: 450,
        char_start: 0,
        char_end: 1200,
        metadata: null,
        created_at: Date.now() / 1000,
      },
    ],
    filename: "long_document.md",
  },
};

/** Without filename */
export const WithoutFilename: Story = {
  args: {
    chunks: mockChunks.slice(0, 2),
  },
};
