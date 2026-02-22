import { ChevronLeft, ChevronRight, ChevronsLeft } from "lucide-react";
import { cn } from "@/utils/cn";
import { Button } from "@/components/Button/Button";
import type { PaginationMeta } from "@/api/generated";

export interface PaginationProps {
  /** Pagination metadata from the API response */
  pagination?: PaginationMeta;
  /** Whether we're on the first page */
  isFirstPage: boolean;
  /** Approximate page number (1-indexed) */
  pageNumber?: number;
  /** Called when user clicks "Previous" button */
  onPrevious: () => void;
  /** Called when user clicks "Next" button */
  onNext: () => void;
  /** Called when user clicks "First Page" button (optional) */
  onFirst?: () => void;
  /** Whether pagination is loading */
  isLoading?: boolean;
  /** Additional CSS classes */
  className?: string;
  /** Size of the pagination buttons */
  size?: "sm" | "md";
}

/**
 * Pagination component for cursor-based pagination.
 * Displays Previous/Next navigation with optional First Page button.
 *
 * @example
 * ```tsx
 * <Pagination
 *   pagination={data?.pagination}
 *   isFirstPage={pagination.info.isFirstPage}
 *   pageNumber={pagination.info.pageNumber}
 *   onPrevious={() => pagination.actions.goToPreviousPage(data!.pagination)}
 *   onNext={() => pagination.actions.goToNextPage(data!.pagination)}
 *   onFirst={() => pagination.actions.goToFirstPage()}
 * />
 * ```
 */
export function Pagination({
  pagination,
  isFirstPage,
  pageNumber,
  onPrevious,
  onNext,
  onFirst,
  isLoading = false,
  className,
  size = "sm",
}: PaginationProps) {
  const hasPrevious = !isFirstPage || pagination?.prev_cursor;
  const hasNext = pagination?.has_more;

  // Don't render if there's no pagination data and we're on the first page
  if (!pagination && isFirstPage) {
    return null;
  }

  return (
    <div
      className={cn(
        "flex items-center justify-between border-t border-border bg-background px-4 py-3",
        className
      )}
    >
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        {pageNumber && <span>Page {pageNumber}</span>}
        {pagination?.limit && (
          <span className="hidden sm:inline">({pagination.limit} per page)</span>
        )}
      </div>

      <div className="flex items-center gap-2">
        {onFirst && !isFirstPage && (
          <Button
            variant="outline"
            size={size}
            onClick={onFirst}
            disabled={isFirstPage || isLoading}
            aria-label="Go to first page"
          >
            <ChevronsLeft className="h-4 w-4" />
            <span className="hidden sm:inline">First</span>
          </Button>
        )}

        <Button
          variant="outline"
          size={size}
          onClick={onPrevious}
          disabled={!hasPrevious || isLoading}
          aria-label="Go to previous page"
        >
          <ChevronLeft className="h-4 w-4" />
          <span className="hidden sm:inline">Previous</span>
        </Button>

        <Button
          variant="outline"
          size={size}
          onClick={onNext}
          disabled={!hasNext || isLoading}
          aria-label="Go to next page"
        >
          <span className="hidden sm:inline">Next</span>
          <ChevronRight className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
