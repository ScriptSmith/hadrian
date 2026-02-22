import type { Meta, StoryObj } from "@storybook/react";
import { ExecutionTimeline } from "./ExecutionTimeline";
import type { ToolExecutionRound, ToolExecution, Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/ToolExecution/ExecutionTimeline",
  component: ExecutionTimeline,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof ExecutionTimeline>;

export default meta;
type Story = StoryObj<typeof meta>;

const makeCodeArtifact = (code: string): Artifact => ({
  id: `code-${Math.random().toString(36).slice(2)}`,
  type: "code",
  data: { language: "python", code },
  role: "input",
});

const makeTableArtifact = (): Artifact => ({
  id: `table-${Math.random().toString(36).slice(2)}`,
  type: "table",
  title: "Results",
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

const makeExecution = (
  toolName: string,
  status: ToolExecution["status"],
  inputCode?: string,
  hasOutput?: boolean,
  duration?: number,
  error?: string
): ToolExecution => ({
  id: `exec-${Math.random().toString(36).slice(2)}`,
  toolName,
  status,
  startTime: Date.now() - (duration || 0),
  endTime: status !== "running" ? Date.now() : undefined,
  duration,
  input: {},
  inputArtifacts: inputCode ? [makeCodeArtifact(inputCode)] : [],
  outputArtifacts: hasOutput ? [makeTableArtifact()] : [],
  round: 1,
  error,
});

const makeRound = (
  round: number,
  executions: ToolExecution[],
  modelReasoning?: string,
  hasError?: boolean
): ToolExecutionRound => ({
  round,
  executions,
  modelReasoning,
  hasError,
  totalDuration: executions.reduce((sum, e) => sum + (e.duration || 0), 0),
});

export const SingleRound: Story = {
  args: {
    rounds: [
      makeRound(1, [
        makeExecution("code_interpreter", "success", 'print("Hello, World!")', true, 1200),
      ]),
    ],
  },
};

export const MultipleRounds: Story = {
  args: {
    rounds: [
      makeRound(
        1,
        [
          makeExecution(
            "code_interpreter",
            "error",
            "import missing_module",
            false,
            500,
            "ModuleNotFoundError"
          ),
        ],
        "I need to try a different approach since the module is not available.",
        true
      ),
      makeRound(2, [
        makeExecution(
          "code_interpreter",
          "success",
          "# Using built-in modules\nimport json",
          true,
          800
        ),
      ]),
    ],
  },
};

export const ParallelExecutions: Story = {
  args: {
    rounds: [
      makeRound(1, [
        makeExecution("code_interpreter", "success", "# Task 1", true, 1000),
        makeExecution("sql_query", "success", undefined, true, 600),
        makeExecution("file_search", "success", undefined, false, 300),
      ]),
    ],
  },
};

export const WithErrors: Story = {
  args: {
    rounds: [
      makeRound(
        1,
        [
          makeExecution(
            "code_interpreter",
            "error",
            "1/0",
            false,
            100,
            "ZeroDivisionError: division by zero"
          ),
        ],
        undefined,
        true
      ),
    ],
  },
};
