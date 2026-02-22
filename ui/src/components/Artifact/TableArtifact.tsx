/**
 * TableArtifact - Data Table with Sorting
 *
 * Renders tabular data from tools like SQL queries or data analysis.
 * Uses @tanstack/react-table for sorting and virtualization support.
 */

import { memo, useState, useMemo } from "react";
import { ArrowUpDown, ArrowUp, ArrowDown, Copy, Check } from "lucide-react";
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  flexRender,
  type SortingState,
  type ColumnDef,
} from "@tanstack/react-table";

import type { Artifact, TableArtifactData } from "@/components/chat-types";
import { Button } from "@/components/Button/Button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/Tooltip/Tooltip";
import { cn } from "@/utils/cn";

export interface TableArtifactProps {
  artifact: Artifact;
  className?: string;
}

function isTableArtifactData(data: unknown): data is TableArtifactData {
  return (
    typeof data === "object" &&
    data !== null &&
    "columns" in data &&
    Array.isArray((data as TableArtifactData).columns) &&
    "rows" in data &&
    Array.isArray((data as TableArtifactData).rows)
  );
}

/** Format cell value for display */
function formatCellValue(value: unknown): string {
  if (value === null || value === undefined) {
    return "—";
  }
  if (typeof value === "boolean") {
    return value ? "true" : "false";
  }
  if (typeof value === "number") {
    // Format numbers with locale
    return value.toLocaleString();
  }
  if (typeof value === "object") {
    return JSON.stringify(value);
  }
  return String(value);
}

/** Convert table data to CSV for copying */
function tableToCsv(
  columns: TableArtifactData["columns"],
  rows: TableArtifactData["rows"]
): string {
  const header = columns.map((c) => `"${c.label.replace(/"/g, '""')}"`).join(",");
  const dataRows = rows.map((row) =>
    columns
      .map((c) => {
        const value = row[c.key];
        const str = formatCellValue(value);
        return `"${str.replace(/"/g, '""')}"`;
      })
      .join(",")
  );
  return [header, ...dataRows].join("\n");
}

function TableArtifactComponent({ artifact, className }: TableArtifactProps) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [copied, setCopied] = useState(false);

  // Validate data - extract or use empty defaults for hooks
  const isValid = isTableArtifactData(artifact.data);
  const emptyData: TableArtifactData = { columns: [], rows: [] };
  const tableData: TableArtifactData = isValid ? (artifact.data as TableArtifactData) : emptyData;
  const { columns: columnDefs, rows } = tableData;

  // Build column definitions for react-table
  const columns = useMemo<ColumnDef<Record<string, unknown>>[]>(
    () =>
      columnDefs.map((col) => ({
        accessorKey: col.key,
        header: ({ column }) => (
          <Button
            variant="ghost"
            size="sm"
            className="-ml-3 h-8 font-medium"
            onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
          >
            {col.label}
            {column.getIsSorted() === "asc" ? (
              <ArrowUp className="ml-1 h-3 w-3" />
            ) : column.getIsSorted() === "desc" ? (
              <ArrowDown className="ml-1 h-3 w-3" />
            ) : (
              <ArrowUpDown className="ml-1 h-3 w-3 opacity-50" />
            )}
          </Button>
        ),
        cell: ({ getValue }) => {
          const value = getValue();
          return <span className="font-mono text-xs">{formatCellValue(value)}</span>;
        },
      })),
    [columnDefs]
  );

  const table = useReactTable({
    data: rows,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const handleCopy = async () => {
    const csv = tableToCsv(columnDefs, rows);
    await navigator.clipboard.writeText(csv);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  // Render error state if data is invalid
  if (!isValid) {
    return <div className="p-4 text-sm text-muted-foreground">Invalid table artifact data</div>;
  }

  return (
    <div className={cn("relative", className)}>
      {/* Header with copy button */}
      <div className="absolute right-2 top-2 z-10">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="secondary" size="sm" className="h-7 px-2 text-xs" onClick={handleCopy}>
              {copied ? (
                <>
                  <Check className="h-3 w-3 mr-1" />
                  Copied
                </>
              ) : (
                <>
                  <Copy className="h-3 w-3 mr-1" />
                  CSV
                </>
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent>Copy as CSV</TooltipContent>
        </Tooltip>
      </div>

      {/* Row count */}
      <div className="px-3 py-2 text-xs text-muted-foreground">
        {rows.length} row{rows.length !== 1 ? "s" : ""}
      </div>

      {/* Table */}
      <div className="overflow-x-auto max-h-[400px] overflow-y-auto">
        <table className="w-full text-sm">
          <thead className="sticky top-0 bg-muted/80 backdrop-blur-sm">
            {table.getHeaderGroups().map((headerGroup) => (
              <tr key={headerGroup.id} className="border-b">
                {headerGroup.headers.map((header) => (
                  <th
                    key={header.id}
                    className="px-3 py-2 text-left font-medium text-muted-foreground"
                  >
                    {header.isPlaceholder
                      ? null
                      : flexRender(header.column.columnDef.header, header.getContext())}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {table.getRowModel().rows.map((row) => (
              <tr
                key={row.id}
                className="border-b last:border-0 hover:bg-muted/30 transition-colors"
              >
                {row.getVisibleCells().map((cell) => (
                  <td key={cell.id} className="px-3 py-2">
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>

        {rows.length === 0 && (
          <div className="px-3 py-8 text-center text-sm text-muted-foreground">No data</div>
        )}
      </div>
    </div>
  );
}

export const TableArtifact = memo(TableArtifactComponent);
