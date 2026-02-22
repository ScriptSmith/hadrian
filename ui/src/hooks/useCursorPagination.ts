import { useState, useCallback, useMemo } from "react";
import type { PaginationMeta } from "@/api/generated";

export type CursorDirection = "forward" | "backward";

export interface CursorPaginationState {
  /** Current cursor for the page being viewed (null for first page) */
  cursor: string | null;
  /** Direction of pagination */
  direction: CursorDirection;
  /** Number of items per page */
  limit: number;
}

export interface CursorPaginationActions {
  /** Go to the next page using the next_cursor from pagination meta */
  goToNextPage: (pagination: PaginationMeta) => void;
  /** Go to the previous page using the prev_cursor from pagination meta */
  goToPreviousPage: (pagination: PaginationMeta) => void;
  /** Reset to the first page */
  goToFirstPage: () => void;
  /** Set a custom limit */
  setLimit: (limit: number) => void;
}

export interface CursorPaginationInfo {
  /** Whether we're on the first page (no cursor) */
  isFirstPage: boolean;
  /** Page number for display (1-indexed) - note: this is an approximation */
  pageNumber: number;
}

export interface UseCursorPaginationResult {
  /** Current pagination state to pass to API queries */
  state: CursorPaginationState;
  /** Actions to navigate between pages */
  actions: CursorPaginationActions;
  /** Computed pagination info */
  info: CursorPaginationInfo;
  /** Query parameters to spread into API call options */
  queryParams: {
    cursor?: string;
    direction?: CursorDirection;
    limit: number;
  };
}

export interface UseCursorPaginationOptions {
  /** Initial limit (items per page). Default: 25 */
  defaultLimit?: number;
}

/**
 * Hook for managing cursor-based pagination state for admin API endpoints.
 *
 * Use this hook for `/admin/v1/*` endpoints that use `cursor`/`direction`
 * query parameters and return `PaginationMeta` with `next_cursor`/`prev_cursor`.
 *
 * For OpenAI-compatible `/api/v1/*` endpoints (like vector stores) that use
 * `after`/`before` parameters and return `first_id`/`last_id`/`has_more`,
 * use `useOpenAIPagination` instead.
 *
 * @example
 * ```tsx
 * const pagination = useCursorPagination({ defaultLimit: 25 });
 *
 * const { data } = useQuery(
 *   organizationListOptions({ query: pagination.queryParams })
 * );
 *
 * return (
 *   <div>
 *     <Table data={data?.data} />
 *     <Pagination
 *       pagination={data?.pagination}
 *       onPrevious={() => pagination.actions.goToPreviousPage(data!.pagination)}
 *       onNext={() => pagination.actions.goToNextPage(data!.pagination)}
 *       isFirstPage={pagination.info.isFirstPage}
 *     />
 *   </div>
 * );
 * ```
 */
export function useCursorPagination(
  options: UseCursorPaginationOptions = {}
): UseCursorPaginationResult {
  const { defaultLimit = 25 } = options;

  // Track the current cursor and direction
  const [cursor, setCursor] = useState<string | null>(null);
  const [direction, setDirection] = useState<CursorDirection>("forward");
  const [limit, setLimit] = useState(defaultLimit);

  // Track page history for approximate page number display
  // This is a stack of cursors we've visited
  const [pageHistory, setPageHistory] = useState<string[]>([]);

  const goToNextPage = useCallback(
    (pagination: PaginationMeta) => {
      if (pagination.next_cursor) {
        setPageHistory((prev) => [...prev, cursor || "first"]);
        setCursor(pagination.next_cursor);
        setDirection("forward");
      }
    },
    [cursor]
  );

  const goToPreviousPage = useCallback(
    (pagination: PaginationMeta) => {
      if (pagination.prev_cursor) {
        setPageHistory((prev) => prev.slice(0, -1));
        setCursor(pagination.prev_cursor);
        setDirection("backward");
      } else if (pageHistory.length > 0) {
        // If no prev_cursor but we have history, go back
        const newHistory = [...pageHistory];
        const prevCursor = newHistory.pop();
        setPageHistory(newHistory);
        setCursor(prevCursor === "first" ? null : prevCursor || null);
        setDirection("backward");
      }
    },
    [pageHistory]
  );

  const goToFirstPage = useCallback(() => {
    setCursor(null);
    setDirection("forward");
    setPageHistory([]);
  }, []);

  const handleSetLimit = useCallback((newLimit: number) => {
    setLimit(newLimit);
    // Reset to first page when limit changes
    setCursor(null);
    setDirection("forward");
    setPageHistory([]);
  }, []);

  const state: CursorPaginationState = useMemo(
    () => ({ cursor, direction, limit }),
    [cursor, direction, limit]
  );

  const actions: CursorPaginationActions = useMemo(
    () => ({
      goToNextPage,
      goToPreviousPage,
      goToFirstPage,
      setLimit: handleSetLimit,
    }),
    [goToNextPage, goToPreviousPage, goToFirstPage, handleSetLimit]
  );

  const info: CursorPaginationInfo = useMemo(
    () => ({
      isFirstPage: cursor === null,
      pageNumber: pageHistory.length + 1,
    }),
    [cursor, pageHistory.length]
  );

  const queryParams = useMemo(
    () => ({
      cursor: cursor ?? undefined,
      direction: cursor ? direction : undefined,
      limit,
    }),
    [cursor, direction, limit]
  );

  return { state, actions, info, queryParams };
}
