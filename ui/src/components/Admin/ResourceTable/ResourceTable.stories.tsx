import type { Meta, StoryObj } from "@storybook/react";
import { createColumnHelper } from "@tanstack/react-table";

import { ResourceTable, type ResourceTableProps } from "./ResourceTable";

const meta: Meta<typeof ResourceTable> = {
  title: "Admin/ResourceTable",
  component: ResourceTable,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<ResourceTableProps<User>>;

interface User {
  id: string;
  name: string;
  email: string;
  createdAt: string;
}

const columnHelper = createColumnHelper<User>();

const columns = [
  columnHelper.accessor("name", {
    header: "Name",
    cell: (info) => <span className="font-medium">{info.getValue()}</span>,
  }),
  columnHelper.accessor("email", {
    header: "Email",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("createdAt", {
    header: "Created",
    cell: (info) => info.getValue(),
  }),
];

const mockData: User[] = [
  { id: "1", name: "John Doe", email: "john@example.com", createdAt: "2024-01-01" },
  { id: "2", name: "Jane Smith", email: "jane@example.com", createdAt: "2024-01-02" },
  { id: "3", name: "Bob Wilson", email: "bob@example.com", createdAt: "2024-01-03" },
];

export const Default: Story = {
  args: {
    title: "All Users",
    columns,
    data: mockData,
  },
};

export const Loading: Story = {
  args: {
    title: "All Users",
    columns,
    data: [],
    isLoading: true,
  },
};

export const Empty: Story = {
  args: {
    title: "All Users",
    columns,
    data: [],
    emptyMessage: "No users yet. Create one to get started.",
  },
};

export const WithError: Story = {
  args: {
    title: "All Users",
    columns,
    data: [],
    error: new Error("Network error"),
    errorMessage: "Failed to load users. Please try again.",
  },
};

export const NoDataMessage: Story = {
  args: {
    title: "All Projects",
    columns,
    data: [],
    noDataMessage: "Create an organization first to manage projects.",
  },
};
