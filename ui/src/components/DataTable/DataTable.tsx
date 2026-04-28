import {
  flexRender,
  getCoreRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  getFilteredRowModel,
  useReactTable,
  type ColumnDef,
  type SortingState,
  type ColumnFiltersState,
  type VisibilityState,
  type Updater,
  type PaginationState,
} from "@tanstack/react-table";
import { ChevronDown, ChevronUp, ChevronsUpDown, Search, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";

import { cn } from "@/utils/cn";

import { Button } from "@/components/Button/Button";
import { Input } from "@/components/Input/Input";
import { Skeleton } from "@/components/Skeleton/Skeleton";

interface DataTableProps<TData, TValue> {
  columns: ColumnDef<TData, TValue>[];
  data: TData[];
  isLoading?: boolean;
  error?: Error | null;
  emptyMessage?: string;
  searchPlaceholder?: string;
  searchColumn?: string;
  enablePagination?: boolean;
  pageSize?: number;
  enableSorting?: boolean;
  className?: string;
  /**
   * If set, the table mirrors search/sort/page state into `useSearchParams`
   * under this prefix (`<prefix>_q`, `<prefix>_sort`, `<prefix>_page`) so
   * deep-linking, refresh, and back-navigation preserve filter state. Pass
   * different prefixes when multiple tables share a page.
   */
  urlStateKey?: string;
}

export function DataTable<TData, TValue>({
  columns,
  data,
  isLoading = false,
  error = null,
  emptyMessage = "No data to display.",
  searchPlaceholder = "Search...",
  searchColumn,
  enablePagination = false,
  pageSize = 10,
  enableSorting = true,
  className,
  urlStateKey,
}: DataTableProps<TData, TValue>) {
  const [searchParams, setSearchParams] = useSearchParams();

  const queryKey = urlStateKey ? `${urlStateKey}_q` : null;
  const sortKey = urlStateKey ? `${urlStateKey}_sort` : null;
  const pageKey = urlStateKey ? `${urlStateKey}_page` : null;

  // When `urlStateKey` is set, the URL is the source of truth and the local
  // state mirrors it (for the column-filter case where react-table needs an
  // object). When unset, behaviour is identical to the previous in-memory
  // implementation.
  const updateUrlParam = useCallback(
    (key: string, value: string | null) => {
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          if (value && value.length > 0) {
            next.set(key, value);
          } else {
            next.delete(key);
          }
          return next;
        },
        { replace: true }
      );
    },
    [setSearchParams]
  );

  // sorting: encoded as `field` or `field:desc`
  const initialSorting: SortingState = (() => {
    if (!sortKey) return [];
    const raw = searchParams.get(sortKey);
    if (!raw) return [];
    const [id, dir] = raw.split(":", 2);
    return id ? [{ id, desc: dir === "desc" }] : [];
  })();
  const [localSorting, setLocalSorting] = useState<SortingState>(initialSorting);
  const sorting = sortKey ? initialSorting : localSorting;
  const handleSortingChange = useCallback(
    (updater: Updater<SortingState>) => {
      const next = typeof updater === "function" ? updater(sorting) : updater;
      if (sortKey) {
        const first = next[0];
        updateUrlParam(sortKey, first ? `${first.id}${first.desc ? ":desc" : ""}` : null);
      } else {
        setLocalSorting(next);
      }
    },
    [sorting, sortKey, updateUrlParam]
  );

  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([]);
  const [columnVisibility, setColumnVisibility] = useState<VisibilityState>({});

  // search: stored as `<prefix>_q` (covers both column-scoped and global filters)
  const initialSearch = queryKey ? (searchParams.get(queryKey) ?? "") : "";
  const [localGlobalFilter, setLocalGlobalFilter] = useState(initialSearch);
  const globalFilter = queryKey ? (searchParams.get(queryKey) ?? "") : localGlobalFilter;

  // pagination: 1-indexed in the URL for human-friendly deep-links
  const initialPageIndex = (() => {
    if (!pageKey) return 0;
    const raw = parseInt(searchParams.get(pageKey) ?? "1", 10);
    return Number.isFinite(raw) && raw > 0 ? raw - 1 : 0;
  })();
  const [localPagination, setLocalPagination] = useState<PaginationState>({
    pageIndex: initialPageIndex,
    pageSize,
  });
  const pagination: PaginationState = pageKey
    ? { pageIndex: initialPageIndex, pageSize }
    : localPagination;
  const handlePaginationChange = useCallback(
    (updater: Updater<PaginationState>) => {
      const next = typeof updater === "function" ? updater(pagination) : updater;
      if (pageKey) {
        updateUrlParam(pageKey, next.pageIndex > 0 ? String(next.pageIndex + 1) : null);
      } else {
        setLocalPagination(next);
      }
    },
    // `pagination` legitimately depends on `localPagination`/URL state per render
    // and we want the closure to read the latest value when the updater fires.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [pageKey, updateUrlParam, pagination.pageIndex, pagination.pageSize]
  );

  // For searchColumn mode, push the URL value into the column filter on mount
  // so deep-links populate the filtered view even though the search value is
  // owned by the column filter rather than `globalFilter`.
  useEffect(() => {
    if (!queryKey || !searchColumn) return;
    const urlValue = searchParams.get(queryKey) ?? "";
    if (urlValue.length === 0) return;
    setColumnFilters((prev) => {
      const existing = prev.find((f) => f.id === searchColumn);
      if (existing && existing.value === urlValue) return prev;
      return [...prev.filter((f) => f.id !== searchColumn), { id: searchColumn, value: urlValue }];
    });
    // We intentionally only run on mount + when the key/column change; keystrokes
    // already push through `handleSearch`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [queryKey, searchColumn]);

  const setGlobalFilter = useCallback(
    (value: string) => {
      if (queryKey) {
        updateUrlParam(queryKey, value);
      } else {
        setLocalGlobalFilter(value);
      }
    },
    [queryKey, updateUrlParam]
  );

  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    ...(enablePagination && {
      getPaginationRowModel: getPaginationRowModel(),
      onPaginationChange: handlePaginationChange,
    }),
    ...(enableSorting && {
      getSortedRowModel: getSortedRowModel(),
      onSortingChange: handleSortingChange,
    }),
    // Always enable the filtered row model when filtering is possible —
    // either column-scoped (searchColumn) or via globalFilter — so the
    // search input doesn't silently no-op when `searchColumn` is unset.
    getFilteredRowModel: getFilteredRowModel(),
    onColumnFiltersChange: setColumnFilters,
    onColumnVisibilityChange: setColumnVisibility,
    onGlobalFilterChange: (updater) => {
      const next =
        typeof updater === "function"
          ? (updater as (prev: string) => string)(globalFilter)
          : updater;
      setGlobalFilter(next);
    },
    state: {
      sorting,
      columnFilters,
      columnVisibility,
      globalFilter,
      ...(enablePagination ? { pagination } : {}),
    },
    initialState: {
      pagination: {
        pageSize,
      },
    },
  });

  // Handle search input
  const handleSearch = (value: string) => {
    if (searchColumn) {
      table.getColumn(searchColumn)?.setFilterValue(value);
      // Also mirror into the URL when present so column-scoped searches survive
      // refresh, even though they don't flow through globalFilter.
      if (queryKey) updateUrlParam(queryKey, value);
    } else {
      setGlobalFilter(value);
    }
  };

  const searchValue = searchColumn
    ? ((table.getColumn(searchColumn)?.getFilterValue() as string) ??
      (queryKey ? (searchParams.get(queryKey) ?? "") : ""))
    : globalFilter;

  if (isLoading) {
    return (
      <div className="space-y-3">
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-10 w-full" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="py-8 text-center text-destructive">
        Failed to load data. Please try again.
      </div>
    );
  }

  return (
    <div className={cn("space-y-4", className)}>
      {/* Search input */}
      {searchColumn !== undefined && (
        <div className="relative max-w-sm">
          <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder={searchPlaceholder}
            value={searchValue}
            onChange={(e) => handleSearch(e.target.value)}
            className="pl-9"
            aria-label={searchPlaceholder}
          />
          {searchValue && (
            <Button
              variant="ghost"
              size="icon"
              className="absolute right-1 top-1 h-7 w-7"
              onClick={() => handleSearch("")}
              aria-label="Clear search"
            >
              <X className="h-3 w-3" />
            </Button>
          )}
        </div>
      )}

      {/* Table */}
      {data.length === 0 && !searchValue ? (
        <div className="py-8 text-center text-muted-foreground">{emptyMessage}</div>
      ) : (
        <>
          <div className="overflow-x-auto rounded-md border">
            <table className="w-full">
              <thead className="bg-muted/50">
                {table.getHeaderGroups().map((headerGroup) => (
                  <tr key={headerGroup.id}>
                    {headerGroup.headers.map((header) => (
                      <th
                        key={header.id}
                        className={cn(
                          "px-4 py-3 text-left text-sm font-medium text-muted-foreground",
                          header.column.getCanSort() &&
                            enableSorting &&
                            "cursor-pointer select-none"
                        )}
                        onClick={
                          header.column.getCanSort() && enableSorting
                            ? header.column.getToggleSortingHandler()
                            : undefined
                        }
                      >
                        {header.isPlaceholder ? null : "accessorKey" in header.column.columnDef ||
                          "accessorFn" in header.column.columnDef ? (
                          <div className="flex items-center gap-2">
                            {flexRender(header.column.columnDef.header, header.getContext())}
                            {header.column.getCanSort() && enableSorting && (
                              <span className="text-muted-foreground/50">
                                {header.column.getIsSorted() === "asc" ? (
                                  <ChevronUp className="h-4 w-4" />
                                ) : header.column.getIsSorted() === "desc" ? (
                                  <ChevronDown className="h-4 w-4" />
                                ) : (
                                  <ChevronsUpDown className="h-4 w-4" />
                                )}
                              </span>
                            )}
                          </div>
                        ) : (
                          <span className="sr-only">Actions</span>
                        )}
                      </th>
                    ))}
                  </tr>
                ))}
              </thead>
              <tbody>
                {table.getRowModel().rows.length === 0 ? (
                  <tr>
                    <td
                      colSpan={columns.length}
                      className="px-4 py-8 text-center text-muted-foreground"
                    >
                      No results found.
                    </td>
                  </tr>
                ) : (
                  table.getRowModel().rows.map((row) => (
                    <tr key={row.id} className="border-t transition-colors hover:bg-muted/50">
                      {row.getVisibleCells().map((cell) => (
                        <td key={cell.id} className="px-4 py-3 text-sm">
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </td>
                      ))}
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          {enablePagination && table.getPageCount() > 1 && (
            <div className="flex items-center justify-between">
              <div className="text-sm text-muted-foreground">
                Page {table.getState().pagination.pageIndex + 1} of {table.getPageCount()}
              </div>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => table.previousPage()}
                  disabled={!table.getCanPreviousPage()}
                >
                  Previous
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => table.nextPage()}
                  disabled={!table.getCanNextPage()}
                >
                  Next
                </Button>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
