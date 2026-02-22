import type { Meta, StoryObj } from "@storybook/react";
import { AgentArtifact } from "./AgentArtifact";
import type { Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/Artifacts/AgentArtifact",
  component: AgentArtifact,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof AgentArtifact>;

export default meta;
type Story = StoryObj<typeof meta>;

const makeArtifact = (data: object): Artifact => ({
  id: "agent-1",
  type: "agent",
  title: "Sub-Agent Result",
  data,
});

export const Default: Story = {
  args: {
    artifact: makeArtifact({
      task: "Research the latest React 19 features and summarize the key changes.",
      model: "anthropic/claude-3-opus",
      internal:
        "I searched for React 19 features and found information about the new compiler, Server Components improvements, and the new use() hook. Let me compile a comprehensive summary...",
      output:
        "## React 19 Key Features\n\n1. **React Compiler** - Automatic memoization\n2. **Actions** - Native form handling\n3. **use() hook** - Better async data handling",
      usage: {
        inputTokens: 150,
        outputTokens: 200,
        totalTokens: 350,
        cost: 0.0025,
      },
    }),
  },
};

export const LongReasoning: Story = {
  args: {
    artifact: makeArtifact({
      task: "Analyze the performance bottlenecks in our database queries.",
      model: "openai/gpt-4",
      internal:
        "Starting analysis of the database queries. First, I need to understand the query patterns. Looking at the logs, I see several N+1 queries and missing indexes. The main issues are:\n\n1. The users table is missing an index on email\n2. The orders query is doing a full table scan\n3. There are multiple sequential queries that could be batched\n\nLet me investigate further and provide recommendations...",
      output:
        "Found 3 critical performance issues:\n- Add index on users.email\n- Optimize orders query with proper JOINs\n- Batch the 5 sequential queries into 1",
    }),
  },
};

export const NoUsage: Story = {
  args: {
    artifact: makeArtifact({
      task: "Quick lookup",
      model: "anthropic/claude-3-haiku",
      internal: "Simple lookup performed.",
      output: "The answer is 42.",
    }),
  },
};
