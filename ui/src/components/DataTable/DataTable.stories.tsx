import type { Meta, StoryObj } from "@storybook/react";
import type { ColumnDef } from "@tanstack/react-table";
import { DataTable } from "./DataTable";
import { Badge } from "../Badge/Badge";

const meta = {
  title: "UI/DataTable",
  component: DataTable,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof DataTable>;

export default meta;
type Story = StoryObj<typeof meta>;

// Sample data types
interface User {
  id: string;
  name: string;
  email: string;
  role: "admin" | "user" | "viewer";
  status: "active" | "inactive";
}

interface ApiKey {
  id: string;
  name: string;
  prefix: string;
  createdAt: Date;
  lastUsed: Date | null;
}

// Sample data
const users: User[] = [
  { id: "1", name: "Alice Johnson", email: "alice@example.com", role: "admin", status: "active" },
  { id: "2", name: "Bob Smith", email: "bob@example.com", role: "user", status: "active" },
  {
    id: "3",
    name: "Charlie Brown",
    email: "charlie@example.com",
    role: "viewer",
    status: "inactive",
  },
  { id: "4", name: "Diana Prince", email: "diana@example.com", role: "user", status: "active" },
  { id: "5", name: "Edward Norton", email: "edward@example.com", role: "viewer", status: "active" },
];

const apiKeys: ApiKey[] = [
  {
    id: "1",
    name: "Production API",
    prefix: "gw_live_abc",
    createdAt: new Date("2024-01-15"),
    lastUsed: new Date("2024-03-10"),
  },
  {
    id: "2",
    name: "Development",
    prefix: "gw_live_def",
    createdAt: new Date("2024-02-20"),
    lastUsed: new Date("2024-03-09"),
  },
  {
    id: "3",
    name: "Testing",
    prefix: "gw_live_ghi",
    createdAt: new Date("2024-03-01"),
    lastUsed: null,
  },
  {
    id: "4",
    name: "CI/CD Pipeline",
    prefix: "gw_live_jkl",
    createdAt: new Date("2024-03-05"),
    lastUsed: new Date("2024-03-10"),
  },
];

// Column definitions
const userColumns: ColumnDef<User>[] = [
  {
    accessorKey: "name",
    header: "Name",
  },
  {
    accessorKey: "email",
    header: "Email",
  },
  {
    accessorKey: "role",
    header: "Role",
    cell: ({ row }) => {
      const role = row.getValue("role") as string;
      return <Badge variant={role === "admin" ? "default" : "secondary"}>{role}</Badge>;
    },
  },
  {
    accessorKey: "status",
    header: "Status",
    cell: ({ row }) => {
      const status = row.getValue("status") as string;
      return <Badge variant={status === "active" ? "default" : "outline"}>{status}</Badge>;
    },
  },
];

const apiKeyColumns: ColumnDef<ApiKey>[] = [
  {
    accessorKey: "name",
    header: "Name",
  },
  {
    accessorKey: "prefix",
    header: "Key Prefix",
    cell: ({ row }) => (
      <code className="rounded bg-muted px-1.5 py-0.5 text-sm">{row.getValue("prefix")}...</code>
    ),
  },
  {
    accessorKey: "createdAt",
    header: "Created",
    cell: ({ row }) => {
      const date = row.getValue("createdAt") as Date;
      return date.toLocaleDateString();
    },
  },
  {
    accessorKey: "lastUsed",
    header: "Last Used",
    cell: ({ row }) => {
      const date = row.getValue("lastUsed") as Date | null;
      return date ? date.toLocaleDateString() : "Never";
    },
  },
];

export const Default: Story = {
  args: {
    columns: userColumns as ColumnDef<unknown, unknown>[],
    data: users,
  },
};

export const WithSearch: Story = {
  args: {
    columns: userColumns as ColumnDef<unknown, unknown>[],
    data: users,
    searchColumn: "name",
    searchPlaceholder: "Search users...",
  },
};

export const WithPagination: Story = {
  args: {
    columns: userColumns as ColumnDef<unknown, unknown>[],
    data: [...users, ...users, ...users], // Triple the data for pagination demo
    enablePagination: true,
    pageSize: 5,
  },
};

export const WithSorting: Story = {
  args: {
    columns: userColumns as ColumnDef<unknown, unknown>[],
    data: users,
    enableSorting: true,
  },
};

export const FullFeatures: Story = {
  args: {
    columns: userColumns as ColumnDef<unknown, unknown>[],
    data: [...users, ...users],
    searchColumn: "name",
    searchPlaceholder: "Search users...",
    enablePagination: true,
    pageSize: 5,
    enableSorting: true,
  },
};

export const ApiKeys: Story = {
  args: {
    columns: apiKeyColumns as ColumnDef<unknown, unknown>[],
    data: apiKeys,
    searchColumn: "name",
    searchPlaceholder: "Search API keys...",
    enableSorting: true,
  },
};

export const Loading: Story = {
  args: {
    columns: userColumns as ColumnDef<unknown, unknown>[],
    data: [],
    isLoading: true,
  },
};

export const WithError: Story = {
  args: {
    columns: userColumns as ColumnDef<unknown, unknown>[],
    data: [],
    error: new Error("Failed to fetch data"),
  },
};

export const Empty: Story = {
  args: {
    columns: userColumns as ColumnDef<unknown, unknown>[],
    data: [],
    emptyMessage: "No users found. Create your first user to get started.",
  },
};

export const NoSorting: Story = {
  args: {
    columns: userColumns as ColumnDef<unknown, unknown>[],
    data: users,
    enableSorting: false,
  },
};
