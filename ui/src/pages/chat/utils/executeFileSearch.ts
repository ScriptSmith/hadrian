/**
 * Execute File Search - Utility for calling the vector store search API
 *
 * This module provides a function to execute file_search tool calls by:
 * 1. Calling the vector store search API
 * 2. Formatting results into a tool-result-friendly format
 * 3. Handling errors gracefully
 *
 * ## Usage
 *
 * ```typescript
 * import { executeFileSearch } from "./executeFileSearch";
 *
 * // Execute search for a single vector store
 * const result = await executeFileSearch({
 *   vectorStoreId: "vs_123",
 *   query: "What is the revenue for Q3?",
 *   maxResults: 5,
 * });
 *
 * // Execute search across multiple vector stores
 * const result = await executeFileSearch({
 *   vectorStoreIds: ["vs_123", "vs_456"],
 *   query: "What is the revenue for Q3?",
 * });
 *
 * if (result.success) {
 *   // Use result.content for tool result
 * } else {
 *   // Handle error in result.error
 * }
 * ```
 */

import { vectorStoreSearch } from "@/api/generated/sdk.gen";
import type { SearchResultItem } from "@/api/generated/types.gen";

/**
 * Options for executing a file search
 */
export interface ExecuteFileSearchOptions {
  /** Single vector store ID to search */
  vectorStoreId?: string;
  /** Multiple vector store IDs to search (results are merged) */
  vectorStoreIds?: string[];
  /** The search query */
  query: string;
  /** Maximum number of results per vector store (default: 10) */
  maxResults?: number;
  /** Minimum similarity score threshold 0.0-1.0 (default: 0.0 = return all results) */
  scoreThreshold?: number;
}

/**
 * A single search result formatted for tool output
 */
export interface FileSearchResult {
  /** The file ID this result came from */
  file_id: string;
  /** The filename */
  filename: string;
  /** Relevance score (0-1) */
  score: number;
  /** The text content of the chunk */
  content: Array<{ type: "text"; text: string }>;
  /** File attributes/metadata */
  attributes: Record<string, unknown>;
}

/**
 * Successful search execution result
 */
export interface FileSearchSuccessResult {
  success: true;
  /** Formatted results for tool output */
  results: FileSearchResult[];
  /** JSON string of results for direct use as tool result content */
  content: string;
  /** Total number of results found */
  totalResults: number;
}

/**
 * Failed search execution result
 */
export interface FileSearchErrorResult {
  success: false;
  /** Error message */
  error: string;
  /** JSON string error for tool result content */
  content: string;
}

/**
 * Result of executing a file search
 */
export type FileSearchExecutionResult = FileSearchSuccessResult | FileSearchErrorResult;

/**
 * Convert internal SearchResultItem to OpenAI-compatible format
 */
function formatSearchResult(item: SearchResultItem): FileSearchResult {
  return {
    file_id: item.file_id,
    filename: item.filename ?? "unknown",
    score: item.score,
    // OpenAI uses array format for content
    content: [{ type: "text" as const, text: item.content }],
    attributes: (item.metadata as Record<string, unknown>) ?? {},
  };
}

/**
 * Execute a file search against one or more vector stores
 *
 * This function calls the vector store search API and formats the results
 * into a format suitable for use as a tool result in LLM conversations.
 *
 * @param options - Search options including vector store ID(s), query, and limits
 * @returns Promise resolving to search results or error
 */
export async function executeFileSearch(
  options: ExecuteFileSearchOptions
): Promise<FileSearchExecutionResult> {
  const { vectorStoreId, vectorStoreIds, query, maxResults = 10, scoreThreshold } = options;

  // Determine which vector stores to search
  const storeIds: string[] = [];
  if (vectorStoreId) {
    storeIds.push(vectorStoreId);
  }
  if (vectorStoreIds) {
    storeIds.push(...vectorStoreIds);
  }

  // Validate we have at least one store to search
  if (storeIds.length === 0) {
    const errorResult: FileSearchErrorResult = {
      success: false,
      error: "No vector store ID provided",
      content: JSON.stringify({
        error: "No vector store ID provided for file search",
      }),
    };
    return errorResult;
  }

  // Validate query
  if (!query || query.trim().length === 0) {
    const errorResult: FileSearchErrorResult = {
      success: false,
      error: "Empty search query",
      content: JSON.stringify({
        error: "Search query cannot be empty",
      }),
    };
    return errorResult;
  }

  try {
    // Execute searches in parallel for multiple stores
    const searchPromises = storeIds.map((storeId) =>
      vectorStoreSearch({
        path: { vector_store_id: storeId },
        body: {
          query: query.trim(),
          max_num_results: maxResults,
          ranking_options:
            scoreThreshold !== undefined ? { score_threshold: scoreThreshold } : undefined,
        },
        throwOnError: true,
      })
    );

    const responses = await Promise.all(searchPromises);

    // Merge and deduplicate results from all stores
    const allResults: SearchResultItem[] = [];
    for (const response of responses) {
      if (response.data?.data) {
        allResults.push(...response.data.data);
      }
    }

    // Sort by score (highest first) and limit total results
    allResults.sort((a, b) => b.score - a.score);
    const limitedResults = allResults.slice(0, maxResults);

    // Format results for tool output
    const formattedResults = limitedResults.map(formatSearchResult);

    // Build the content string for the tool result
    const content = JSON.stringify({
      object: "file_search_results",
      search_query: query,
      results: formattedResults,
    });

    return {
      success: true,
      results: formattedResults,
      content,
      totalResults: formattedResults.length,
    };
  } catch (error) {
    // Handle API errors gracefully
    const errorMessage = error instanceof Error ? error.message : "Unknown error occurred";

    // Try to extract more specific error info
    let detailedError = errorMessage;
    if (error && typeof error === "object" && "response" in error) {
      const responseError = error as { response?: { data?: { error?: { message?: string } } } };
      if (responseError.response?.data?.error?.message) {
        detailedError = responseError.response.data.error.message;
      }
    }

    const errorResult: FileSearchErrorResult = {
      success: false,
      error: detailedError,
      content: JSON.stringify({
        error: `File search failed: ${detailedError}`,
      }),
    };

    return errorResult;
  }
}

/**
 * Format file search results as a human-readable string
 *
 * This is useful for displaying results in the UI or for models
 * that prefer text over JSON.
 *
 * @param result - The search execution result
 * @returns Human-readable string representation
 */
export function formatFileSearchResultsAsText(result: FileSearchExecutionResult): string {
  if (!result.success) {
    return `Error: ${result.error}`;
  }

  if (result.results.length === 0) {
    return "No relevant results found.";
  }

  const lines: string[] = [`Found ${result.totalResults} relevant result(s):\n`];

  result.results.forEach((item, index) => {
    const scorePercent = (item.score * 100).toFixed(1);
    lines.push(`[${index + 1}] ${item.filename} (${scorePercent}% relevance)`);
    lines.push(`    File ID: ${item.file_id}`);
    // Truncate content for display
    const contentText = item.content[0]?.text ?? "";
    const truncated = contentText.length > 200 ? contentText.slice(0, 200) + "..." : contentText;
    lines.push(`    ${truncated}`);
    lines.push("");
  });

  return lines.join("\n");
}
