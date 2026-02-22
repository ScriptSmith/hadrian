import { flexRender, getCoreRowModel, useReactTable, type ColumnDef } from "@tanstack/react-table";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { Skeleton } from "@/components/Skeleton/Skeleton";
import { Pagination } from "@/components/Pagination/Pagination";
import type { PaginationMeta } from "@/api/generated";

export interface ResourceTablePaginationProps {
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
}

export interface ResourceTableProps<T> {
  title: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  columns: ColumnDef<T, any>[];
  data: T[];
  isLoading?: boolean;
  error?: unknown;
  emptyMessage?: string;
  noDataMessage?: string;
  errorMessage?: string;
  /** Optional pagination props for cursor-based pagination */
  paginationProps?: ResourceTablePaginationProps;
}

export function ResourceTable<T>({
  title,
  columns,
  data,
  isLoading = false,
  error,
  emptyMessage = "No items yet. Create one to get started.",
  noDataMessage,
  errorMessage = "Failed to load data. Please try again.",
  paginationProps,
}: ResourceTableProps<T>) {
  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent>
        {noDataMessage ? (
          <div className="py-8 text-center text-muted-foreground">{noDataMessage}</div>
        ) : isLoading ? (
          <div className="space-y-3">
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
          </div>
        ) : error ? (
          <div className="py-8 text-center text-destructive">{errorMessage}</div>
        ) : data.length === 0 ? (
          <div className="py-8 text-center text-muted-foreground">{emptyMessage}</div>
        ) : (
          <>
            <div className="overflow-x-auto">
              <table className="w-full">
                <thead>
                  {table.getHeaderGroups().map((headerGroup) => (
                    <tr key={headerGroup.id} className="border-b">
                      {headerGroup.headers.map((header) => (
                        <th
                          key={header.id}
                          className="px-4 py-3 text-left text-sm font-medium text-muted-foreground"
                        >
                          {header.isPlaceholder ? null : "accessorKey" in header.column.columnDef ||
                            "accessorFn" in header.column.columnDef ? (
                            flexRender(header.column.columnDef.header, header.getContext())
                          ) : (
                            <span className="sr-only">Actions</span>
                          )}
                        </th>
                      ))}
                    </tr>
                  ))}
                </thead>
                <tbody>
                  {table.getRowModel().rows.map((row) => (
                    <tr key={row.id} className="border-b last:border-0">
                      {row.getVisibleCells().map((cell) => (
                        <td key={cell.id} className="px-4 py-3 text-sm">
                          {flexRender(cell.column.columnDef.cell, cell.getContext())}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {paginationProps && (
              <Pagination
                pagination={paginationProps.pagination}
                isFirstPage={paginationProps.isFirstPage}
                pageNumber={paginationProps.pageNumber}
                onPrevious={paginationProps.onPrevious}
                onNext={paginationProps.onNext}
                onFirst={paginationProps.onFirst}
                isLoading={isLoading}
                className="mt-4"
              />
            )}
          </>
        )}
      </CardContent>
    </Card>
  );
}
