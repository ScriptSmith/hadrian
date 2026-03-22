import type { Meta, StoryObj } from "@storybook/react";
import { expect, within, fn } from "storybook/test";
import { ContentRound } from "./ContentRound";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { useChatUIStore } from "@/stores/chatUIStore";
import type { ToolExecutionRound, ToolExecution, Artifact } from "@/components/chat-types";

const makeExecution = (
  toolName: string,
  status: ToolExecution["status"],
  duration?: number,
): ToolExecution => ({
  id: `exec-${Math.random().toString(36).slice(2)}`,
  toolName,
  status,
  startTime: Date.now() - (duration || 0),
  endTime: status !== "running" ? Date.now() : undefined,
  duration,
  input: {},
  inputArtifacts: [],
  outputArtifacts: [],
  round: 1,
});

const makeRound = (round: number, executions: ToolExecution[]): ToolExecutionRound => ({
  round,
  executions,
  hasError: executions.some((e) => e.status === "error"),
  totalDuration: executions.reduce((sum, e) => sum + (e.duration || 0), 0),
});

const makeArtifact = (id: string, title: string): Artifact => ({
  id,
  type: "table",
  title,
  data: {
    columns: [
      { key: "name", label: "Name" },
      { key: "value", label: "Value" },
    ],
    rows: [
      { name: "Item 1", value: 100 },
      { name: "Item 2", value: 200 },
    ],
  },
  role: "output",
});

const meta: Meta<typeof ContentRound> = {
  title: "Chat/MultiModelResponse/ContentRound",
  component: ContentRound,
  parameters: {
    layout: "padded",
  },
  decorators: [
    (Story) => {
      useChatUIStore.setState({
        compactMode: false,
        viewMode: "grid",
        expandedModel: null,
        editingKey: null,
      });
      return (
        <PreferencesProvider>
          <div style={{ maxWidth: 700 }}>
            <Story />
          </div>
        </PreferencesProvider>
      );
    },
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

/** Basic text content renders markdown */
export const TextOnly: Story = {
  args: {
    content: "Hello! This is a **bold** response with `inline code` and a list:\n\n- Item one\n- Item two",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/Hello!/)).toBeInTheDocument();
  },
};

/** Reasoning section shown above content */
export const WithReasoning: Story = {
  args: {
    reasoning:
      "Let me think step by step...\n\n1. First consideration\n2. Second consideration",
    reasoningTokenCount: 42,
    content: "Based on my analysis, the answer is 42.",
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/the answer is 42/)).toBeInTheDocument();
    // Reasoning section should be present (collapsed by default shows token count)
    await expect(canvas.getByText(/42 tokens/)).toBeInTheDocument();
  },
};

/** Tool execution summary bar with expand/collapse */
export const WithToolExecution: Story = {
  args: {
    content: "I ran the code and got the results.",
    toolExecutionRound: makeRound(1, [
      makeExecution("code_interpreter", "success", 1200),
      makeExecution("file_search", "success", 300),
    ]),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/I ran the code/)).toBeInTheDocument();
    // Tool summary bar should be visible
    await expect(canvas.getByText(/2 tools/i)).toBeInTheDocument();
  },
};

/** Tool execution still in progress */
export const ToolsStreaming: Story = {
  args: {
    content: "Running analysis...",
    isStreaming: true,
    toolExecutionRound: makeRound(1, [makeExecution("code_interpreter", "running")]),
    isToolsStreaming: true,
  },
};

/** Display selection with inline layout */
export const WithDisplayedArtifacts: Story = {
  args: {
    content: "Here are the results:",
    displaySelection: { artifactIds: ["art-1", "art-2"], layout: "inline" },
    allOutputArtifacts: [makeArtifact("art-1", "Sales Data"), makeArtifact("art-2", "Revenue")],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/Here are the results/)).toBeInTheDocument();
    await expect(canvas.getByText("Sales Data")).toBeInTheDocument();
    await expect(canvas.getByText("Revenue")).toBeInTheDocument();
  },
};

/** Display selection with gallery (grid) layout */
export const GalleryLayout: Story = {
  args: {
    content: "Gallery view:",
    displaySelection: { artifactIds: ["art-1", "art-2"], layout: "gallery" },
    allOutputArtifacts: [makeArtifact("art-1", "Chart A"), makeArtifact("art-2", "Chart B")],
  },
};

/** Full round: reasoning + content + tools + artifacts */
export const FullRound: Story = {
  args: {
    reasoning: "Analyzing the data set...\n\nFound 3 relevant patterns.",
    reasoningTokenCount: 128,
    content:
      "I analyzed the dataset and found interesting patterns. Here's a summary:\n\n```python\ndf.describe()\n```",
    toolExecutionRound: makeRound(1, [makeExecution("code_interpreter", "success", 2400)]),
    displaySelection: { artifactIds: ["art-1"], layout: "inline" },
    allOutputArtifacts: [makeArtifact("art-1", "Analysis Results")],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/interesting patterns/)).toBeInTheDocument();
    await expect(canvas.getByText("Analysis Results")).toBeInTheDocument();
  },
};

/** Compact mode shows only content and artifacts */
export const CompactMode: Story = {
  decorators: [
    (Story) => {
      useChatUIStore.setState({ compactMode: true });
      return (
        <PreferencesProvider>
          <div style={{ maxWidth: 700 }}>
            <Story />
          </div>
        </PreferencesProvider>
      );
    },
  ],
  args: {
    reasoning: "This reasoning should be hidden in compact mode.",
    reasoningTokenCount: 50,
    content: "Only this content shows in compact mode.",
    toolExecutionRound: makeRound(1, [makeExecution("code_interpreter", "success", 500)]),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByText(/Only this content shows/)).toBeInTheDocument();
    // Reasoning and tools should not be visible
    await expect(canvas.queryByText(/50 tokens/)).not.toBeInTheDocument();
    await expect(canvas.queryByText(/1 tool/i)).not.toBeInTheDocument();
  },
};

/** Compact mode with no content returns null (empty round) */
export const CompactModeEmpty: Story = {
  decorators: [
    (Story) => {
      useChatUIStore.setState({ compactMode: true });
      return (
        <PreferencesProvider>
          <div style={{ maxWidth: 700 }} data-testid="wrapper">
            <Story />
          </div>
        </PreferencesProvider>
      );
    },
  ],
  args: {
    reasoning: "Only reasoning, no content.",
    reasoningTokenCount: 30,
    toolExecutionRound: makeRound(1, [makeExecution("code_interpreter", "success", 200)]),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const wrapper = canvas.getByTestId("wrapper");
    // Component should render nothing — wrapper should be empty
    await expect(wrapper.children.length).toBe(0);
  },
};

/** Empty round returns null */
export const EmptyRound: Story = {
  args: {},
  play: async ({ canvasElement }) => {
    // No visible content
    await expect(canvasElement.querySelector(".space-y-1")).not.toBeInTheDocument();
  },
};

/** Streaming content with active cursor */
export const StreamingContent: Story = {
  args: {
    content: "I'm currently generating this response and it's still being",
    isStreaming: true,
  },
};

/** Reasoning streaming without content yet */
export const ReasoningStreaming: Story = {
  args: {
    reasoning: "Hmm, let me think about this...",
    isReasoningStreaming: true,
  },
};

/** Artifact click callback fires */
export const ArtifactClickCallback: Story = {
  args: {
    content: "Check the results below.",
    onArtifactClick: fn(),
    displaySelection: { artifactIds: ["art-1"], layout: "inline" },
    allOutputArtifacts: [makeArtifact("art-1", "Clickable Artifact")],
  },
};
