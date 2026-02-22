import { useState, useCallback, useMemo } from "react";

/**
 * OpenAI-style pagination response shape.
 * Used by `/api/v1/*` endpoints that follow OpenAI's API conventions.
 */
export interface OpenAIPaginationResponse {
  /** ID of the first object in the list (for backward pagination with `before`) */
  first_id?: string | null;
  /** ID of the last object in the list (for forward pagination with `after`) */
  last_id?: string | null;
  /** Whether there are more results available beyond this page */
  has_more: boolean;
}

export type OpenAIPaginationDirection = "forward" | "backward";

export interface OpenAIPaginationState {
  /** Current cursor for the page being viewed (null for first page) */
  after: string | null;
  before: string | null;
  /** Number of items per page */
  limit: number;
}

export interface OpenAIPaginationActions {
  /** Go to the next page using the last_id from pagination response */
  goToNextPage: (response: OpenAIPaginationResponse) => void;
  /** Go to the previous page using the first_id from pagination response */
  goToPreviousPage: (response: OpenAIPaginationResponse) => void;
  /** Reset to the first page */
  goToFirstPage: () => void;
  /** Set a custom limit */
  setLimit: (limit: number) => void;
}

export interface OpenAIPaginationInfo {
  /** Whether we're on the first page (no cursor) */
  isFirstPage: boolean;
  /** Page number for display (1-indexed) - note: this is an approximation */
  pageNumber: number;
}

export interface UseOpenAIPaginationResult {
  /** Current pagination state */
  state: OpenAIPaginationState;
  /** Actions to navigate between pages */
  actions: OpenAIPaginationActions;
  /** Computed pagination info */
  info: OpenAIPaginationInfo;
  /** Query parameters to spread into API call options */
  queryParams: {
    after?: string;
    before?: string;
    limit: number;
  };
  /**
   * Convert OpenAI pagination response to PaginationMeta format for UI components.
   * Use this when passing pagination to ResourceTable or Pagination components.
   */
  toPaginationMeta: (response?: OpenAIPaginationResponse) =>
    | {
        has_more: boolean;
        limit: number;
        next_cursor: string | null;
        prev_cursor: string | null;
      }
    | undefined;
}

export interface UseOpenAIPaginationOptions {
  /** Initial limit (items per page). Default: 20 */
  defaultLimit?: number;
}

/**
 * Hook for managing OpenAI-style cursor-based pagination.
 *
 * Use this hook for OpenAI-compatible `/api/v1/*` endpoints that use
 * `after`/`before` query parameters and return `first_id`/`last_id`/`has_more`.
 *
 * For admin `/admin/v1/*` endpoints that use `cursor`/`direction` parameters
 * and return `PaginationMeta`, use `useCursorPagination` instead.
 *
 * @example
 * ```tsx
 * const pagination = useOpenAIPagination({ defaultLimit: 20 });
 *
 * const { data } = useQuery({
 *   ...vectorStoreListOptions({
 *     query: {
 *       owner_type: "organization",
 *       owner_id: orgId,
 *       ...pagination.queryParams,
 *     },
 *   }),
 * });
 *
 * return (
 *   <ResourceTable
 *     data={data?.data}
 *     paginationProps={{
 *       pagination: pagination.toPaginationMeta(data),
 *       isFirstPage: pagination.info.isFirstPage,
 *       pageNumber: pagination.info.pageNumber,
 *       onPrevious: () => pagination.actions.goToPreviousPage(data!),
 *       onNext: () => pagination.actions.goToNextPage(data!),
 *       onFirst: () => pagination.actions.goToFirstPage(),
 *     }}
 *   />
 * );
 * ```
 */
export function useOpenAIPagination(
  options: UseOpenAIPaginationOptions = {}
): UseOpenAIPaginationResult {
  const { defaultLimit = 20 } = options;

  // Track the current cursor and direction
  const [after, setAfter] = useState<string | null>(null);
  const [before, setBefore] = useState<string | null>(null);
  const [limit, setLimit] = useState(defaultLimit);

  // Track page history for approximate page number display
  const [pageHistory, setPageHistory] = useState<string[]>([]);

  const goToNextPage = useCallback(
    (response: OpenAIPaginationResponse) => {
      if (response.has_more && response.last_id) {
        setPageHistory((prev) => [...prev, after || "first"]);
        setAfter(response.last_id);
        setBefore(null);
      }
    },
    [after]
  );

  const goToPreviousPage = useCallback(
    (response: OpenAIPaginationResponse) => {
      if (pageHistory.length > 0) {
        const newHistory = [...pageHistory];
        const prevCursor = newHistory.pop();
        setPageHistory(newHistory);
        setAfter(prevCursor === "first" ? null : prevCursor || null);
        setBefore(null);
      } else if (response.first_id) {
        // Fallback: use before cursor if no history
        setBefore(response.first_id);
        setAfter(null);
      }
    },
    [pageHistory]
  );

  const goToFirstPage = useCallback(() => {
    setAfter(null);
    setBefore(null);
    setPageHistory([]);
  }, []);

  const handleSetLimit = useCallback((newLimit: number) => {
    setLimit(newLimit);
    // Reset to first page when limit changes
    setAfter(null);
    setBefore(null);
    setPageHistory([]);
  }, []);

  const state: OpenAIPaginationState = useMemo(
    () => ({ after, before, limit }),
    [after, before, limit]
  );

  const actions: OpenAIPaginationActions = useMemo(
    () => ({
      goToNextPage,
      goToPreviousPage,
      goToFirstPage,
      setLimit: handleSetLimit,
    }),
    [goToNextPage, goToPreviousPage, goToFirstPage, handleSetLimit]
  );

  const info: OpenAIPaginationInfo = useMemo(
    () => ({
      isFirstPage: after === null && before === null,
      pageNumber: pageHistory.length + 1,
    }),
    [after, before, pageHistory.length]
  );

  const queryParams = useMemo(
    () => ({
      after: after ?? undefined,
      before: before ?? undefined,
      limit,
    }),
    [after, before, limit]
  );

  const toPaginationMeta = useCallback(
    (response?: OpenAIPaginationResponse) => {
      if (!response) return undefined;
      return {
        has_more: response.has_more,
        limit,
        next_cursor: response.has_more ? (response.last_id ?? null) : null,
        prev_cursor: pageHistory.length > 0 ? (response.first_id ?? null) : null,
      };
    },
    [limit, pageHistory.length]
  );

  return { state, actions, info, queryParams, toPaginationMeta };
}
