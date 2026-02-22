import type { Meta, StoryObj } from "@storybook/react";
import { ExecutionSummaryBar } from "./ExecutionSummaryBar";
import type { ToolExecutionRound, ToolExecution } from "@/components/chat-types";

const meta = {
  title: "Chat/ToolExecution/ExecutionSummaryBar",
  component: ExecutionSummaryBar,
  parameters: {
    layout: "centered",
  },
} satisfies Meta<typeof ExecutionSummaryBar>;

export default meta;
type Story = StoryObj<typeof meta>;

const makeExecution = (
  toolName: string,
  status: ToolExecution["status"],
  duration?: number,
  statusMessage?: string
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
  statusMessage,
});

const makeRound = (
  round: number,
  executions: ToolExecution[],
  hasError?: boolean
): ToolExecutionRound => ({
  round,
  executions,
  hasError,
  totalDuration: executions.reduce((sum, e) => sum + (e.duration || 0), 0),
});

export const SingleTool: Story = {
  args: {
    rounds: [makeRound(1, [makeExecution("code_interpreter", "success", 1200)])],
    isExpanded: false,
    onToggle: () => {},
  },
};

export const MultipleTools: Story = {
  args: {
    rounds: [
      makeRound(1, [
        makeExecution("code_interpreter", "success", 800),
        makeExecution("sql_query", "success", 450),
        makeExecution("file_search", "success", 200),
      ]),
    ],
    isExpanded: false,
    onToggle: () => {},
  },
};

export const WithRetries: Story = {
  args: {
    rounds: [
      makeRound(1, [makeExecution("code_interpreter", "error", 500)], true),
      makeRound(2, [makeExecution("code_interpreter", "success", 1200)]),
    ],
    isExpanded: false,
    onToggle: () => {},
  },
};

export const Running: Story = {
  args: {
    rounds: [makeRound(1, [makeExecution("code_interpreter", "running", 0, "Executing code...")])],
    isExpanded: false,
    onToggle: () => {},
    isStreaming: true,
  },
};

export const Expanded: Story = {
  args: {
    rounds: [makeRound(1, [makeExecution("code_interpreter", "success", 1500)])],
    isExpanded: true,
    onToggle: () => {},
  },
};

export const ManyTools: Story = {
  args: {
    rounds: [
      makeRound(1, [
        makeExecution("code_interpreter", "success", 800),
        makeExecution("sql_query", "success", 450),
        makeExecution("file_search", "success", 200),
        makeExecution("chart_render", "success", 300),
        makeExecution("web_search", "success", 600),
      ]),
    ],
    isExpanded: false,
    onToggle: () => {},
  },
};
