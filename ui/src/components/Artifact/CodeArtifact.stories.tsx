import type { Meta, StoryObj } from "@storybook/react";
import { CodeArtifact } from "./CodeArtifact";
import type { Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/Artifacts/CodeArtifact",
  component: CodeArtifact,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof CodeArtifact>;

export default meta;
type Story = StoryObj<typeof meta>;

const makeArtifact = (code: string, language: string): Artifact => ({
  id: "code-1",
  type: "code",
  title: "Code Output",
  data: { code, language },
});

export const Python: Story = {
  args: {
    artifact: makeArtifact(
      `def greet(name: str) -> str:
    """Return a greeting message."""
    return f"Hello, {name}!"

# Example usage
print(greet("World"))`,
      "python"
    ),
  },
};

export const JavaScript: Story = {
  args: {
    artifact: makeArtifact(
      `const fetchUsers = async () => {
  const response = await fetch('/api/users');
  const data = await response.json();
  return data.users;
};

fetchUsers().then(console.log);`,
      "javascript"
    ),
  },
};

export const LongCode: Story = {
  args: {
    artifact: makeArtifact(
      Array(50)
        .fill(null)
        .map((_, i) => `console.log("Line ${i + 1}");`)
        .join("\n"),
      "javascript"
    ),
  },
};

export const NoLanguage: Story = {
  args: {
    artifact: {
      id: "code-plain",
      type: "code",
      title: "Plain Text",
      data: { code: "Just some plain text output" },
    },
  },
};
