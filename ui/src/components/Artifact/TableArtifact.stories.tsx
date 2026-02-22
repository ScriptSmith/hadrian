import type { Meta, StoryObj } from "@storybook/react";
import { TableArtifact } from "./TableArtifact";
import type { Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/Artifacts/TableArtifact",
  component: TableArtifact,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof TableArtifact>;

export default meta;
type Story = StoryObj<typeof meta>;

const makeArtifact = (
  columns: Array<{ key: string; label: string }>,
  rows: Array<Record<string, unknown>>
): Artifact => ({
  id: "table-1",
  type: "table",
  title: "Query Results",
  data: { columns, rows },
});

export const Default: Story = {
  args: {
    artifact: makeArtifact(
      [
        { key: "id", label: "ID" },
        { key: "name", label: "Name" },
        { key: "email", label: "Email" },
        { key: "status", label: "Status" },
      ],
      [
        { id: 1, name: "Alice Johnson", email: "alice@example.com", status: "active" },
        { id: 2, name: "Bob Smith", email: "bob@example.com", status: "pending" },
        { id: 3, name: "Charlie Brown", email: "charlie@example.com", status: "inactive" },
        { id: 4, name: "Diana Prince", email: "diana@example.com", status: "active" },
      ]
    ),
  },
};

export const NumericData: Story = {
  args: {
    artifact: makeArtifact(
      [
        { key: "product", label: "Product" },
        { key: "revenue", label: "Revenue" },
        { key: "units", label: "Units Sold" },
        { key: "margin", label: "Margin %" },
      ],
      [
        { product: "Widget A", revenue: 125000, units: 5000, margin: 32.5 },
        { product: "Widget B", revenue: 89000, units: 3200, margin: 28.1 },
        { product: "Gadget X", revenue: 210000, units: 7500, margin: 45.0 },
      ]
    ),
  },
};

export const Empty: Story = {
  args: {
    artifact: makeArtifact(
      [
        { key: "id", label: "ID" },
        { key: "name", label: "Name" },
      ],
      []
    ),
  },
};

export const ManyRows: Story = {
  args: {
    artifact: makeArtifact(
      [
        { key: "index", label: "Index" },
        { key: "value", label: "Value" },
      ],
      Array(50)
        .fill(null)
        .map((_, i) => ({ index: i + 1, value: Math.random().toFixed(4) }))
    ),
  },
};
