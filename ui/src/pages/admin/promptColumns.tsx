import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { MoreHorizontal, Pencil, Trash2 } from "lucide-react";

import type { Template } from "@/api/generated/types.gen";
import {
  Dropdown,
  DropdownContent,
  DropdownItem,
  DropdownTrigger,
} from "@/components/Dropdown/Dropdown";
import { formatDateTime } from "@/utils/formatters";

const columnHelper = createColumnHelper<Template>();

export function createTemplateColumns(
  onEdit: (prompt: Template) => void,
  onDelete: (prompt: Template) => void
): ColumnDef<Template, unknown>[] {
  return [
    columnHelper.accessor("name", {
      header: "Name",
      cell: (info) => <span className="font-medium">{info.getValue()}</span>,
    }),
    columnHelper.accessor("description", {
      header: "Description",
      cell: (info) => {
        const val = info.getValue();
        return val ? (
          <span className="line-clamp-1">{val}</span>
        ) : (
          <span className="text-muted-foreground">-</span>
        );
      },
    }),
    columnHelper.accessor("content", {
      header: "Content",
      cell: (info) => (
        <span className="text-muted-foreground line-clamp-1 max-w-[200px]">
          {info.getValue().slice(0, 80)}
          {info.getValue().length > 80 ? "..." : ""}
        </span>
      ),
    }),
    columnHelper.accessor("created_at", {
      header: "Created",
      cell: (info) => formatDateTime(info.getValue()),
    }),
    columnHelper.display({
      id: "actions",
      cell: ({ row }) => (
        <Dropdown>
          <DropdownTrigger aria-label="Template actions" variant="ghost" className="h-8 w-8 p-0">
            <MoreHorizontal className="h-4 w-4" />
          </DropdownTrigger>
          <DropdownContent align="end">
            <DropdownItem onClick={() => onEdit(row.original)}>
              <Pencil className="mr-2 h-4 w-4" />
              Edit
            </DropdownItem>
            <DropdownItem className="text-destructive" onClick={() => onDelete(row.original)}>
              <Trash2 className="mr-2 h-4 w-4" />
              Delete
            </DropdownItem>
          </DropdownContent>
        </Dropdown>
      ),
    }),
  ] as ColumnDef<Template, unknown>[];
}
