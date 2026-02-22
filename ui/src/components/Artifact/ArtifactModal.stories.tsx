import type { Meta, StoryObj } from "@storybook/react";
import { ArtifactModal } from "./ArtifactModal";
import type { Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/Artifacts/ArtifactModal",
  component: ArtifactModal,
  parameters: {
    layout: "fullscreen",
  },
} satisfies Meta<typeof ArtifactModal>;

export default meta;
type Story = StoryObj<typeof meta>;

const codeArtifact: Artifact = {
  id: "code-1",
  type: "code",
  title: "Python Script",
  data: {
    language: "python",
    code: `def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

# Calculate first 10 Fibonacci numbers
for i in range(10):
    print(f"F({i}) = {fibonacci(i)}")`,
  },
};

const tableArtifact: Artifact = {
  id: "table-1",
  type: "table",
  title: "Query Results",
  data: {
    columns: [
      { key: "id", label: "ID" },
      { key: "name", label: "Name" },
      { key: "status", label: "Status" },
    ],
    rows: [
      { id: 1, name: "Alice", status: "active" },
      { id: 2, name: "Bob", status: "pending" },
      { id: 3, name: "Charlie", status: "inactive" },
    ],
  },
};

export const CodeArtifact: Story = {
  args: {
    artifact: codeArtifact,
    open: true,
    onClose: () => {},
  },
};

export const TableArtifact: Story = {
  args: {
    artifact: tableArtifact,
    open: true,
    onClose: () => {},
  },
};

export const Closed: Story = {
  args: {
    artifact: codeArtifact,
    open: false,
    onClose: () => {},
  },
};
